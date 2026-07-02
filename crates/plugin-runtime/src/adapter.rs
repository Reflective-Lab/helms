// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! Adapter bridging WASM modules to `converge_core::Invariant`.
//!
//! `WasmInvariant` wraps a compiled WASM module and implements the
//! `Invariant` trait so it can be registered in the engine's
//! `InvariantRegistry` alongside native Rust invariants.
//!
//! # Feature Gate
//!
//!
use std::sync::Arc;

use converge_core::{ContextKey, InvariantClass, InvariantResult, Violation};
use strum::IntoEnumIterator;

use super::contract::*;
use super::engine::{CompiledModule, WasmEngine};

/// Convert a `converge_core::Context` to a `GuestContext` for WASM modules.
///
/// Iterates all `ContextKey` variants and serializes facts to `GuestFact`.
/// Proposals and Diagnostic keys are intentionally **included** in the
/// iteration but will be empty in practice since the engine doesn't expose
/// them to invariant checks. The `parse_context_key` function in the
/// `convert` module blocks them on the way back.
pub fn context_to_guest(ctx: &dyn converge_core::Context, cycle: u32) -> GuestContext {
    let mut facts = std::collections::HashMap::new();

    for key in ContextKey::iter() {
        // Skip internal-only keys
        match key {
            ContextKey::Proposals | ContextKey::Diagnostic => continue,
            _ => {}
        }

        let key_facts = ctx.get(key);
        if !key_facts.is_empty() {
            let guest_facts: Vec<GuestFact> = key_facts
                .iter()
                .map(|f| GuestFact {
                    id: f.id().to_string(),
                    content: f.text().map(str::to_string).unwrap_or_else(|| {
                        f.to_wire()
                            .map(|wire| wire.payload.payload.to_string())
                            .unwrap_or_default()
                    }),
                })
                .collect();
            facts.insert(format!("{key:?}"), guest_facts);
        }
    }

    GuestContext {
        facts,
        version: ctx.version(),
        cycle,
    }
}

/// A WASM module that implements `converge_core::Invariant`.
///
/// This adapter compiles and instantiates the WASM module on each
/// `check()` call, enforcing fuel and memory quotas per invocation.
/// The module is content-addressed and immutable after creation.
pub struct WasmInvariant {
    engine: Arc<WasmEngine>,
    compiled: Arc<CompiledModule>,
    manifest: WasmManifest,
    quota: WasmQuota,
}

impl WasmInvariant {
    /// Create a new WASM invariant adapter from raw bytes.
    ///
    /// Compiles the WASM bytes, reads the manifest, and validates that
    /// the module is an Invariant type.
    ///
    /// # Errors
    ///
    /// Returns `WasmError::CompilationFailed` if the module cannot be compiled.
    /// Returns `WasmError::InvalidManifest` if the manifest is not an Invariant module.
    pub fn new(
        engine: Arc<WasmEngine>,
        wasm_bytes: &[u8],
        quota: WasmQuota,
    ) -> Result<Self, WasmError> {
        let compiled = Arc::new(engine.compile(wasm_bytes)?);

        // Read manifest from the module
        let context = GuestContext {
            facts: std::collections::HashMap::new(),
            version: 0,
            cycle: 0,
        };
        let mut instance = engine.instantiate(&compiled, context, quota, vec![])?;
        let manifest = instance.call_manifest()?;

        Self::validate_invariant_manifest(&manifest)?;

        Ok(Self {
            engine,
            compiled,
            manifest,
            quota,
        })
    }

    /// Create a WASM invariant adapter from a pre-compiled module and manifest.
    ///
    /// Avoids recompilation when the module has already been compiled by the
    /// [`ModuleStore`](super::store::ModuleStore). The manifest must have been
    /// read during the store's upload phase.
    ///
    /// # Errors
    ///
    /// Returns `WasmError::InvalidManifest` if the manifest is not an Invariant module.
    pub fn from_compiled(
        engine: Arc<WasmEngine>,
        compiled: Arc<CompiledModule>,
        manifest: WasmManifest,
        quota: WasmQuota,
    ) -> Result<Self, WasmError> {
        Self::validate_invariant_manifest(&manifest)?;

        Ok(Self {
            engine,
            compiled,
            manifest,
            quota,
        })
    }

    fn validate_invariant_manifest(manifest: &WasmManifest) -> Result<(), WasmError> {
        if manifest.kind != ModuleKind::Invariant {
            return Err(WasmError::InvalidManifest(format!(
                "expected Invariant module, got {:?}",
                manifest.kind
            )));
        }

        if manifest.invariant_class.is_none() {
            return Err(WasmError::InvalidManifest(
                "Invariant module must declare invariant_class".to_string(),
            ));
        }

        Ok(())
    }
}

impl converge_core::Invariant for WasmInvariant {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn class(&self) -> InvariantClass {
        self.manifest
            .invariant_class
            .map(convert::to_invariant_class)
            .unwrap_or(InvariantClass::Semantic)
    }

    fn check(&self, ctx: &dyn converge_core::Context) -> InvariantResult {
        let guest_ctx = context_to_guest(ctx, 0);
        let capabilities = self.manifest.capabilities.clone();

        // Instantiate with fresh fuel for each check
        let mut instance =
            match self
                .engine
                .instantiate(&self.compiled, guest_ctx, self.quota, capabilities)
            {
                Ok(inst) => inst,
                Err(e) => {
                    return InvariantResult::Violated(Violation::new(format!(
                        "WASM instantiation failed: {e}"
                    )));
                }
            };

        // Call check_invariant
        match instance.call_check_invariant() {
            Ok(guest_result) => convert::to_invariant_result(&guest_result),
            Err(e) => InvariantResult::Violated(Violation::new(format!("WASM error: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use converge_core::{ContextKey, ContextState, Engine};
    use std::collections::HashMap;

    /// Build the WAT for an invariant that always returns ok.
    fn invariant_ok_wat() -> String {
        let manifest_json = serde_json::to_string(&WasmManifest {
            name: "test-ok".to_string(),
            version: "1.0.0".to_string(),
            kind: ModuleKind::Invariant,
            invariant_class: Some(WasmInvariantClass::Structural),
            dependencies: vec![],
            capabilities: vec![],
            requires_human_approval: false,
            jtbd: None,
            metadata: HashMap::new(),
        })
        .unwrap();
        let manifest_len = manifest_json.len();

        let result_json = r#"{"ok":true}"#;
        let result_len = result_json.len();
        let result_offset = manifest_len;

        format!(
            r#"
            (module
                (memory (export "memory") 1)
                (global $bump (mut i32) (i32.const {bump_start}))
                (data (i32.const 0) "{manifest_escaped}")
                (data (i32.const {result_offset}) "{result_escaped}")
                (func (export "alloc") (param $size i32) (result i32)
                    (local $ptr i32)
                    (local.set $ptr (global.get $bump))
                    (global.set $bump (i32.add (global.get $bump) (local.get $size)))
                    (local.get $ptr))
                (func (export "dealloc") (param $ptr i32) (param $len i32))
                (func (export "converge_abi_version") (result i32) (i32.const 1))
                (func (export "converge_manifest") (result i32 i32)
                    (i32.const 0) (i32.const {manifest_len}))
                (func (export "check_invariant") (param $ctx_ptr i32) (param $ctx_len i32) (result i32 i32)
                    (i32.const {result_offset}) (i32.const {result_len}))
            )
            "#,
            bump_start = result_offset + result_len + 16,
            manifest_escaped = escape_wat(&manifest_json),
            manifest_len = manifest_len,
            result_offset = result_offset,
            result_escaped = escape_wat(result_json),
            result_len = result_len,
        )
    }

    /// Build WAT for an invariant that always returns violated.
    fn invariant_violated_wat() -> String {
        let manifest_json = serde_json::to_string(&WasmManifest {
            name: "test-violated".to_string(),
            version: "1.0.0".to_string(),
            kind: ModuleKind::Invariant,
            invariant_class: Some(WasmInvariantClass::Acceptance),
            dependencies: vec![],
            capabilities: vec![],
            requires_human_approval: false,
            jtbd: None,
            metadata: HashMap::new(),
        })
        .unwrap();
        let manifest_len = manifest_json.len();

        let result_json =
            r#"{"ok":false,"reason":"need at least 2 strategies","fact_ids":["strat-1"]}"#;
        let result_len = result_json.len();
        let result_offset = manifest_len;

        format!(
            r#"
            (module
                (memory (export "memory") 1)
                (global $bump (mut i32) (i32.const {bump_start}))
                (data (i32.const 0) "{manifest_escaped}")
                (data (i32.const {result_offset}) "{result_escaped}")
                (func (export "alloc") (param $size i32) (result i32)
                    (local $ptr i32)
                    (local.set $ptr (global.get $bump))
                    (global.set $bump (i32.add (global.get $bump) (local.get $size)))
                    (local.get $ptr))
                (func (export "dealloc") (param $ptr i32) (param $len i32))
                (func (export "converge_abi_version") (result i32) (i32.const 1))
                (func (export "converge_manifest") (result i32 i32)
                    (i32.const 0) (i32.const {manifest_len}))
                (func (export "check_invariant") (param $ctx_ptr i32) (param $ctx_len i32) (result i32 i32)
                    (i32.const {result_offset}) (i32.const {result_len}))
            )
            "#,
            bump_start = result_offset + result_len + 16,
            manifest_escaped = escape_wat(&manifest_json),
            manifest_len = manifest_len,
            result_offset = result_offset,
            result_escaped = escape_wat(result_json),
            result_len = result_len,
        )
    }

    /// Build WAT for an agent module (not invariant).
    fn agent_module_wat() -> String {
        let manifest_json = serde_json::to_string(&WasmManifest {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            kind: ModuleKind::Suggestor,
            invariant_class: None,
            dependencies: vec!["Seeds".to_string()],
            capabilities: vec![],
            requires_human_approval: false,
            jtbd: None,
            metadata: HashMap::new(),
        })
        .unwrap();
        let manifest_len = manifest_json.len();

        format!(
            r#"
            (module
                (memory (export "memory") 1)
                (global $bump (mut i32) (i32.const {bump_start}))
                (data (i32.const 0) "{manifest_escaped}")
                (func (export "alloc") (param $size i32) (result i32)
                    (local $ptr i32)
                    (local.set $ptr (global.get $bump))
                    (global.set $bump (i32.add (global.get $bump) (local.get $size)))
                    (local.get $ptr))
                (func (export "dealloc") (param $ptr i32) (param $len i32))
                (func (export "converge_abi_version") (result i32) (i32.const 1))
                (func (export "converge_manifest") (result i32 i32)
                    (i32.const 0) (i32.const {manifest_len}))
            )
            "#,
            bump_start = manifest_len + 16,
            manifest_escaped = escape_wat(&manifest_json),
            manifest_len = manifest_len,
        )
    }

    fn escape_wat(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }

    fn make_engine() -> Arc<WasmEngine> {
        Arc::new(WasmEngine::new().unwrap())
    }

    fn promoted_context(entries: &[(ContextKey, &str, &str)]) -> ContextState {
        let mut ctx = ContextState::new();
        for (key, id, content) in entries {
            ctx.add_input(*key, *id, *content).unwrap();
        }
        tokio_test::block_on(Engine::new().run(ctx))
            .unwrap()
            .context
    }

    fn context_with_strategies() -> ContextState {
        promoted_context(&[
            (ContextKey::Strategies, "strat-1", "SEO strategy"),
            (ContextKey::Strategies, "strat-2", "Content marketing"),
        ])
    }

    // =========================================================================
    // context_to_guest tests
    // =========================================================================

    #[test]
    fn context_to_guest_includes_all_public_keys() {
        let ctx = promoted_context(&[
            (ContextKey::Seeds, "s1", "seed"),
            (ContextKey::Strategies, "st1", "strat"),
            (ContextKey::Evaluations, "e1", "eval"),
        ]);

        let guest = context_to_guest(&ctx, 5);
        assert_eq!(guest.cycle, 5);
        assert!(guest.facts.contains_key("Seeds"));
        assert!(guest.facts.contains_key("Strategies"));
        assert!(guest.facts.contains_key("Evaluations"));
        assert_eq!(guest.facts["Seeds"].len(), 1);
        assert_eq!(guest.facts["Seeds"][0].id, "s1");
    }

    #[test]
    fn context_to_guest_excludes_proposals_and_diagnostic() {
        let ctx = ContextState::new();
        let guest = context_to_guest(&ctx, 0);

        // Even if Proposals/Diagnostic had facts, they should be excluded
        assert!(!guest.facts.contains_key("Proposals"));
        assert!(!guest.facts.contains_key("Diagnostic"));
    }

    #[test]
    fn context_to_guest_empty_context() {
        let ctx = ContextState::new();
        let guest = context_to_guest(&ctx, 0);
        assert!(guest.facts.is_empty());
        assert_eq!(guest.version, 0);
    }

    #[test]
    fn context_to_guest_preserves_version() {
        let ctx = promoted_context(&[(ContextKey::Seeds, "s1", "v")]);
        let guest = context_to_guest(&ctx, 3);
        assert_eq!(guest.version, ctx.version());
        assert_eq!(guest.cycle, 3);
    }

    // =========================================================================
    // WasmInvariant creation tests
    // =========================================================================

    #[test]
    fn wasm_invariant_name_from_manifest() {
        let engine = make_engine();
        let wat = invariant_ok_wat();
        let inv = WasmInvariant::new(engine, wat.as_bytes(), WasmQuota::default()).unwrap();

        assert_eq!(converge_core::Invariant::name(&inv), "test-ok");
    }

    #[test]
    fn wasm_invariant_class_from_manifest() {
        let engine = make_engine();
        let wat = invariant_ok_wat();
        let inv = WasmInvariant::new(engine, wat.as_bytes(), WasmQuota::default()).unwrap();

        assert_eq!(
            converge_core::Invariant::class(&inv),
            InvariantClass::Structural
        );
    }

    #[test]
    fn wasm_invariant_rejects_agent_module() {
        let engine = make_engine();
        let wat = agent_module_wat();
        let result = WasmInvariant::new(engine, wat.as_bytes(), WasmQuota::default());
        assert!(result.is_err());
    }

    // =========================================================================
    // check() integration tests
    // =========================================================================

    #[test]
    fn check_returns_ok() {
        let engine = make_engine();
        let wat = invariant_ok_wat();
        let inv = WasmInvariant::new(engine, wat.as_bytes(), WasmQuota::default()).unwrap();

        let ctx = context_with_strategies();
        let result = converge_core::Invariant::check(&inv, &ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn check_returns_violated() {
        let engine = make_engine();
        let wat = invariant_violated_wat();
        let inv = WasmInvariant::new(engine, wat.as_bytes(), WasmQuota::default()).unwrap();

        let ctx = ContextState::new();
        let result = converge_core::Invariant::check(&inv, &ctx);
        assert!(result.is_violated());
    }

    // =========================================================================
    // Negative tests
    // =========================================================================

    #[test]
    fn check_with_fuel_exhaustion_returns_violated() {
        let engine = make_engine();
        let wat = invariant_ok_wat();
        let quota = WasmQuota {
            max_fuel: 1,
            ..WasmQuota::default()
        };
        let inv = WasmInvariant::new(Arc::clone(&engine), wat.as_bytes(), quota);

        // Creation might fail or check might fail depending on fuel.
        // Err is expected: not enough fuel even for manifest read.
        if let Ok(inv) = inv {
            let result = converge_core::Invariant::check(&inv, &ContextState::new());
            assert!(result.is_violated());
        }
    }

    #[test]
    fn check_with_invalid_wasm_fails() {
        let engine = make_engine();
        let result = WasmInvariant::new(engine, b"not wasm", WasmQuota::default());
        assert!(result.is_err());
    }
}
