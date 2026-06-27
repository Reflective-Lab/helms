//! HTTP adapter for the coordination service.
//!
//! Thin handlers over [`CoordinationService`]. All routes are mounted under
//! `/v1/coordination/`. Business semantics live in the service, not here.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use runway_app_host::{EventCursor, EventEnvelope, EventSubscription};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::CoordinationError;
use crate::ledger::{DecisionOutcome, GateDecisionKind};
use crate::principal::PrincipalClaim;
use crate::service::CoordinationService;
use crate::subject::SubjectRef;

/// Build the coordination router over a shared service.
pub fn router(service: Arc<CoordinationService>) -> Router {
    Router::new()
        .route("/v1/coordination/sessions", post(open_session).get(list_sessions))
        .route("/v1/coordination/sessions/{id}/heartbeat", post(heartbeat))
        .route("/v1/coordination/sessions/{id}", axum::routing::delete(close_session))
        .route("/v1/coordination/presence", get(list_presence))
        .route("/v1/coordination/presence/focus", post(focus))
        .route("/v1/coordination/presence/claim", post(claim))
        .route("/v1/coordination/presence/release", post(release))
        .route("/v1/coordination/gates/{ref_id}/decision", post(decide_gate))
        .route("/v1/coordination/stream", get(stream))
        .with_state(service)
}

// ── Sessions ────────────────────────────────────────────────────────────────

async fn open_session(
    State(service): State<Arc<CoordinationService>>,
    Json(claim): Json<PrincipalClaim>,
) -> Result<(StatusCode, Json<crate::session::Session>), CoordinationError> {
    let session = service.open_session(&claim)?;
    Ok((StatusCode::CREATED, Json(session)))
}

async fn heartbeat(
    State(service): State<Arc<CoordinationService>>,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::session::Session>, CoordinationError> {
    service.heartbeat(id).map(Json)
}

async fn close_session(
    State(service): State<Arc<CoordinationService>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, CoordinationError> {
    service.close_session(id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct WorkspaceQuery {
    workspace: String,
    #[serde(default)]
    subject_kind: Option<String>,
    #[serde(default)]
    subject_id: Option<String>,
}

async fn list_sessions(
    State(service): State<Arc<CoordinationService>>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<Vec<crate::session::Session>> {
    Json(service.list_sessions(&query.workspace))
}

// ── Presence ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PresenceBody {
    session_id: Uuid,
    subject_kind: String,
    subject_id: String,
    #[serde(flatten)]
    claim: PrincipalClaim,
}

async fn focus(
    State(service): State<Arc<CoordinationService>>,
    Json(body): Json<PresenceBody>,
) -> Result<Json<crate::presence::PresenceEntry>, CoordinationError> {
    let subject = SubjectRef::new(body.subject_kind, body.subject_id);
    service.focus(body.session_id, &body.claim, subject).map(Json)
}

async fn claim(
    State(service): State<Arc<CoordinationService>>,
    Json(body): Json<PresenceBody>,
) -> Result<Json<crate::presence::PresenceEntry>, CoordinationError> {
    let subject = SubjectRef::new(body.subject_kind, body.subject_id);
    service.claim(body.session_id, &body.claim, subject).map(Json)
}

async fn release(
    State(service): State<Arc<CoordinationService>>,
    Json(body): Json<PresenceBody>,
) -> Result<Json<serde_json::Value>, CoordinationError> {
    let subject = SubjectRef::new(body.subject_kind, body.subject_id);
    let released = service.release(body.session_id, &body.claim, subject)?;
    Ok(Json(json!({ "released": released })))
}

async fn list_presence(
    State(service): State<Arc<CoordinationService>>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<Vec<crate::presence::PresenceEntry>> {
    let subject = match (query.subject_kind, query.subject_id) {
        (Some(kind), Some(id)) => Some(SubjectRef::new(kind, id)),
        _ => None,
    };
    Json(service.list_presence(&query.workspace, subject.as_ref()))
}

// ── Gate decisions ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GateDecisionBody {
    decision: GateDecisionKind,
    #[serde(default)]
    note: Option<String>,
    #[serde(flatten)]
    claim: PrincipalClaim,
}

async fn decide_gate(
    State(service): State<Arc<CoordinationService>>,
    Path(ref_id): Path<String>,
    Json(body): Json<GateDecisionBody>,
) -> Result<Response, CoordinationError> {
    let outcome = service.decide_gate(&ref_id, &body.claim, body.decision, body.note)?;
    Ok(decision_response(outcome))
}

fn decision_response(outcome: DecisionOutcome) -> Response {
    match outcome {
        DecisionOutcome::Recorded(record) => (
            StatusCode::OK,
            Json(json!({ "status": "recorded", "decision": record })),
        )
            .into_response(),
        DecisionOutcome::Idempotent(record) => (
            StatusCode::OK,
            Json(json!({ "status": "idempotent", "decision": record })),
        )
            .into_response(),
        DecisionOutcome::Conflict {
            existing,
            attempted,
            attempted_by,
        } => (
            StatusCode::CONFLICT,
            Json(json!({
                "status": "conflict",
                "existing": existing,
                "attempted": attempted,
                "attempted_by": attempted_by,
            })),
        )
            .into_response(),
    }
}

// ── Coordination stream ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct StreamQuery {
    workspace: String,
}

async fn stream(
    State(service): State<Arc<CoordinationService>>,
    Query(query): Query<StreamQuery>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let subscription = service
        .hub()
        .subscribe_with_cursor(EventCursor::default())
        .await;
    let stream = build_stream(subscription, query.workspace);
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

fn build_stream(
    subscription: EventSubscription,
    workspace_id: String,
) -> impl tokio_stream::Stream<Item = Result<Event, Infallible>> {
    runway_app_host::sse::event_stream(
        subscription,
        move |env| include(env, &workspace_id),
        |_| false,
    )
}

fn include(env: &EventEnvelope, workspace_id: &str) -> bool {
    let event_workspace = env.payload.get("workspace_id").and_then(|v| v.as_str());
    CoordinationService::stream_includes(&env.r#type, event_workspace, workspace_id)
}
