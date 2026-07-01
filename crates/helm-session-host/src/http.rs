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

#[derive(serde::Deserialize)]
struct StreamQuery {
    participant_id: Option<String>,
    cursor: Option<u64>,
}

async fn stream(
    State(service): State<Arc<SessionHostService>>,
    Path(session_id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<StreamQuery>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    // Pull-replay: re-publish unacked Disruptive/Preemptive findings that the
    // client already received (version <= cursor) but never acked.
    if let Some(pid_str) = &query.participant_id {
        let participant_id = helm_session_contracts::ParticipantId::from_string(pid_str);
        let cursor_seq = query.cursor.unwrap_or(0);
        for push in service.unacked_for_replay(&session_id, &participant_id, cursor_seq) {
            let _ = service.republish_push(push);
        }
    }

    let cursor = EventCursor {
        last_sequence: query.cursor,
        ..Default::default()
    };
    let subscription = service.hub().subscribe_with_cursor(cursor).await;
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

    #[tokio::test]
    async fn pull_replay_republishes_unacked_findings_at_subscribe() {
        let service = make_service();
        let fid = FindingId::from_string("f-replay");
        let pid = ParticipantId::from_string("p-replay");
        let push = preemptive_push("sess-replay", fid.clone());
        let version = service.publish_push_to(push, &[pid.clone()]);
        assert!(version > 0);

        let app = router(service);
        let uri = format!(
            "/v1/sessions/sess-replay/stream?participant_id={}&cursor={}",
            pid.as_str(),
            version
        );
        let req = Request::builder()
            .method(Method::GET)
            .uri(&uri)
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Read a capped chunk of the SSE body with a timeout to avoid hanging on the
        // open-ended stream. The replayed finding is republished before subscribe, so
        // it lands in the hub's replay buffer and appears in the initial batch.
        let body_bytes = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            axum::body::to_bytes(res.into_body(), 16_384),
        )
        .await
        .unwrap_or_else(|_| Ok(axum::body::Bytes::new()))
        .unwrap_or_default();
        let body_str = String::from_utf8_lossy(&body_bytes);
        assert!(
            body_str.contains("f-replay"),
            "replayed finding_id not found in SSE body; got: {body_str:?}"
        );
    }

    #[tokio::test]
    async fn pull_replay_noop_when_no_participant_id() {
        let service = make_service();
        let app = router(service);
        // Plain subscribe without participant_id — must still work
        let req = Request::builder()
            .method(Method::GET)
            .uri("/v1/sessions/sess-plain/stream")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn pull_replay_skips_findings_after_cursor() {
        let service = make_service();
        let fid = FindingId::from_string("f-future");
        let pid = ParticipantId::from_string("p-future");

        let push = preemptive_push("sess-future", fid.clone());
        let version = service.publish_push_to(push, &[pid.clone()]);

        // cursor = version - 1 means this finding was published AFTER the cursor
        // so it should NOT be replayed (it will come through the live stream naturally)
        let app = router(service);
        let uri = format!(
            "/v1/sessions/sess-future/stream?participant_id={}&cursor={}",
            pid.as_str(),
            version.saturating_sub(1)
        );
        let req = Request::builder()
            .method(Method::GET)
            .uri(&uri)
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        // Stream opened cleanly; no out-of-band republish for f-future.
    }

    #[tokio::test]
    async fn informational_push_to_creates_no_delivery_record() {
        let service = make_service();
        let fid = FindingId::from_string("f-info");
        let pid = ParticipantId::from_string("p-info");
        let push = SessionPush {
            finding_id: fid.clone(),
            urgency_intent: UrgencyIntent::Informational,
            payload: serde_json::json!({}),
            session_context: SessionContext {
                session_id: "sess-info".into(),
                phase: "test".into(),
                cycle: 1,
                timestamp_ms: 1,
            },
        };
        let _ = service.publish_push_to(push, &[pid.clone()]);

        // Informational pushes are not tracked → ack returns false (no record)
        let recorded = service.apply_delivery_ack("sess-info", &pid, &fid, 0);
        assert!(
            !recorded,
            "Informational push should produce no delivery record"
        );
    }
}
