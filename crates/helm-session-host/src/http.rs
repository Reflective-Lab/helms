// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! HTTP adapter — session-scoped SSE and ack endpoints.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use helm_session_contracts::{CompletionAck, DeliveryAck};
use runway_app_host::{EventCursor, EventSubscription};

use crate::service::SessionHostService;

/// Build the session-host router.
pub fn router(service: Arc<SessionHostService>) -> Router {
    Router::new()
        .route("/v1/sessions/{session_id}/stream", get(stream))
        .route("/v1/sessions/{session_id}/ack/delivery", post(delivery_ack))
        .route(
            "/v1/sessions/{session_id}/ack/completion",
            post(completion_ack),
        )
        .with_state(service)
}

async fn stream(
    State(service): State<Arc<SessionHostService>>,
    Path(session_id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let subscription = service
        .hub()
        .subscribe_with_cursor(EventCursor::default())
        .await;
    let stream = build_stream(subscription, session_id);
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn delivery_ack(
    State(service): State<Arc<SessionHostService>>,
    Path(session_id): Path<String>,
    Json(body): Json<DeliveryAck>,
) -> StatusCode {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if service.apply_delivery_ack(&session_id, &body.participant_id, &body.finding_id, now_ms) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn completion_ack(
    State(service): State<Arc<SessionHostService>>,
    Path(session_id): Path<String>,
    Json(body): Json<CompletionAck>,
) -> StatusCode {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if service.apply_completion_ack(
        &session_id,
        &body.participant_id,
        &body.finding_id,
        body.produced_output,
        now_ms,
    ) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

fn build_stream(
    subscription: EventSubscription,
    session_id: String,
) -> impl tokio_stream::Stream<Item = Result<Event, Infallible>> {
    runway_app_host::sse::event_stream(
        subscription,
        move |env| SessionHostService::stream_includes(env, &session_id),
        |_| false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use helm_session_contracts::{
        FindingId, ParticipantId, SessionContext, SessionPush, UrgencyIntent,
    };
    use runway_app_host::EventHub;
    use tower::ServiceExt;

    fn make_service() -> Arc<SessionHostService> {
        let hub = EventHub::with_capacity(8);
        Arc::new(SessionHostService::from_hub(hub.handle(), "test.sse"))
    }

    fn preemptive_push(session_id: &str, finding_id: FindingId) -> SessionPush {
        SessionPush {
            finding_id,
            urgency_intent: UrgencyIntent::Preemptive,
            payload: serde_json::json!({}),
            session_context: SessionContext {
                session_id: session_id.to_string(),
                phase: "test".into(),
                cycle: 1,
                timestamp_ms: 1,
            },
        }
    }

    #[tokio::test]
    async fn delivery_ack_returns_204_when_record_exists() {
        let service = make_service();
        let fid = FindingId::from_string("f-1");
        let pid = ParticipantId::from_string("p-1");
        let push = preemptive_push("sess-a", fid.clone());

        // Record delivery via publish_push_to
        let _ = service.publish_push_to(push, &[pid.clone()]);

        let app = router(service);
        let body = serde_json::json!({
            "participant_id": pid.as_str(),
            "finding_id": fid.as_str()
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/sessions/sess-a/ack/delivery")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn delivery_ack_returns_404_for_unknown_finding() {
        let service = make_service();
        let app = router(service);
        let body = serde_json::json!({
            "participant_id": "p-x",
            "finding_id": "f-unknown"
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/sessions/sess-x/ack/delivery")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delivery_ack_is_idempotent() {
        let service = make_service();
        let fid = FindingId::from_string("f-idem");
        let pid = ParticipantId::from_string("p-1");
        let push = preemptive_push("sess-b", fid.clone());
        let _ = service.publish_push_to(push, &[pid.clone()]);

        let app = router(service);
        let body = serde_json::to_vec(&serde_json::json!({
            "participant_id": pid.as_str(),
            "finding_id": fid.as_str()
        }))
        .unwrap();

        // First ack
        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/sessions/sess-b/ack/delivery")
            .header("content-type", "application/json")
            .body(Body::from(body.clone()))
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        // Second ack — must also be 204
        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/sessions/sess-b/ack/delivery")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn completion_ack_returns_204_when_record_exists() {
        let service = make_service();
        let fid = FindingId::from_string("f-comp");
        let pid = ParticipantId::from_string("p-1");
        let push = preemptive_push("sess-c", fid.clone());
        let _ = service.publish_push_to(push, &[pid.clone()]);

        let app = router(service);
        let body = serde_json::json!({
            "participant_id": pid.as_str(),
            "finding_id": fid.as_str(),
            "produced_output": false
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/sessions/sess-c/ack/completion")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn completion_ack_returns_404_for_unknown_finding() {
        let service = make_service();
        let app = router(service);
        let body = serde_json::json!({
            "participant_id": "p-x",
            "finding_id": "f-unknown",
            "produced_output": true
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/sessions/sess-x/ack/completion")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
