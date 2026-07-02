// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! End-to-end integration tests for the Gherkin → WASM → engine pipeline.
//!
//! Tests marked `#[ignore]` require `wasm32-unknown-unknown` target installed
//! (`rustup target add wasm32-unknown-unknown`). Run with `cargo test -- --ignored`.

use std::collections::HashMap;
use std::sync::Arc;

use converge_core::{Context, ContextState, Engine};
use helm_plugin_runtime::contract::*;
use helm_plugin_runtime::engine::WasmEngine;
use helm_plugin_runtime::integration::{load_and_register, register_wasm_invariants};
use helm_plugin_runtime::store::ModuleStore;

// =============================================================================
// Helpers
// =============================================================================

fn make_engine() -> Arc<WasmEngine> {
    Arc::new(WasmEngine::new().unwrap())
}

fn make_store(wasm_engine: &Arc<WasmEngine>) -> ModuleStore {
    let mut store = ModuleStore::new(Arc::clone(wasm_engine));
    store.set_tenant_quota(
        "test-tenant",
        TenantQuota {
            max_active_modules: 20,
            max_total_module_bytes: 50_000_000,
            per_invocation: WasmQuota::default(),
            allowed_capabilities: vec![
                HostCapability::ReadContext,
                HostCapability::Log,
                HostCapability::Clock,
            ],
        },
    );
    store
}

fn escape_wat(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// WAT module: structural invariant that always passes.
fn structural_ok_wat() -> String {
    let manifest_json = serde_json::to_string(&WasmManifest {
        name: "structural-ok".to_string(),
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

/// WAT module: structural invariant that always fails.
#[allow(dead_code)]
fn structural_violated_wat() -> String {
    let manifest_json = serde_json::to_string(&WasmManifest {
        name: "structural-violated".to_string(),
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

    let result_json = r#"{"ok":false,"reason":"brand safety violation detected"}"#;
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

/// WAT module: acceptance invariant that always passes.
#[allow(dead_code)]
fn acceptance_ok_wat() -> String {
    let manifest_json = serde_json::to_string(&WasmManifest {
        name: "acceptance-ok".to_string(),
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

/// WAT module: acceptance invariant that always fails with fact IDs.
fn acceptance_violated_wat() -> String {
    let manifest_json = serde_json::to_string(&WasmManifest {
        name: "acceptance-violated".to_string(),
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

// =============================================================================
// Happy Path: WASM + Native Invariant Coexistence
// =============================================================================

/// Native invariant for testing coexistence with WASM invariants.
/// Always passes — the point is to verify both native and WASM invariants
/// can be registered and checked in the same engine run.
struct NativeAlwaysOk;

impl converge_core::Invariant for NativeAlwaysOk {
    fn name(&self) -> &str {
        "native-always-ok"
    }

    fn class(&self) -> converge_core::InvariantClass {
        converge_core::InvariantClass::Acceptance
    }

    fn check(&self, _ctx: &dyn Context) -> converge_core::InvariantResult {
        converge_core::InvariantResult::Ok
    }
}

#[tokio::test]
async fn wasm_and_native_invariants_coexist() {
    let wasm_engine = make_engine();
    let mut store = make_store(&wasm_engine);
    let mut engine = Engine::new();

    // Register native invariant
    engine.register_invariant(NativeAlwaysOk);

    // Register WASM invariant
    let wat = structural_ok_wat();
    load_and_register(
        &mut engine,
        &wasm_engine,
        &mut store,
        "test-tenant",
        wat.as_bytes(),
    )
    .unwrap();

    // Both invariants should be registered
    let ctx = ContextState::new();
    let result = engine.run(ctx).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn acceptance_violation_rejects_convergence() {
    let wasm_engine = make_engine();
    let mut store = make_store(&wasm_engine);
    let mut engine = Engine::new();

    // Acceptance invariants are checked when convergence is claimed.
    // With no agents the engine converges immediately, triggering acceptance checks.
    let wat = acceptance_violated_wat();
    load_and_register(
        &mut engine,
        &wasm_engine,
        &mut store,
        "test-tenant",
        wat.as_bytes(),
    )
    .unwrap();

    let ctx = ContextState::new();
    let result = engine.run(ctx).await;

    // The acceptance invariant always fails → engine returns InvariantViolation
    assert!(result.is_err(), "expected acceptance violation");
    let err = result.unwrap_err();
    let err_msg = format!("{err}");
    assert!(
        err_msg.contains("invariant")
            || err_msg.contains("violation")
            || err_msg.contains("Invariant"),
        "error should mention invariant violation, got: {err_msg}"
    );
}

// =============================================================================
// Multi-tenant isolation
// =============================================================================

#[test]
fn multi_tenant_isolation() {
    let wasm_engine = make_engine();
    let mut store = make_store(&wasm_engine);
    store.set_tenant_quota(
        "tenant-b",
        TenantQuota {
            max_active_modules: 10,
            max_total_module_bytes: 50_000_000,
            per_invocation: WasmQuota::default(),
            allowed_capabilities: vec![HostCapability::ReadContext],
        },
    );

    // Tenant A gets structural-ok
    let desc_a = store
        .upload("test-tenant", structural_ok_wat().as_bytes(), None)
        .unwrap();
    store.validate(&desc_a.id, "test-tenant").unwrap();
    store.activate(&desc_a.id).unwrap();

    // Tenant B gets acceptance-violated
    let desc_b = store
        .upload("tenant-b", acceptance_violated_wat().as_bytes(), None)
        .unwrap();
    store.validate(&desc_b.id, "tenant-b").unwrap();
    store.activate(&desc_b.id).unwrap();

    // Tenant A should only see structural-ok
    let a_invariants = store.get_invariants("test-tenant");
    assert_eq!(a_invariants.len(), 1);
    assert_eq!(a_invariants[0].manifest.name, "structural-ok");

    // Tenant B should only see acceptance-violated
    let b_invariants = store.get_invariants("tenant-b");
    assert_eq!(b_invariants.len(), 1);
    assert_eq!(b_invariants[0].manifest.name, "acceptance-violated");

    // Registering for tenant A should only register tenant A's invariants
    let mut engine = Engine::new();
    let ids = register_wasm_invariants(&mut engine, &wasm_engine, &store, "test-tenant").unwrap();
    assert_eq!(ids.len(), 1);
}

// =============================================================================
// Negative: Invalid WASM
// =============================================================================

#[test]
fn invalid_wasm_bytes_produce_clean_error() {
    let wasm_engine = make_engine();
    let mut store = make_store(&wasm_engine);
    let mut engine = Engine::new();

    let result = load_and_register(
        &mut engine,
        &wasm_engine,
        &mut store,
        "test-tenant",
        b"not wasm bytes at all",
    );
    assert!(result.is_err());
}

#[test]
fn wrong_abi_version_rejected() {
    let wasm_engine = make_engine();
    let mut store = make_store(&wasm_engine);

    // Create a module that reports ABI version 99
    let manifest_json = serde_json::to_string(&WasmManifest {
        name: "wrong-abi".to_string(),
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

    let wat = format!(
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
            (func (export "check_invariant") (param $ctx_ptr i32) (param $ctx_len i32) (result i32 i32)
                (i32.const 0) (i32.const 0))
        )
        "#,
        bump_start = manifest_len + 16,
        manifest_escaped = escape_wat(&manifest_json),
        manifest_len = manifest_len,
    );

    let result = store.upload("test-tenant", wat.as_bytes(), None);
    assert!(result.is_err());
}

#[test]
fn denied_capability_rejected() {
    let wasm_engine = make_engine();
    let mut store = ModuleStore::new(Arc::clone(&wasm_engine));
    // Only allow ReadContext, not Clock
    store.set_tenant_quota(
        "restricted",
        TenantQuota {
            max_active_modules: 10,
            max_total_module_bytes: 50_000_000,
            per_invocation: WasmQuota::default(),
            allowed_capabilities: vec![HostCapability::ReadContext],
        },
    );

    // Module requests Clock capability
    let manifest_json = serde_json::to_string(&WasmManifest {
        name: "wants-clock".to_string(),
        version: "1.0.0".to_string(),
        kind: ModuleKind::Invariant,
        invariant_class: Some(WasmInvariantClass::Structural),
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

    let wat = format!(
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
    );

    let desc = store.upload("restricted", wat.as_bytes(), None).unwrap();
    // validate should reject because Clock capability is denied
    let result = store.validate(&desc.id, "restricted");
    assert!(result.is_err());
}

#[test]
fn fuel_exhaustion_returns_violation() {
    let wasm_engine = make_engine();
    let mut store = ModuleStore::new(Arc::clone(&wasm_engine));
    store.set_tenant_quota(
        "fuel-test",
        TenantQuota {
            max_active_modules: 10,
            max_total_module_bytes: 50_000_000,
            per_invocation: WasmQuota {
                max_fuel: 1, // Extremely low fuel
                ..WasmQuota::default()
            },
            allowed_capabilities: vec![HostCapability::ReadContext],
        },
    );

    let wat = structural_ok_wat();
    let desc = store.upload("fuel-test", wat.as_bytes(), None).unwrap();
    store.validate(&desc.id, "fuel-test").unwrap();
    store.activate(&desc.id).unwrap();

    // Creating the adapter with very low fuel may fail or
    // the check call will fail — either way it's handled cleanly
    let compiled = store.get_compiled(&desc.id).unwrap();
    let adapter = helm_plugin_runtime::adapter::WasmInvariant::from_compiled(
        Arc::clone(&wasm_engine),
        compiled,
        desc.manifest.clone(),
        WasmQuota {
            max_fuel: 1,
            ..WasmQuota::default()
        },
    );

    // Err is expected: not enough fuel even for manifest read.
    if let Ok(inv) = adapter {
        // If adapter creation succeeds, check should fail
        let result = converge_core::Invariant::check(&inv, &ContextState::new());
        assert!(result.is_violated());
    }
}

// =============================================================================
// Module lifecycle
// =============================================================================

#[test]
fn full_lifecycle_upload_validate_activate_register_retire() {
    let wasm_engine = make_engine();
    let mut store = make_store(&wasm_engine);
    let mut engine = Engine::new();

    // Upload
    let wat = structural_ok_wat();
    let desc = store.upload("test-tenant", wat.as_bytes(), None).unwrap();
    assert_eq!(desc.state, ModuleState::Compiled);

    // Validate
    store.validate(&desc.id, "test-tenant").unwrap();
    assert_eq!(store.get(&desc.id).unwrap().state, ModuleState::Validated);

    // Activate
    store.activate(&desc.id).unwrap();
    assert_eq!(store.get(&desc.id).unwrap().state, ModuleState::Active);

    // Register with engine
    let ids = register_wasm_invariants(&mut engine, &wasm_engine, &store, "test-tenant").unwrap();
    assert_eq!(ids.len(), 1);

    // Retire
    store.retire(&desc.id).unwrap();
    assert_eq!(store.get(&desc.id).unwrap().state, ModuleState::Retired);

    // After retirement, no active invariants for tenant
    let active = store.get_invariants("test-tenant");
    assert!(active.is_empty());
}

#[test]
fn module_replacement_on_same_name() {
    let wasm_engine = make_engine();
    let mut store = make_store(&wasm_engine);

    // Upload v1
    let wat_v1 = structural_ok_wat();
    let desc_v1 = store
        .upload("test-tenant", wat_v1.as_bytes(), None)
        .unwrap();
    store.validate(&desc_v1.id, "test-tenant").unwrap();
    store.activate(&desc_v1.id).unwrap();

    // Upload v2 with same name but slightly different bytes
    // We append a comment to make the bytes different
    let wat_v2 = format!("{}\n;; v2", structural_ok_wat());
    let desc_v2 = store
        .upload("test-tenant", wat_v2.as_bytes(), None)
        .unwrap();
    store.validate(&desc_v2.id, "test-tenant").unwrap();
    store.activate(&desc_v2.id).unwrap();

    // v1 should be retired, v2 should be active
    assert_eq!(store.get(&desc_v1.id).unwrap().state, ModuleState::Retired);
    assert_eq!(store.get(&desc_v2.id).unwrap().state, ModuleState::Active);

    // Only v2 in active list
    let active = store.get_invariants("test-tenant");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, desc_v2.id);
}
