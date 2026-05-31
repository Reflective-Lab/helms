//! Governed job stream — `POST /v1/jobs/{key}/stream`
//!
//! Adapted from `application-server/src/job_stream.rs` in Phase 4b redo.
//!
//! # Key changes from the original (Phase 4b redo vs Phase 4b draft)
//!
//! - `crate::http_api::HttpState<S>` → `JobStreamState` (self-contained state).
//!   The generic `S: KernelStore` is resolved to the concrete `AppKernelStore`
//!   enum (same resolution used by `helm-truth-execution`).
//! - `crate::truth_runtime::{execute_truth, supports_truth_execution}` →
//!   `helm_truth_execution::dispatcher::{execute_truth, supports_truth_execution}`
//!   called with `&state.truths` as the registry.
//! - `crate::realtime::RealtimeHub` → `runway_app_host::EventHubHandle` (Phase 1.6
//!   landed replay buffer + cursor subscribe + `EventEnvelope.job_id`, so no local
//!   hub copy is needed).
//! - `RealtimeHub::publish(input).await` → `EventHubHandle::publish(env)` (sync).
//!   `EventHubHandle` has no internal sequence counter, so `JobStreamState` owns
//!   an `Arc<AtomicU64>` that stamps monotonically increasing sequence numbers.
//! - SSE catch-up: `hub.subscribe(cursor)` →
//!   `hub.subscribe_with_cursor(EventCursor { run_id, .. })` which returns
//!   `EventSubscription { replay, receiver }`. Replay is already filtered by
//!   `run_id`; live channel is unfiltered and we filter by `run_id` in the stream.
//! - `crate::sse::*` helpers → inline SSE logic.
//!
//! # HITL approval flow
//!
//! The full HITL gate flow is preserved: pre-gate execute → gate.paused event →
//! `oneshot` waiter (10-minute timeout) → post-gate execute → job.completed.
//! The `JobStreamState` exposes `register_gate_waiter`/`signal_gate` for the
//! operator-control module (or any caller) to drive approvals.
//!
//! # Zero-arg constructor safety
//!
//! `JobStreamState::default()` constructs an empty `TruthExecutionModule` (no
//! truths registered) and a freestanding `EventHub`. Routes built with this state
//! will return `501 Not Implemented` for every truth key, which is the same
//! behaviour as `application-server`'s maintenance-mode `supports_truth_execution`.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use application_kernel::Actor;
use application_storage::{AppKernelStore, AppRuntimeStores, InMemoryKernelStore};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use converge_core::ContextState;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{broadcast, oneshot};
use truth_catalog::{
    admission::{
        TruthFormationSelection, admit_truth_intent, default_helms_capabilities,
        select_formation_for_intent,
    },
    find_truth,
};
use uuid::Uuid;

use helm_truth_execution::{
    TruthExecutionArtifacts, TruthExecutionModule,
    dispatcher::{TruthExecutionContext, execute_truth, supports_truth_execution},
};
use runway_app_host::{EventCursor, EventEnvelope, EventHub, EventHubHandle, EventSubscription};

// ── Gate Coordination ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum GateDecision {
    Approved,
    Rejected,
}

pub struct JobGateWaiter {
    runtime_ref: String,
    runtime_scope_id: String,
    tx: oneshot::Sender<GateDecision>,
}

impl JobGateWaiter {
    pub fn runtime_ref(&self) -> &str {
        &self.runtime_ref
    }

    pub fn runtime_scope_id(&self) -> &str {
        &self.runtime_scope_id
    }

    pub fn signal(self, decision: GateDecision) {
        let _ = self.tx.send(decision);
    }
}

// ── State ────────────────────────────────────────────────────────────

/// Route state for the governed-jobs stream.
///
/// Construct with `JobStreamState::default()` for a zero-arg shell (routes return
/// 501), or with `JobStreamState::new(store, runtime_stores, truths, hub, app_id)` for
/// real wiring.
#[derive(Clone)]
pub struct JobStreamState {
    pub store: AppKernelStore,
    pub runtime_stores: AppRuntimeStores,
    pub truths: Arc<TruthExecutionModule>,
    pub hub: EventHubHandle,
    pub app_id: String,
    /// HITL gate wait timeout. Defaults to 600s (10 minutes) — preserved from
    /// the original RealtimeHub. Override via struct-literal + `..default()`
    /// when shorter timeouts are needed (e.g. integration tests).
    pub gate_timeout: Duration,
    #[doc(hidden)]
    pub gate_waiters: Arc<Mutex<HashMap<String, JobGateWaiter>>>,
    /// Sequence counter for events published through this state.
    /// `EventHubHandle::publish` takes envelopes as-is (no auto-sequence),
    /// so we stamp them here.
    #[doc(hidden)]
    pub next_sequence: Arc<AtomicU64>,
}

const DEFAULT_GATE_TIMEOUT: Duration = Duration::from_secs(600);

impl JobStreamState {
    pub fn new(
        store: AppKernelStore,
        runtime_stores: AppRuntimeStores,
        truths: Arc<TruthExecutionModule>,
        hub: EventHubHandle,
        app_id: impl Into<String>,
    ) -> Self {
        Self {
            store,
            runtime_stores,
            truths,
            hub,
            app_id: app_id.into(),
            gate_timeout: DEFAULT_GATE_TIMEOUT,
            gate_waiters: Arc::new(Mutex::new(HashMap::new())),
            next_sequence: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn register_gate_waiter(
        &self,
        ref_id: impl Into<String>,
        runtime_ref: impl Into<String>,
        runtime_scope_id: impl Into<String>,
    ) -> oneshot::Receiver<GateDecision> {
        let (tx, rx) = oneshot::channel();
        if let Ok(mut map) = self.gate_waiters.lock() {
            map.insert(
                ref_id.into(),
                JobGateWaiter {
                    runtime_ref: runtime_ref.into(),
                    runtime_scope_id: runtime_scope_id.into(),
                    tx,
                },
            );
        }
        rx
    }

    pub fn take_gate_waiter(&self, ref_id: &str) -> Option<JobGateWaiter> {
        self.gate_waiters
            .lock()
            .ok()
            .and_then(|mut map| map.remove(ref_id))
    }

    pub fn restore_gate_waiter(&self, ref_id: impl Into<String>, waiter: JobGateWaiter) {
        if let Ok(mut map) = self.gate_waiters.lock() {
            map.insert(ref_id.into(), waiter);
        }
    }

    pub fn signal_gate(&self, ref_id: &str, decision: GateDecision) -> bool {
        if let Some(waiter) = self.take_gate_waiter(ref_id) {
            waiter.signal(decision);
            return true;
        }
        false
    }

    fn publisher(&self, run_id: &str, job_id: &str, app_id: &str) -> Publisher {
        Publisher {
            hub: self.hub.clone(),
            seq: Arc::clone(&self.next_sequence),
            run_id: run_id.to_string(),
            job_id: job_id.to_string(),
            app_id: app_id.to_string(),
        }
    }
}

impl Default for JobStreamState {
    fn default() -> Self {
        // Freestanding hub — not wired to any host. Routes built with this
        // default state return 501 for every truth key (no truths registered).
        let hub = EventHub::with_capacity(256);
        Self {
            store: AppKernelStore::Memory(InMemoryKernelStore::default_local()),
            runtime_stores: AppRuntimeStores::default(),
            truths: Arc::new(TruthExecutionModule::new()),
            hub: hub.handle(),
            app_id: "helm.governed-jobs".into(),
            gate_timeout: DEFAULT_GATE_TIMEOUT,
            gate_waiters: Arc::new(Mutex::new(HashMap::new())),
            next_sequence: Arc::new(AtomicU64::new(1)),
        }
    }
}

// ── Publisher ─────────────────────────────────────────────────────────

/// Thin wrapper that stamps monotonically-increasing sequence numbers onto
/// `EventEnvelope`s before handing them to `EventHubHandle::publish`.
///
/// `EventHubHandle` has no internal sequence counter — it accepts envelopes
/// exactly as provided. The `Publisher` owns the counter so every event emitted
/// by a single job run gets a unique, ordered sequence number that SSE clients
/// can use for dedup and catch-up.
struct Publisher {
    hub: EventHubHandle,
    seq: Arc<AtomicU64>,
    run_id: String,
    job_id: String,
    app_id: String,
}

impl Publisher {
    fn emit(&self, event_type: &str, payload: serde_json::Value) {
        let sequence = self.seq.fetch_add(1, Ordering::SeqCst);
        self.hub.publish(EventEnvelope {
            event_id: Uuid::new_v4(),
            sequence,
            r#type: event_type.to_string(),
            schema_version: 1,
            occurred_at: Utc::now(),
            app_id: self.app_id.clone(),
            run_id: Some(self.run_id.clone()),
            job_id: Some(self.job_id.clone()),
            correlation_id: None,
            actor: None,
            payload,
        });
    }

    fn step(&self, step_index: u32, label: &str, progress: u32) {
        self.emit(
            "job.step.completed",
            json!({
                "step_index": step_index,
                "label": label,
                "progress": progress,
            }),
        );
    }

    fn runtime_events(&self, phase: &str, execution: &TruthExecutionArtifacts) {
        for (event_index, event) in execution.experience_events.iter().enumerate() {
            let kind = experience_event_kind_name(event);
            let event_type = format!("converge.runtime.{kind}");
            let payload = serde_json::to_value(event)
                .unwrap_or_else(|_| json!({ "debug": format!("{event:?}") }));
            self.emit(
                &event_type,
                json!({
                    "stage": "converge-runtime",
                    "phase": phase,
                    "runtime_scope_id": execution.runtime_scope_id,
                    "event_index": event_index,
                    "kind": kind,
                    "event": payload,
                }),
            );
        }
    }

    fn completion_receipt(&self, execution: &TruthExecutionArtifacts) -> serde_json::Value {
        let receipt = completion_summary(execution);
        self.emit(
            "job.receipt.recorded",
            json!({
                "receipt_id": format!("{}:completion", self.run_id),
                "status": "completed",
                "result": receipt.clone(),
            }),
        );
        receipt
    }
}

// ── Router ───────────────────────────────────────────────────────────

/// Returns the Axum router for the job stream route.
///
/// Mounts `POST /v1/jobs/{key}/stream`.
pub fn router(state: Arc<JobStreamState>) -> Router {
    Router::new()
        .route("/v1/jobs/{key}/stream", post(stream_job))
        .with_state(state)
}

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct StreamJobRequest {
    #[serde(default)]
    inputs: HashMap<String, String>,
    app_id: Option<String>,
}

// ── Handler ──────────────────────────────────────────────────────────

async fn stream_job(
    State(state): State<Arc<JobStreamState>>,
    Path(key): Path<String>,
    Json(request): Json<StreamJobRequest>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)>
{
    let truth_key = key.trim().to_string();
    if truth_key.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "job key is required".into()));
    }
    if find_truth(&truth_key).is_none() {
        return Err((StatusCode::NOT_FOUND, format!("job not found: {truth_key}")));
    }
    if !supports_truth_execution(&state.truths, &truth_key) {
        return Err((
            StatusCode::NOT_IMPLEMENTED,
            format!("job is not executable yet: {truth_key}"),
        ));
    }

    let run_id = Uuid::new_v4().to_string();
    let app_id = request
        .app_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| state.app_id.clone());

    // Subscribe before spawning so no events are missed.
    // Use subscribe_with_cursor filtered to this run_id so replay only
    // returns events belonging to this specific run.
    let subscription = state.hub.subscribe_with_cursor(EventCursor {
        last_sequence: None,
        run_id: Some(run_id.clone()),
        job_id: None,
    });

    let state_clone = state.clone();
    let run_id_clone = run_id.clone();
    let truth_key_clone = truth_key.clone();

    tokio::spawn(async move {
        run_job_task(JobRunTask {
            state: state_clone,
            run_id: run_id_clone,
            truth_key: truth_key_clone,
            app_id,
            inputs: request.inputs,
        })
        .await;
    });

    let stream = build_run_sse_stream(subscription, run_id);
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

// ── Background Job Task ───────────────────────────────────────────────

/// Inputs for `run_job_task`. Exposed so integration tests can drive the
/// job loop directly without going through the HTTP layer.
pub struct JobRunTask {
    pub state: Arc<JobStreamState>,
    pub run_id: String,
    pub truth_key: String,
    pub app_id: String,
    pub inputs: HashMap<String, String>,
}

/// Drive a single governed job to completion (or gate-failure) end-to-end.
///
/// Mirrors what the `/v1/jobs/{key}/stream` HTTP handler spawns, minus the
/// SSE wiring. Pub so integration tests can call it directly.
pub async fn run_job_task(task: JobRunTask) {
    let JobRunTask {
        state,
        run_id,
        truth_key,
        app_id,
        inputs,
    } = task;

    let pub_ = state.publisher(&run_id, &truth_key, &app_id);

    pub_.emit("job.started", json!({}));
    if let Err(error) = admit_job(&pub_, &truth_key) {
        pub_.emit("job.failed", json!({ "error": error }));
        return;
    }

    let actor = Actor::system();

    let first = match execute_truth(
        &state.truths,
        &truth_key,
        TruthExecutionContext {
            store: state.store.clone(),
            runtime_stores: state.runtime_stores.clone(),
            inputs: inputs.clone(),
            actor: actor.clone(),
            persist_projection: false,
        },
    )
    .await
    {
        Ok(r) => r,
        Err(status) => {
            pub_.emit("job.failed", json!({ "error": status.message() }));
            return;
        }
    };
    pub_.runtime_events("pre-gate", &first);

    let total = first.result.criteria_outcomes.len().max(1);
    let mut emitted = 0usize;
    for (i, outcome) in first.result.criteria_outcomes.iter().enumerate() {
        match &outcome.result {
            converge_core::CriterionResult::Met { .. } => {
                let progress = ((i + 1) * 100 / total) as u32;
                pub_.step(i as u32, &outcome.criterion.description, progress);
                emitted = i + 1;
            }
            converge_core::CriterionResult::Blocked { .. } => break,
            _ => {}
        }
    }

    let Some(initial_blocked_gate) = blocked_gate(&first) else {
        let receipt = pub_.completion_receipt(&first);
        pub_.emit("job.completed", json!({ "result": receipt }));
        return;
    };
    let runtime_ref = match initial_blocked_gate.ref_id.clone() {
        Some(ref_id) => ref_id,
        None => {
            pub_.emit(
                "job.failed",
                json!({
                    "error": "gate blocked without approval reference",
                    "gate_reason": initial_blocked_gate.reason,
                }),
            );
            return;
        }
    };

    let progress_at_gate = (emitted * 100 / total) as u32;
    let gate_ref = scoped_gate_ref(&run_id, &runtime_ref);
    let runtime_scope_id = first.runtime_scope_id.clone();
    let decision_rx = state.register_gate_waiter(
        &gate_ref,
        runtime_ref.clone(),
        runtime_scope_id.clone(),
    );

    pub_.emit(
        "gate.paused",
        json!({
            "gate_name": "hitl",
            "gate_label": "operator-approval",
            "gate_reason": initial_blocked_gate.reason,
            "ref_id": gate_ref.clone(),
            "runtime_ref": runtime_ref.clone(),
            "runtime_scope_id": runtime_scope_id,
            "progress": progress_at_gate,
        }),
    );

    let decision = match tokio::time::timeout(state.gate_timeout, decision_rx).await {
        Ok(Ok(d)) => d,
        Ok(Err(_)) => {
            pub_.emit(
                "job.failed",
                json!({ "error": "gate waiter cancelled" }),
            );
            return;
        }
        Err(_) => {
            pub_.emit(
                "gate.timeout",
                json!({
                    "ref_id": gate_ref.clone(),
                    "runtime_ref": runtime_ref.clone(),
                    "timeout_ms": state.gate_timeout.as_millis() as u64,
                }),
            );
            pub_.emit(
                "job.failed",
                json!({ "error": "gate waiter timed out" }),
            );
            return;
        }
    };

    match decision {
        GateDecision::Rejected => {
            pub_.emit(
                "gate.rejected",
                json!({ "ref_id": gate_ref.clone(), "runtime_ref": runtime_ref.clone() }),
            );
            pub_.emit("job.failed", json!({ "error": "gate rejected by operator" }));
            return;
        }
        GateDecision::Approved => {
            pub_.emit(
                "gate.approved",
                json!({ "ref_id": gate_ref.clone(), "runtime_ref": runtime_ref.clone() }),
            );
        }
    }

    // Re-execute now that the approval is in runtime_stores.
    let second = match execute_truth(
        &state.truths,
        &truth_key,
        TruthExecutionContext {
            store: state.store.clone(),
            runtime_stores: state.runtime_stores.clone(),
            inputs,
            actor,
            persist_projection: false,
        },
    )
    .await
    {
        Ok(r) => r,
        Err(status) => {
            pub_.emit("job.failed", json!({ "error": status.message() }));
            return;
        }
    };
    pub_.runtime_events("post-gate", &second);

    if let Some(blocked_gate) = blocked_gate(&second) {
        pub_.emit(
            "job.failed",
            json!({
                "error": "gate remained blocked after approval",
                "gate_reason": blocked_gate.reason,
                "runtime_ref": blocked_gate.ref_id,
            }),
        );
        return;
    }

    let second_total = second.result.criteria_outcomes.len().max(1);
    for (i, outcome) in second.result.criteria_outcomes.iter().enumerate() {
        if i < emitted {
            continue;
        }
        if let converge_core::CriterionResult::Met { .. } = &outcome.result {
            let progress = ((i + 1) * 100 / second_total) as u32;
            pub_.step(i as u32, &outcome.criterion.description, progress);
        }
    }

    let receipt = pub_.completion_receipt(&second);
    pub_.emit("job.completed", json!({ "result": receipt }));
}

fn admit_job(pub_: &Publisher, truth_key: &str) -> Result<(), String> {
    let mut context = ContextState::new();
    let intent = admit_truth_intent(
        truth_key,
        &pub_.app_id,
        &format!("truth:{truth_key}"),
        &mut context,
    )
    .map_err(|error| format!("axiom intent admission failed: {error}"))?;

    pub_.emit("axiom.intent.compiled", axiom_intent_payload(&intent));

    let selection = select_formation_for_intent(&intent, &default_helms_capabilities())
        .map_err(|error| format!("organism formation selection failed: {error}"))?;
    pub_.emit(
        "organism.formation.selected",
        formation_selection_payload(&selection),
    );

    Ok(())
}

fn axiom_intent_payload(intent: &organism_pack::IntentPacket) -> serde_json::Value {
    json!({
        "stage": "axiom-intent",
        "message": "Axiom truth source compiled into an admitted IntentPacket",
        "intent_id": intent.id.to_string(),
        "outcome": &intent.outcome,
        "context": &intent.context,
        "constraints": &intent.constraints,
        "authority": &intent.authority,
        "forbidden_count": intent.forbidden.len(),
        "reversibility": format!("{:?}", intent.reversibility).to_ascii_lowercase(),
        "expires_at": intent.expires.to_rfc3339(),
        "expiry_action": format!("{:?}", intent.expiry_action).to_ascii_lowercase(),
    })
}

fn formation_selection_payload(selection: &TruthFormationSelection) -> serde_json::Value {
    json!({
        "stage": "organism-formation",
        "message": "Organism FormationGuru selected a runtime formation",
        "primary_template_id": &selection.primary_template_id,
        "alternate_template_ids": &selection.alternate_template_ids,
        "trace": &selection.trace,
    })
}

fn completion_summary(execution: &TruthExecutionArtifacts) -> serde_json::Value {
    let criteria = execution
        .result
        .criteria_outcomes
        .iter()
        .map(criterion_summary)
        .collect::<Vec<_>>();
    let facts = execution
        .result
        .context
        .all_keys()
        .into_iter()
        .flat_map(|key| {
            execution.result.context.get(key).iter().map(move |fact| {
                json!({
                    "key": format!("{key:?}"),
                    "id": fact.id().to_string(),
                    "content": fact.text().unwrap_or_default(),
                    "created_at": fact.created_at().as_str(),
                })
            })
        })
        .collect::<Vec<_>>();
    let events = execution
        .experience_events
        .iter()
        .take(24)
        .map(|event| {
            serde_json::to_value(event).unwrap_or_else(|_| json!({ "debug": format!("{event:?}") }))
        })
        .collect::<Vec<_>>();

    json!({
        "runtime_scope_id": execution.runtime_scope_id,
        "stop_reason": format!("{:?}", execution.result.stop_reason),
        "criteria": criteria,
        "facts": facts,
        "audit": {
            "experience_event_count": execution.experience_events.len(),
            "events": events,
        },
    })
}

fn experience_event_kind_name(event: &converge_core::ExperienceEvent) -> &'static str {
    match event.kind() {
        converge_core::ExperienceEventKind::ProposalCreated => "proposal-created",
        converge_core::ExperienceEventKind::ProposalValidated => "proposal-validated",
        converge_core::ExperienceEventKind::FactPromoted => "fact-promoted",
        converge_core::ExperienceEventKind::RecallExecuted => "recall-executed",
        converge_core::ExperienceEventKind::ReplayabilityDowngraded => "replayability-downgraded",
        converge_core::ExperienceEventKind::ArtifactStateTransitioned => {
            "artifact-state-transitioned"
        }
        converge_core::ExperienceEventKind::ArtifactRollbackRecorded => {
            "artifact-rollback-recorded"
        }
        converge_core::ExperienceEventKind::BackendInvoked => "backend-invoked",
        converge_core::ExperienceEventKind::OutcomeRecorded => "outcome-recorded",
        converge_core::ExperienceEventKind::BudgetExceeded => "budget-exceeded",
        converge_core::ExperienceEventKind::PolicySnapshotCaptured => "policy-snapshot-captured",
        converge_core::ExperienceEventKind::ReplayTraceRecorded => "replay-trace-recorded",
        converge_core::ExperienceEventKind::HypothesisResolved => "hypothesis-resolved",
        converge_core::ExperienceEventKind::GateDecisionRecorded => "gate-decision-recorded",
    }
}

fn criterion_summary(outcome: &converge_core::CriterionOutcome) -> serde_json::Value {
    match &outcome.result {
        converge_core::CriterionResult::Met { evidence } => json!({
            "id": outcome.criterion.id.to_string(),
            "label": outcome.criterion.description,
            "status": "met",
            "evidence": evidence.iter().map(ToString::to_string).collect::<Vec<_>>(),
        }),
        converge_core::CriterionResult::Blocked {
            reason,
            approval_ref,
        } => json!({
            "id": outcome.criterion.id.to_string(),
            "label": outcome.criterion.description,
            "status": "blocked",
            "reason": reason,
            "approval_ref": approval_ref.as_ref().map(ToString::to_string),
        }),
        converge_core::CriterionResult::Unmet { reason } => json!({
            "id": outcome.criterion.id.to_string(),
            "label": outcome.criterion.description,
            "status": "unmet",
            "reason": reason,
        }),
        converge_core::CriterionResult::Indeterminate => json!({
            "id": outcome.criterion.id.to_string(),
            "label": outcome.criterion.description,
            "status": "indeterminate",
        }),
    }
}

struct BlockedGate {
    reason: String,
    ref_id: Option<String>,
}

fn blocked_gate(execution: &TruthExecutionArtifacts) -> Option<BlockedGate> {
    execution
        .result
        .criteria_outcomes
        .iter()
        .find_map(|outcome| match &outcome.result {
            converge_core::CriterionResult::Blocked {
                reason,
                approval_ref,
            } => Some(BlockedGate {
                reason: reason.clone(),
                ref_id: approval_ref.as_ref().map(ToString::to_string),
            }),
            _ => None,
        })
}

fn scoped_gate_ref(run_id: &str, runtime_ref: &str) -> String {
    format!("{run_id}:{runtime_ref}")
}

// ── SSE Stream ────────────────────────────────────────────────────────

fn build_run_sse_stream(
    subscription: EventSubscription,
    run_id: String,
) -> impl tokio_stream::Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        let mut last_sequence = 0u64;

        // Replay events are already filtered by run_id via subscribe_with_cursor.
        for env in subscription.replay {
            last_sequence = env.sequence;
            let terminal = is_terminal(&env);
            if let Some(frame) = encode_frame(&env) {
                yield Ok(frame);
            }
            if terminal { return; }
        }

        let mut live = subscription.receiver;
        loop {
            match live.recv().await {
                Ok(env) => {
                    if env.sequence <= last_sequence { continue; }
                    last_sequence = env.sequence;
                    // Live channel is unfiltered; filter by run_id here.
                    if env.run_id.as_deref() != Some(run_id.as_str()) { continue; }
                    let terminal = is_terminal(&env);
                    if let Some(frame) = encode_frame(&env) {
                        yield Ok(frame);
                    }
                    if terminal { break; }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}

fn is_terminal(env: &EventEnvelope) -> bool {
    matches!(env.r#type.as_str(), "job.completed" | "job.failed")
}

fn encode_frame(env: &EventEnvelope) -> Option<Event> {
    serde_json::to_string(env)
        .ok()
        .map(|data| Event::default().id(env.sequence.to_string()).data(data))
}
