// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! # Helm Plugin Runtime Contract v0.1
//!
//! This module defines the **constitutional boundary** between the Converge
//! platform (host) and tenant-supplied WASM modules (guests). It governs
//! how custom JTBD invariants and agents are loaded, sandboxed, executed,
//! and audited before they interact with Converge engine APIs.
//!
//! # Design Principles
//!
//! This contract is derived from the nine Converge axioms:
//!
//! | Axiom | Enforcement in WASM |
//! |-------|---------------------|
//! | Agents Suggest, Engine Decides | Guest returns `GuestEffect`, never mutates `Context` |
//! | Append-Only Truth | Guest receives read-only context bytes; cannot delete facts |
//! | Explicit Authority | Capabilities declared in manifest; host validates before grant |
//! | Safety by Construction | Type-state `Module<Compiled>` → `Module<Validated>` → `Module<Active>` |
//! | Transparent Determinism | Every execution produces `WasmTraceLink` with module hash |
//! | Human Authority First-Class | Manifest `requires_human_approval` flag gates auto-promotion |
//! | No Hidden Work | Fuel metering, memory caps, and host call logs in trace envelope |
//! | Scale by Intent Replication | Modules are content-addressed; same hash = same behavior |
//! | System Tells Truth About Itself | `ExecutionTrace` records fuel consumed, host calls, timing |
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    Helm Plugin Host                       │
//! │                                                         │
//! │  InvariantRegistry         Engine.run() loop            │
//! │  ┌──────────────┐         ┌──────────────────┐         │
//! │  │ Box<dyn      │         │ find_eligible()  │         │
//! │  │  Invariant>   │────────│ execute_agents() │         │
//! │  │              │         │ merge_effects()  │         │
//! │  │ WasmInvariant│         │ check_invariants │         │
//! │  │ WasmAgent    │         └──────────────────┘         │
//! │  └──────┬───────┘                                      │
//! │         │                                               │
//! │  ───────┼─── WASM Sandbox Boundary ─────────────────── │
//! │         │                                               │
//! │  ┌──────▼───────┐  ┌──────────────┐  ┌─────────────┐  │
//! │  │ WasmRuntime   │  │ ModuleStore  │  │ FuelMeter   │  │
//! │  │ (wasmtime)    │  │ (registry)   │  │ (quotas)    │  │
//! │  └──────┬───────┘  └──────────────┘  └─────────────┘  │
//! │         │                                               │
//! │  ┌──────▼───────────────────────────────────────────┐  │
//! │  │           Guest WASM Module (.wasm)               │  │
//! │  │                                                   │  │
//! │  │  Exports:                 Imports (host fns):     │  │
//! │  │  ├─ converge_abi_version  ├─ host_read_context    │  │
//! │  │  ├─ converge_manifest     ├─ host_log             │  │
//! │  │  ├─ check_invariant       ├─ host_now_millis      │  │
//! │  │  ├─ agent_name            ├─ host_alloc_result    │  │
//! │  │  ├─ agent_dependencies    │                       │  │
//! │  │  ├─ agent_accepts         │                       │  │
//! │  │  └─ agent_execute         │                       │  │
//! │  └───────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Module Lifecycle
//!
//! ```text
//! Upload (.wasm bytes)
//!     │
//!     ▼
//! Module<Uploaded>  ── validate ABI version, parse manifest
//!     │
//!     ▼
//! Module<Compiled>  ── wasmtime pre-compilation, content-hash
//!     │
//!     ▼
//! Module<Validated> ── capability audit, quota check, signature verify
//!     │
//!     ▼
//! Module<Active>    ── registered in engine, callable
//!     │
//!     ▼
//! Module<Retired>   ── replaced by newer version, kept for audit
//! ```
//!
//! # Guest ABI (v1)
//!
//! The guest module must export a specific set of symbols and may import
//! a constrained set of host functions. The ABI is versioned so the host
//! can reject incompatible modules at upload time.
//!
//! ## Required Exports (all modules)
//!
//! | Export                    | Signature                        | Purpose |
//! |--------------------------|----------------------------------|---------|
//! | `converge_abi_version`   | `() -> u32`                      | ABI compatibility check |
//! | `converge_manifest`      | `() -> (ptr: u32, len: u32)`     | JSON manifest describing module |
//! | `alloc`                  | `(size: u32) -> ptr: u32`        | Guest allocator for host→guest data |
//! | `dealloc`                | `(ptr: u32, len: u32)`           | Guest deallocator |
//!
//! ## Invariant Module Exports
//!
//! | Export                    | Signature                        | Purpose |
//! |--------------------------|----------------------------------|---------|
//! | `check_invariant`        | `(ctx_ptr: u32, ctx_len: u32) -> (ptr: u32, len: u32)` | Core invariant check |
//!
//! ## Suggestor Module Exports (optional, for full JTBD agents)
//!
//! | Export                    | Signature                        | Purpose |
//! |--------------------------|----------------------------------|---------|
//! | `agent_accepts`          | `(ctx_ptr: u32, ctx_len: u32) -> u32` | Eligibility check (0 or 1) |
//! | `agent_execute`          | `(ctx_ptr: u32, ctx_len: u32) -> (ptr: u32, len: u32)` | Produce effects |
//!
//! ## Host Imports (provided by Helm)
//!
//! | Import                   | Signature                        | Capability |
//! |--------------------------|----------------------------------|------------|
//! | `host_read_context`      | `(key: u32) -> (ptr: u32, len: u32)` | Read facts by ContextKey ordinal |
//! | `host_log`               | `(level: u32, ptr: u32, len: u32)` | Structured logging |
//! | `host_now_millis`        | `() -> u64`                      | Deterministic logical clock |
//! | `host_alloc_result`      | `(size: u32) -> ptr: u32`        | Host-side allocation for results |

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// ABI Version
// ============================================================================

/// Current ABI version for WASM guest modules.
///
/// The host checks `converge_abi_version()` against this constant.
/// Modules with a different major version are rejected at upload.
/// Minor version differences are tolerated (backward compatible).
pub const WASM_ABI_VERSION: u32 = 1;

/// Minimum ABI version the host will accept.
pub const WASM_ABI_MIN_VERSION: u32 = 1;

// ============================================================================
// Module Manifest: The Self-Declaration Contract
// ============================================================================

/// Manifest declared by the guest module.
///
/// This is the **self-declaration** contract. The guest returns this as JSON
/// from `converge_manifest()`. The host validates it against tenant quotas
/// and capability policies before activating the module.
///
/// # Axiom: Explicit Authority
///
/// Nothing in the manifest grants capabilities implicitly. The host must
/// explicitly allow each declared capability against the tenant's policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmManifest {
    /// Human-readable module name (e.g., "escalation-jtbd").
    pub name: String,

    /// Semantic version of this module (e.g., "1.2.0").
    pub version: String,

    /// Module kind: what role this module plays.
    pub kind: ModuleKind,

    /// For invariant modules: the invariant class.
    /// Required when `kind` is `Invariant`.
    pub invariant_class: Option<WasmInvariantClass>,

    /// For agent modules: declared context key dependencies.
    /// Required when `kind` is `Suggestor`.
    pub dependencies: Vec<String>,

    /// Host capabilities this module requires.
    ///
    /// The host will deny activation if any capability is not
    /// allowed by the tenant's policy.
    pub capabilities: Vec<HostCapability>,

    /// Whether outputs from this module require human approval
    /// before promotion to Facts.
    ///
    /// # Axiom: Human Authority First-Class
    ///
    /// When `true`, the engine will never auto-promote proposals
    /// from this module. A human must explicitly approve.
    pub requires_human_approval: bool,

    /// Optional: JTBD metadata linking this module to its source Truth.
    pub jtbd: Option<JtbdRef>,

    /// Optional: free-form metadata for the tenant's own use.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// What kind of module this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleKind {
    /// A runtime invariant (compiled from Gherkin).
    /// Implements `check_invariant`.
    Invariant,

    /// A full agent (compiled from JTBD spec).
    /// Implements `agent_accepts` + `agent_execute`.
    Suggestor,
}

/// Invariant class for WASM invariant modules.
///
/// Maps directly to `converge_core::InvariantClass`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WasmInvariantClass {
    /// Checked after every merge. Violation = immediate failure.
    Structural,
    /// Checked per cycle. Violation = blocks convergence.
    Semantic,
    /// Checked at convergence. Violation = rejects results.
    Acceptance,
}

/// Host capabilities that a guest module may request.
///
/// This is the **capability surface** of the WASM runtime.
/// Each capability must be explicitly granted by tenant policy.
///
/// # Axiom: Explicit Authority
///
/// Default is deny-all. Every capability requires explicit grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostCapability {
    /// Read context facts by key (most modules need this).
    ReadContext,
    /// Write structured log entries visible in execution trace.
    Log,
    /// Read monotonic clock (relative timing, no wall clock).
    Clock,
}

/// Reference linking this module back to its source JTBD / Truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JtbdRef {
    /// Source Truth file path or identifier.
    pub truth_id: String,
    /// Actor from JTBD metadata.
    pub actor: Option<String>,
    /// Functional job statement.
    pub job_functional: Option<String>,
    /// Hash of the source Gherkin that was compiled to this module.
    pub source_hash: Option<String>,
}

// ============================================================================
// Module Identity: Content-Addressed
// ============================================================================

/// Unique, content-addressed identity of a WASM module.
///
/// # Axiom: Scale by Intent Replication
///
/// Modules are identified by their content hash. The same `.wasm` bytes
/// always produce the same `ModuleId`. This enables:
/// - Deduplication across tenants
/// - Deterministic replay (same hash = same behavior)
/// - Cache-friendly compiled module storage
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModuleId {
    /// SHA-256 hash of the raw `.wasm` bytes.
    pub content_hash: String,
}

/// Full module descriptor as stored in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDescriptor {
    /// Content-addressed module identity.
    pub id: ModuleId,
    /// Tenant that owns this module.
    pub tenant_id: String,
    /// Parsed manifest from the module.
    pub manifest: WasmManifest,
    /// Current lifecycle state.
    pub state: ModuleState,
    /// Size of the raw `.wasm` bytes.
    pub size_bytes: u64,
    /// When this module was uploaded (unix millis).
    pub uploaded_at: u64,
    /// When this module was last activated (unix millis).
    pub activated_at: Option<u64>,
    /// Previous version this module replaced (if any).
    pub replaces: Option<ModuleId>,
    /// Detached ed25519 signature, if present.
    pub signature: Option<super::signing::ModuleSignature>,
}

/// Module lifecycle state.
///
/// # Axiom: Safety by Construction
///
/// The type-state progression enforces that modules cannot be executed
/// until they pass validation. Each transition is audited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleState {
    /// Uploaded, ABI version checked, manifest parsed.
    Compiled,
    /// Capabilities audited, quotas verified, signature checked.
    Validated,
    /// Registered in engine, callable by convergence loop.
    Active,
    /// Replaced by newer version. Retained for audit trail.
    Retired,
    /// Rejected during validation. Retained with rejection reason.
    Rejected,
}

// ============================================================================
// Guest → Host: Serialized Data Envelope
// ============================================================================

/// Context slice provided to the guest module.
///
/// This is the **read-only projection** of `converge_core::Context`
/// serialized as JSON and passed to the guest via linear memory.
///
/// # Axiom: Agents Suggest, Engine Decides
///
/// The guest receives an immutable snapshot. It cannot mutate context.
/// All contributions must be returned as a `GuestInvariantResult` or
/// `GuestAgentEffect`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestContext {
    /// Facts organized by context key name.
    ///
    /// Key is the string name of the `ContextKey` variant
    /// (e.g., "Seeds", "Strategies", "Signals").
    pub facts: HashMap<String, Vec<GuestFact>>,

    /// Context version (monotonic counter).
    pub version: u64,

    /// Current cycle number in the convergence loop.
    pub cycle: u32,
}

/// A fact as seen by the guest module.
///
/// Intentionally simplified from the internal `Fact` type.
/// No provenance metadata is exposed to guests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestFact {
    /// Unique identifier within context.
    pub id: String,
    /// The fact content.
    pub content: String,
}

// ============================================================================
// Invariant Guest Response
// ============================================================================

/// Result returned by `check_invariant`.
///
/// Serialized as JSON, written to guest memory, pointer+length returned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestInvariantResult {
    /// Whether the invariant holds.
    pub ok: bool,
    /// If violated: human-readable reason.
    pub reason: Option<String>,
    /// If violated: IDs of contributing facts (for traceability).
    #[serde(default)]
    pub fact_ids: Vec<String>,
}

impl GuestInvariantResult {
    /// Invariant holds.
    #[must_use]
    pub fn ok() -> Self {
        Self {
            ok: true,
            reason: None,
            fact_ids: Vec::new(),
        }
    }

    /// Invariant violated.
    #[must_use]
    pub fn violated(reason: impl Into<String>) -> Self {
        Self {
            ok: false,
            reason: Some(reason.into()),
            fact_ids: Vec::new(),
        }
    }

    /// Invariant violated with contributing fact IDs.
    #[must_use]
    pub fn violated_with_facts(reason: impl Into<String>, fact_ids: Vec<String>) -> Self {
        Self {
            ok: false,
            reason: Some(reason.into()),
            fact_ids,
        }
    }
}

// ============================================================================
// Suggestor Guest Response
// ============================================================================

/// Effect returned by `agent_execute`.
///
/// Maps to `converge_core::AgentEffect` but serialized across the WASM
/// boundary. The host converts this to native `AgentEffect` after
/// deserialization and validation.
///
/// # Axiom: Agents Suggest, Engine Decides
///
/// Guests emit `GuestProposedFact` (proposals), not `Fact`.
/// The engine validates and promotes through the standard gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestAgentEffect {
    /// Proposed facts to be validated by the engine.
    #[serde(default)]
    pub proposals: Vec<GuestProposedFact>,
}

/// A fact proposed by a guest agent module.
///
/// # Axiom: Append-Only Truth
///
/// Guests can only propose new facts. They cannot delete or modify
/// existing facts. The `key` field determines the target context category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestProposedFact {
    /// Target context key (e.g., "Hypotheses", "Strategies").
    pub key: String,
    /// Unique identifier for this proposed fact.
    pub id: String,
    /// The proposed content.
    pub content: String,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
}

// ============================================================================
// Resource Quotas: Guaranteed Termination
// ============================================================================

/// Resource quotas enforced by the host during WASM execution.
///
/// These are **hard limits**. Exceeding any quota causes immediate
/// termination of the guest with a `WasmTrap::QuotaExceeded` error.
///
/// # Axiom: No Hidden Work
///
/// Every resource dimension is metered and reported in the execution trace.
/// There are no unmetered operations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WasmQuota {
    /// Maximum fuel (instruction count proxy) per invocation.
    ///
    /// Wasmtime fuel maps roughly to instruction count.
    /// Default: 1_000_000 (~1M instructions).
    pub max_fuel: u64,

    /// Maximum linear memory in bytes.
    ///
    /// The guest cannot grow memory beyond this limit.
    /// Default: 16 MiB.
    pub max_memory_bytes: u64,

    /// Maximum wall-clock execution time in milliseconds.
    ///
    /// Enforced via async cancellation. Protects against
    /// pathological fuel consumption patterns.
    /// Default: 5_000 ms.
    pub max_duration_ms: u64,

    /// Maximum number of host function calls per invocation.
    ///
    /// Prevents abuse of host capabilities.
    /// Default: 1_000.
    pub max_host_calls: u32,

    /// Maximum total bytes the guest can write to result buffers.
    ///
    /// Prevents memory exhaustion via oversized results.
    /// Default: 1 MiB.
    pub max_result_bytes: u64,
}

impl Default for WasmQuota {
    fn default() -> Self {
        Self {
            max_fuel: 1_000_000,
            max_memory_bytes: 16 * 1024 * 1024, // 16 MiB
            max_duration_ms: 5_000,             // 5 seconds
            max_host_calls: 1_000,
            max_result_bytes: 1024 * 1024, // 1 MiB
        }
    }
}

/// Per-tenant quota policy.
///
/// Tenants may have different resource allocations based on their plan.
/// The host enforces the **minimum** of tenant quota and per-module quota.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantQuota {
    /// Maximum number of active modules for this tenant.
    pub max_active_modules: u32,

    /// Maximum total `.wasm` bytes across all modules.
    pub max_total_module_bytes: u64,

    /// Per-invocation resource quota.
    pub per_invocation: WasmQuota,

    /// Which host capabilities this tenant may grant to modules.
    pub allowed_capabilities: Vec<HostCapability>,
}

impl Default for TenantQuota {
    fn default() -> Self {
        Self {
            max_active_modules: 10,
            max_total_module_bytes: 50 * 1024 * 1024, // 50 MiB total
            per_invocation: WasmQuota::default(),
            allowed_capabilities: vec![
                HostCapability::ReadContext,
                HostCapability::Log,
                HostCapability::Clock,
            ],
        }
    }
}

// ============================================================================
// Execution Trace: Full Observability
// ============================================================================

/// Trace envelope for a single WASM module invocation.
///
/// # Axiom: Transparent Determinism
///
/// Every invocation is fully traced. The trace includes the module
/// content hash, fuel consumed, host calls made, and timing. This
/// enables post-hoc auditing and (for deterministic modules) replay.
///
/// # Axiom: System Tells Truth About Itself
///
/// The trace records exactly what happened, not what was intended.
/// If the module was terminated due to quota exhaustion, the trace
/// says so. If a host call was denied, the trace says so.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Module that was executed.
    pub module_id: ModuleId,

    /// Module name (from manifest).
    pub module_name: String,

    /// Module version (from manifest).
    pub module_version: String,

    /// Tenant that owns the module.
    pub tenant_id: String,

    /// Invocation kind.
    pub invocation: InvocationKind,

    /// How the invocation ended.
    pub outcome: InvocationOutcome,

    /// Fuel consumed (instruction count proxy).
    pub fuel_consumed: u64,

    /// Peak memory usage in bytes.
    pub peak_memory_bytes: u64,

    /// Wall-clock duration in microseconds.
    pub duration_us: u64,

    /// Host function calls made during this invocation.
    pub host_calls: Vec<HostCallRecord>,

    /// Total bytes returned in result buffers.
    pub result_bytes: u64,

    /// Converge engine cycle number when this was invoked.
    pub engine_cycle: u32,

    /// Timestamp of invocation start (unix millis).
    pub started_at: u64,
}

/// What function was invoked on the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationKind {
    /// Called `check_invariant`.
    CheckInvariant,
    /// Called `agent_accepts`.
    AgentAccepts,
    /// Called `agent_execute`.
    AgentExecute,
    /// Called `converge_manifest` (during validation).
    ReadManifest,
}

/// How the invocation ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationOutcome {
    /// Completed normally, result deserialized successfully.
    Ok,
    /// Guest trapped (panicked, unreachable, etc.).
    Trapped(String),
    /// Quota exceeded (fuel, memory, time, host calls, result size).
    QuotaExceeded(QuotaKind),
    /// Host denied a capability the guest requested.
    CapabilityDenied(String),
    /// Result deserialization failed.
    MalformedResult(String),
}

/// Which quota was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuotaKind {
    Fuel,
    Memory,
    Duration,
    HostCalls,
    ResultBytes,
}

/// Record of a single host function call from the guest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCallRecord {
    /// Which host function was called.
    pub function: String,
    /// Arguments summary (not full payload, for privacy).
    pub args_summary: String,
    /// Duration of the host call in microseconds.
    pub duration_us: u64,
    /// Whether the call succeeded.
    pub success: bool,
}

// ============================================================================
// WASM Trace Link: For Kernel Boundary Integration
// ============================================================================

/// Trace link specific to WASM module execution.
///
/// This integrates with the `converge_core::kernel_boundary::TraceLink`
/// system. WASM executions are always `Replayability::Deterministic`
/// because WASM execution is deterministic given the same inputs.
///
/// # Axiom: Transparent Determinism
///
/// A WASM module with the same content hash, receiving the same
/// context bytes, will always produce the same output. This makes
/// WASM invariants and agents inherently replay-eligible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmTraceLink {
    /// Content hash of the module that executed.
    pub module_hash: String,
    /// Module name + version for human readability.
    pub module_ref: String,
    /// SHA-256 hash of the serialized context input.
    pub input_hash: String,
    /// SHA-256 hash of the serialized result output.
    pub output_hash: String,
    /// Fuel consumed (determinism marker — same fuel = same path).
    pub fuel_consumed: u64,
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during WASM module operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WasmError {
    /// Module ABI version is incompatible with this host.
    IncompatibleAbi {
        module_version: u32,
        host_min: u32,
        host_current: u32,
    },
    /// Module manifest is invalid or incomplete.
    InvalidManifest(String),
    /// Module requests capabilities not allowed by tenant policy.
    CapabilityDenied {
        requested: Vec<HostCapability>,
        denied: Vec<HostCapability>,
    },
    /// Tenant quota would be exceeded by activating this module.
    TenantQuotaExceeded(String),
    /// Module compilation failed (invalid WASM).
    CompilationFailed(String),
    /// Module trapped during execution.
    Trapped { function: String, message: String },
    /// Resource quota exceeded during execution.
    QuotaExceeded {
        kind: QuotaKind,
        limit: u64,
        consumed: u64,
    },
    /// Guest returned malformed result.
    MalformedResult { function: String, message: String },
    /// Module not found in registry.
    ModuleNotFound(ModuleId),
    /// Module is not in the expected state for the requested operation.
    InvalidState {
        module: ModuleId,
        current: ModuleState,
        expected: ModuleState,
    },
    /// Module signature verification failed.
    SignatureInvalid(String),
    /// Module signature is missing but required by policy.
    SignatureMissing(String),
}

impl std::fmt::Display for WasmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IncompatibleAbi {
                module_version,
                host_min,
                host_current,
            } => write!(
                f,
                "incompatible ABI: module v{module_version}, host accepts v{host_min}..v{host_current}"
            ),
            Self::InvalidManifest(msg) => write!(f, "invalid manifest: {msg}"),
            Self::CapabilityDenied { denied, .. } => {
                write!(f, "capabilities denied: {denied:?}")
            }
            Self::TenantQuotaExceeded(msg) => write!(f, "tenant quota exceeded: {msg}"),
            Self::CompilationFailed(msg) => write!(f, "compilation failed: {msg}"),
            Self::Trapped { function, message } => {
                write!(f, "wasm trap in {function}: {message}")
            }
            Self::QuotaExceeded {
                kind,
                limit,
                consumed,
            } => write!(f, "{kind:?} quota exceeded: {consumed}/{limit}"),
            Self::MalformedResult { function, message } => {
                write!(f, "malformed result from {function}: {message}")
            }
            Self::ModuleNotFound(id) => write!(f, "module not found: {}", id.content_hash),
            Self::InvalidState {
                current, expected, ..
            } => write!(f, "module in state {current:?}, expected {expected:?}"),
            Self::SignatureInvalid(msg) => write!(f, "signature verification failed: {msg}"),
            Self::SignatureMissing(msg) => write!(f, "signature missing: {msg}"),
        }
    }
}

impl std::error::Error for WasmError {}

// ============================================================================
// Conversion: Guest Types ↔ Core Types
// ============================================================================

/// Conversion utilities between WASM guest types and `converge-core` types.
///
/// These are implemented as free functions (not trait impls) because
/// the WASM types live in `helm-plugin-runtime` while the core types
/// live in `converge-core`. Neither crate owns both sides.
pub mod convert {
    use super::*;

    /// Convert a `GuestInvariantResult` to a `converge_core` `InvariantResult`.
    pub fn to_invariant_result(guest: &GuestInvariantResult) -> converge_core::InvariantResult {
        if guest.ok {
            converge_core::InvariantResult::Ok
        } else {
            let reason = guest
                .reason
                .as_deref()
                .unwrap_or("invariant violated (no reason given)");

            converge_core::InvariantResult::Violated(converge_core::Violation::with_facts(
                reason.to_string(),
                guest
                    .fact_ids
                    .iter()
                    .cloned()
                    .map(converge_core::FactId::from)
                    .collect(),
            ))
        }
    }

    /// Convert a `WasmInvariantClass` to a `converge_core` `InvariantClass`.
    pub fn to_invariant_class(class: WasmInvariantClass) -> converge_core::InvariantClass {
        match class {
            WasmInvariantClass::Structural => converge_core::InvariantClass::Structural,
            WasmInvariantClass::Semantic => converge_core::InvariantClass::Semantic,
            WasmInvariantClass::Acceptance => converge_core::InvariantClass::Acceptance,
        }
    }

    /// Parse a context key string from a guest into a `ContextKey`.
    ///
    /// Returns `None` for unrecognized keys. This prevents guests from
    /// inventing new context categories.
    pub fn parse_context_key(key: &str) -> Option<converge_core::ContextKey> {
        match key {
            "Seeds" => Some(converge_core::ContextKey::Seeds),
            "Hypotheses" => Some(converge_core::ContextKey::Hypotheses),
            "Strategies" => Some(converge_core::ContextKey::Strategies),
            "Constraints" => Some(converge_core::ContextKey::Constraints),
            "Signals" => Some(converge_core::ContextKey::Signals),
            "Competitors" => Some(converge_core::ContextKey::Competitors),
            "Evaluations" => Some(converge_core::ContextKey::Evaluations),
            _ => None,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrip() {
        let manifest = WasmManifest {
            name: "escalation-jtbd".to_string(),
            version: "1.0.0".to_string(),
            kind: ModuleKind::Invariant,
            invariant_class: Some(WasmInvariantClass::Acceptance),
            dependencies: vec![],
            capabilities: vec![HostCapability::ReadContext, HostCapability::Log],
            requires_human_approval: false,
            jtbd: Some(JtbdRef {
                truth_id: "escalation.truth".to_string(),
                actor: Some("Ops Manager".to_string()),
                job_functional: Some("Escalate delayed access-control rollout".to_string()),
                source_hash: Some("sha256:abc123".to_string()),
            }),
            metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&manifest).expect("serialize");
        let back: WasmManifest = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(back.name, "escalation-jtbd");
        assert_eq!(back.kind, ModuleKind::Invariant);
        assert_eq!(back.invariant_class, Some(WasmInvariantClass::Acceptance));
        assert!(back.jtbd.is_some());
    }

    #[test]
    fn guest_invariant_result_ok() {
        let result = GuestInvariantResult::ok();
        assert!(result.ok);
        assert!(result.reason.is_none());
    }

    #[test]
    fn guest_invariant_result_violated() {
        let result = GuestInvariantResult::violated_with_facts(
            "need at least 2 strategies",
            vec!["strat-1".to_string()],
        );
        assert!(!result.ok);
        assert_eq!(result.reason.as_deref(), Some("need at least 2 strategies"));
        assert_eq!(result.fact_ids, vec!["strat-1"]);
    }

    #[test]
    fn guest_invariant_result_to_core() {
        let ok = GuestInvariantResult::ok();
        let core_ok = convert::to_invariant_result(&ok);
        assert!(core_ok.is_ok());

        let violated = GuestInvariantResult::violated("reason");
        let core_violated = convert::to_invariant_result(&violated);
        assert!(core_violated.is_violated());
    }

    #[test]
    fn invariant_class_conversion() {
        assert_eq!(
            convert::to_invariant_class(WasmInvariantClass::Structural),
            converge_core::InvariantClass::Structural,
        );
        assert_eq!(
            convert::to_invariant_class(WasmInvariantClass::Semantic),
            converge_core::InvariantClass::Semantic,
        );
        assert_eq!(
            convert::to_invariant_class(WasmInvariantClass::Acceptance),
            converge_core::InvariantClass::Acceptance,
        );
    }

    #[test]
    fn context_key_parsing() {
        assert_eq!(
            convert::parse_context_key("Seeds"),
            Some(converge_core::ContextKey::Seeds),
        );
        assert_eq!(
            convert::parse_context_key("Strategies"),
            Some(converge_core::ContextKey::Strategies),
        );
        assert_eq!(convert::parse_context_key("Invented"), None);
        assert_eq!(convert::parse_context_key("Proposals"), None);
        assert_eq!(convert::parse_context_key("Diagnostic"), None);
    }

    #[test]
    fn quota_defaults_are_reasonable() {
        let quota = WasmQuota::default();
        assert_eq!(quota.max_fuel, 1_000_000);
        assert_eq!(quota.max_memory_bytes, 16 * 1024 * 1024);
        assert_eq!(quota.max_duration_ms, 5_000);
        assert_eq!(quota.max_host_calls, 1_000);
        assert_eq!(quota.max_result_bytes, 1024 * 1024);
    }

    #[test]
    fn tenant_quota_defaults() {
        let tenant = TenantQuota::default();
        assert_eq!(tenant.max_active_modules, 10);
        assert_eq!(tenant.max_total_module_bytes, 50 * 1024 * 1024);
        assert_eq!(tenant.allowed_capabilities.len(), 3);
    }

    #[test]
    fn module_id_equality_by_content_hash() {
        let a = ModuleId {
            content_hash: "sha256:abc".to_string(),
        };
        let b = ModuleId {
            content_hash: "sha256:abc".to_string(),
        };
        let c = ModuleId {
            content_hash: "sha256:def".to_string(),
        };

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn guest_context_serde() {
        let ctx = GuestContext {
            facts: {
                let mut m = HashMap::new();
                m.insert(
                    "Seeds".to_string(),
                    vec![GuestFact {
                        id: "seed-1".to_string(),
                        content: "initial".to_string(),
                    }],
                );
                m
            },
            version: 3,
            cycle: 1,
        };

        let json = serde_json::to_string(&ctx).expect("serialize");
        let back: GuestContext = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.version, 3);
        assert_eq!(back.facts["Seeds"].len(), 1);
    }

    #[test]
    fn wasm_error_display() {
        let err = WasmError::QuotaExceeded {
            kind: QuotaKind::Fuel,
            limit: 1_000_000,
            consumed: 1_200_000,
        };
        assert_eq!(err.to_string(), "Fuel quota exceeded: 1200000/1000000");
    }

    #[test]
    fn execution_trace_serde() {
        let trace = ExecutionTrace {
            module_id: ModuleId {
                content_hash: "sha256:abc".to_string(),
            },
            module_name: "test-invariant".to_string(),
            module_version: "1.0.0".to_string(),
            tenant_id: "tenant-x".to_string(),
            invocation: InvocationKind::CheckInvariant,
            outcome: InvocationOutcome::Ok,
            fuel_consumed: 42_000,
            peak_memory_bytes: 1024,
            duration_us: 150,
            host_calls: vec![HostCallRecord {
                function: "host_read_context".to_string(),
                args_summary: "key=Strategies".to_string(),
                duration_us: 5,
                success: true,
            }],
            result_bytes: 64,
            engine_cycle: 3,
            started_at: 1700000000000,
        };

        let json = serde_json::to_string(&trace).expect("serialize");
        let back: ExecutionTrace = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(back.fuel_consumed, 42_000);
        assert_eq!(back.host_calls.len(), 1);
        assert_eq!(back.outcome, InvocationOutcome::Ok);
    }

    #[test]
    fn guest_agent_effect_proposals_only() {
        let effect = GuestAgentEffect {
            proposals: vec![GuestProposedFact {
                key: "Hypotheses".to_string(),
                id: "hyp-custom-1".to_string(),
                content: "market growing 15%".to_string(),
                confidence: 0.85,
            }],
        };

        let json = serde_json::to_string(&effect).expect("serialize");
        let back: GuestAgentEffect = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.proposals.len(), 1);
        assert_eq!(back.proposals[0].key, "Hypotheses");
    }

    #[test]
    fn guest_proposed_fact_rejects_internal_keys() {
        // Guests should not be able to write to Proposals or Diagnostic keys
        assert!(convert::parse_context_key("Proposals").is_none());
        assert!(convert::parse_context_key("Diagnostic").is_none());
    }

    #[test]
    fn wasm_trace_link_determinism() {
        // Same module + same input → same trace link fields
        let link = WasmTraceLink {
            module_hash: "sha256:module_abc".to_string(),
            module_ref: "escalation-jtbd@1.0.0".to_string(),
            input_hash: "sha256:input_def".to_string(),
            output_hash: "sha256:output_ghi".to_string(),
            fuel_consumed: 42_000,
        };

        let json = serde_json::to_string(&link).expect("serialize");
        let back: WasmTraceLink = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.module_hash, link.module_hash);
        assert_eq!(back.fuel_consumed, 42_000);
    }
}
