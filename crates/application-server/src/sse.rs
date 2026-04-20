//! Server-Sent Events for live convergence visibility + pipeline orchestration.
//!
//! Surface-neutral: works for desktop, CLI, browser, and automation clients.
//!
//! Endpoints:
//! - GET  /v1/pipeline/showcase/stream  — SSE stream of pipeline execution events
//! - POST /v1/pipeline/showcase/run     — Start pipeline, returns immediately with run_id
//! - GET  /v1/pipeline/showcase/status  — Get current pipeline status
//! - POST /v1/pipeline/showcase/reset   — Reset kernel state for repeatable demo
//! - GET  /v1/approvals/pending         — List blocked steps needing approval
//! - POST /v1/approvals/{ref}/approve   — Approve a blocked step
//! - POST /v1/approvals/{ref}/reject    — Reject a blocked step

use std::collections::HashMap;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use application_kernel::Actor;
use application_storage::{AppRuntimeStores, KernelStore};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::http_api::HttpState;
use crate::pipeline::{
    PipelineResult, PipelineStatus, ShowcasePipelineInput, load_prospect_context_from_seed,
    run_showcase_pipeline,
};

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
    pub events_tx: broadcast::Sender<PipelineEvent>,
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
        let (tx, _) = broadcast::channel(256);
        Self {
            events_tx: tx,
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
        .route("/v1/pipeline/showcase/stream", get(stream_pipeline::<S>))
        .route("/v1/pipeline/showcase/run", post(run_pipeline::<S>))
        .route("/v1/pipeline/showcase/status", get(pipeline_status::<S>))
        .route("/v1/pipeline/showcase/reset", post(reset_pipeline::<S>))
        .route("/v1/approvals/pending", get(list_pending_approvals::<S>))
        .route("/v1/approvals/{approval_ref}/approve", post(approve_step::<S>))
        .route("/v1/approvals/{approval_ref}/reject", post(reject_step::<S>))
}

// ── SSE Stream ──────────────────────────────────────────────────────

async fn stream_pipeline<S>(
    axum::Extension(state): axum::Extension<PipelineState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let rx = state.events_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| {
        result.ok().map(|event| {
            let data = serde_json::to_string(&event).unwrap_or_default();
            Ok(Event::default().data(data))
        })
    });

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

    let tx = pipeline_state.events_tx.clone();
    let result_lock = pipeline_state.current_result.clone();
    let approvals_lock = pipeline_state.pending_approvals.clone();
    let run_id_clone = run_id.clone();

    // Emit start event
    let _ = tx.send(PipelineEvent::PipelineStarted {
        run_id: run_id.clone(),
        prospect: input.prospect_name.clone(),
    });

    // Run pipeline in background
    let store = http_state.store.clone();
    let runtime_stores = http_state.runtime_stores.clone();

    tokio::spawn(async move {
        let actor = Actor::system();

        // Emit step events as pipeline runs
        let _ = tx.send(PipelineEvent::StepStarted {
            step: 0,
            truth_key: "score-inbound-fit".into(),
        });

        let result = run_showcase_pipeline(&store, &runtime_stores, input, actor).await;

        // Emit completion events based on result
        for (i, step) in result.steps.iter().enumerate() {
            match &step.status {
                crate::pipeline::StepStatus::Completed => {
                    let _ = tx.send(PipelineEvent::StepCompleted {
                        step: i,
                        truth_key: step.truth_key.clone(),
                        cycles: step.cycles.unwrap_or(0),
                        fact_count: step.fact_count.unwrap_or(0),
                    });
                    // Emit next step started if not last
                    if i + 1 < result.steps.len() {
                        let _ = tx.send(PipelineEvent::StepStarted {
                            step: i + 1,
                            truth_key: result.steps[i + 1].truth_key.clone(),
                        });
                    }
                }
                crate::pipeline::StepStatus::Blocked { reason } => {
                    let approval_ref = format!("approval-{}-{}", run_id_clone, i);
                    let _ = tx.send(PipelineEvent::StepBlocked {
                        step: i,
                        truth_key: step.truth_key.clone(),
                        reason: reason.clone(),
                        approval_ref: Some(approval_ref.clone()),
                    });

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
                    let _ = tx.send(PipelineEvent::StepFailed {
                        step: i,
                        truth_key: step.truth_key.clone(),
                        error: error.clone(),
                    });
                }
                crate::pipeline::StepStatus::Skipped => {}
            }
        }

        let status_str = match &result.status {
            PipelineStatus::Completed => "completed",
            PipelineStatus::BlockedAtStep { .. } => "blocked",
            PipelineStatus::Failed { .. } => "failed",
        };

        let _ = tx.send(PipelineEvent::PipelineCompleted {
            run_id: run_id_clone,
            status: status_str.into(),
        });

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
    pub reason: Option<String>,
}

async fn approve_step<S>(
    Path(approval_ref): Path<String>,
    axum::Extension(state): axum::Extension<PipelineState>,
    Json(decision): Json<ApprovalDecision>,
) -> impl IntoResponse
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let mut approvals = state.pending_approvals.write().await;
    if let Some(pos) = approvals.iter().position(|a| a.approval_ref == approval_ref) {
        let approval = approvals.remove(pos);
        let _ = state.events_tx.send(PipelineEvent::StepCompleted {
            step: approval.step,
            truth_key: approval.truth_key,
            cycles: 0,
            fact_count: 0,
        });
        (StatusCode::OK, format!("approved: {approval_ref}"))
    } else {
        (StatusCode::NOT_FOUND, format!("approval not found: {approval_ref}"))
    }
}

async fn reject_step<S>(
    Path(approval_ref): Path<String>,
    axum::Extension(state): axum::Extension<PipelineState>,
    Json(decision): Json<ApprovalDecision>,
) -> impl IntoResponse
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let mut approvals = state.pending_approvals.write().await;
    if let Some(pos) = approvals.iter().position(|a| a.approval_ref == approval_ref) {
        let approval = approvals.remove(pos);
        let reason = decision.reason.unwrap_or_else(|| "rejected by operator".into());
        let _ = state.events_tx.send(PipelineEvent::StepFailed {
            step: approval.step,
            truth_key: approval.truth_key,
            error: reason,
        });
        (StatusCode::OK, format!("rejected: {approval_ref}"))
    } else {
        (StatusCode::NOT_FOUND, format!("approval not found: {approval_ref}"))
    }
}
