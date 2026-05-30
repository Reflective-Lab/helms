//! Governed job stream — `POST /v1/jobs/{key}/stream`
//!
//! Adapted from `application-server/src/job_stream.rs` in Phase 4b.
//!
//! # Key changes from the original
//!
//! - `crate::http_api::HttpState<S>` → `JobStreamState` (self-contained state).
//!   The generic `S: KernelStore` is resolved to the concrete `AppKernelStore`
//!   enum (same resolution used by `helm-truth-execution`).
//! - `crate::truth_runtime::{execute_truth, supports_truth_execution}` →
//!   `helm_truth_execution::dispatcher::{execute_truth, supports_truth_execution}`
//!   called with `&state.truths` as the registry.
//! - `crate::realtime::{RealtimeHub, RealtimeEvent, ...}` → local `hub` module
//!   (verbatim copy; `runway_app_host::EventHubHandle` lacks sequence numbers
//!   and per-run-id replay, which are required for SSE delivery).
//! - `crate::sse::*` helpers → inline SSE logic (no sse.rs was imported; the
//!   original job_stream.rs already had its SSE helpers inline).
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
//! truths registered) and a fresh `RealtimeHub`. Routes built with this state
//! will return `501 Not Implemented` for every truth key, which is the same
//! behaviour as `application-server`'s maintenance-mode `supports_truth_execution`.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use application_kernel::Actor;
use application_storage::{AppKernelStore, AppRuntimeStores, InMemoryKernelStore};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::post;
use axum::{Json, Router};
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

use crate::hub::{RealtimeCursor, RealtimeEvent, RealtimeEventInput, RealtimeHub};
use helm_truth_execution::{
    TruthExecutionArtifacts, TruthExecutionModule,
    dispatcher::{TruthExecutionContext, execute_truth, supports_truth_execution},
};

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
/// 501), or with `JobStreamState::new(store, runtime_stores, truths, hub)` for
/// real wiring.
#[derive(Clone)]
pub struct JobStreamState {
    pub store: AppKernelStore,
    pub runtime_stores: AppRuntimeStores,
    pub truths: Arc<TruthExecutionModule>,
    pub hub: RealtimeHub,
    gate_waiters: Arc<Mutex<HashMap<String, JobGateWaiter>>>,
}

impl JobStreamState {
    pub fn new(
        store: AppKernelStore,
        runtime_stores: AppRuntimeStores,
        truths: Arc<TruthExecutionModule>,
        hub: RealtimeHub,
    ) -> Self {
        Self {
            store,
            runtime_stores,
            truths,
            hub,
            gate_waiters: Arc::new(Mutex::new(HashMap::new())),
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
}

impl Default for JobStreamState {
    fn default() -> Self {
        Self::new(
            AppKernelStore::Memory(InMemoryKernelStore::default_local()),
            AppRuntimeStores::default(),
            Arc::new(TruthExecutionModule::new()),
            RealtimeHub::new(256),
        )
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
        .unwrap_or("helms")
        .to_string();

    // Subscribe before spawning so no events are missed.
    let subscription = state.hub.subscribe(RealtimeCursor::default()).await;

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

struct JobRunTask {
    state: Arc<JobStreamState>,
    run_id: String,
    truth_key: String,
    app_id: String,
    inputs: HashMap<String, String>,
}

async fn run_job_task(task: JobRunTask) {
    let JobRunTask {
        state,
        run_id,
        truth_key,
        app_id,
        inputs,
    } = task;

    let hub = &state.hub;

    publish(hub, &run_id, &truth_key, &app_id, "job.started", json!({})).await;
    if let Err(error) = admit_job(hub, &run_id, &truth_key, &app_id).await {
        publish(
            hub,
            &run_id,
            &truth_key,
            &app_id,
            "job.failed",
            json!({ "error": error }),
        )
        .await;
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
            publish(
                hub,
                &run_id,
                &truth_key,
                &app_id,
                "job.failed",
                json!({ "error": status.message() }),
            )
            .await;
            return;
        }
    };
    publish_runtime_events(hub, &run_id, &truth_key, &app_id, "pre-gate", &first).await;

    let total = first.result.criteria_outcomes.len().max(1);
    let mut emitted = 0usize;
    for (i, outcome) in first.result.criteria_outcomes.iter().enumerate() {
        match &outcome.result {
            converge_core::CriterionResult::Met { .. } => {
                let progress = ((i + 1) * 100 / total) as u32;
                publish_step(
                    hub,
                    &run_id,
                    &truth_key,
                    &app_id,
                    i as u32,
                    &outcome.criterion.description,
                    progress,
                )
                .await;
                emitted = i + 1;
            }
            converge_core::CriterionResult::Blocked { .. } => break,
            _ => {}
        }
    }

    let Some(initial_blocked_gate) = blocked_gate(&first) else {
        let receipt = publish_completion_receipt(hub, &run_id, &truth_key, &app_id, &first).await;
        publish(
            hub,
            &run_id,
            &truth_key,
            &app_id,
            "job.completed",
            json!({ "result": receipt }),
        )
        .await;
        return;
    };
    let runtime_ref = match initial_blocked_gate.ref_id.clone() {
        Some(ref_id) => ref_id,
        None => {
            publish(
                hub,
                &run_id,
                &truth_key,
                &app_id,
                "job.failed",
                json!({
                    "error": "gate blocked without approval reference",
                    "gate_reason": initial_blocked_gate.reason,
                }),
            )
            .await;
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

    publish(
        hub,
        &run_id,
        &truth_key,
        &app_id,
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
    )
    .await;

    let decision = match tokio::time::timeout(Duration::from_secs(600), decision_rx).await {
        Ok(Ok(d)) => d,
        Ok(Err(_)) | Err(_) => {
            publish(
                hub,
                &run_id,
                &truth_key,
                &app_id,
                "job.failed",
                json!({ "error": "gate waiter expired or cancelled" }),
            )
            .await;
            return;
        }
    };

    match decision {
        GateDecision::Rejected => {
            publish(
                hub,
                &run_id,
                &truth_key,
                &app_id,
                "gate.rejected",
                json!({ "ref_id": gate_ref.clone(), "runtime_ref": runtime_ref.clone() }),
            )
            .await;
            publish(
                hub,
                &run_id,
                &truth_key,
                &app_id,
                "job.failed",
                json!({ "error": "gate rejected by operator" }),
            )
            .await;
            return;
        }
        GateDecision::Approved => {
            publish(
                hub,
                &run_id,
                &truth_key,
                &app_id,
                "gate.approved",
                json!({ "ref_id": gate_ref.clone(), "runtime_ref": runtime_ref.clone() }),
            )
            .await;
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
            publish(
                hub,
                &run_id,
                &truth_key,
                &app_id,
                "job.failed",
                json!({ "error": status.message() }),
            )
            .await;
            return;
        }
    };
    publish_runtime_events(hub, &run_id, &truth_key, &app_id, "post-gate", &second).await;

    if let Some(blocked_gate) = blocked_gate(&second) {
        publish(
            hub,
            &run_id,
            &truth_key,
            &app_id,
            "job.failed",
            json!({
                "error": "gate remained blocked after approval",
                "gate_reason": blocked_gate.reason,
                "runtime_ref": blocked_gate.ref_id,
            }),
        )
        .await;
        return;
    }

    let second_total = second.result.criteria_outcomes.len().max(1);
    for (i, outcome) in second.result.criteria_outcomes.iter().enumerate() {
        if i < emitted {
            continue;
        }
        if let converge_core::CriterionResult::Met { .. } = &outcome.result {
            let progress = ((i + 1) * 100 / second_total) as u32;
            publish_step(
                hub,
                &run_id,
                &truth_key,
                &app_id,
                i as u32,
                &outcome.criterion.description,
                progress,
            )
            .await;
        }
    }

    let receipt = publish_completion_receipt(hub, &run_id, &truth_key, &app_id, &second).await;
    publish(
        hub,
        &run_id,
        &truth_key,
        &app_id,
        "job.completed",
        json!({ "result": receipt }),
    )
    .await;
}

async fn admit_job(
    hub: &RealtimeHub,
    run_id: &str,
    truth_key: &str,
    app_id: &str,
) -> Result<(), String> {
    let mut context = ContextState::new();
    let intent = admit_truth_intent(
        truth_key,
        app_id,
        &format!("truth:{truth_key}"),
        &mut context,
    )
    .map_err(|error| format!("axiom intent admission failed: {error}"))?;

    publish(
        hub,
        run_id,
        truth_key,
        app_id,
        "axiom.intent.compiled",
        axiom_intent_payload(&intent),
    )
    .await;

    let selection = select_formation_for_intent(&intent, &default_helms_capabilities())
        .map_err(|error| format!("organism formation selection failed: {error}"))?;
    publish(
        hub,
        run_id,
        truth_key,
        app_id,
        "organism.formation.selected",
        formation_selection_payload(&selection),
    )
    .await;

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

async fn publish_runtime_events(
    hub: &RealtimeHub,
    run_id: &str,
    job_id: &str,
    app_id: &str,
    phase: &str,
    execution: &TruthExecutionArtifacts,
) {
    for (event_index, event) in execution.experience_events.iter().enumerate() {
        let kind = experience_event_kind_name(event);
        let event_type = format!("converge.runtime.{kind}");
        let payload = serde_json::to_value(event)
            .unwrap_or_else(|_| json!({ "debug": format!("{event:?}") }));
        publish(
            hub,
            run_id,
            job_id,
            app_id,
            &event_type,
            json!({
                "stage": "converge-runtime",
                "phase": phase,
                "runtime_scope_id": execution.runtime_scope_id,
                "event_index": event_index,
                "kind": kind,
                "event": payload,
            }),
        )
        .await;
    }
}

async fn publish_completion_receipt(
    hub: &RealtimeHub,
    run_id: &str,
    job_id: &str,
    app_id: &str,
    execution: &TruthExecutionArtifacts,
) -> serde_json::Value {
    let receipt = completion_summary(execution);
    publish(
        hub,
        run_id,
        job_id,
        app_id,
        "job.receipt.recorded",
        json!({
            "receipt_id": format!("{run_id}:completion"),
            "status": "completed",
            "result": receipt.clone(),
        }),
    )
    .await;
    receipt
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
    subscription: crate::hub::RealtimeSubscription,
    run_id: String,
) -> impl tokio_stream::Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        let mut last_sequence = 0u64;

        for event in subscription.replay {
            if event.run_id.as_deref() == Some(run_id.as_str()) {
                last_sequence = event.sequence;
                let terminal = is_terminal(&event);
                if let Some(frame) = encode_frame(&event) {
                    yield Ok(frame);
                }
                if terminal { return; }
            }
        }

        let mut live = subscription.live;
        loop {
            match live.recv().await {
                Ok(event) => {
                    if event.sequence <= last_sequence { continue; }
                    last_sequence = event.sequence;
                    if event.run_id.as_deref() != Some(run_id.as_str()) { continue; }
                    let terminal = is_terminal(&event);
                    if let Some(frame) = encode_frame(&event) {
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

fn is_terminal(event: &RealtimeEvent) -> bool {
    matches!(event.event_type.as_str(), "job.completed" | "job.failed")
}

fn encode_frame(event: &RealtimeEvent) -> Option<Event> {
    serde_json::to_string(event)
        .ok()
        .map(|data| Event::default().id(event.sequence.to_string()).data(data))
}

// ── Publish Helpers ───────────────────────────────────────────────────

async fn publish(
    hub: &RealtimeHub,
    run_id: &str,
    job_id: &str,
    app_id: &str,
    event_type: &str,
    payload: serde_json::Value,
) -> RealtimeEvent {
    hub.publish(RealtimeEventInput {
        event_type: event_type.to_string(),
        app_id: Some(app_id.to_string()),
        run_id: Some(run_id.to_string()),
        job_id: Some(job_id.to_string()),
        correlation_id: None,
        actor: None,
        payload,
    })
    .await
}

async fn publish_step(
    hub: &RealtimeHub,
    run_id: &str,
    job_id: &str,
    app_id: &str,
    step_index: u32,
    label: &str,
    progress: u32,
) -> RealtimeEvent {
    publish(
        hub,
        run_id,
        job_id,
        app_id,
        "job.step.completed",
        json!({
            "step_index": step_index,
            "label": label,
            "progress": progress,
        }),
    )
    .await
}
