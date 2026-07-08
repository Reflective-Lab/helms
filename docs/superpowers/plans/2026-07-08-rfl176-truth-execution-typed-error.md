# RFL-176: Remove tonic::Status from helm-truth-execution Public Surface

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove `tonic::Status` from `helm-truth-execution`'s public trait surface by introducing a typed `TruthExecutionError` enum, keeping tonic behind a feature-gated `grpc` cargo feature that is OFF by default.

**Architecture:** Introduce `src/error.rs` with `TruthExecutionError` (11 variants mirroring the actual gRPC codes used today) plus `thiserror` derives and a `message()` helper. All internal helpers in `common.rs` and `dispatcher.rs` return this type. Behind `features = ["grpc"]`, a `From<TruthExecutionError> for tonic::Status` maps variants back to the identical Status codes, preserving zero behavior change for gRPC consumers.

**Tech Stack:** Rust, thiserror 2.x, tonic 0.12 (optional dep), async-trait, cargo features.

## Global Constraints

- No `#[allow(...)]` attributes — fix root causes.
- No new `tonic::Status` in the public trait surface without the `grpc` feature gate.
- `cargo test -p helm-truth-execution` (default features) must be green.
- `cargo test -p helm-truth-execution --features grpc` must be green.
- `cargo tree -p helm-truth-execution -e no-dev | grep -c tonic` must output `0` with default features.
- All existing tests in `helm-governed-jobs`, `helm-coordination`, `helm-operator-control` must remain green.
- `cargo test --workspace` must have no NEW failures (pre-existing RFL-182 proptest failure in application-kernel is known — report but do not fix).
- Branch: `e12/rfl-176-truth-execution-typed-error` from main.
- Commits per task; push + PR to main after all tasks complete.
- Linear: https://linear.app/reflective-labs/issue/RFL-176
- Working directory: `/Users/kpernyer/dev/reflective/bedrock-platform/helms`
- Report written to: `/Users/kpernyer/dev/reflective/.superpowers/sdd/rfl176-report.md`

---

## Status-Code Mapping Oracle (captured from current source before any change)

This table is the ground-truth oracle for Task 5's grpc mapping tests.

| Scenario | Old `tonic::Status` code | `TruthExecutionError` variant | Back to `tonic::Status` code |
|---|---|---|---|
| `required_input` missing | `invalid_argument` | `InvalidArgument` | `Status::invalid_argument(message)` |
| `required_uuid` bad value | `invalid_argument` | `InvalidArgument` | `Status::invalid_argument(message)` |
| `required_datetime` bad value | `invalid_argument` | `InvalidArgument` | `Status::invalid_argument(message)` |
| `optional_uuid` bad value | `invalid_argument` | `InvalidArgument` | `Status::invalid_argument(message)` |
| `payload_from_result` missing fact | `failed_precondition` | `FailedPrecondition` | `Status::failed_precondition(message)` |
| `payload_from_result` bad JSON | `internal` | `Internal` | `Status::internal(message)` |
| `ConvergeError::BudgetExhausted` | `resource_exhausted` | `ResourceExhausted` | `Status::resource_exhausted(message)` |
| `ConvergeError::InvariantViolation` | `failed_precondition` | `FailedPrecondition` | `Status::failed_precondition(message)` |
| `ConvergeError::AgentFailed` | `internal` | `Internal` | `Status::internal(message)` |
| `ConvergeError::EmptyProvenance` | `failed_precondition` | `FailedPrecondition` | `Status::failed_precondition(message)` |
| `ConvergeError::Conflict` | `aborted` | `Aborted` | `Status::aborted(message)` |
| `ConvergeError::InvalidResume` | `failed_precondition` | `FailedPrecondition` | `Status::failed_precondition(message)` |
| `ConvergeError::InvalidAdmission` | `invalid_argument` | `InvalidArgument` | `Status::invalid_argument(message)` |
| `ConvergeError::InvalidSnapshot` | `data_loss` | `DataLoss` | `Status::data_loss(message)` |
| `StorageError::LockPoisoned` | `internal` | `Internal` | `Status::internal(message)` |
| `StorageError::Kernel(Validation)` | `invalid_argument` | `InvalidArgument` | `Status::invalid_argument(message)` |
| `StorageError::Kernel(NotFound)` | `not_found` | `NotFound` | `Status::not_found(message)` |
| `StorageError::Kernel(Invariant)` | `failed_precondition` | `FailedPrecondition` | `Status::failed_precondition(message)` |
| `StorageError::Kernel(Conflict)` | `already_exists` | `AlreadyExists` | `Status::already_exists(message)` |
| `StorageError::ConnectionFailed` | `unavailable` | `Unavailable` | `Status::unavailable(message)` |
| `StorageError::SerializationFailed` | `internal` | `Internal` | `Status::internal(message)` |
| `StorageError::Timeout` | `deadline_exceeded` | `DeadlineExceeded` | `Status::deadline_exceeded(message)` |
| `StorageError::RuntimeStore` | `internal` | `Internal` | `Status::internal(message)` |
| `execute_truth` — no body for key | `unimplemented` | `Unimplemented` | `Status::unimplemented(message)` |

---

## File Map

| Path | Action |
|---|---|
| `crates/helm-truth-execution/src/error.rs` | **CREATE** — `TruthExecutionError` enum, `message()` method, `grpc` feature `From` impl |
| `crates/helm-truth-execution/src/lib.rs` | **MODIFY** — `mod error; pub use error::TruthExecutionError;`, update `TruthBody::execute` return type, update re-exports |
| `crates/helm-truth-execution/src/common.rs` | **MODIFY** — replace `tonic::Status` with `TruthExecutionError`; remove `result_large_err` allow |
| `crates/helm-truth-execution/src/dispatcher.rs` | **MODIFY** — replace `tonic::Status` with `TruthExecutionError`; rename `status_from_*` to `error_from_*`; remove `result_large_err` allow |
| `crates/helm-truth-execution/Cargo.toml` | **MODIFY** — add `thiserror`, make `tonic` optional, add `[features] grpc` |
| `crates/helm-truth-execution/tests/registry_test.rs` | **MODIFY** — update `StubTruth` return type and import |
| `crates/helm-truth-execution/tests/error_test.rs` | **CREATE** — negative tests + grpc feature mapping tests |
| `crates/helm-governed-jobs/Cargo.toml` | **MODIFY** — remove `tonic` from `[dependencies]` |
| `crates/helm-governed-jobs/tests/characterization.rs` | **MODIFY** — update `TruthBody` impl return type, remove tonic import |
| `crates/helm-governed-jobs/tests/gate_test.rs` | **MODIFY** — update `TruthBody` impl return type, remove tonic import |
| `crates/helm-governed-jobs/src/job_stream.rs` | **MODIFY** — `.message()` calls on error (same method name, compatible) — no change needed if `TruthExecutionError` exposes `.message()` |
| `crates/helm-coordination/Cargo.toml` | **MODIFY** — remove `tonic` from `[dev-dependencies]` |
| `crates/helm-coordination/tests/characterization.rs` | **MODIFY** — update `TruthBody` impl return type, remove tonic import |
| `crates/helm-coordination/tests/coordination_test.rs` | **MODIFY** — update `TruthBody` impl return type, remove tonic import |
| `crates/helm-coordination/tests/host_mount_test.rs` | **MODIFY** — update `TruthBody` impl return type, remove tonic import |

---

## Task 1: Create branch and introduce `TruthExecutionError`

**Files:**
- Create: `crates/helm-truth-execution/src/error.rs`
- Modify: `crates/helm-truth-execution/Cargo.toml`

**Interfaces:**
- Produces: `TruthExecutionError` enum (pub), `TruthExecutionError::message() -> &str` (pub), `From<TruthExecutionError> for tonic::Status` (pub, `#[cfg(feature = "grpc")]`)

- [ ] **Step 1: Create the branch**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
git checkout main
git pull origin main
git checkout -b e12/rfl-176-truth-execution-typed-error
```

Expected: branch created from current main.

- [ ] **Step 2: Update `Cargo.toml` to add thiserror, make tonic optional, add grpc feature**

Edit `crates/helm-truth-execution/Cargo.toml`. The `[dependencies]` section becomes:

```toml
[package]
name = "helm-truth-execution"
description = "helm-truth-execution — Helm's truth dispatcher framework as a HelmModule."
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
publish.workspace = true

[features]
grpc = ["dep:tonic"]

[dependencies]
anyhow.workspace = true
async-trait = "0.1"
axum.workspace = true
chrono.workspace = true
converge-core.workspace = true
converge-kernel.workspace = true
converge-pack.workspace = true
helm-module-contracts = { version = "0.3.0", path = "../../contracts/crates/helm-module-contracts" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tonic = { workspace = true, optional = true }
tracing.workspace = true
uuid.workspace = true

application-kernel = { version = "0.2.1", path = "../application-kernel" }
application-storage = { version = "0.2.1", path = "../application-storage" }
```

- [ ] **Step 3: Create `crates/helm-truth-execution/src/error.rs`**

```rust
//! Typed error for truth body execution.
//!
//! [`TruthExecutionError`] replaces `tonic::Status` on the public
//! [`crate::TruthBody::execute`] surface.  All variants carry a `message`
//! string that preserves the exact human-readable text the old `Status`
//! constructors produced.
//!
//! # gRPC transport mapping
//!
//! Enable the `grpc` cargo feature to activate
//! `impl From<TruthExecutionError> for tonic::Status`.  The mapping is
//! one-to-one with the Status codes used before this change, so gRPC
//! consumers see zero behavior difference.

use thiserror::Error;

/// Error returned by [`crate::TruthBody::execute`] and the dispatcher helpers.
///
/// Variants mirror the semantic failure classes that truth bodies and the
/// dispatcher infrastructure can produce.  The names intentionally avoid
/// gRPC terminology; the mapping to transport codes lives behind the `grpc`
/// feature.
#[derive(Debug, Error)]
pub enum TruthExecutionError {
    /// A required input value is missing or syntactically invalid.
    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },

    /// A referenced entity was not found in the kernel store.
    #[error("not found: {message}")]
    NotFound { message: String },

    /// A precondition for the operation was not met (invariant violation,
    /// missing fact, invalid resume, empty provenance, etc.).
    #[error("failed precondition: {message}")]
    FailedPrecondition { message: String },

    /// An internal error occurred (lock poisoned, serialization failed,
    /// converge agent failed, runtime store failure, invalid payload JSON, etc.).
    #[error("internal error: {message}")]
    Internal { message: String },

    /// A conflicting entity already exists in the kernel store.
    #[error("already exists: {message}")]
    AlreadyExists { message: String },

    /// A converge budget (cycle limit, etc.) was exhausted.
    #[error("resource exhausted: {message}")]
    ResourceExhausted { message: String },

    /// A concurrent fact conflict was detected during converge.
    #[error("aborted: {message}")]
    Aborted { message: String },

    /// A converge context snapshot is corrupted or unreadable.
    #[error("data loss: {message}")]
    DataLoss { message: String },

    /// A backing service (database, etc.) is unavailable.
    #[error("unavailable: {message}")]
    Unavailable { message: String },

    /// An operation timed out.
    #[error("deadline exceeded: {message}")]
    DeadlineExceeded { message: String },

    /// This truth key has no registered body.
    #[error("unimplemented: {message}")]
    Unimplemented { message: String },
}

impl TruthExecutionError {
    /// Returns the inner message string without the variant prefix.
    ///
    /// Callers that previously used `tonic::Status::message()` can switch to
    /// this method for a drop-in replacement.
    pub fn message(&self) -> &str {
        match self {
            Self::InvalidArgument { message }
            | Self::NotFound { message }
            | Self::FailedPrecondition { message }
            | Self::Internal { message }
            | Self::AlreadyExists { message }
            | Self::ResourceExhausted { message }
            | Self::Aborted { message }
            | Self::DataLoss { message }
            | Self::Unavailable { message }
            | Self::DeadlineExceeded { message }
            | Self::Unimplemented { message } => message,
        }
    }
}

// ── gRPC transport mapping (feature-gated) ─────────────────────────────────────

#[cfg(feature = "grpc")]
impl From<TruthExecutionError> for tonic::Status {
    /// Maps each variant to the identical `tonic::Status` code that was
    /// produced before RFL-176.  Behavior-preserving for gRPC consumers.
    fn from(e: TruthExecutionError) -> Self {
        match e {
            TruthExecutionError::InvalidArgument { message } => Self::invalid_argument(message),
            TruthExecutionError::NotFound { message } => Self::not_found(message),
            TruthExecutionError::FailedPrecondition { message } => {
                Self::failed_precondition(message)
            }
            TruthExecutionError::Internal { message } => Self::internal(message),
            TruthExecutionError::AlreadyExists { message } => Self::already_exists(message),
            TruthExecutionError::ResourceExhausted { message } => {
                Self::resource_exhausted(message)
            }
            TruthExecutionError::Aborted { message } => Self::aborted(message),
            TruthExecutionError::DataLoss { message } => Self::data_loss(message),
            TruthExecutionError::Unavailable { message } => Self::unavailable(message),
            TruthExecutionError::DeadlineExceeded { message } => Self::deadline_exceeded(message),
            TruthExecutionError::Unimplemented { message } => Self::unimplemented(message),
        }
    }
}
```

- [ ] **Step 4: Verify the new file compiles in isolation**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
cargo check -p helm-truth-execution 2>&1 | head -40
```

Expected: errors will reference `tonic::Status` usage in common.rs and dispatcher.rs (not yet changed) but `error.rs` itself should parse without error. The output will include compile errors from the not-yet-updated files — that is expected.

- [ ] **Step 5: Commit checkpoint**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
git add crates/helm-truth-execution/Cargo.toml crates/helm-truth-execution/src/error.rs
git commit -m "$(cat <<'EOF'
feat(helm-truth-execution): introduce TruthExecutionError with grpc feature gate (RFL-176)

Adds typed error enum with 11 semantic variants replacing tonic::Status on
the public surface. Feature `grpc` (OFF by default) enables
From<TruthExecutionError> for tonic::Status with behavior-identical mapping.
tonic dep is now optional and wired to the grpc feature.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Update `common.rs` to return `TruthExecutionError`

**Files:**
- Modify: `crates/helm-truth-execution/src/common.rs`

**Interfaces:**
- Consumes: `TruthExecutionError` from `crate` (via `crate::TruthExecutionError`)
- Produces: updated signatures for `payload_from_result`, `required_input`, `required_uuid`, `required_datetime`, `optional_uuid`

- [ ] **Step 1: Replace `common.rs` in its entirety**

Replace the full content of `crates/helm-truth-execution/src/common.rs` with:

```rust
use std::collections::HashMap;
use std::future::Future;

use chrono::{DateTime, Utc};
use converge_kernel::{ContextKey, ConvergeResult};
use converge_pack::{Context as ContextView, ProposalId, ProposedFact, Provenance, TextPayload};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::TruthExecutionError;

pub fn has_fact_id(ctx: &dyn ContextView, key: ContextKey, fact_id: &str) -> bool {
    ctx.get(key).iter().any(|fact| fact.id() == fact_id)
}

pub fn proposed_text_fact(
    key: ContextKey,
    id: impl Into<ProposalId>,
    text: impl Into<String>,
    provenance: Provenance,
) -> ProposedFact {
    ProposedFact::new(key, id, TextPayload::new(text), provenance)
}

pub fn payload_from_result<T: DeserializeOwned>(
    result: &ConvergeResult,
    key: ContextKey,
    fact_id: &str,
) -> Result<T, TruthExecutionError> {
    let fact = result
        .context
        .get(key)
        .iter()
        .find(|fact| fact.id() == fact_id)
        .ok_or_else(|| TruthExecutionError::FailedPrecondition {
            message: format!("missing fact in converge context: {fact_id}"),
        })?;
    serde_json::from_str(fact.text().unwrap_or_default()).map_err(|error| {
        TruthExecutionError::Internal {
            message: format!("invalid {fact_id} payload: {error}"),
        }
    })
}

pub fn required_input<'a>(
    inputs: &'a HashMap<String, String>,
    key: &str,
) -> Result<&'a str, TruthExecutionError> {
    inputs
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| TruthExecutionError::InvalidArgument {
            message: format!("missing required input: {key}"),
        })
}

pub fn optional_input(inputs: &HashMap<String, String>, key: &str) -> Option<String> {
    inputs
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn required_uuid(inputs: &HashMap<String, String>, key: &str) -> Result<Uuid, TruthExecutionError> {
    required_input(inputs, key).and_then(|value| {
        Uuid::parse_str(value).map_err(|error| TruthExecutionError::InvalidArgument {
            message: format!("invalid uuid for {key}: {error}"),
        })
    })
}

pub fn required_datetime(
    inputs: &HashMap<String, String>,
    key: &str,
) -> Result<DateTime<Utc>, TruthExecutionError> {
    required_input(inputs, key).and_then(|value| {
        chrono::DateTime::parse_from_rfc3339(value)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|error| TruthExecutionError::InvalidArgument {
                message: format!("invalid RFC3339 datetime for {key}: {error}"),
            })
    })
}

pub fn optional_uuid(
    inputs: &HashMap<String, String>,
    key: &str,
) -> Result<Option<Uuid>, TruthExecutionError> {
    optional_input(inputs, key)
        .map(|value| {
            Uuid::parse_str(&value).map_err(|error| TruthExecutionError::InvalidArgument {
                message: format!("invalid uuid for {key}: {error}"),
            })
        })
        .transpose()
}

pub fn optional_i64(inputs: &HashMap<String, String>, key: &str) -> Option<i64> {
    optional_input(inputs, key).and_then(|value| value.parse::<i64>().ok())
}

pub fn optional_bool(inputs: &HashMap<String, String>, key: &str) -> Option<bool> {
    optional_input(inputs, key).and_then(|value| match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    })
}

pub fn converge_confidence_to_bps(confidence: f64) -> u16 {
    (confidence.clamp(0.0, 1.0) * 10_000.0).round() as u16
}

pub fn block_on_async<F>(future: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("temporary tokio runtime should build")
            .block_on(future)
    })
    .join()
    .expect("knowledge async thread should join")
}
```

- [ ] **Step 2: Commit**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
git add crates/helm-truth-execution/src/common.rs
git commit -m "$(cat <<'EOF'
refactor(helm-truth-execution): common.rs returns TruthExecutionError (RFL-176)

All helper functions that previously returned tonic::Status now return
TruthExecutionError. Removes the result_large_err allow directive.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Update `dispatcher.rs` to return `TruthExecutionError`

**Files:**
- Modify: `crates/helm-truth-execution/src/dispatcher.rs`

**Interfaces:**
- Consumes: `TruthExecutionError` from `crate`
- Produces: renamed `error_from_converge`, `error_from_storage`; updated signatures for `run_engine_with_runtime`, `execute_truth`, `RuntimeGateDecisions::load`

- [ ] **Step 1: Replace `dispatcher.rs` in its entirety**

Replace the full content of `crates/helm-truth-execution/src/dispatcher.rs` with:

```rust
use std::collections::HashMap;
use std::sync::Mutex;

use application_kernel::{
    Actor as CrmActor, Document, Entitlement, Fact as CrmFact, LedgerEntry, Opportunity,
    OrderSubscription, Organization, Person, WorkflowCase,
};
use application_storage::{AppKernelStore, AppRuntimeStores, StorageError};
use converge_core::FactId;
use converge_kernel::{
    ContextState as Context, ConvergeError, ConvergeResult, Criterion, CriterionEvaluator,
    CriterionResult, Engine, EventQuery, ExperienceEvent, ExperienceEventEnvelope,
    ExperienceEventObserver, ExperienceRecord, OverrideTarget, TypesRootIntent, TypesRunHooks,
    UserExperienceEvent,
};
use uuid::Uuid;

use crate::{TruthExecutionError, TruthExecutionModule};

// ── Public output types ────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TruthExecutionArtifacts {
    pub result: ConvergeResult,
    pub experience_events: Vec<ExperienceEvent>,
    pub projection: Option<TruthProjection>,
    pub runtime_scope_id: String,
}

#[derive(Debug)]
pub struct TruthProjection {
    pub organization: Option<Organization>,
    pub person: Option<Person>,
    pub opportunity: Option<Opportunity>,
    pub subscription: Option<OrderSubscription>,
    pub entitlements: Vec<Entitlement>,
    pub ledger_entries: Vec<LedgerEntry>,
    pub documents: Vec<Document>,
    pub workflow_cases: Vec<WorkflowCase>,
    pub facts: Vec<CrmFact>,
    pub domain_event_kinds: Vec<&'static str>,
}

// ── Internal runtime types used by truth bodies ────────────────────────────────

/// Scope identifier for a single truth execution run.
pub struct RuntimeContext {
    pub scope_id: String,
}

/// Accumulates `ExperienceEvent`s emitted during an engine run.
#[derive(Default)]
pub struct RecordingObserver {
    events: Mutex<Vec<ExperienceEvent>>,
}

impl RecordingObserver {
    pub fn snapshot(&self) -> Vec<ExperienceEvent> {
        self.events
            .lock()
            .expect("recording observer lock poisoned")
            .clone()
    }
}

impl ExperienceEventObserver for RecordingObserver {
    fn on_event(&self, event: &ExperienceEvent) {
        self.events
            .lock()
            .expect("recording observer lock poisoned")
            .push(event.clone());
    }
}

// ── Approval-aware criterion evaluator ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeGateDecision {
    Approved,
    Rejected,
}

#[derive(Debug, Default)]
struct RuntimeGateDecisions {
    decisions: HashMap<String, RuntimeGateDecision>,
}

impl RuntimeGateDecisions {
    fn load(
        runtime_stores: &AppRuntimeStores,
        runtime_ctx: &RuntimeContext,
    ) -> Result<Self, TruthExecutionError> {
        let records = runtime_stores
            .query_experience_records(&EventQuery {
                correlation_id: Some(runtime_ctx.scope_id.clone().into()),
                ..EventQuery::default()
            })
            .map_err(error_from_storage)?;
        let mut decisions = HashMap::new();

        for record in records {
            let ExperienceRecord::User(envelope) = record else {
                continue;
            };

            match envelope.event {
                UserExperienceEvent::UserApprovalGranted {
                    gate_request_id, ..
                } => {
                    decisions.insert(gate_request_id.to_string(), RuntimeGateDecision::Approved);
                }
                UserExperienceEvent::UserApprovalRejected {
                    gate_request_id, ..
                } => {
                    decisions.insert(gate_request_id.to_string(), RuntimeGateDecision::Rejected);
                }
                UserExperienceEvent::UserOverrideIssued {
                    target: OverrideTarget::Constraint(constraint),
                    ..
                } => {
                    decisions.insert(constraint.to_string(), RuntimeGateDecision::Rejected);
                }
                _ => {}
            }
        }

        Ok(Self { decisions })
    }

    fn decision_for(
        &self,
        runtime_scope_id: &str,
        approval_ref: &str,
    ) -> Option<RuntimeGateDecision> {
        self.decisions
            .get(&runtime_gate_request_id(runtime_scope_id, approval_ref))
            .copied()
            .or_else(|| self.decisions.get(approval_ref).copied())
    }
}

struct ApprovalAwareCriterionEvaluator {
    inner: std::sync::Arc<dyn CriterionEvaluator>,
    runtime_scope_id: String,
    decisions: RuntimeGateDecisions,
}

impl CriterionEvaluator for ApprovalAwareCriterionEvaluator {
    fn evaluate(
        &self,
        criterion: &Criterion,
        context: &dyn converge_kernel::Context,
    ) -> CriterionResult {
        match self.inner.evaluate(criterion, context) {
            CriterionResult::Blocked {
                reason,
                approval_ref: Some(approval_ref),
            } => match self
                .decisions
                .decision_for(&self.runtime_scope_id, approval_ref.as_str())
            {
                Some(RuntimeGateDecision::Approved) => CriterionResult::Met {
                    evidence: vec![FactId::new(approval_ref.to_string())],
                },
                Some(RuntimeGateDecision::Rejected) => CriterionResult::Unmet {
                    reason: format!("approval rejected for {approval_ref}: {reason}"),
                },
                None => CriterionResult::Blocked {
                    reason,
                    approval_ref: Some(approval_ref),
                },
            },
            result => result,
        }
    }
}

// ── Public helpers consumed by truth bodies ────────────────────────────────────

pub fn runtime_gate_request_id(runtime_scope_id: &str, approval_ref: &str) -> String {
    format!("{runtime_scope_id}:{approval_ref}")
}

pub async fn run_engine_with_runtime(
    runtime_stores: &AppRuntimeStores,
    engine: &mut Engine,
    runtime_ctx: &RuntimeContext,
    seed_context: Context,
    intent: &TypesRootIntent,
    criterion_evaluator: std::sync::Arc<dyn CriterionEvaluator>,
) -> Result<(ConvergeResult, Vec<ExperienceEvent>), TruthExecutionError> {
    let observer = std::sync::Arc::new(RecordingObserver::default());
    let decisions = RuntimeGateDecisions::load(runtime_stores, runtime_ctx)?;
    let criterion_evaluator = std::sync::Arc::new(ApprovalAwareCriterionEvaluator {
        inner: criterion_evaluator,
        runtime_scope_id: runtime_ctx.scope_id.clone(),
        decisions,
    });
    let result = engine
        .run_with_types_intent_and_hooks(
            seed_context,
            intent,
            TypesRunHooks {
                criterion_evaluator: Some(criterion_evaluator),
                event_observer: Some(observer.clone()),
            },
        )
        .await
        .map_err(error_from_converge)?;

    runtime_stores
        .save_context(&runtime_ctx.scope_id, &result.context)
        .map_err(error_from_storage)?;

    let experience_events = observer.snapshot();
    let envelopes = experience_events
        .iter()
        .enumerate()
        .map(|(index, event)| {
            ExperienceEventEnvelope::new(
                format!("evt-{}-{:04}", Uuid::new_v4().simple(), index + 1),
                event.clone(),
            )
            .with_correlation(runtime_ctx.scope_id.clone())
        })
        .collect::<Vec<_>>();
    runtime_stores
        .append_experience_events(&envelopes)
        .map_err(error_from_storage)?;

    Ok((result, experience_events))
}

// ── Error mappers ──────────────────────────────────────────────────────────────

/// Maps a [`ConvergeError`] to the corresponding [`TruthExecutionError`] variant.
pub fn error_from_converge(error: ConvergeError) -> TruthExecutionError {
    match error {
        ConvergeError::BudgetExhausted { kind } => TruthExecutionError::ResourceExhausted {
            message: format!("converge budget exhausted: {kind}"),
        },
        ConvergeError::InvariantViolation { name, reason, .. } => {
            TruthExecutionError::FailedPrecondition {
                message: format!("converge invariant violated: {name}: {reason}"),
            }
        }
        ConvergeError::AgentFailed { agent_id } => TruthExecutionError::Internal {
            message: format!("converge agent failed: {agent_id}"),
        },
        ConvergeError::EmptyProvenance { suggestor } => TruthExecutionError::FailedPrecondition {
            message: format!("converge suggestor emitted empty provenance: {suggestor}"),
        },
        ConvergeError::Conflict { id, .. } => TruthExecutionError::Aborted {
            message: format!("converge fact conflict: {id}"),
        },
        ConvergeError::InvalidResume { reason } => TruthExecutionError::FailedPrecondition {
            message: format!("converge invalid resume: {reason}"),
        },
        ConvergeError::InvalidAdmission { reason } => TruthExecutionError::InvalidArgument {
            message: format!("converge invalid admission: {reason}"),
        },
        ConvergeError::InvalidSnapshot { reason } => TruthExecutionError::DataLoss {
            message: format!("converge invalid context snapshot: {reason}"),
        },
    }
}

/// Maps a [`StorageError`] to the corresponding [`TruthExecutionError`] variant.
pub fn error_from_storage(error: StorageError) -> TruthExecutionError {
    match error {
        StorageError::LockPoisoned => TruthExecutionError::Internal {
            message: "storage lock poisoned".into(),
        },
        StorageError::Kernel(application_kernel::KernelError::Validation(message)) => {
            TruthExecutionError::InvalidArgument { message }
        }
        StorageError::Kernel(application_kernel::KernelError::NotFound { kind, id }) => {
            TruthExecutionError::NotFound {
                message: format!("{kind} not found: {id}"),
            }
        }
        StorageError::Kernel(application_kernel::KernelError::Invariant(message)) => {
            TruthExecutionError::FailedPrecondition { message }
        }
        StorageError::Kernel(application_kernel::KernelError::Conflict(message)) => {
            TruthExecutionError::AlreadyExists { message }
        }
        StorageError::ConnectionFailed { backend, message } => TruthExecutionError::Unavailable {
            message: format!("{backend} connection failed: {message}"),
        },
        StorageError::SerializationFailed { message } => TruthExecutionError::Internal { message },
        StorageError::Timeout { operation } => TruthExecutionError::DeadlineExceeded {
            message: operation,
        },
        StorageError::RuntimeStore { message } => TruthExecutionError::Internal { message },
    }
}

pub fn domain_event_kind_name(event: &application_kernel::DomainEvent) -> &'static str {
    match event {
        application_kernel::DomainEvent::OrganizationUpserted { .. } => "organization-upserted",
        application_kernel::DomainEvent::PersonUpserted { .. } => "person-upserted",
        application_kernel::DomainEvent::RelationshipLinked { .. } => "relationship-linked",
        application_kernel::DomainEvent::OpportunityCreated { .. } => "opportunity-created",
        application_kernel::DomainEvent::OpportunityStageChanged { .. } => {
            "opportunity-stage-changed"
        }
        application_kernel::DomainEvent::ActivityAppended { .. } => "activity-appended",
        application_kernel::DomainEvent::NoteAppended { .. } => "note-appended",
        application_kernel::DomainEvent::DocumentAttached { .. } => "document-attached",
        application_kernel::DomainEvent::CommunicationRecorded { .. } => "communication-recorded",
        application_kernel::DomainEvent::WorkflowCaseCreated { .. } => "workflow-case-created",
        application_kernel::DomainEvent::WorkflowCaseStateChanged { .. } => {
            "workflow-case-state-changed"
        }
        application_kernel::DomainEvent::PermissionGranted { .. } => "permission-granted",
        application_kernel::DomainEvent::CatalogItemUpserted { .. } => "catalog-item-upserted",
        application_kernel::DomainEvent::OrderSubscriptionCreated { .. } => "subscription-created",
        application_kernel::DomainEvent::OrderSubscriptionStateChanged { .. } => {
            "subscription-state-changed"
        }
        application_kernel::DomainEvent::OrderSubscriptionPlanChanged { .. } => {
            "subscription-plan-changed"
        }
        application_kernel::DomainEvent::EntitlementsGranted { .. } => "entitlements-granted",
        application_kernel::DomainEvent::EntitlementsReplaced { .. } => "entitlements-replaced",
        application_kernel::DomainEvent::EntitlementAdjusted { .. } => "entitlement-adjusted",
        application_kernel::DomainEvent::LedgerEntryAppended { .. } => "ledger-entry-appended",
        application_kernel::DomainEvent::FactRecorded { .. } => "fact-recorded",
        application_kernel::DomainEvent::ObjectDefinitionUpserted { .. } => {
            "object-definition-upserted"
        }
        application_kernel::DomainEvent::ViewDefinitionUpserted { .. } => {
            "view-definition-upserted"
        }
        application_kernel::DomainEvent::AuditRecorded { .. } => "audit-recorded",
        application_kernel::DomainEvent::TimelineEntryRecorded { .. } => "timeline-entry-recorded",
    }
}

// ── Registry-based dispatcher ──────────────────────────────────────────────────

/// Context passed to every `TruthBody::execute` call.
///
/// Carries everything the original `execute_truth` function parameters did:
/// the kernel store, the app runtime stores, the flat key→value input map,
/// actor identity, and whether to persist a domain projection.
///
/// # Simplification note
///
/// The original dispatcher was generic over `S: KernelStore`.  To allow
/// trait-object registration without propagating the generic, we use the
/// concrete `AppKernelStore` enum here — it covers both the in-memory and
/// SurrealDB variants.  Phases 3b/4b can refine if a different concrete
/// type is needed.
pub struct TruthExecutionContext {
    pub store: AppKernelStore,
    pub runtime_stores: AppRuntimeStores,
    pub inputs: HashMap<String, String>,
    pub actor: CrmActor,
    pub persist_projection: bool,
}

/// Dispatch a truth body by key, using the registry held by `module`.
///
/// Returns `Err(TruthExecutionError::Unimplemented(...))` if no body is
/// registered for `truth_key`, matching the behaviour of the original
/// hard-coded match.
pub async fn execute_truth(
    module: &TruthExecutionModule,
    truth_key: &str,
    ctx: TruthExecutionContext,
) -> Result<TruthExecutionArtifacts, TruthExecutionError> {
    let body = module.lookup(truth_key).ok_or_else(|| {
        TruthExecutionError::Unimplemented {
            message: format!("truth execution is not implemented yet for {truth_key}"),
        }
    })?;
    body.execute(ctx).await
}

/// Returns `true` if a body is registered for `truth_key`.
///
/// Replaces the original hard-coded `supports_truth_execution` match list.
pub fn supports_truth_execution(module: &TruthExecutionModule, truth_key: &str) -> bool {
    module.lookup(truth_key).is_some()
}
```

- [ ] **Step 2: Verify dispatcher.rs compiles (expect errors only in lib.rs re-exports until next task)**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
cargo check -p helm-truth-execution 2>&1 | head -40
```

Expected: errors in `lib.rs` about `status_from_converge`/`status_from_storage` not found and `TruthBody::execute` return type mismatch — all fixable in Task 4.

- [ ] **Step 3: Commit**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
git add crates/helm-truth-execution/src/dispatcher.rs
git commit -m "$(cat <<'EOF'
refactor(helm-truth-execution): dispatcher.rs returns TruthExecutionError (RFL-176)

Renames status_from_converge/status_from_storage to error_from_converge/
error_from_storage and changes all return types to TruthExecutionError.
Removes the result_large_err allow directive.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Update `lib.rs` — trait surface and re-exports

**Files:**
- Modify: `crates/helm-truth-execution/src/lib.rs`

**Interfaces:**
- Produces: `TruthBody::execute` returns `Result<TruthExecutionArtifacts, TruthExecutionError>`; re-exports `TruthExecutionError`, `error_from_converge`, `error_from_storage`

- [ ] **Step 1: Replace `lib.rs` in its entirety**

```rust
//! helm-truth-execution — Helm's truth dispatcher framework as a HelmModule.
//!
//! Truth bodies live with their consumer (Catalyst, atelier-showcase) and
//! register with this module at startup via [`TruthExecutionModule::register`].
//!
//! # Architecture
//!
//! ```text
//! Consumer (Catalyst / CRM showcase)
//!   └─ implements TruthBody  →  registers with TruthExecutionModule
//!
//! TruthExecutionModule (HelmModule)
//!   ├─ init():   logs registered truth count
//!   ├─ router(): mounts dispatcher routes
//!   └─ execute_truth(key, ctx):  registry lookup → body.execute(ctx)
//! ```
//!
//! # Phase 3b / 4b unblocking
//!
//! This crate is the Phase 5 extraction that `helm-governed-jobs` and
//! `helm-operator-control` were waiting for.  Phases 3b and 4b can now
//! consume `TruthBody` + `TruthExecutionContext` instead of depending on
//! `application-server`-internal types.
//!
//! # `KernelStore` generic resolution
//!
//! The original `execute_truth` was generic over `S: KernelStore`.  Because
//! `KernelStore` requires `Clone + Sized`, it is not dyn-compatible and cannot
//! be erased behind `Arc<dyn KernelStore>`.  The generic is resolved by using
//! the concrete `AppKernelStore` enum (which covers both in-memory and
//! SurrealDB variants) in [`dispatcher::TruthExecutionContext`].
//! Phases 3b/4b can revisit if a different concrete type is needed.

pub mod common;
pub mod dispatcher;
mod error;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use axum::Router;
use helm_module_contracts::HelmModule;

pub use error::TruthExecutionError;
pub use dispatcher::{
    RecordingObserver, RuntimeContext, TruthExecutionArtifacts, TruthProjection,
    domain_event_kind_name, error_from_converge, error_from_storage, execute_truth,
    run_engine_with_runtime, runtime_gate_request_id, supports_truth_execution,
};

// ── TruthBody trait ────────────────────────────────────────────────────────────

/// A single executable truth body.
///
/// Implement this trait for each truth (e.g. `score-inbound-fit`) and register
/// the implementation with [`TruthExecutionModule::register`] at application
/// startup.
///
/// # Generic plumbing
///
/// The original per-truth `execute` functions were generic over `S: KernelStore`.
/// That generic is resolved at the trait boundary by using the concrete
/// `AppKernelStore` enum in [`dispatcher::TruthExecutionContext`].  Truth
/// bodies access the store via `ctx.store`.
#[async_trait]
pub trait TruthBody: Send + Sync + 'static {
    /// The stable kebab-case key that identifies this truth (e.g. `"score-inbound-fit"`).
    fn key(&self) -> &'static str;

    /// Execute the truth body.  The dispatcher routes here based on [`Self::key`].
    async fn execute(
        &self,
        ctx: dispatcher::TruthExecutionContext,
    ) -> Result<TruthExecutionArtifacts, TruthExecutionError>;
}

// ── Registry ───────────────────────────────────────────────────────────────────

/// A mountable [`HelmModule`] that owns the truth-body registry and dispatcher.
///
/// Build with [`TruthExecutionModule::new`], chain [`TruthExecutionModule::register`]
/// calls for each truth body, then wrap in `Arc` before passing to the host builder.
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use helm_truth_execution::TruthExecutionModule;
///
/// let module = Arc::new(
///     TruthExecutionModule::new()
///         // .register(Arc::new(MyTruthBody))
/// );
/// ```
pub struct TruthExecutionModule {
    registry: RwLock<HashMap<&'static str, Arc<dyn TruthBody>>>,
}

impl TruthExecutionModule {
    pub fn new() -> Self {
        Self {
            registry: RwLock::new(HashMap::new()),
        }
    }

    /// Register a truth body.  Returns `self` for chaining.
    ///
    /// If two bodies share the same key the last registration wins.
    pub fn register(self, body: Arc<dyn TruthBody>) -> Self {
        self.registry
            .write()
            .expect("truth registry write lock poisoned")
            .insert(body.key(), body);
        self
    }

    /// Look up a registered body by key.
    pub fn lookup(&self, key: &str) -> Option<Arc<dyn TruthBody>> {
        self.registry
            .read()
            .expect("truth registry read lock poisoned")
            .get(key)
            .cloned()
    }

    /// Returns the number of registered truth bodies.
    pub fn registered_count(&self) -> usize {
        self.registry
            .read()
            .expect("truth registry read lock poisoned")
            .len()
    }
}

impl Default for TruthExecutionModule {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HelmModule for TruthExecutionModule {
    fn module_id(&self) -> &'static str {
        "helm.truth-execution"
    }

    async fn init(&self) -> anyhow::Result<()> {
        let count = self.registered_count();
        tracing::info!(
            module = self.module_id(),
            registered_truths = count,
            "initialized"
        );
        Ok(())
    }

    /// The dispatcher does not mount its own Axum routes in Phase 5.
    ///
    /// The `/v1/truths/{key}/execute` HTTP surface belongs to `application-server`
    /// (via gRPC / its existing HTTP API) and is wired there using the original
    /// `execute_truth` call-site.  Phases 3b/4b will add an HTTP route here
    /// once the `HttpState<S>` extractor is decoupled from `application-server`.
    fn router(self: Arc<Self>) -> Router {
        Router::new()
    }
}
```

- [ ] **Step 2: Verify the crate now compiles cleanly (default features)**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
cargo check -p helm-truth-execution 2>&1
```

Expected: no errors. There may be warnings about unused `tonic` in test files (registry_test.rs still uses `tonic::Status`) — those are fixed in Task 5.

- [ ] **Step 3: Verify tonic is absent from the default dep tree**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
cargo tree -p helm-truth-execution -e no-dev | grep -c tonic
```

Expected: `0`

- [ ] **Step 4: Commit**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
git add crates/helm-truth-execution/src/lib.rs
git commit -m "$(cat <<'EOF'
refactor(helm-truth-execution): TruthBody::execute returns TruthExecutionError (RFL-176)

Updates the public trait surface. Re-exports TruthExecutionError and the
renamed error_from_converge/error_from_storage helpers. tonic is no longer
imported in lib.rs.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Tests — negative tests and grpc mapping

**Files:**
- Modify: `crates/helm-truth-execution/tests/registry_test.rs`
- Create: `crates/helm-truth-execution/tests/error_test.rs`

**Interfaces:**
- Consumes: `TruthExecutionError` (pub from `helm_truth_execution`)

- [ ] **Step 1: Update `registry_test.rs` — remove tonic, use TruthExecutionError**

Replace the full content of `crates/helm-truth-execution/tests/registry_test.rs` with:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use helm_module_contracts::HelmModule;
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionError, TruthExecutionModule,
};

// ── Stub truth body ────────────────────────────────────────────────────────────

struct StubTruth;

#[async_trait]
impl TruthBody for StubTruth {
    fn key(&self) -> &'static str {
        "test.stub"
    }

    async fn execute(
        &self,
        _ctx: helm_truth_execution::dispatcher::TruthExecutionContext,
    ) -> Result<TruthExecutionArtifacts, TruthExecutionError> {
        // Construct a minimal artifact.  ConvergeResult has no Default impl
        // in the test environment, so we verify the registry path separately
        // in `registered_truth_is_dispatchable` without executing.
        Err(TruthExecutionError::Unimplemented {
            message: "stub — not callable in unit tests".into(),
        })
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[test]
fn module_id_is_stable() {
    let m = TruthExecutionModule::new();
    assert_eq!(m.module_id(), "helm.truth-execution");
}

#[test]
fn empty_module_has_zero_registered_truths() {
    let m = TruthExecutionModule::new();
    assert_eq!(m.registered_count(), 0);
}

#[test]
fn registered_truth_is_found_by_key() {
    let m = TruthExecutionModule::new().register(Arc::new(StubTruth));
    assert!(m.lookup("test.stub").is_some());
}

#[test]
fn unregistered_key_returns_none() {
    let m = TruthExecutionModule::new().register(Arc::new(StubTruth));
    assert!(m.lookup("nonexistent").is_none());
}

#[test]
fn registered_count_reflects_registrations() {
    let m = TruthExecutionModule::new().register(Arc::new(StubTruth));
    assert_eq!(m.registered_count(), 1);
}

#[test]
fn supports_truth_execution_matches_registry() {
    let m = TruthExecutionModule::new().register(Arc::new(StubTruth));
    assert!(helm_truth_execution::supports_truth_execution(
        &m,
        "test.stub"
    ));
    assert!(!helm_truth_execution::supports_truth_execution(
        &m,
        "score-inbound-fit"
    ));
}
```

- [ ] **Step 2: Create `crates/helm-truth-execution/tests/error_test.rs`**

```rust
//! Tests for TruthExecutionError variants and the grpc feature mapping.
//!
//! Negative tests assert exact error variants for the failure paths in
//! `common.rs` and the dispatcher.  The `grpc` module (behind `#[cfg(feature = "grpc")]`)
//! asserts that every variant maps to the exact `tonic::Status` code that was
//! produced before RFL-176 (oracle captured from the pre-change source).

use std::collections::HashMap;

use helm_truth_execution::TruthExecutionError;
use helm_truth_execution::common::{
    optional_uuid, payload_from_result, required_datetime, required_input, required_uuid,
};

// ── common.rs negative tests ───────────────────────────────────────────────────

#[test]
fn required_input_missing_key_is_invalid_argument() {
    let inputs = HashMap::new();
    let err = required_input(&inputs, "org_id").unwrap_err();
    assert!(
        matches!(err, TruthExecutionError::InvalidArgument { .. }),
        "expected InvalidArgument, got {err:?}"
    );
    assert!(
        err.message().contains("org_id"),
        "message should name the missing key"
    );
}

#[test]
fn required_input_empty_value_is_invalid_argument() {
    let mut inputs = HashMap::new();
    inputs.insert("org_id".into(), "   ".into());
    let err = required_input(&inputs, "org_id").unwrap_err();
    assert!(matches!(err, TruthExecutionError::InvalidArgument { .. }));
}

#[test]
fn required_uuid_malformed_is_invalid_argument() {
    let mut inputs = HashMap::new();
    inputs.insert("id".into(), "not-a-uuid".into());
    let err = required_uuid(&inputs, "id").unwrap_err();
    assert!(
        matches!(err, TruthExecutionError::InvalidArgument { .. }),
        "expected InvalidArgument, got {err:?}"
    );
    assert!(err.message().contains("id"));
}

#[test]
fn required_datetime_malformed_is_invalid_argument() {
    let mut inputs = HashMap::new();
    inputs.insert("due_at".into(), "not-a-date".into());
    let err = required_datetime(&inputs, "due_at").unwrap_err();
    assert!(
        matches!(err, TruthExecutionError::InvalidArgument { .. }),
        "expected InvalidArgument, got {err:?}"
    );
    assert!(err.message().contains("due_at"));
}

#[test]
fn optional_uuid_malformed_is_invalid_argument() {
    let mut inputs = HashMap::new();
    inputs.insert("ref_id".into(), "bad-uuid".into());
    let err = optional_uuid(&inputs, "ref_id").unwrap_err();
    assert!(matches!(err, TruthExecutionError::InvalidArgument { .. }));
}

// ── message() helper ───────────────────────────────────────────────────────────

#[test]
fn message_returns_inner_string_without_prefix() {
    let e = TruthExecutionError::InvalidArgument {
        message: "missing required input: foo".into(),
    };
    // message() must return the bare string, not "invalid argument: ..."
    assert_eq!(e.message(), "missing required input: foo");
}

#[test]
fn display_includes_variant_prefix() {
    let e = TruthExecutionError::NotFound {
        message: "org not found: 123".into(),
    };
    let s = e.to_string();
    assert!(s.contains("not found"), "Display should include prefix: {s}");
    assert!(s.contains("org not found: 123"), "Display should include message: {s}");
}

// ── payload_from_result negative tests ────────────────────────────────────────

#[test]
fn payload_from_result_missing_fact_is_failed_precondition() {
    use converge_core::ContextState;
    use converge_kernel::ContextKey;

    let result = converge_core::ConvergeResult {
        context: ContextState::default(),
        cycles: 0,
        converged: false,
        stop_reason: converge_core::StopReason::Converged,
        criteria_outcomes: vec![],
        integrity: converge_core::integrity::IntegrityProof {
            merkle_root: converge_core::integrity::MerkleRoot(
                converge_core::integrity::ContentHash([0u8; 32]),
            ),
            clock_time: 0,
            fact_count: 0,
        },
    };

    let err = payload_from_result::<serde_json::Value>(
        &result,
        ContextKey::new("fit-score"),
        "fit-score-fact",
    )
    .unwrap_err();

    assert!(
        matches!(err, TruthExecutionError::FailedPrecondition { .. }),
        "missing fact should be FailedPrecondition, got {err:?}"
    );
    assert!(err.message().contains("fit-score-fact"));
}

// ── grpc feature — Status code mapping oracle ─────────────────────────────────

#[cfg(feature = "grpc")]
mod grpc_mapping {
    use helm_truth_execution::TruthExecutionError;
    use tonic::Code;

    fn assert_maps_to(e: TruthExecutionError, expected_code: Code, expected_message: &str) {
        let status = tonic::Status::from(e);
        assert_eq!(
            status.code(),
            expected_code,
            "wrong gRPC code for {expected_message}"
        );
        assert_eq!(
            status.message(),
            expected_message,
            "message must be preserved verbatim"
        );
    }

    #[test]
    fn invalid_argument_maps_to_invalid_argument() {
        assert_maps_to(
            TruthExecutionError::InvalidArgument {
                message: "missing required input: org_id".into(),
            },
            Code::InvalidArgument,
            "missing required input: org_id",
        );
    }

    #[test]
    fn not_found_maps_to_not_found() {
        assert_maps_to(
            TruthExecutionError::NotFound {
                message: "Organization not found: abc-123".into(),
            },
            Code::NotFound,
            "Organization not found: abc-123",
        );
    }

    #[test]
    fn failed_precondition_maps_to_failed_precondition() {
        assert_maps_to(
            TruthExecutionError::FailedPrecondition {
                message: "missing fact in converge context: fit-score-fact".into(),
            },
            Code::FailedPrecondition,
            "missing fact in converge context: fit-score-fact",
        );
    }

    #[test]
    fn internal_maps_to_internal() {
        assert_maps_to(
            TruthExecutionError::Internal {
                message: "storage lock poisoned".into(),
            },
            Code::Internal,
            "storage lock poisoned",
        );
    }

    #[test]
    fn already_exists_maps_to_already_exists() {
        assert_maps_to(
            TruthExecutionError::AlreadyExists {
                message: "conflict on entity xyz".into(),
            },
            Code::AlreadyExists,
            "conflict on entity xyz",
        );
    }

    #[test]
    fn resource_exhausted_maps_to_resource_exhausted() {
        assert_maps_to(
            TruthExecutionError::ResourceExhausted {
                message: "converge budget exhausted: cycles".into(),
            },
            Code::ResourceExhausted,
            "converge budget exhausted: cycles",
        );
    }

    #[test]
    fn aborted_maps_to_aborted() {
        assert_maps_to(
            TruthExecutionError::Aborted {
                message: "converge fact conflict: fact-id-xyz".into(),
            },
            Code::Aborted,
            "converge fact conflict: fact-id-xyz",
        );
    }

    #[test]
    fn data_loss_maps_to_data_loss() {
        assert_maps_to(
            TruthExecutionError::DataLoss {
                message: "converge invalid context snapshot: corrupt".into(),
            },
            Code::DataLoss,
            "converge invalid context snapshot: corrupt",
        );
    }

    #[test]
    fn unavailable_maps_to_unavailable() {
        assert_maps_to(
            TruthExecutionError::Unavailable {
                message: "surrealdb connection failed: refused".into(),
            },
            Code::Unavailable,
            "surrealdb connection failed: refused",
        );
    }

    #[test]
    fn deadline_exceeded_maps_to_deadline_exceeded() {
        assert_maps_to(
            TruthExecutionError::DeadlineExceeded {
                message: "query_records".into(),
            },
            Code::DeadlineExceeded,
            "query_records",
        );
    }

    #[test]
    fn unimplemented_maps_to_unimplemented() {
        assert_maps_to(
            TruthExecutionError::Unimplemented {
                message: "truth execution is not implemented yet for mystery-truth".into(),
            },
            Code::Unimplemented,
            "truth execution is not implemented yet for mystery-truth",
        );
    }
}
```

- [ ] **Step 3: Run default-features tests**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
cargo test -p helm-truth-execution 2>&1
```

Expected: all tests pass, including the new `error_test.rs` tests. The `#[cfg(feature = "grpc")]` module is skipped.

Note: if `payload_from_result` test fails to compile because `converge_core::ConvergeResult` or `ContextKey` types differ, adjust the import paths to match what's actually exported.

- [ ] **Step 4: Run grpc-feature tests**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
cargo test -p helm-truth-execution --features grpc 2>&1
```

Expected: all tests pass, including the 11 grpc_mapping tests.

- [ ] **Step 5: Verify tonic gate with features**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
# Default — must be 0
cargo tree -p helm-truth-execution -e no-dev | grep -c tonic
# grpc feature — must be ≥1
cargo tree -p helm-truth-execution --features grpc -e no-dev | grep -c tonic
```

Expected: first command outputs `0`, second outputs a number ≥ 1.

- [ ] **Step 6: Commit**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
git add crates/helm-truth-execution/tests/registry_test.rs \
        crates/helm-truth-execution/tests/error_test.rs
git commit -m "$(cat <<'EOF'
test(helm-truth-execution): typed error negative tests + grpc mapping oracle (RFL-176)

Adds error_test.rs with negative-path tests for common.rs helpers and a
grpc_mapping module (requires --features grpc) asserting that every
TruthExecutionError variant maps to the identical tonic::Status code as
before RFL-176. Updates registry_test.rs stub to use TruthExecutionError.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Update downstream crates — helm-governed-jobs

**Files:**
- Modify: `crates/helm-governed-jobs/Cargo.toml`
- Modify: `crates/helm-governed-jobs/tests/characterization.rs`
- Modify: `crates/helm-governed-jobs/tests/gate_test.rs`

**Interfaces:**
- Consumes: `TruthExecutionError` (imported from `helm_truth_execution`)

- [ ] **Step 1: Remove `tonic` from `helm-governed-jobs/Cargo.toml`**

In `crates/helm-governed-jobs/Cargo.toml`, find and remove the line:
```toml
tonic.workspace = true
```

No replacement needed — the source files don't use `tonic::` directly; only the test impls reference `tonic::Status` in the return type, which we're about to fix.

- [ ] **Step 2: Update `tests/characterization.rs`**

Find and replace the `TruthBody` implementation signature. The file currently has:

```rust
    async fn execute(
        &self,
        _ctx: TruthExecutionContext,
    ) -> Result<TruthExecutionArtifacts, tonic::Status> {
```

Change to:

```rust
    async fn execute(
        &self,
        _ctx: TruthExecutionContext,
    ) -> Result<TruthExecutionArtifacts, helm_truth_execution::TruthExecutionError> {
```

Also remove the `tonic` usage from imports. Find the import line that includes `tonic::Status` or `tonic` and remove it. The current import is:

```rust
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionModule, dispatcher::TruthExecutionContext,
};
```

Update to:

```rust
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionError, TruthExecutionModule,
    dispatcher::TruthExecutionContext,
};
```

And change the return type to `Result<TruthExecutionArtifacts, TruthExecutionError>`.

- [ ] **Step 3: Update `tests/gate_test.rs`**

Same pattern. Find:

```rust
    async fn execute(
        &self,
        _ctx: TruthExecutionContext,
    ) -> Result<TruthExecutionArtifacts, tonic::Status> {
```

Change to:

```rust
    async fn execute(
        &self,
        _ctx: TruthExecutionContext,
    ) -> Result<TruthExecutionArtifacts, TruthExecutionError> {
```

Update import from:
```rust
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionModule, dispatcher::TruthExecutionContext,
};
```

To:
```rust
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionError, TruthExecutionModule,
    dispatcher::TruthExecutionContext,
};
```

Remove any `use tonic` line that remains.

- [ ] **Step 4: Verify helm-governed-jobs tests pass**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
cargo test -p helm-governed-jobs 2>&1
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
git add crates/helm-governed-jobs/Cargo.toml \
        crates/helm-governed-jobs/tests/characterization.rs \
        crates/helm-governed-jobs/tests/gate_test.rs
git commit -m "$(cat <<'EOF'
refactor(helm-governed-jobs): use TruthExecutionError in TruthBody impls (RFL-176)

Test stubs updated to return TruthExecutionError. tonic removed from
[dependencies] since it was only needed for the old return type.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Update downstream crates — helm-coordination

**Files:**
- Modify: `crates/helm-coordination/Cargo.toml`
- Modify: `crates/helm-coordination/tests/characterization.rs`
- Modify: `crates/helm-coordination/tests/coordination_test.rs`
- Modify: `crates/helm-coordination/tests/host_mount_test.rs`

**Interfaces:**
- Consumes: `TruthExecutionError` (imported from `helm_truth_execution`)

- [ ] **Step 1: Remove `tonic` from `helm-coordination/Cargo.toml` dev-dependencies**

In `crates/helm-coordination/Cargo.toml`, in `[dev-dependencies]`, find and remove:
```toml
tonic.workspace = true
```

- [ ] **Step 2: Update `tests/characterization.rs`**

Current import:
```rust
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionModule, dispatcher::TruthExecutionContext,
};
```

Update to:
```rust
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionError, TruthExecutionModule,
    dispatcher::TruthExecutionContext,
};
```

Change the `TruthBody::execute` return type from `Result<TruthExecutionArtifacts, tonic::Status>` to `Result<TruthExecutionArtifacts, TruthExecutionError>`.

- [ ] **Step 3: Update `tests/coordination_test.rs`**

Same pattern. Current import includes:
```rust
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionModule, dispatcher::TruthExecutionContext,
};
```

Update to add `TruthExecutionError`. Change return type.

- [ ] **Step 4: Update `tests/host_mount_test.rs`**

Same pattern. Current import:
```rust
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionModule, dispatcher::TruthExecutionContext,
};
```

Update to add `TruthExecutionError`. Change return type.

- [ ] **Step 5: Verify helm-coordination tests pass**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
cargo test -p helm-coordination 2>&1
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
git add crates/helm-coordination/Cargo.toml \
        crates/helm-coordination/tests/characterization.rs \
        crates/helm-coordination/tests/coordination_test.rs \
        crates/helm-coordination/tests/host_mount_test.rs
git commit -m "$(cat <<'EOF'
refactor(helm-coordination): use TruthExecutionError in TruthBody impls (RFL-176)

Test stubs updated to return TruthExecutionError. tonic removed from
[dev-dependencies] since it was only needed for the old return type.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Run all gates, push, PR, merge, report

**Files:**
- Create (report): `/Users/kpernyer/dev/reflective/.superpowers/sdd/rfl176-report.md`

- [ ] **Step 1: Run full gate suite**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms

# Gate 1: default features tests
cargo test -p helm-truth-execution 2>&1
echo "EXIT: $?"

# Gate 2: grpc feature tests
cargo test -p helm-truth-execution --features grpc 2>&1
echo "EXIT: $?"

# Gate 3: tonic absent from default dep tree
TONIC_COUNT=$(cargo tree -p helm-truth-execution -e no-dev | grep -c tonic)
echo "tonic in default tree: $TONIC_COUNT (expected: 0)"

# Gate 4: tonic present with grpc feature
TONIC_GRPC=$(cargo tree -p helm-truth-execution --features grpc -e no-dev | grep -c tonic)
echo "tonic in grpc tree: $TONIC_GRPC (expected: >=1)"

# Gate 5: downstream crates
cargo test -p helm-governed-jobs 2>&1 | tail -5
cargo test -p helm-coordination 2>&1 | tail -5
cargo test -p helm-operator-control 2>&1 | tail -5

# Gate 6: workspace (exclude known failing proptest — if it fires, note it)
cargo test --workspace 2>&1 | tail -20
```

All must pass. If `application-kernel` proptest (RFL-182) fires, note it in the report but do not fix.

- [ ] **Step 2: Push branch**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
git push -u origin e12/rfl-176-truth-execution-typed-error
```

- [ ] **Step 3: Create PR**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
gh pr create \
  --title "feat(helm-truth-execution): remove tonic::Status from public trait surface (RFL-176)" \
  --body "$(cat <<'EOF'
## Summary

- Introduces `TruthExecutionError` (11 typed variants) replacing `tonic::Status` on the `TruthBody::execute` public surface and all dispatcher helpers.
- `tonic` dep is now `optional = true`, activated only by the new `grpc` feature (OFF by default).
- `grpc` feature provides `From<TruthExecutionError> for tonic::Status` with behavior-identical code mapping for gRPC consumers.
- Downstream crates (`helm-governed-jobs`, `helm-coordination`) updated; `tonic` removed from their deps where it was only needed for the return type.

Closes https://linear.app/reflective-labs/issue/RFL-176

## Test plan

- [ ] `cargo test -p helm-truth-execution` (default features) — all green
- [ ] `cargo test -p helm-truth-execution --features grpc` — all green including 11 grpc_mapping tests
- [ ] `cargo tree -p helm-truth-execution -e no-dev | grep -c tonic` → `0`
- [ ] `cargo tree -p helm-truth-execution --features grpc -e no-dev | grep -c tonic` → ≥1
- [ ] `cargo test -p helm-governed-jobs` — green
- [ ] `cargo test -p helm-coordination` — green
- [ ] `cargo test --workspace` — no new failures

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 4: Wait 6 seconds, then merge**

```bash
sleep 6
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
gh pr merge --merge --delete-branch
```

- [ ] **Step 5: Pull main and capture SHA**

```bash
cd /Users/kpernyer/dev/reflective/bedrock-platform/helms
git checkout main
git pull origin main
git rev-parse HEAD
```

Record the 40-hex SHA.

- [ ] **Step 6: Write report to `/Users/kpernyer/dev/reflective/.superpowers/sdd/rfl176-report.md`**

The report must include:
- Status (Done / Partial / Failed)
- PR URL
- Post-merge main SHA (40-hex)
- Gate one-liners (pass/fail for each of the 6 gates)
- The Status-code mapping table (from the Global Constraints section above)
- Any concerns or deviations
