// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! WASM execution engine backed by wasmtime.
//!
//! This module provides `WasmEngine` for compiling and running guest
//! WASM modules inside the Helm plugin sandbox, enforcing fuel metering,
//! memory limits, and capability-based access control.
//!
//! # Architecture
//!
//! ```text
//! WasmEngine          CompiledModule         WasmInstance
//! ┌──────────┐        ┌──────────────┐       ┌────────────────┐
//! │ wasmtime │compile │ wasmtime     │instan │ Store<HostState>│
//! │ ::Engine ├───────►│ ::Module     ├──────►│ + Instance      │
//! │ + Linker │        └──────────────┘       │ + fuel/trace    │
//! └──────────┘                               └────────────────┘
//! ```
//!
use wasmtime::*;

use super::contract::*;
use super::host::{HostState, LogEntry};

/// WASM execution engine.
///
/// Wraps a `wasmtime::Engine` configured with fuel metering.
/// Use `compile()` to create `CompiledModule`, then `instantiate()`
/// to create a callable `WasmInstance`.
pub struct WasmEngine {
    engine: Engine,
}

/// A compiled WASM module, ready for instantiation.
pub struct CompiledModule {
    module: Module,
}

/// A live WASM module instance with host state.
///
/// Wraps `wasmtime::Store<HostState>` and `wasmtime::Instance`.
/// Each invocation method (`call_check_invariant`, etc.) updates
/// the internal execution trace.
pub struct WasmInstance {
    store: Store<HostState>,
    instance: Instance,
    initial_fuel: u64,
}

impl WasmEngine {
    /// Create a new WASM engine with fuel metering enabled.
    ///
    /// # Errors
    ///
    /// Returns `WasmError::CompilationFailed` if engine creation fails.
    pub fn new() -> Result<Self, WasmError> {
        let mut config = Config::new();
        config.consume_fuel(true);
        // Enable multi-value returns (needed for ptr+len pairs)
        config.wasm_multi_value(true);

        let engine =
            Engine::new(&config).map_err(|e| WasmError::CompilationFailed(e.to_string()))?;

        Ok(Self { engine })
    }

    /// Compile raw WASM bytes (or WAT text) into a `CompiledModule`.
    ///
    /// Accepts both `.wasm` binary and `.wat` text format.
    ///
    /// # Errors
    ///
    /// Returns `WasmError::CompilationFailed` if the bytes are invalid WASM.
    pub fn compile(&self, wasm_bytes: &[u8]) -> Result<CompiledModule, WasmError> {
        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| WasmError::CompilationFailed(e.to_string()))?;

        Ok(CompiledModule { module })
    }

    /// Instantiate a compiled module with host state and linked functions.
    ///
    /// Sets up fuel metering, links host functions (`host_read_context`,
    /// `host_log`, `host_now_millis`), and creates the instance.
    ///
    /// # Errors
    ///
    /// Returns `WasmError` if linking or instantiation fails.
    pub fn instantiate(
        &self,
        compiled: &CompiledModule,
        context: GuestContext,
        quota: WasmQuota,
        capabilities: Vec<HostCapability>,
    ) -> Result<WasmInstance, WasmError> {
        let host_state = HostState::new(context, quota, capabilities);
        let mut store = Store::new(&self.engine, host_state);

        // Set fuel limit
        store
            .set_fuel(quota.max_fuel)
            .map_err(|e| WasmError::CompilationFailed(format!("failed to set fuel: {e}")))?;

        // Create linker with host functions
        let mut linker = Linker::new(&self.engine);
        Self::link_host_functions(&mut linker)?;

        // Instantiate
        let instance = linker
            .instantiate(&mut store, &compiled.module)
            .map_err(|e| WasmError::CompilationFailed(format!("instantiation failed: {e}")))?;

        Ok(WasmInstance {
            store,
            instance,
            initial_fuel: quota.max_fuel,
        })
    }

    /// Link host functions into the linker.
    fn link_host_functions(linker: &mut Linker<HostState>) -> Result<(), WasmError> {
        // host_read_context(key: i32) -> (ptr: i32, len: i32)
        linker
            .func_wrap(
                "converge",
                "host_read_context",
                |mut caller: Caller<'_, HostState>, key: i32| -> Result<(i32, i32)> {
                    // Check capability
                    let has_cap = caller.data().has_capability(HostCapability::ReadContext);
                    if !has_cap {
                        let state = caller.data_mut();
                        state.record_host_call("host_read_context", "DENIED", 0, false);
                        return Err(wasmtime::Error::msg("ReadContext capability denied"));
                    }

                    // Check host call quota
                    let within_quota = caller.data_mut().check_host_call_quota();
                    if !within_quota {
                        return Err(wasmtime::Error::msg("host call quota exceeded"));
                    }

                    // Resolve key name and serialize facts
                    let key_name = HostState::context_key_name(key as u32);
                    let json = match key_name {
                        Some(name) => {
                            let facts = caller
                                .data()
                                .context
                                .facts
                                .get(name)
                                .cloned()
                                .unwrap_or_default();
                            serde_json::to_vec(&facts).unwrap_or_default()
                        }
                        None => b"[]".to_vec(),
                    };
                    let json_len = json.len();

                    // Call guest's alloc function to get a pointer
                    let alloc_fn = caller
                        .get_export("alloc")
                        .and_then(|e| e.into_func())
                        .ok_or_else(|| wasmtime::Error::msg("guest missing 'alloc' export"))?;
                    let alloc_typed = alloc_fn
                        .typed::<i32, i32>(&caller)
                        .map_err(|e| wasmtime::Error::msg(format!("alloc type mismatch: {e}")))?;
                    let ptr = alloc_typed.call(&mut caller, json_len as i32)?;

                    // Write to guest memory
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .ok_or_else(|| wasmtime::Error::msg("guest missing 'memory' export"))?;
                    memory.write(&mut caller, ptr as usize, &json)?;

                    // Record
                    let summary = format!("key={}", key_name.unwrap_or("unknown"));
                    caller
                        .data_mut()
                        .record_host_call("host_read_context", &summary, 0, true);

                    Ok((ptr, json_len as i32))
                },
            )
            .map_err(|e| WasmError::CompilationFailed(format!("link host_read_context: {e}")))?;

        // host_log(level: i32, ptr: i32, len: i32)
        linker
            .func_wrap(
                "converge",
                "host_log",
                |mut caller: Caller<'_, HostState>, level: i32, ptr: i32, len: i32| -> Result<()> {
                    // Check capability
                    let has_cap = caller.data().has_capability(HostCapability::Log);
                    if !has_cap {
                        let state = caller.data_mut();
                        state.record_host_call("host_log", "DENIED", 0, false);
                        return Err(wasmtime::Error::msg("Log capability denied"));
                    }

                    // Check host call quota
                    let within_quota = caller.data_mut().check_host_call_quota();
                    if !within_quota {
                        return Err(wasmtime::Error::msg("host call quota exceeded"));
                    }

                    // Read message from guest memory
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .ok_or_else(|| wasmtime::Error::msg("guest missing 'memory' export"))?;

                    let mut buf = vec![0u8; len as usize];
                    memory.read(&caller, ptr as usize, &mut buf)?;
                    let message = String::from_utf8_lossy(&buf).to_string();

                    let summary = format!("level={level}");
                    let state = caller.data_mut();
                    state.log_entries.push(LogEntry {
                        level: level as u32,
                        message,
                    });
                    state.record_host_call("host_log", &summary, 0, true);

                    Ok(())
                },
            )
            .map_err(|e| WasmError::CompilationFailed(format!("link host_log: {e}")))?;

        // host_now_millis() -> i64
        linker
            .func_wrap(
                "converge",
                "host_now_millis",
                |mut caller: Caller<'_, HostState>| -> Result<i64> {
                    // Check capability
                    let has_cap = caller.data().has_capability(HostCapability::Clock);
                    if !has_cap {
                        let state = caller.data_mut();
                        state.record_host_call("host_now_millis", "DENIED", 0, false);
                        return Err(wasmtime::Error::msg("Clock capability denied"));
                    }

                    // Check host call quota
                    let within_quota = caller.data_mut().check_host_call_quota();
                    if !within_quota {
                        return Err(wasmtime::Error::msg("host call quota exceeded"));
                    }

                    // Return a deterministic logical clock value derived from the
                    // Converge context version/cycle and host call sequence.
                    let elapsed = caller.data().logical_now_millis();

                    caller
                        .data_mut()
                        .record_host_call("host_now_millis", "", 0, true);

                    Ok(elapsed)
                },
            )
            .map_err(|e| WasmError::CompilationFailed(format!("link host_now_millis: {e}")))?;

        Ok(())
    }
}

impl WasmInstance {
    /// Call `converge_abi_version()` on the guest module.
    ///
    /// # Errors
    ///
    /// Returns `WasmError::Trapped` if the function is missing or traps.
    pub fn call_abi_version(&mut self) -> Result<u32, WasmError> {
        let func = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, "converge_abi_version")
            .map_err(|e| WasmError::Trapped {
                function: "converge_abi_version".to_string(),
                message: e.to_string(),
            })?;

        let version = func
            .call(&mut self.store, ())
            .map_err(|e| WasmError::Trapped {
                function: "converge_abi_version".to_string(),
                message: e.to_string(),
            })?;

        Ok(version as u32)
    }

    /// Call `converge_manifest()` on the guest and parse the JSON result.
    ///
    /// # Errors
    ///
    /// Returns `WasmError::Trapped` or `WasmError::MalformedResult`.
    pub fn call_manifest(&mut self) -> Result<WasmManifest, WasmError> {
        let func = self
            .instance
            .get_typed_func::<(), (i32, i32)>(&mut self.store, "converge_manifest")
            .map_err(|e| WasmError::Trapped {
                function: "converge_manifest".to_string(),
                message: e.to_string(),
            })?;

        let (ptr, len) = func
            .call(&mut self.store, ())
            .map_err(|e| WasmError::Trapped {
                function: "converge_manifest".to_string(),
                message: e.to_string(),
            })?;

        let bytes = self.read_guest_memory(ptr, len)?;

        serde_json::from_slice(&bytes).map_err(|e| WasmError::MalformedResult {
            function: "converge_manifest".to_string(),
            message: format!("invalid JSON manifest: {e}"),
        })
    }

    /// Call `check_invariant(ctx_ptr, ctx_len)` on the guest.
    ///
    /// Serializes the context, writes it to guest memory via `alloc`,
    /// calls the function, and deserializes the result.
    ///
    /// # Errors
    ///
    /// Returns `WasmError::Trapped` or `WasmError::MalformedResult`.
    pub fn call_check_invariant(&mut self) -> Result<GuestInvariantResult, WasmError> {
        // Serialize context
        let ctx = self.store.data().context.clone();
        let ctx_json = serde_json::to_vec(&ctx).map_err(|e| WasmError::Trapped {
            function: "check_invariant".to_string(),
            message: format!("failed to serialize context: {e}"),
        })?;

        // Allocate space in guest memory
        let ctx_ptr = self.call_alloc(ctx_json.len() as i32)?;

        // Write context to guest memory
        self.write_guest_memory(ctx_ptr, &ctx_json)?;

        // Call check_invariant
        let func = self
            .instance
            .get_typed_func::<(i32, i32), (i32, i32)>(&mut self.store, "check_invariant")
            .map_err(|e| WasmError::Trapped {
                function: "check_invariant".to_string(),
                message: e.to_string(),
            })?;

        let (result_ptr, result_len) = func
            .call(&mut self.store, (ctx_ptr, ctx_json.len() as i32))
            .map_err(|e| WasmError::Trapped {
                function: "check_invariant".to_string(),
                message: e.to_string(),
            })?;

        // Track result bytes
        self.store.data_mut().result_bytes += result_len as u64;

        let bytes = self.read_guest_memory(result_ptr, result_len)?;

        serde_json::from_slice(&bytes).map_err(|e| WasmError::MalformedResult {
            function: "check_invariant".to_string(),
            message: format!("invalid JSON result: {e}"),
        })
    }

    /// Get the execution trace for this invocation.
    pub fn trace(&mut self, module_id: &ModuleId, manifest: &WasmManifest) -> ExecutionTrace {
        let remaining_fuel = self.store.get_fuel().unwrap_or(0);
        let fuel_consumed = self.initial_fuel.saturating_sub(remaining_fuel);

        // Extract host state values before calling peak_memory_bytes
        let elapsed_us = self.store.data().elapsed_us();
        let host_calls = self.store.data().host_calls.clone();
        let result_bytes = self.store.data().result_bytes;
        let engine_cycle = self.store.data().context.cycle;
        let peak_memory = self.peak_memory_bytes();

        ExecutionTrace {
            module_id: module_id.clone(),
            module_name: manifest.name.clone(),
            module_version: manifest.version.clone(),
            tenant_id: String::new(), // set by caller
            invocation: InvocationKind::CheckInvariant,
            outcome: InvocationOutcome::Ok,
            fuel_consumed,
            peak_memory_bytes: peak_memory,
            duration_us: elapsed_us,
            host_calls,
            result_bytes,
            engine_cycle,
            started_at: 0, // set by caller
        }
    }

    /// Get log entries collected during execution.
    pub fn log_entries(&self) -> &[LogEntry] {
        &self.store.data().log_entries
    }

    /// Get fuel consumed so far.
    pub fn fuel_consumed(&self) -> u64 {
        let remaining = self.store.get_fuel().unwrap_or(0);
        self.initial_fuel.saturating_sub(remaining)
    }

    // ---- Private helpers ----

    /// Call guest's `alloc(size)` to allocate memory.
    fn call_alloc(&mut self, size: i32) -> Result<i32, WasmError> {
        let alloc = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, "alloc")
            .map_err(|e| WasmError::Trapped {
                function: "alloc".to_string(),
                message: e.to_string(),
            })?;

        alloc
            .call(&mut self.store, size)
            .map_err(|e| WasmError::Trapped {
                function: "alloc".to_string(),
                message: e.to_string(),
            })
    }

    /// Read bytes from guest linear memory.
    fn read_guest_memory(&mut self, ptr: i32, len: i32) -> Result<Vec<u8>, WasmError> {
        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| WasmError::Trapped {
                function: "memory".to_string(),
                message: "guest missing 'memory' export".to_string(),
            })?;

        let data = memory.data(&self.store);
        let start = ptr as usize;
        let end = start
            .checked_add(len as usize)
            .ok_or_else(|| WasmError::Trapped {
                function: "memory".to_string(),
                message: "ptr+len overflow".to_string(),
            })?;

        if end > data.len() {
            return Err(WasmError::Trapped {
                function: "memory".to_string(),
                message: format!("out of bounds: {start}+{} > {}", len as usize, data.len()),
            });
        }

        Ok(data[start..end].to_vec())
    }

    /// Write bytes to guest linear memory.
    fn write_guest_memory(&mut self, ptr: i32, data: &[u8]) -> Result<(), WasmError> {
        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| WasmError::Trapped {
                function: "memory".to_string(),
                message: "guest missing 'memory' export".to_string(),
            })?;

        memory
            .write(&mut self.store, ptr as usize, data)
            .map_err(|e| WasmError::Trapped {
                function: "memory".to_string(),
                message: format!("write failed: {e}"),
            })
    }

    /// Get peak memory usage in bytes.
    fn peak_memory_bytes(&mut self) -> u64 {
        self.instance
            .get_memory(&mut self.store, "memory")
            .map(|m| m.data_size(&self.store) as u64)
            .unwrap_or(0)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Minimal WAT module that exports ABI version 1.
    const MINIMAL_WAT: &str = r#"
        (module
            (memory (export "memory") 1)
            (global $bump (mut i32) (i32.const 1024))

            ;; Bump allocator
            (func (export "alloc") (param $size i32) (result i32)
                (local $ptr i32)
                (local.set $ptr (global.get $bump))
                (global.set $bump (i32.add (global.get $bump) (local.get $size)))
                (local.get $ptr)
            )

            ;; Dealloc (no-op)
            (func (export "dealloc") (param $ptr i32) (param $len i32))

            ;; ABI version
            (func (export "converge_abi_version") (result i32)
                (i32.const 1)
            )
        )
    "#;

    /// WAT module with manifest and check_invariant that always returns ok.
    fn invariant_ok_wat() -> String {
        let manifest_json = serde_json::to_string(&WasmManifest {
            name: "test-invariant".to_string(),
            version: "1.0.0".to_string(),
            kind: ModuleKind::Invariant,
            invariant_class: Some(WasmInvariantClass::Structural),
            dependencies: vec![],
            capabilities: vec![HostCapability::ReadContext],
            requires_human_approval: false,
            jtbd: None,
            metadata: HashMap::new(),
        })
        .unwrap();
        let manifest_bytes = manifest_json.as_bytes();
        let manifest_len = manifest_bytes.len();

        let result_json = r#"{"ok":true}"#;
        let result_bytes = result_json.as_bytes();
        let result_len = result_bytes.len();

        // Place manifest at offset 0, result at offset manifest_len
        let result_offset = manifest_len;

        format!(
            r#"
            (module
                (memory (export "memory") 1)
                (global $bump (mut i32) (i32.const {bump_start}))

                ;; Static data: manifest JSON at offset 0
                (data (i32.const 0) "{manifest_escaped}")
                ;; Static data: result JSON
                (data (i32.const {result_offset}) "{result_escaped}")

                ;; Bump allocator
                (func (export "alloc") (param $size i32) (result i32)
                    (local $ptr i32)
                    (local.set $ptr (global.get $bump))
                    (global.set $bump (i32.add (global.get $bump) (local.get $size)))
                    (local.get $ptr)
                )

                ;; Dealloc (no-op)
                (func (export "dealloc") (param $ptr i32) (param $len i32))

                ;; ABI version
                (func (export "converge_abi_version") (result i32)
                    (i32.const 1)
                )

                ;; Manifest
                (func (export "converge_manifest") (result i32 i32)
                    (i32.const 0)
                    (i32.const {manifest_len})
                )

                ;; check_invariant: always ok
                (func (export "check_invariant") (param $ctx_ptr i32) (param $ctx_len i32) (result i32 i32)
                    (i32.const {result_offset})
                    (i32.const {result_len})
                )
            )
            "#,
            bump_start = result_offset + result_len + 16,
            manifest_escaped = escape_wat_string(&manifest_json),
            manifest_len = manifest_len,
            result_offset = result_offset,
            result_escaped = escape_wat_string(result_json),
            result_len = result_len,
        )
    }

    /// WAT module where check_invariant always returns violated.
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
                    (local.get $ptr)
                )

                (func (export "dealloc") (param $ptr i32) (param $len i32))

                (func (export "converge_abi_version") (result i32)
                    (i32.const 1)
                )

                (func (export "converge_manifest") (result i32 i32)
                    (i32.const 0)
                    (i32.const {manifest_len})
                )

                (func (export "check_invariant") (param $ctx_ptr i32) (param $ctx_len i32) (result i32 i32)
                    (i32.const {result_offset})
                    (i32.const {result_len})
                )
            )
            "#,
            bump_start = result_offset + result_len + 16,
            manifest_escaped = escape_wat_string(&manifest_json),
            manifest_len = manifest_len,
            result_offset = result_offset,
            result_escaped = escape_wat_string(result_json),
            result_len = result_len,
        )
    }

    /// Escape a string for embedding in WAT data section.
    /// WAT data strings use `\"` for quotes and `\\` for backslash.
    fn escape_wat_string(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }

    fn empty_context() -> GuestContext {
        GuestContext {
            facts: HashMap::new(),
            version: 1,
            cycle: 0,
        }
    }

    fn context_with_strategies() -> GuestContext {
        let mut facts = HashMap::new();
        facts.insert(
            "Strategies".to_string(),
            vec![
                GuestFact {
                    id: "strat-1".to_string(),
                    content: "SEO strategy".to_string(),
                },
                GuestFact {
                    id: "strat-2".to_string(),
                    content: "Content marketing".to_string(),
                },
            ],
        );
        GuestContext {
            facts,
            version: 3,
            cycle: 2,
        }
    }

    // =========================================================================
    // Engine creation tests
    // =========================================================================

    #[test]
    fn engine_creation() {
        let engine = WasmEngine::new().expect("engine creation");
        assert!(engine.compile(MINIMAL_WAT.as_bytes()).is_ok());
    }

    #[test]
    fn compile_invalid_wasm_fails() {
        let engine = WasmEngine::new().unwrap();
        let result = engine.compile(b"this is not valid wasm");
        assert!(result.is_err());
    }

    // =========================================================================
    // ABI version tests
    // =========================================================================

    #[test]
    fn call_abi_version() {
        let engine = WasmEngine::new().unwrap();
        let module = engine.compile(MINIMAL_WAT.as_bytes()).unwrap();
        let mut instance = engine
            .instantiate(&module, empty_context(), WasmQuota::default(), vec![])
            .unwrap();

        let version = instance.call_abi_version().unwrap();
        assert_eq!(version, WASM_ABI_VERSION);
    }

    // =========================================================================
    // Manifest tests
    // =========================================================================

    #[test]
    fn call_manifest_ok() {
        let engine = WasmEngine::new().unwrap();
        let wat = invariant_ok_wat();
        let module = engine.compile(wat.as_bytes()).unwrap();
        let mut instance = engine
            .instantiate(&module, empty_context(), WasmQuota::default(), vec![])
            .unwrap();

        let manifest = instance.call_manifest().unwrap();
        assert_eq!(manifest.name, "test-invariant");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.kind, ModuleKind::Invariant);
        assert_eq!(
            manifest.invariant_class,
            Some(WasmInvariantClass::Structural)
        );
    }

    // =========================================================================
    // Invariant check tests
    // =========================================================================

    #[test]
    fn check_invariant_ok() {
        let engine = WasmEngine::new().unwrap();
        let wat = invariant_ok_wat();
        let module = engine.compile(wat.as_bytes()).unwrap();
        let mut instance = engine
            .instantiate(
                &module,
                context_with_strategies(),
                WasmQuota::default(),
                vec![HostCapability::ReadContext],
            )
            .unwrap();

        let result = instance.call_check_invariant().unwrap();
        assert!(result.ok);
        assert!(result.reason.is_none());
    }

    #[test]
    fn check_invariant_violated() {
        let engine = WasmEngine::new().unwrap();
        let wat = invariant_violated_wat();
        let module = engine.compile(wat.as_bytes()).unwrap();
        let mut instance = engine
            .instantiate(&module, empty_context(), WasmQuota::default(), vec![])
            .unwrap();

        let result = instance.call_check_invariant().unwrap();
        assert!(!result.ok);
        assert_eq!(result.reason.as_deref(), Some("need at least 2 strategies"));
        assert_eq!(result.fact_ids, vec!["strat-1"]);
    }

    // =========================================================================
    // Fuel metering tests
    // =========================================================================

    #[test]
    fn fuel_is_consumed() {
        let engine = WasmEngine::new().unwrap();
        let wat = invariant_ok_wat();
        let module = engine.compile(wat.as_bytes()).unwrap();
        let mut instance = engine
            .instantiate(&module, empty_context(), WasmQuota::default(), vec![])
            .unwrap();

        let _ = instance.call_abi_version().unwrap();
        let consumed = instance.fuel_consumed();
        assert!(consumed > 0, "should have consumed some fuel");
    }

    #[test]
    fn fuel_exhaustion_traps() {
        let engine = WasmEngine::new().unwrap();
        let wat = invariant_ok_wat();
        let module = engine.compile(wat.as_bytes()).unwrap();

        // Give very little fuel
        let quota = WasmQuota {
            max_fuel: 1, // almost no fuel
            ..WasmQuota::default()
        };
        let mut instance = engine
            .instantiate(&module, empty_context(), quota, vec![])
            .unwrap();

        // Should trap due to fuel exhaustion
        let result = instance.call_check_invariant();
        assert!(result.is_err());
    }

    // =========================================================================
    // Execution trace tests
    // =========================================================================

    #[test]
    fn trace_records_fuel_and_timing() {
        let engine = WasmEngine::new().unwrap();
        let wat = invariant_ok_wat();
        let module = engine.compile(wat.as_bytes()).unwrap();
        let mut instance = engine
            .instantiate(&module, empty_context(), WasmQuota::default(), vec![])
            .unwrap();

        let _ = instance.call_check_invariant().unwrap();

        let module_id = ModuleId {
            content_hash: "sha256:test".to_string(),
        };
        let manifest = WasmManifest {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            kind: ModuleKind::Invariant,
            invariant_class: Some(WasmInvariantClass::Structural),
            dependencies: vec![],
            capabilities: vec![],
            requires_human_approval: false,
            jtbd: None,
            metadata: HashMap::new(),
        };

        let trace = instance.trace(&module_id, &manifest);
        assert!(trace.fuel_consumed > 0);
        assert_eq!(trace.module_name, "test");
        assert_eq!(trace.outcome, InvocationOutcome::Ok);
    }

    // =========================================================================
    // Negative tests
    // =========================================================================

    #[test]
    fn incompatible_abi_detected() {
        let engine = WasmEngine::new().unwrap();
        // Module returns ABI version 99
        let wat = r#"
            (module
                (memory (export "memory") 1)
                (global $bump (mut i32) (i32.const 1024))
                (func (export "alloc") (param $size i32) (result i32)
                    (local $ptr i32)
                    (local.set $ptr (global.get $bump))
                    (global.set $bump (i32.add (global.get $bump) (local.get $size)))
                    (local.get $ptr)
                )
                (func (export "dealloc") (param $ptr i32) (param $len i32))
                (func (export "converge_abi_version") (result i32)
                    (i32.const 99)
                )
            )
        "#;
        let module = engine.compile(wat.as_bytes()).unwrap();
        let mut instance = engine
            .instantiate(&module, empty_context(), WasmQuota::default(), vec![])
            .unwrap();

        let version = instance.call_abi_version().unwrap();
        assert_ne!(version, WASM_ABI_VERSION);
        assert_eq!(version, 99);
    }

    #[test]
    fn missing_export_is_error() {
        let engine = WasmEngine::new().unwrap();
        // Module with no exports except memory and alloc
        let wat = r#"
            (module
                (memory (export "memory") 1)
                (func (export "alloc") (param $size i32) (result i32) (i32.const 0))
                (func (export "dealloc") (param $ptr i32) (param $len i32))
            )
        "#;
        let module = engine.compile(wat.as_bytes()).unwrap();
        let mut instance = engine
            .instantiate(&module, empty_context(), WasmQuota::default(), vec![])
            .unwrap();

        let result = instance.call_abi_version();
        assert!(result.is_err());
    }
}
