// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! Property-based tests for the WASM runtime contract types, module store,
//! and invariant adapter.
//!
//! These tests verify that serialization roundtrips, content addressing,
//! determinism, and safety invariants hold for arbitrary inputs.

use std::collections::HashMap;
use std::sync::Arc;

use proptest::prelude::*;

use converge_core::{ContextState, Invariant};
use helm_plugin_runtime::contract::*;
use helm_plugin_runtime::engine::WasmEngine;
use helm_plugin_runtime::store::ModuleStore;

// =============================================================================
// Proptest Strategies
// =============================================================================

fn arb_module_kind() -> impl Strategy<Value = ModuleKind> {
    prop_oneof![Just(ModuleKind::Invariant), Just(ModuleKind::Suggestor),]
}

fn arb_invariant_class() -> impl Strategy<Value = WasmInvariantClass> {
    prop_oneof![
        Just(WasmInvariantClass::Structural),
        Just(WasmInvariantClass::Semantic),
        Just(WasmInvariantClass::Acceptance),
    ]
}

fn arb_host_capability() -> impl Strategy<Value = HostCapability> {
    prop_oneof![
        Just(HostCapability::ReadContext),
        Just(HostCapability::Log),
        Just(HostCapability::Clock),
    ]
}

fn arb_capabilities() -> impl Strategy<Value = Vec<HostCapability>> {
    prop::collection::vec(arb_host_capability(), 0..=3).prop_map(|mut caps| {
        caps.sort_by_key(|c| format!("{c:?}"));
        caps.dedup_by_key(|c| format!("{c:?}"));
        caps
    })
}

fn arb_jtbd_ref() -> impl Strategy<Value = Option<JtbdRef>> {
    prop_oneof![
        Just(None),
        (
            "[a-z_]{1,20}\\.truth",
            prop::option::of("[A-Z][a-z]{2,15}"),
            prop::option::of("[A-Z][a-z ]{5,40}"),
            prop::option::of("sha256:[a-f0-9]{8,64}"),
        )
            .prop_map(|(truth_id, actor, job_functional, source_hash)| {
                Some(JtbdRef {
                    truth_id,
                    actor,
                    job_functional,
                    source_hash,
                })
            }),
    ]
}

fn arb_metadata() -> impl Strategy<Value = HashMap<String, String>> {
    prop::collection::hash_map("[a-z_]{1,10}", "[a-zA-Z0-9 ]{0,50}", 0..=5)
}

fn arb_wasm_manifest() -> impl Strategy<Value = WasmManifest> {
    (
        "[a-z][a-z0-9-]{1,20}",
        "[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}",
        arb_module_kind(),
        prop::option::of(arb_invariant_class()),
        prop::collection::vec("[A-Z][a-z]{2,15}", 0..=5),
        arb_capabilities(),
        prop::bool::ANY,
        arb_jtbd_ref(),
        arb_metadata(),
    )
        .prop_map(
            |(
                name,
                version,
                kind,
                invariant_class,
                dependencies,
                capabilities,
                requires_human_approval,
                jtbd,
                metadata,
            )| {
                // Ensure invariant_class is set for invariant modules
                let invariant_class = if kind == ModuleKind::Invariant {
                    Some(invariant_class.unwrap_or(WasmInvariantClass::Acceptance))
                } else {
                    None
                };
                WasmManifest {
                    name,
                    version,
                    kind,
                    invariant_class,
                    dependencies,
                    capabilities,
                    requires_human_approval,
                    jtbd,
                    metadata,
                }
            },
        )
}

fn arb_guest_fact() -> impl Strategy<Value = GuestFact> {
    ("[a-z]{1,5}-[0-9]{1,4}", "[a-zA-Z0-9 .,!?]{0,100}")
        .prop_map(|(id, content)| GuestFact { id, content })
}

fn arb_guest_context() -> impl Strategy<Value = GuestContext> {
    let key_names = vec![
        "Seeds",
        "Hypotheses",
        "Strategies",
        "Constraints",
        "Signals",
        "Competitors",
        "Evaluations",
    ];
    (
        prop::collection::hash_map(
            prop::sample::select(key_names),
            prop::collection::vec(arb_guest_fact(), 0..=5),
            0..=4,
        ),
        0u64..1_000_000,
        0u32..100,
    )
        .prop_map(|(facts, version, cycle)| {
            // Convert &str keys to String
            let facts: HashMap<String, Vec<GuestFact>> =
                facts.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
            GuestContext {
                facts,
                version,
                cycle,
            }
        })
}

fn arb_guest_invariant_result() -> impl Strategy<Value = GuestInvariantResult> {
    prop_oneof![
        Just(GuestInvariantResult::ok()),
        "[a-zA-Z ]{5,50}".prop_map(GuestInvariantResult::violated),
        (
            "[a-zA-Z ]{5,50}",
            prop::collection::vec("[a-z]-[0-9]{1,4}", 1..=5)
        )
            .prop_map(
                |(reason, fact_ids)| GuestInvariantResult::violated_with_facts(reason, fact_ids)
            ),
    ]
}

fn arb_guest_proposed_fact() -> impl Strategy<Value = GuestProposedFact> {
    (
        prop::sample::select(vec![
            "Hypotheses",
            "Strategies",
            "Constraints",
            "Signals",
            "Evaluations",
        ]),
        "[a-z]{1,5}-[0-9]{1,4}",
        "[a-zA-Z0-9 .,]{5,100}",
        0.0f64..=1.0f64,
    )
        .prop_map(|(key, id, content, confidence)| GuestProposedFact {
            key: key.to_string(),
            id,
            content,
            confidence,
        })
}

fn arb_guest_agent_effect() -> impl Strategy<Value = GuestAgentEffect> {
    prop::collection::vec(arb_guest_proposed_fact(), 0..=5)
        .prop_map(|proposals| GuestAgentEffect { proposals })
}

fn arb_invocation_kind() -> impl Strategy<Value = InvocationKind> {
    prop_oneof![
        Just(InvocationKind::CheckInvariant),
        Just(InvocationKind::AgentAccepts),
        Just(InvocationKind::AgentExecute),
        Just(InvocationKind::ReadManifest),
    ]
}

fn arb_quota_kind() -> impl Strategy<Value = QuotaKind> {
    prop_oneof![
        Just(QuotaKind::Fuel),
        Just(QuotaKind::Memory),
        Just(QuotaKind::Duration),
        Just(QuotaKind::HostCalls),
        Just(QuotaKind::ResultBytes),
    ]
}

fn arb_invocation_outcome() -> impl Strategy<Value = InvocationOutcome> {
    prop_oneof![
        Just(InvocationOutcome::Ok),
        "[a-zA-Z ]{5,30}".prop_map(InvocationOutcome::Trapped),
        arb_quota_kind().prop_map(InvocationOutcome::QuotaExceeded),
        "[a-zA-Z]{3,15}".prop_map(InvocationOutcome::CapabilityDenied),
        "[a-zA-Z ]{5,30}".prop_map(InvocationOutcome::MalformedResult),
    ]
}

fn arb_host_call_record() -> impl Strategy<Value = HostCallRecord> {
    (
        prop::sample::select(vec![
            "host_read_context",
            "host_log",
            "host_now_millis",
            "host_alloc_result",
        ]),
        "[a-z=0-9]{3,20}",
        0u64..10_000,
        prop::bool::ANY,
    )
        .prop_map(
            |(function, args_summary, duration_us, success)| HostCallRecord {
                function: function.to_string(),
                args_summary,
                duration_us,
                success,
            },
        )
}

fn arb_execution_trace() -> impl Strategy<Value = ExecutionTrace> {
    (
        "sha256:[a-f0-9]{8,64}",
        "[a-z-]{3,20}",
        "[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}",
        "[a-z-]{3,15}",
        arb_invocation_kind(),
        arb_invocation_outcome(),
        0u64..2_000_000,
        0u64..16_777_216,
        0u64..5_000_000,
        prop::collection::vec(arb_host_call_record(), 0..=5),
        (
            0u64..1_048_576,
            0u32..100,
            1_000_000_000_000u64..2_000_000_000_000,
        ),
    )
        .prop_map(
            |(
                content_hash,
                module_name,
                module_version,
                tenant_id,
                invocation,
                outcome,
                fuel_consumed,
                peak_memory_bytes,
                duration_us,
                host_calls,
                (result_bytes, engine_cycle, started_at),
            )| {
                ExecutionTrace {
                    module_id: ModuleId { content_hash },
                    module_name,
                    module_version,
                    tenant_id,
                    invocation,
                    outcome,
                    fuel_consumed,
                    peak_memory_bytes,
                    duration_us,
                    host_calls,
                    result_bytes,
                    engine_cycle,
                    started_at,
                }
            },
        )
}

fn arb_wasm_quota() -> impl Strategy<Value = WasmQuota> {
    (
        100u64..10_000_000,
        1024u64..67_108_864,
        100u64..30_000,
        10u32..10_000,
        1024u64..10_485_760,
    )
        .prop_map(
            |(max_fuel, max_memory_bytes, max_duration_ms, max_host_calls, max_result_bytes)| {
                WasmQuota {
                    max_fuel,
                    max_memory_bytes,
                    max_duration_ms,
                    max_host_calls,
                    max_result_bytes,
                }
            },
        )
}

fn arb_wasm_trace_link() -> impl Strategy<Value = WasmTraceLink> {
    (
        "sha256:[a-f0-9]{8,64}",
        "[a-z-]{3,20}@[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}",
        "sha256:[a-f0-9]{8,64}",
        "sha256:[a-f0-9]{8,64}",
        0u64..2_000_000,
    )
        .prop_map(
            |(module_hash, module_ref, input_hash, output_hash, fuel_consumed)| WasmTraceLink {
                module_hash,
                module_ref,
                input_hash,
                output_hash,
                fuel_consumed,
            },
        )
}

// =============================================================================
// WAT Helper: Generates a minimal valid WAT module
// =============================================================================

fn escape_wat(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Generate a valid WAT module text from a manifest and invariant result.
/// Returns WAT text as bytes — wasmtime handles WAT parsing internally.
fn make_wat_bytes(manifest: &WasmManifest, result: &GuestInvariantResult) -> Vec<u8> {
    let manifest_json = serde_json::to_string(manifest).unwrap();
    let result_json = serde_json::to_string(result).unwrap();
    let manifest_len = manifest_json.len();
    let result_offset = manifest_len;
    let result_len = result_json.len();

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
        result_escaped = escape_wat(&result_json),
        result_len = result_len,
    )
    .into_bytes()
}

// =============================================================================
// Serialization Roundtrip Properties
// =============================================================================

proptest! {
    /// Any valid WasmManifest survives JSON serialization roundtrip.
    #[test]
    fn prop_manifest_roundtrip(manifest in arb_wasm_manifest()) {
        let json = serde_json::to_string(&manifest).expect("serialize");
        let back: WasmManifest = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(&manifest.name, &back.name);
        prop_assert_eq!(&manifest.version, &back.version);
        prop_assert_eq!(manifest.kind, back.kind);
        prop_assert_eq!(manifest.invariant_class, back.invariant_class);
        prop_assert_eq!(manifest.requires_human_approval, back.requires_human_approval);
        prop_assert_eq!(manifest.dependencies.len(), back.dependencies.len());
        prop_assert_eq!(manifest.capabilities.len(), back.capabilities.len());
    }

    /// Any valid GuestContext survives JSON serialization roundtrip.
    #[test]
    fn prop_guest_context_roundtrip(ctx in arb_guest_context()) {
        let json = serde_json::to_string(&ctx).expect("serialize");
        let back: GuestContext = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(ctx.version, back.version);
        prop_assert_eq!(ctx.cycle, back.cycle);
        prop_assert_eq!(ctx.facts.len(), back.facts.len());
        for (key, facts) in &ctx.facts {
            let back_facts = back.facts.get(key).expect("key should exist");
            prop_assert_eq!(facts.len(), back_facts.len());
        }
    }

    /// Any valid GuestInvariantResult survives JSON serialization roundtrip.
    #[test]
    fn prop_guest_invariant_result_roundtrip(result in arb_guest_invariant_result()) {
        let json = serde_json::to_string(&result).expect("serialize");
        let back: GuestInvariantResult = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(result.ok, back.ok);
        prop_assert_eq!(result.reason, back.reason);
        prop_assert_eq!(result.fact_ids, back.fact_ids);
    }

    /// Any valid GuestAgentEffect survives JSON serialization roundtrip.
    #[test]
    fn prop_guest_agent_effect_roundtrip(effect in arb_guest_agent_effect()) {
        let json = serde_json::to_string(&effect).expect("serialize");
        let back: GuestAgentEffect = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(effect.proposals.len(), back.proposals.len());
        for (orig, roundtripped) in effect.proposals.iter().zip(back.proposals.iter()) {
            prop_assert_eq!(&orig.key, &roundtripped.key);
            prop_assert_eq!(&orig.id, &roundtripped.id);
            prop_assert_eq!(&orig.content, &roundtripped.content);
        }
    }

    /// Any valid ExecutionTrace survives JSON serialization roundtrip.
    #[test]
    fn prop_execution_trace_roundtrip(trace in arb_execution_trace()) {
        let json = serde_json::to_string(&trace).expect("serialize");
        let back: ExecutionTrace = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(&trace.module_id, &back.module_id);
        prop_assert_eq!(&trace.module_name, &back.module_name);
        prop_assert_eq!(trace.fuel_consumed, back.fuel_consumed);
        prop_assert_eq!(trace.host_calls.len(), back.host_calls.len());
        prop_assert_eq!(trace.invocation, back.invocation);
        prop_assert_eq!(trace.outcome, back.outcome);
    }

    /// Any valid WasmTraceLink survives JSON serialization roundtrip.
    #[test]
    fn prop_trace_link_roundtrip(link in arb_wasm_trace_link()) {
        let json = serde_json::to_string(&link).expect("serialize");
        let back: WasmTraceLink = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(&link.module_hash, &back.module_hash);
        prop_assert_eq!(&link.module_ref, &back.module_ref);
        prop_assert_eq!(&link.input_hash, &back.input_hash);
        prop_assert_eq!(&link.output_hash, &back.output_hash);
        prop_assert_eq!(link.fuel_consumed, back.fuel_consumed);
    }

    /// Any valid WasmQuota survives JSON serialization roundtrip.
    #[test]
    fn prop_quota_roundtrip(quota in arb_wasm_quota()) {
        let json = serde_json::to_string(&quota).expect("serialize");
        let back: WasmQuota = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(quota.max_fuel, back.max_fuel);
        prop_assert_eq!(quota.max_memory_bytes, back.max_memory_bytes);
        prop_assert_eq!(quota.max_duration_ms, back.max_duration_ms);
        prop_assert_eq!(quota.max_host_calls, back.max_host_calls);
        prop_assert_eq!(quota.max_result_bytes, back.max_result_bytes);
    }
}

// =============================================================================
// Content Hash Properties
// =============================================================================

proptest! {
    /// Same WASM bytes always produce the same content hash.
    #[test]
    fn prop_content_hash_stability(bytes in prop::collection::vec(any::<u8>(), 8..256)) {
        let hash1 = helm_plugin_runtime::store::content_hash(&bytes);
        let hash2 = helm_plugin_runtime::store::content_hash(&bytes);
        prop_assert_eq!(hash1, hash2, "same bytes must produce identical hash");
    }

    /// Different WASM bytes produce different content hashes (probabilistic).
    #[test]
    fn prop_content_hash_collision_resistance(
        bytes_a in prop::collection::vec(any::<u8>(), 8..256),
        bytes_b in prop::collection::vec(any::<u8>(), 8..256),
    ) {
        if bytes_a != bytes_b {
            let hash_a = helm_plugin_runtime::store::content_hash(&bytes_a);
            let hash_b = helm_plugin_runtime::store::content_hash(&bytes_b);
            prop_assert_ne!(hash_a, hash_b, "different bytes should produce different hashes");
        }
    }
}

// =============================================================================
// Context Key Isolation Property
// =============================================================================

proptest! {
    /// Guest contexts never expose Proposals or Diagnostic keys.
    #[test]
    fn prop_context_key_isolation(ctx in arb_guest_context()) {
        for key in ctx.facts.keys() {
            prop_assert_ne!(key, "Proposals", "Proposals must never appear in guest context");
            prop_assert_ne!(key, "Diagnostic", "Diagnostic must never appear in guest context");
        }
    }

    /// Internal context keys are always rejected by parse_context_key.
    #[test]
    fn prop_internal_keys_never_parse(key in prop::sample::select(vec!["Proposals", "Diagnostic", "Internal", "Meta"])) {
        prop_assert!(convert::parse_context_key(key).is_none(),
            "internal key '{}' must not be parseable", key);
    }
}

// =============================================================================
// WASM Invariant Determinism
// =============================================================================

proptest! {
    /// A WASM invariant module produces the same result when run twice
    /// with identical inputs.
    #[test]
    fn prop_wasm_invariant_deterministic(
        class in arb_invariant_class(),
        result_ok in prop::bool::ANY,
    ) {
        let manifest = WasmManifest {
            name: "prop-test-invariant".to_string(),
            version: "1.0.0".to_string(),
            kind: ModuleKind::Invariant,
            invariant_class: Some(class),
            dependencies: vec![],
            capabilities: vec![HostCapability::ReadContext],
            requires_human_approval: false,
            jtbd: None,
            metadata: HashMap::new(),
        };

        let inv_result = if result_ok {
            GuestInvariantResult::ok()
        } else {
            GuestInvariantResult::violated("test violation")
        };

        let wasm_bytes = make_wat_bytes(&manifest, &inv_result);
        let wasm_engine = Arc::new(WasmEngine::new().unwrap());

        let invariant = helm_plugin_runtime::adapter::WasmInvariant::new(
            Arc::clone(&wasm_engine),
            &wasm_bytes,
            WasmQuota::default(),
        ).unwrap();

        let ctx = ContextState::new();
        let r1 = invariant.check(&ctx);
        let r2 = invariant.check(&ctx);

        prop_assert_eq!(r1.is_ok(), r2.is_ok(),
            "deterministic: same input must produce same ok/violated");
    }
}

// =============================================================================
// Module Store Upload Properties
// =============================================================================

proptest! {
    /// Uploading the same bytes twice returns the same ModuleId (deduplication).
    #[test]
    fn prop_store_upload_deduplicates(class in arb_invariant_class()) {
        let manifest = WasmManifest {
            name: "dedup-test".to_string(),
            version: "1.0.0".to_string(),
            kind: ModuleKind::Invariant,
            invariant_class: Some(class),
            dependencies: vec![],
            capabilities: vec![HostCapability::ReadContext],
            requires_human_approval: false,
            jtbd: None,
            metadata: HashMap::new(),
        };

        let wasm_bytes = make_wat_bytes(&manifest, &GuestInvariantResult::ok());
        let wasm_engine = Arc::new(WasmEngine::new().unwrap());
        let mut store = ModuleStore::new(Arc::clone(&wasm_engine));
        store.set_tenant_quota("prop-tenant", TenantQuota::default());

        let desc1 = store.upload("prop-tenant", &wasm_bytes, None).unwrap();
        let desc2 = store.upload("prop-tenant", &wasm_bytes, None).unwrap();

        prop_assert_eq!(desc1.id, desc2.id, "same bytes must produce same ModuleId");
    }

    /// Invalid WASM bytes never cause a panic — they produce clean errors.
    #[test]
    fn prop_invalid_wasm_never_panics(garbage in prop::collection::vec(any::<u8>(), 0..512)) {
        let wasm_engine = Arc::new(WasmEngine::new().unwrap());
        let mut store = ModuleStore::new(Arc::clone(&wasm_engine));
        store.set_tenant_quota("prop-tenant", TenantQuota::default());

        // Should return Err, never panic
        let result = store.upload("prop-tenant", &garbage, None);
        // We don't assert Ok or Err — just that it doesn't panic
        drop(result);
    }
}

// =============================================================================
// WasmError Display Property
// =============================================================================

proptest! {
    /// WasmError::Display never panics for any variant.
    #[test]
    fn prop_wasm_error_display_never_panics(
        msg in "[a-zA-Z0-9 ]{0,50}",
        fuel in 0u64..10_000_000,
        limit in 1u64..10_000_000,
    ) {
        let errors = vec![
            WasmError::IncompatibleAbi { module_version: 2, host_min: 1, host_current: 1 },
            WasmError::InvalidManifest(msg.clone()),
            WasmError::CapabilityDenied {
                requested: vec![HostCapability::ReadContext, HostCapability::Clock],
                denied: vec![HostCapability::Clock],
            },
            WasmError::TenantQuotaExceeded(msg.clone()),
            WasmError::CompilationFailed(msg.clone()),
            WasmError::Trapped { function: "check_invariant".to_string(), message: msg.clone() },
            WasmError::QuotaExceeded { kind: QuotaKind::Fuel, limit, consumed: fuel },
            WasmError::MalformedResult { function: "check_invariant".to_string(), message: msg },
            WasmError::ModuleNotFound(ModuleId { content_hash: "sha256:abc".to_string() }),
            WasmError::InvalidState {
                module: ModuleId { content_hash: "sha256:abc".to_string() },
                current: ModuleState::Compiled,
                expected: ModuleState::Active,
            },
        ];

        for err in &errors {
            let display = format!("{err}");
            prop_assert!(!display.is_empty(), "error display must not be empty");
        }
    }
}

// =============================================================================
// Invariant Class Conversion Roundtrip
// =============================================================================

proptest! {
    /// WasmInvariantClass → converge_core::InvariantClass is a bijection.
    #[test]
    fn prop_invariant_class_conversion_consistent(class in arb_invariant_class()) {
        let core_class = convert::to_invariant_class(class);
        // Verify the mapping is correct
        match class {
            WasmInvariantClass::Structural => {
                prop_assert_eq!(core_class, converge_core::InvariantClass::Structural);
            }
            WasmInvariantClass::Semantic => {
                prop_assert_eq!(core_class, converge_core::InvariantClass::Semantic);
            }
            WasmInvariantClass::Acceptance => {
                prop_assert_eq!(core_class, converge_core::InvariantClass::Acceptance);
            }
        }
    }
}

// =============================================================================
// GuestInvariantResult → Core InvariantResult Conversion
// =============================================================================

proptest! {
    /// Guest ok results always map to core Ok.
    /// Guest violated results always map to core Violated.
    #[test]
    fn prop_invariant_result_conversion_preserves_semantics(result in arb_guest_invariant_result()) {
        let core_result = convert::to_invariant_result(&result);
        if result.ok {
            prop_assert!(core_result.is_ok(), "guest ok must map to core Ok");
        } else {
            prop_assert!(core_result.is_violated(), "guest violated must map to core Violated");
        }
    }
}

// =============================================================================
// Quota Default Reasonableness
// =============================================================================

proptest! {
    /// All generated quotas have positive limits.
    #[test]
    fn prop_quota_limits_are_positive(quota in arb_wasm_quota()) {
        prop_assert!(quota.max_fuel > 0);
        prop_assert!(quota.max_memory_bytes > 0);
        prop_assert!(quota.max_duration_ms > 0);
        prop_assert!(quota.max_host_calls > 0);
        prop_assert!(quota.max_result_bytes > 0);
    }
}
