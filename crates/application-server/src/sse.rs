//! Server-Sent Events for live convergence visibility + pipeline orchestration.
//!
//! Surface-neutral: works for desktop, CLI, browser, and automation clients.
//!
//! Endpoints:
//! - GET  /v1/realtime/stream           — SSE stream of typed realtime events
//! - GET  /v1/pipeline/showcase/stream  — SSE stream of pipeline execution events
//! - POST /v1/pipeline/showcase/run     — Start pipeline, returns immediately with run_id
//! - GET  /v1/pipeline/showcase/status  — Get current pipeline status
//! - POST /v1/pipeline/showcase/reset   — Reset kernel state for repeatable demo
//! - GET  /v1/approvals/pending         — List blocked steps needing approval
//! - POST /v1/approvals/{ref}/approve   — Approve a blocked step
//! - POST /v1/approvals/{ref}/reject    — Reject a blocked step

use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use application_kernel::Actor;
use application_storage::KernelStore;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::http_api::HttpState;
use crate::pipeline::{
    PipelineResult, PipelineStatus, load_prospect_context_from_seed, run_showcase_pipeline,
};
use crate::realtime::{RealtimeCursor, RealtimeEvent, RealtimeEventInput, RealtimeHub};

// ── Event Types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PipelineEvent {
    PipelineStarted {
        run_id: String,
        prospect: String,
    },
    StepStarted {
        step: usize,
        truth_key: String,
    },
    FactProposed {
        step: usize,
        fact_id: String,
        context_key: String,
        confidence: f64,
    },
    StepCompleted {
        step: usize,
        truth_key: String,
        cycles: u32,
        fact_count: usize,
    },
    StepBlocked {
        step: usize,
        truth_key: String,
        reason: String,
        approval_ref: Option<String>,
    },
    StepFailed {
        step: usize,
        truth_key: String,
        error: String,
    },
    PipelineCompleted {
        run_id: String,
        status: String,
    },
}

// ── Shared State for SSE ────────────────────────────────────────────

#[derive(Clone)]
pub struct PipelineState {
    pub realtime: RealtimeHub,
    pub current_result: Arc<tokio::sync::RwLock<Option<PipelineResult>>>,
    pub pending_approvals: Arc<tokio::sync::RwLock<Vec<PendingApproval>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingApproval {
    pub approval_ref: String,
    pub truth_key: String,
    pub step: usize,
    pub reason: String,
    pub created_at: String,
}

impl PipelineState {
    pub fn new() -> Self {
        Self::with_hub(RealtimeHub::new(512))
    }

    pub fn with_hub(realtime: RealtimeHub) -> Self {
        Self {
            realtime,
            current_result: Arc::new(tokio::sync::RwLock::new(None)),
            pending_approvals: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }
}

// ── Router ──────────────────────────────────────────────────────────

pub fn pipeline_routes<S>() -> Router<HttpState<S>>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/v1/realtime/stream", get(stream_realtime))
        .route("/v1/pipeline/showcase/stream", get(stream_pipeline::<S>))
        .route("/v1/pipeline/showcase/run", post(run_pipeline::<S>))
        .route("/v1/pipeline/showcase/status", get(pipeline_status::<S>))
        .route("/v1/pipeline/showcase/reset", post(reset_pipeline::<S>))
        .route("/v1/approvals/pending", get(list_pending_approvals::<S>))
        .route(
            "/v1/approvals/{approval_ref}/approve",
            post(approve_step::<S>),
        )
        .route(
            "/v1/approvals/{approval_ref}/reject",
            post(reject_step::<S>),
        )
}

// ── SSE Stream ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct RealtimeStreamQuery {
    since_seq: Option<u64>,
    last_event_id: Option<String>,
}

async fn stream_realtime(
    Query(query): Query<RealtimeStreamQuery>,
    headers: HeaderMap,
    axum::Extension(hub): axum::Extension<RealtimeHub>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let subscription = hub.subscribe(realtime_cursor(query, &headers)).await;
    let stream = realtime_sse_stream(subscription, |_| true, realtime_event_frame);

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn stream_pipeline<S>(
    Query(query): Query<RealtimeStreamQuery>,
    headers: HeaderMap,
    axum::Extension(state): axum::Extension<PipelineState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let subscription = state
        .realtime
        .subscribe(realtime_cursor(query, &headers))
        .await;
    let stream = realtime_sse_stream(
        subscription,
        |event| event.event_type.starts_with("pipeline."),
        pipeline_payload_frame,
    );

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

// ── Run Pipeline ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RunPipelineRequest {
    pub prospect_id: Option<String>,
    pub prospect_name: Option<String>,
    pub inbound_summary: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RunPipelineResponse {
    pub run_id: String,
    pub status: String,
}

async fn run_pipeline<S>(
    State(http_state): State<HttpState<S>>,
    axum::Extension(pipeline_state): axum::Extension<PipelineState>,
    Json(request): Json<RunPipelineRequest>,
) -> Result<Json<RunPipelineResponse>, (StatusCode, String)>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let run_id = uuid::Uuid::new_v4().to_string();
    let prospect_id = request.prospect_id.unwrap_or_else(|| "prospect-001".into());

    // Load from seed data
    let seed_dir = PathBuf::from("data/seed");
    let mut input = load_prospect_context_from_seed(&seed_dir, &prospect_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    // Override with request fields if provided
    if let Some(name) = request.prospect_name {
        input.prospect_name = name;
    }
    if let Some(summary) = request.inbound_summary {
        input.inbound_summary = summary;
    }

    let realtime = pipeline_state.realtime.clone();
    let result_lock = pipeline_state.current_result.clone();
    let approvals_lock = pipeline_state.pending_approvals.clone();
    let run_id_clone = run_id.clone();

    // Emit start event
    publish_pipeline_event(
        &realtime,
        Some(&run_id),
        PipelineEvent::PipelineStarted {
            run_id: run_id.clone(),
            prospect: input.prospect_name.clone(),
        },
    )
    .await;

    // Run pipeline in background
    let store = http_state.store.clone();
    let runtime_stores = http_state.runtime_stores.clone();

    tokio::spawn(async move {
        let actor = Actor::system();

        // Emit step events as pipeline runs
        publish_pipeline_event(
            &realtime,
            Some(&run_id_clone),
            PipelineEvent::StepStarted {
                step: 0,
                truth_key: "score-inbound-fit".into(),
            },
        )
        .await;

        let result = run_showcase_pipeline(&store, &runtime_stores, input, actor).await;

        // Emit completion events based on result
        for (i, step) in result.steps.iter().enumerate() {
            match &step.status {
                crate::pipeline::StepStatus::Completed => {
                    publish_pipeline_event(
                        &realtime,
                        Some(&run_id_clone),
                        PipelineEvent::StepCompleted {
                            step: i,
                            truth_key: step.truth_key.clone(),
                            cycles: step.cycles.unwrap_or(0),
                            fact_count: step.fact_count.unwrap_or(0),
                        },
                    )
                    .await;
                    // Emit next step started if not last
                    if i + 1 < result.steps.len() {
                        publish_pipeline_event(
                            &realtime,
                            Some(&run_id_clone),
                            PipelineEvent::StepStarted {
                                step: i + 1,
                                truth_key: result.steps[i + 1].truth_key.clone(),
                            },
                        )
                        .await;
                    }
                }
                crate::pipeline::StepStatus::Blocked { reason } => {
                    let approval_ref = format!("approval-{}-{}", run_id_clone, i);
                    publish_pipeline_event(
                        &realtime,
                        Some(&run_id_clone),
                        PipelineEvent::StepBlocked {
                            step: i,
                            truth_key: step.truth_key.clone(),
                            reason: reason.clone(),
                            approval_ref: Some(approval_ref.clone()),
                        },
                    )
                    .await;

                    // Register pending approval
                    let mut approvals = approvals_lock.write().await;
                    approvals.push(PendingApproval {
                        approval_ref,
                        truth_key: step.truth_key.clone(),
                        step: i,
                        reason: reason.clone(),
                        created_at: chrono::Utc::now().to_rfc3339(),
                    });
                }
                crate::pipeline::StepStatus::Failed { error } => {
                    publish_pipeline_event(
                        &realtime,
                        Some(&run_id_clone),
                        PipelineEvent::StepFailed {
                            step: i,
                            truth_key: step.truth_key.clone(),
                            error: error.clone(),
                        },
                    )
                    .await;
                }
                crate::pipeline::StepStatus::Skipped => {}
            }
        }

        let status_str = match &result.status {
            PipelineStatus::Completed => "completed",
            PipelineStatus::BlockedAtStep { .. } => "blocked",
            PipelineStatus::Failed { .. } => "failed",
        };

        publish_pipeline_event(
            &realtime,
            Some(&run_id_clone),
            PipelineEvent::PipelineCompleted {
                run_id: run_id_clone.clone(),
                status: status_str.into(),
            },
        )
        .await;

        // Store result
        let mut current = result_lock.write().await;
        *current = Some(result);
    });

    Ok(Json(RunPipelineResponse {
        run_id,
        status: "started".into(),
    }))
}

// ── Pipeline Status ─────────────────────────────────────────────────

async fn pipeline_status<S>(
    axum::Extension(state): axum::Extension<PipelineState>,
) -> impl IntoResponse
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let result = state.current_result.read().await;
    match result.as_ref() {
        Some(r) => Json(serde_json::to_value(r).unwrap_or_default()).into_response(),
        None => (StatusCode::NO_CONTENT, "no pipeline has run yet").into_response(),
    }
}

// ── Reset ───────────────────────────────────────────────────────────

async fn reset_pipeline<S>(
    axum::Extension(state): axum::Extension<PipelineState>,
) -> impl IntoResponse
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let mut result = state.current_result.write().await;
    *result = None;
    let mut approvals = state.pending_approvals.write().await;
    approvals.clear();
    (StatusCode::OK, "pipeline state reset")
}

// ── Approvals ───────────────────────────────────────────────────────

async fn list_pending_approvals<S>(
    axum::Extension(state): axum::Extension<PipelineState>,
) -> impl IntoResponse
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let approvals = state.pending_approvals.read().await;
    Json(approvals.clone())
}

#[derive(Debug, Deserialize)]
pub struct ApprovalDecision {
    pub actor: Option<String>,
    pub approver_name: Option<String>,
    pub reason: Option<String>,
    pub approval_note: Option<String>,
    pub rejection_note: Option<String>,
    pub policy_snapshot_hash: Option<String>,
    pub delegate_to_policy: Option<bool>,
}

async fn approve_step<S>(
    State(http_state): State<HttpState<S>>,
    Path(approval_ref): Path<String>,
    axum::Extension(state): axum::Extension<PipelineState>,
    axum::Extension(job_stream_state): axum::Extension<crate::job_stream::JobStreamState>,
    Json(decision): Json<ApprovalDecision>,
) -> (StatusCode, String)
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let job_waiter = job_stream_state.take_gate_waiter(&approval_ref);
    if let Some(waiter) = job_waiter {
        if let Err(error) = append_user_approval(
            &http_state.runtime_stores,
            waiter.runtime_scope_id(),
            waiter.runtime_ref(),
            &decision,
        ) {
            job_stream_state.restore_gate_waiter(approval_ref, waiter);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("approval persistence failed: {error}"),
            );
        }
        waiter.signal(crate::job_stream::GateDecision::Approved);
        return (StatusCode::OK, format!("approved: {approval_ref}"));
    }

    let mut approvals = state.pending_approvals.write().await;
    if let Some(pos) = approvals
        .iter()
        .position(|a| a.approval_ref == approval_ref)
    {
        let approval = approvals.remove(pos);
        publish_pipeline_event(
            &state.realtime,
            None,
            PipelineEvent::StepCompleted {
                step: approval.step,
                truth_key: approval.truth_key,
                cycles: 0,
                fact_count: 0,
            },
        )
        .await;
        (StatusCode::OK, format!("approved: {approval_ref}"))
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("approval not found: {approval_ref}"),
        )
    }
}

async fn reject_step<S>(
    State(http_state): State<HttpState<S>>,
    Path(approval_ref): Path<String>,
    axum::Extension(state): axum::Extension<PipelineState>,
    axum::Extension(job_stream_state): axum::Extension<crate::job_stream::JobStreamState>,
    Json(decision): Json<ApprovalDecision>,
) -> (StatusCode, String)
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let job_waiter = job_stream_state.take_gate_waiter(&approval_ref);
    if let Some(waiter) = job_waiter {
        if let Err(error) = append_user_rejection(
            &http_state.runtime_stores,
            waiter.runtime_scope_id(),
            waiter.runtime_ref(),
            &decision,
        ) {
            job_stream_state.restore_gate_waiter(approval_ref, waiter);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("rejection persistence failed: {error}"),
            );
        }
        waiter.signal(crate::job_stream::GateDecision::Rejected);
        return (StatusCode::OK, format!("rejected: {approval_ref}"));
    }

    let mut approvals = state.pending_approvals.write().await;
    if let Some(pos) = approvals
        .iter()
        .position(|a| a.approval_ref == approval_ref)
    {
        let approval = approvals.remove(pos);
        let reason = decision
            .reason
            .unwrap_or_else(|| "rejected by operator".into());
        publish_pipeline_event(
            &state.realtime,
            None,
            PipelineEvent::StepFailed {
                step: approval.step,
                truth_key: approval.truth_key,
                error: reason,
            },
        )
        .await;
        (StatusCode::OK, format!("rejected: {approval_ref}"))
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("approval not found: {approval_ref}"),
        )
    }
}

fn append_user_approval(
    runtime_stores: &application_storage::AppRuntimeStores,
    runtime_scope_id: &str,
    approval_ref: &str,
    decision: &ApprovalDecision,
) -> Result<(), application_storage::StorageError> {
    let gate_request_id =
        crate::truth_runtime::runtime_gate_request_id(runtime_scope_id, approval_ref);
    let event_id = format!("evt-user-{}", Uuid::new_v4().simple());
    let envelope = converge_core::UserExperienceEventEnvelope::new(
        event_id.as_str(),
        converge_core::UserExperienceEvent::UserApprovalGranted {
            gate_request_id: converge_core::GateId::new(gate_request_id),
            actor: converge_pack::ActorId::new(decision_actor(decision)),
            policy_snapshot_hash: policy_snapshot_hash(decision),
            reason: approval_reason(decision),
        },
    )
    .with_correlation(runtime_scope_id.to_string());

    runtime_stores.append_user_event(envelope)
}

fn append_user_rejection(
    runtime_stores: &application_storage::AppRuntimeStores,
    runtime_scope_id: &str,
    approval_ref: &str,
    decision: &ApprovalDecision,
) -> Result<(), application_storage::StorageError> {
    let gate_request_id =
        crate::truth_runtime::runtime_gate_request_id(runtime_scope_id, approval_ref);
    let event_id = format!("evt-user-{}", Uuid::new_v4().simple());
    let envelope = converge_core::UserExperienceEventEnvelope::new(
        event_id.as_str(),
        converge_core::UserExperienceEvent::UserApprovalRejected {
            gate_request_id: converge_core::GateId::new(gate_request_id),
            actor: converge_pack::ActorId::new(decision_actor(decision)),
            policy_snapshot_hash: policy_snapshot_hash(decision),
            reason: Some(rejection_reason(decision)),
        },
    )
    .with_correlation(runtime_scope_id.to_string());

    runtime_stores.append_user_event(envelope)
}

fn decision_actor(decision: &ApprovalDecision) -> String {
    decision
        .actor
        .as_deref()
        .or(decision.approver_name.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("operator")
        .to_string()
}

fn approval_reason(decision: &ApprovalDecision) -> Option<String> {
    decision
        .approval_note
        .as_deref()
        .or(decision.reason.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn rejection_reason(decision: &ApprovalDecision) -> String {
    decision
        .rejection_note
        .as_deref()
        .or(decision.reason.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("rejected by operator")
        .to_string()
}

fn policy_snapshot_hash(decision: &ApprovalDecision) -> Option<converge_core::ContentHash> {
    if decision.delegate_to_policy == Some(false) {
        return None;
    }
    decision
        .policy_snapshot_hash
        .as_deref()
        .map(converge_core::ContentHash::from_hex)
}

fn realtime_cursor(query: RealtimeStreamQuery, headers: &HeaderMap) -> RealtimeCursor {
    RealtimeCursor {
        since_sequence: query.since_seq,
        last_event_id: query.last_event_id.or_else(|| {
            headers
                .get("last-event-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        }),
    }
}

fn realtime_sse_stream<F, E>(
    mut subscription: crate::realtime::RealtimeSubscription,
    filter: F,
    encode: E,
) -> impl tokio_stream::Stream<Item = Result<Event, Infallible>>
where
    F: Fn(&RealtimeEvent) -> bool + Send + Sync + 'static,
    E: Fn(&RealtimeEvent) -> Option<Event> + Send + Sync + 'static,
{
    async_stream::stream! {
        let mut last_sequence = subscription
            .replay
            .last()
            .map(|event| event.sequence)
            .unwrap_or(0);

        for event in subscription.replay {
            if filter(&event) {
                if let Some(frame) = encode(&event) {
                    yield Ok(frame);
                }
            }
        }

        loop {
            match subscription.live.recv().await {
                Ok(event) => {
                    if event.sequence <= last_sequence {
                        continue;
                    }
                    last_sequence = event.sequence;

                    if filter(&event) {
                        if let Some(frame) = encode(&event) {
                            yield Ok(frame);
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}

fn realtime_event_frame(event: &RealtimeEvent) -> Option<Event> {
    serde_json::to_string(event)
        .ok()
        .map(|data| Event::default().id(event.sequence.to_string()).data(data))
}

fn pipeline_payload_frame(event: &RealtimeEvent) -> Option<Event> {
    serde_json::to_string(&event.payload)
        .ok()
        .map(|data| Event::default().id(event.sequence.to_string()).data(data))
}

async fn publish_pipeline_event(
    realtime: &RealtimeHub,
    run_id: Option<&str>,
    event: PipelineEvent,
) -> RealtimeEvent {
    let event_type = pipeline_realtime_type(&event).to_string();
    let payload = serde_json::to_value(&event).unwrap_or_default();

    realtime
        .publish(RealtimeEventInput {
            event_type,
            app_id: Some("helms".into()),
            run_id: run_id.map(str::to_owned),
            job_id: None,
            correlation_id: None,
            actor: None,
            payload,
        })
        .await
}

fn pipeline_realtime_type(event: &PipelineEvent) -> &'static str {
    match event {
        PipelineEvent::PipelineStarted { .. } => "pipeline.started",
        PipelineEvent::StepStarted { .. } => "pipeline.step.started",
        PipelineEvent::FactProposed { .. } => "pipeline.fact.proposed",
        PipelineEvent::StepCompleted { .. } => "pipeline.step.completed",
        PipelineEvent::StepBlocked { .. } => "pipeline.step.blocked",
        PipelineEvent::StepFailed { .. } => "pipeline.step.failed",
        PipelineEvent::PipelineCompleted { .. } => "pipeline.completed",
    }
}
