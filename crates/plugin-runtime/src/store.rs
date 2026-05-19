// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! WASM module store with lifecycle management.
//!
//! Manages the full lifecycle of tenant-supplied WASM modules:
//! upload → validate → activate → retire.
//!
//! # Module Lifecycle
//!
//! ```text
//! upload()     → Module<Compiled>   ABI + manifest checked
//! validate()   → Module<Validated>  Quota + capability audit
//! activate()   → Module<Active>     Registered, callable
//! retire()     → Module<Retired>    Replaced, kept for audit
//! ```
//!
//! # Content Addressing
//!
//! Modules are identified by SHA-256 content hash. Uploading the same
//! bytes twice returns the existing descriptor (deduplication).
//!
use std::collections::HashMap;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::contract::*;
use super::engine::{CompiledModule, WasmEngine};
use super::signing::{ModuleSignature, SignaturePolicy, TrustedKeySet, verify_module_with_policy};

/// Module store managing the lifecycle of WASM modules.
///
/// Stores compiled modules, tracks their lifecycle state, and enforces
/// tenant quotas and capability policies.
pub struct ModuleStore {
    engine: Arc<WasmEngine>,
    modules: HashMap<ModuleId, ModuleDescriptor>,
    compiled: HashMap<ModuleId, Arc<CompiledModule>>,
    active_by_tenant: HashMap<String, Vec<ModuleId>>,
    tenant_quotas: HashMap<String, TenantQuota>,
    signature_policy: SignaturePolicy,
    trusted_keys: TrustedKeySet,
    logical_clock: u64,
}

impl ModuleStore {
    /// Create a new empty module store with signature verification disabled.
    ///
    /// Use [`ModuleStore::with_signing`] for production deployments.
    pub fn new(engine: Arc<WasmEngine>) -> Self {
        Self {
            engine,
            modules: HashMap::new(),
            compiled: HashMap::new(),
            active_by_tenant: HashMap::new(),
            tenant_quotas: HashMap::new(),
            signature_policy: SignaturePolicy::Disabled,
            trusted_keys: TrustedKeySet::empty(),
            logical_clock: 0,
        }
    }

    /// Create a module store with signature verification enabled.
    ///
    /// In production, use `SignaturePolicy::Enforce` with a populated
    /// `TrustedKeySet`. Unsigned or tampered modules will be rejected.
    pub fn with_signing(
        engine: Arc<WasmEngine>,
        policy: SignaturePolicy,
        trusted_keys: TrustedKeySet,
    ) -> Self {
        Self {
            engine,
            modules: HashMap::new(),
            compiled: HashMap::new(),
            active_by_tenant: HashMap::new(),
            tenant_quotas: HashMap::new(),
            signature_policy: policy,
            trusted_keys,
            logical_clock: 0,
        }
    }

    fn tick(&mut self) -> u64 {
        self.logical_clock = self.logical_clock.saturating_add(1);
        self.logical_clock
    }

    /// Set the quota policy for a tenant.
    pub fn set_tenant_quota(&mut self, tenant_id: &str, quota: TenantQuota) {
        self.tenant_quotas.insert(tenant_id.to_string(), quota);
    }

    /// Upload a WASM module for a tenant.
    ///
    /// 1. Verifies the module signature against the store's policy
    /// 2. Content-hashes the bytes to produce a `ModuleId`
    /// 3. Returns existing descriptor if duplicate (dedup)
    /// 4. Compiles and instantiates temporarily to read ABI version and manifest
    /// 5. Validates ABI compatibility and manifest completeness
    /// 6. Stores the compiled module with state `Compiled`
    ///
    /// # Errors
    ///
    /// - `WasmError::SignatureMissing` if policy requires a signature
    /// - `WasmError::SignatureInvalid` if the signature is invalid or signer untrusted
    /// - `WasmError::CompilationFailed` if the bytes are not valid WASM
    /// - `WasmError::IncompatibleAbi` if the ABI version is wrong
    /// - `WasmError::InvalidManifest` if the manifest is missing or invalid
    pub fn upload(
        &mut self,
        tenant_id: &str,
        wasm_bytes: &[u8],
        signature: Option<&ModuleSignature>,
    ) -> Result<ModuleDescriptor, WasmError> {
        // Verify signature BEFORE any compilation (fail fast on tampered modules)
        verify_module_with_policy(
            wasm_bytes,
            signature,
            &self.trusted_keys,
            &self.signature_policy,
        )?;

        let module_id = content_hash(wasm_bytes);

        // Dedup: return existing if already uploaded
        if let Some(existing) = self.modules.get(&module_id) {
            return Ok(existing.clone());
        }

        // Compile
        let compiled = Arc::new(self.engine.compile(wasm_bytes)?);

        // Instantiate temporarily to read ABI version and manifest
        let empty_ctx = GuestContext {
            facts: HashMap::new(),
            version: 0,
            cycle: 0,
        };
        let mut instance =
            self.engine
                .instantiate(&compiled, empty_ctx, WasmQuota::default(), vec![])?;

        // Check ABI version
        let abi_version = instance.call_abi_version()?;
        if abi_version < WASM_ABI_MIN_VERSION || abi_version > WASM_ABI_VERSION {
            return Err(WasmError::IncompatibleAbi {
                module_version: abi_version,
                host_min: WASM_ABI_MIN_VERSION,
                host_current: WASM_ABI_VERSION,
            });
        }

        // Read manifest
        let manifest = instance.call_manifest()?;

        // Validate manifest
        if manifest.name.is_empty() {
            return Err(WasmError::InvalidManifest(
                "module name cannot be empty".to_string(),
            ));
        }
        if manifest.kind == ModuleKind::Invariant && manifest.invariant_class.is_none() {
            return Err(WasmError::InvalidManifest(
                "invariant module must declare invariant_class".to_string(),
            ));
        }

        let now = self.tick();

        let descriptor = ModuleDescriptor {
            id: module_id.clone(),
            tenant_id: tenant_id.to_string(),
            manifest,
            state: ModuleState::Compiled,
            size_bytes: wasm_bytes.len() as u64,
            uploaded_at: now,
            activated_at: None,
            replaces: None,
            signature: signature.cloned(),
        };

        self.modules.insert(module_id.clone(), descriptor.clone());
        self.compiled.insert(module_id, compiled);

        Ok(descriptor)
    }

    /// Validate a compiled module against tenant quotas and policies.
    ///
    /// Transitions the module from `Compiled` to `Validated`.
    ///
    /// # Errors
    ///
    /// - `WasmError::InvalidState` if the module is not in `Compiled` state
    /// - `WasmError::TenantQuotaExceeded` if tenant limits would be exceeded
    /// - `WasmError::CapabilityDenied` if the module requests disallowed capabilities
    pub fn validate(&mut self, module_id: &ModuleId, tenant_id: &str) -> Result<(), WasmError> {
        let descriptor = self
            .modules
            .get(module_id)
            .ok_or_else(|| WasmError::ModuleNotFound(module_id.clone()))?;

        // Check state
        if descriptor.state != ModuleState::Compiled {
            return Err(WasmError::InvalidState {
                module: module_id.clone(),
                current: descriptor.state,
                expected: ModuleState::Compiled,
            });
        }

        // Check tenant quota
        let quota = self
            .tenant_quotas
            .get(tenant_id)
            .cloned()
            .unwrap_or_default();

        let active_count = self
            .active_by_tenant
            .get(tenant_id)
            .map(|v| v.len() as u32)
            .unwrap_or(0);
        if active_count >= quota.max_active_modules {
            return Err(WasmError::TenantQuotaExceeded(format!(
                "max active modules ({}) reached",
                quota.max_active_modules
            )));
        }

        // Check total module bytes
        let total_bytes: u64 = self
            .modules
            .values()
            .filter(|m| m.tenant_id == tenant_id && m.state == ModuleState::Active)
            .map(|m| m.size_bytes)
            .sum();
        if total_bytes + descriptor.size_bytes > quota.max_total_module_bytes {
            return Err(WasmError::TenantQuotaExceeded(format!(
                "total module bytes would exceed {} bytes",
                quota.max_total_module_bytes
            )));
        }

        // Check capabilities
        let denied: Vec<HostCapability> = descriptor
            .manifest
            .capabilities
            .iter()
            .filter(|cap| !quota.allowed_capabilities.contains(cap))
            .copied()
            .collect();
        if !denied.is_empty() {
            return Err(WasmError::CapabilityDenied {
                requested: descriptor.manifest.capabilities.clone(),
                denied,
            });
        }

        // Transition to Validated
        if let Some(desc) = self.modules.get_mut(module_id) {
            desc.state = ModuleState::Validated;
        }

        Ok(())
    }

    /// Activate a validated module.
    ///
    /// Transitions from `Validated` to `Active`. If another module with
    /// the same name exists for this tenant, it is retired (replaced).
    ///
    /// # Errors
    ///
    /// - `WasmError::InvalidState` if the module is not in `Validated` state
    /// - `WasmError::ModuleNotFound` if the module ID doesn't exist
    pub fn activate(&mut self, module_id: &ModuleId) -> Result<(), WasmError> {
        let descriptor = self
            .modules
            .get(module_id)
            .ok_or_else(|| WasmError::ModuleNotFound(module_id.clone()))?;

        if descriptor.state != ModuleState::Validated {
            return Err(WasmError::InvalidState {
                module: module_id.clone(),
                current: descriptor.state,
                expected: ModuleState::Validated,
            });
        }

        let tenant_id = descriptor.tenant_id.clone();
        let module_name = descriptor.manifest.name.clone();

        // Find and retire any existing active module with the same name
        let to_retire: Option<ModuleId> = self
            .active_by_tenant
            .get(&tenant_id)
            .and_then(|active_ids| {
                active_ids.iter().find(|id| {
                    self.modules
                        .get(id)
                        .map(|m| m.manifest.name == module_name)
                        .unwrap_or(false)
                })
            })
            .cloned();

        if let Some(old_id) = &to_retire {
            if let Some(old_desc) = self.modules.get_mut(old_id) {
                old_desc.state = ModuleState::Retired;
            }
            if let Some(active) = self.active_by_tenant.get_mut(&tenant_id) {
                active.retain(|id| id != old_id);
            }
        }

        let now = self.tick();

        // Activate
        if let Some(desc) = self.modules.get_mut(module_id) {
            desc.state = ModuleState::Active;
            desc.activated_at = Some(now);
            desc.replaces = to_retire;
        }

        self.active_by_tenant
            .entry(tenant_id)
            .or_default()
            .push(module_id.clone());

        Ok(())
    }

    /// Retire an active module.
    ///
    /// Transitions from `Active` to `Retired` and removes from the
    /// active index.
    ///
    /// # Errors
    ///
    /// - `WasmError::InvalidState` if the module is not in `Active` state
    /// - `WasmError::ModuleNotFound` if the module ID doesn't exist
    pub fn retire(&mut self, module_id: &ModuleId) -> Result<(), WasmError> {
        let descriptor = self
            .modules
            .get(module_id)
            .ok_or_else(|| WasmError::ModuleNotFound(module_id.clone()))?;

        if descriptor.state != ModuleState::Active {
            return Err(WasmError::InvalidState {
                module: module_id.clone(),
                current: descriptor.state,
                expected: ModuleState::Active,
            });
        }

        let tenant_id = descriptor.tenant_id.clone();

        if let Some(desc) = self.modules.get_mut(module_id) {
            desc.state = ModuleState::Retired;
        }

        if let Some(active) = self.active_by_tenant.get_mut(&tenant_id) {
            active.retain(|id| id != module_id);
        }

        Ok(())
    }

    /// Get all active module descriptors for a tenant.
    pub fn get_active(&self, tenant_id: &str) -> Vec<&ModuleDescriptor> {
        self.active_by_tenant
            .get(tenant_id)
            .map(|ids| ids.iter().filter_map(|id| self.modules.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get active invariant modules for a tenant.
    pub fn get_invariants(&self, tenant_id: &str) -> Vec<&ModuleDescriptor> {
        self.get_active(tenant_id)
            .into_iter()
            .filter(|m| m.manifest.kind == ModuleKind::Invariant)
            .collect()
    }

    /// Get active agent modules for a tenant.
    pub fn get_agents(&self, tenant_id: &str) -> Vec<&ModuleDescriptor> {
        self.get_active(tenant_id)
            .into_iter()
            .filter(|m| m.manifest.kind == ModuleKind::Suggestor)
            .collect()
    }

    /// Get a module descriptor by ID.
    pub fn get(&self, module_id: &ModuleId) -> Option<&ModuleDescriptor> {
        self.modules.get(module_id)
    }

    /// Get the compiled module for instantiation.
    pub fn get_compiled(&self, module_id: &ModuleId) -> Option<Arc<CompiledModule>> {
        self.compiled.get(module_id).cloned()
    }
}

/// Compute the SHA-256 content hash of WASM bytes.
pub fn content_hash(bytes: &[u8]) -> ModuleId {
    let hash = Sha256::digest(bytes);
    ModuleId {
        content_hash: format!("sha256:{}", hex::encode(hash)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // =========================================================================
    // WAT module helpers (reused from adapter tests)
    // =========================================================================

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

    /// WAT module with wrong ABI version.
    fn bad_abi_wat() -> String {
        let manifest_json = serde_json::to_string(&WasmManifest {
            name: "bad-abi".to_string(),
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
                (func (export "converge_abi_version") (result i32) (i32.const 99))
                (func (export "converge_manifest") (result i32 i32)
                    (i32.const 0) (i32.const {manifest_len}))
            )
            "#,
            bump_start = manifest_len + 16,
            manifest_escaped = escape_wat(&manifest_json),
            manifest_len = manifest_len,
        )
    }

    /// WAT module with invalid manifest (empty name).
    fn bad_manifest_wat() -> String {
        let manifest_json = serde_json::to_string(&WasmManifest {
            name: String::new(),
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

    /// Invariant WAT that requests a disallowed capability.
    fn capability_clock_wat() -> String {
        let manifest_json = serde_json::to_string(&WasmManifest {
            name: "needs-clock".to_string(),
            version: "1.0.0".to_string(),
            kind: ModuleKind::Invariant,
            invariant_class: Some(WasmInvariantClass::Semantic),
            dependencies: vec![],
            capabilities: vec![HostCapability::ReadContext, HostCapability::Clock],
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

    fn escape_wat(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }

    fn make_engine() -> Arc<WasmEngine> {
        Arc::new(WasmEngine::new().unwrap())
    }

    fn restrictive_quota() -> TenantQuota {
        TenantQuota {
            max_active_modules: 1,
            max_total_module_bytes: 50 * 1024 * 1024,
            per_invocation: WasmQuota::default(),
            allowed_capabilities: vec![HostCapability::ReadContext],
        }
    }

    // =========================================================================
    // Upload tests
    // =========================================================================

    #[test]
    fn upload_valid_invariant_module() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);

        let wat = invariant_ok_wat();
        let desc = store.upload("tenant-1", wat.as_bytes(), None).unwrap();

        assert_eq!(desc.state, ModuleState::Compiled);
        assert_eq!(desc.tenant_id, "tenant-1");
        assert_eq!(desc.manifest.name, "test-invariant");
        assert_eq!(desc.manifest.kind, ModuleKind::Invariant);
        assert!(desc.id.content_hash.starts_with("sha256:"));
    }

    #[test]
    fn upload_valid_agent_module() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);

        let wat = agent_module_wat();
        let desc = store.upload("tenant-1", wat.as_bytes(), None).unwrap();

        assert_eq!(desc.manifest.kind, ModuleKind::Suggestor);
        assert_eq!(desc.manifest.name, "test-agent");
    }

    #[test]
    fn upload_duplicate_returns_existing() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);

        let wat = invariant_ok_wat();
        let desc1 = store.upload("tenant-1", wat.as_bytes(), None).unwrap();
        let desc2 = store.upload("tenant-1", wat.as_bytes(), None).unwrap();

        assert_eq!(desc1.id, desc2.id);
    }

    // =========================================================================
    // Validate tests
    // =========================================================================

    #[test]
    fn validate_with_sufficient_quota() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);
        store.set_tenant_quota("tenant-1", TenantQuota::default());

        let wat = invariant_ok_wat();
        let desc = store.upload("tenant-1", wat.as_bytes(), None).unwrap();
        store.validate(&desc.id, "tenant-1").unwrap();

        let updated = store.get(&desc.id).unwrap();
        assert_eq!(updated.state, ModuleState::Validated);
    }

    // =========================================================================
    // Activate tests
    // =========================================================================

    #[test]
    fn activate_makes_module_active() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);
        store.set_tenant_quota("tenant-1", TenantQuota::default());

        let wat = invariant_ok_wat();
        let desc = store.upload("tenant-1", wat.as_bytes(), None).unwrap();
        store.validate(&desc.id, "tenant-1").unwrap();
        store.activate(&desc.id).unwrap();

        let updated = store.get(&desc.id).unwrap();
        assert_eq!(updated.state, ModuleState::Active);
        assert!(updated.activated_at.is_some());

        let active = store.get_active("tenant-1");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].manifest.name, "test-invariant");
    }

    #[test]
    fn activate_appears_in_get_invariants() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);
        store.set_tenant_quota("tenant-1", TenantQuota::default());

        let wat = invariant_ok_wat();
        let desc = store.upload("tenant-1", wat.as_bytes(), None).unwrap();
        store.validate(&desc.id, "tenant-1").unwrap();
        store.activate(&desc.id).unwrap();

        assert_eq!(store.get_invariants("tenant-1").len(), 1);
        assert_eq!(store.get_agents("tenant-1").len(), 0);
    }

    // =========================================================================
    // Retire tests
    // =========================================================================

    #[test]
    fn retire_removes_from_active() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);
        store.set_tenant_quota("tenant-1", TenantQuota::default());

        let wat = invariant_ok_wat();
        let desc = store.upload("tenant-1", wat.as_bytes(), None).unwrap();
        store.validate(&desc.id, "tenant-1").unwrap();
        store.activate(&desc.id).unwrap();
        store.retire(&desc.id).unwrap();

        let updated = store.get(&desc.id).unwrap();
        assert_eq!(updated.state, ModuleState::Retired);
        assert!(store.get_active("tenant-1").is_empty());
    }

    // =========================================================================
    // Negative tests
    // =========================================================================

    #[test]
    fn upload_wrong_abi_version_fails() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);

        let wat = bad_abi_wat();
        let result = store.upload("tenant-1", wat.as_bytes(), None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WasmError::IncompatibleAbi {
                module_version: 99,
                ..
            }
        ));
    }

    #[test]
    fn upload_invalid_manifest_fails() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);

        let wat = bad_manifest_wat();
        let result = store.upload("tenant-1", wat.as_bytes(), None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WasmError::InvalidManifest(_)));
    }

    #[test]
    fn upload_invalid_wasm_fails() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);

        let result = store.upload("tenant-1", b"not wasm", None);
        assert!(result.is_err());
    }

    #[test]
    fn validate_too_many_modules_fails() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);
        store.set_tenant_quota("tenant-1", restrictive_quota());

        // Upload and activate first module (different WAT to get different hash)
        let wat1 = capability_clock_wat();
        let desc1 = store.upload("tenant-1", wat1.as_bytes(), None).unwrap();
        // Use default quota for first validate (allows Clock)
        store.set_tenant_quota("tenant-1", TenantQuota::default());
        store.validate(&desc1.id, "tenant-1").unwrap();
        store.activate(&desc1.id).unwrap();

        // Set restrictive quota: max 1 active module
        store.set_tenant_quota("tenant-1", restrictive_quota());

        // Upload second module
        let wat2 = invariant_ok_wat();
        let desc2 = store.upload("tenant-1", wat2.as_bytes(), None).unwrap();

        // Validate should fail — already at max
        let result = store.validate(&desc2.id, "tenant-1");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WasmError::TenantQuotaExceeded(_)
        ));
    }

    #[test]
    fn validate_disallowed_capability_fails() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);
        // Only allow ReadContext, not Clock
        store.set_tenant_quota("tenant-1", restrictive_quota());

        let wat = capability_clock_wat();
        let desc = store.upload("tenant-1", wat.as_bytes(), None).unwrap();

        let result = store.validate(&desc.id, "tenant-1");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WasmError::CapabilityDenied { denied, .. } if denied.contains(&HostCapability::Clock)
        ));
    }

    #[test]
    fn activate_from_compiled_state_fails() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);

        let wat = invariant_ok_wat();
        let desc = store.upload("tenant-1", wat.as_bytes(), None).unwrap();

        // Try to activate without validating first
        let result = store.activate(&desc.id);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WasmError::InvalidState {
                current: ModuleState::Compiled,
                expected: ModuleState::Validated,
                ..
            }
        ));
    }

    #[test]
    fn retire_non_active_module_fails() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);
        store.set_tenant_quota("tenant-1", TenantQuota::default());

        let wat = invariant_ok_wat();
        let desc = store.upload("tenant-1", wat.as_bytes(), None).unwrap();
        store.validate(&desc.id, "tenant-1").unwrap();

        // Try to retire a Validated (not Active) module
        let result = store.retire(&desc.id);
        assert!(result.is_err());
    }

    // =========================================================================
    // Content addressing
    // =========================================================================

    #[test]
    fn content_hash_is_deterministic() {
        let bytes = b"hello wasm";
        let id1 = content_hash(bytes);
        let id2 = content_hash(bytes);
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_bytes_produce_different_hashes() {
        let id1 = content_hash(b"module-a");
        let id2 = content_hash(b"module-b");
        assert_ne!(id1, id2);
    }

    // =========================================================================
    // Query tests
    // =========================================================================

    #[test]
    fn get_active_empty_for_unknown_tenant() {
        let engine = make_engine();
        let store = ModuleStore::new(engine);
        assert!(store.get_active("nonexistent").is_empty());
    }

    #[test]
    fn get_compiled_returns_cached_module() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);

        let wat = invariant_ok_wat();
        let desc = store.upload("tenant-1", wat.as_bytes(), None).unwrap();
        assert!(store.get_compiled(&desc.id).is_some());
    }

    // =========================================================================
    // Property test
    // =========================================================================

    #[test]
    fn lifecycle_sequence_leaves_consistent_state() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine);
        store.set_tenant_quota("tenant-1", TenantQuota::default());

        let wat = invariant_ok_wat();
        let desc = store.upload("tenant-1", wat.as_bytes(), None).unwrap();
        let id = desc.id.clone();

        // Compiled → Validated → Active → Retired
        assert_eq!(store.get(&id).unwrap().state, ModuleState::Compiled);

        store.validate(&id, "tenant-1").unwrap();
        assert_eq!(store.get(&id).unwrap().state, ModuleState::Validated);

        store.activate(&id).unwrap();
        assert_eq!(store.get(&id).unwrap().state, ModuleState::Active);
        assert_eq!(store.get_active("tenant-1").len(), 1);

        store.retire(&id).unwrap();
        assert_eq!(store.get(&id).unwrap().state, ModuleState::Retired);
        assert!(store.get_active("tenant-1").is_empty());
    }

    // =========================================================================
    // Signature verification tests (store integration)
    // =========================================================================

    #[test]
    fn enforce_policy_rejects_unsigned_upload() {
        let engine = make_engine();
        let trusted = TrustedKeySet::empty();
        let mut store = ModuleStore::with_signing(engine, SignaturePolicy::Enforce, trusted);

        let wat = invariant_ok_wat();
        let result = store.upload("tenant-1", wat.as_bytes(), None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WasmError::SignatureMissing(_)
        ));
    }

    #[test]
    fn enforce_policy_accepts_signed_upload() {
        use crate::signing::sign_module;
        use ed25519_dalek::SigningKey;

        let signing_key = SigningKey::from_bytes(&[11; 32]);
        let verifying_key = signing_key.verifying_key();

        let mut trusted = TrustedKeySet::empty();
        trusted.add_key(verifying_key);

        let engine = make_engine();
        let mut store = ModuleStore::with_signing(engine, SignaturePolicy::Enforce, trusted);

        let wat = invariant_ok_wat();
        let sig = sign_module(wat.as_bytes(), &signing_key);
        let result = store.upload("tenant-1", wat.as_bytes(), Some(&sig));
        assert!(result.is_ok());
    }

    #[test]
    fn enforce_policy_rejects_tampered_module() {
        use crate::signing::sign_module;
        use ed25519_dalek::SigningKey;

        let signing_key = SigningKey::from_bytes(&[12; 32]);
        let verifying_key = signing_key.verifying_key();

        let mut trusted = TrustedKeySet::empty();
        trusted.add_key(verifying_key);

        let engine = make_engine();
        let mut store = ModuleStore::with_signing(engine, SignaturePolicy::Enforce, trusted);

        let wat = invariant_ok_wat();
        let sig = sign_module(wat.as_bytes(), &signing_key);

        // Tamper: append bytes to the module
        let mut tampered = wat.into_bytes();
        tampered.extend_from_slice(b"\x00\x00\x00\x00");

        let result = store.upload("tenant-1", &tampered, Some(&sig));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WasmError::SignatureInvalid(_)
        ));
    }

    #[test]
    fn enforce_policy_rejects_untrusted_signer() {
        use crate::signing::sign_module;
        use ed25519_dalek::SigningKey;

        // Sign with key A
        let signing_key_a = SigningKey::from_bytes(&[13; 32]);

        // Trust only key B
        let signing_key_b = SigningKey::from_bytes(&[14; 32]);
        let verifying_key_b = signing_key_b.verifying_key();

        let mut trusted = TrustedKeySet::empty();
        trusted.add_key(verifying_key_b);

        let engine = make_engine();
        let mut store = ModuleStore::with_signing(engine, SignaturePolicy::Enforce, trusted);

        let wat = invariant_ok_wat();
        let sig = sign_module(wat.as_bytes(), &signing_key_a);

        let result = store.upload("tenant-1", wat.as_bytes(), Some(&sig));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WasmError::SignatureInvalid(_)
        ));
    }

    #[test]
    fn disabled_policy_ignores_missing_signature() {
        let engine = make_engine();
        let mut store = ModuleStore::new(engine); // default = Disabled

        let wat = invariant_ok_wat();
        let result = store.upload("tenant-1", wat.as_bytes(), None);
        assert!(result.is_ok());
    }
}
