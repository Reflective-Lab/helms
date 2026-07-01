// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! Integration tests for SessionOwnershipLayer wiring in helm-session-host.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::middleware;
use axum::response::Response;
use axum::routing::post;
use axum::Router;
use runway_app_host::{EventHub, HelmModule, SessionOwnershipLayer};
use runway_auth::{AuthContext, FirebaseClaims};
use runway_storage::{LeaseStore, StorageKit};
use tower::ServiceExt;

use helm_session_host::mount_session_host;

async fn inject_test_auth(mut req: Request<Body>, next: middleware::Next) -> Response {
    req.extensions_mut().insert(AuthContext {
        claims: FirebaseClaims {
            uid: "u1".into(),
            email: None,
            org_id: Some("test-org".into()),
            apps: vec![],
            role: None,
        },
    });
    next.run(req).await
}

fn ack_delivery_request(session_id: &str) -> Request<Body> {
    let body = serde_json::json!({"participant_id": "p-1", "finding_id": "f-1"});
    Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/sessions/{session_id}/ack/delivery"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

/// The module wires SessionOwnershipLayer. Without AuthContext, POST returns 400
/// (not the service-level 204/404), proving the layer is present.
#[tokio::test]
async fn ownership_layer_is_wired_returns_400_without_auth() {
    let dir = tempfile::tempdir().unwrap();
    let storage = StorageKit::local(dir.path()).await.unwrap();
    let hub = EventHub::with_capacity(8);
    let module = mount_session_host(hub.handle(), "test.session-host", storage.leases.clone());

    let router = Arc::clone(&module).router();
    // No auth middleware — should get 400 "ownership_requires_auth"
    let res = router
        .oneshot(ack_delivery_request("sess-noauth"))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "ownership layer must reject mutating requests without AuthContext"
    );
}

/// GET /stream is exempt from the ownership layer (pass-through).
#[tokio::test]
async fn get_stream_is_exempt_from_ownership_layer() {
    let dir = tempfile::tempdir().unwrap();
    let storage = StorageKit::local(dir.path()).await.unwrap();
    let hub = EventHub::with_capacity(8);
    let module = mount_session_host(hub.handle(), "test.session-host", storage.leases.clone());

    let router = Arc::clone(&module).router();
    // No auth middleware — GET is exempt, so it reaches the handler
    let res = router
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/v1/sessions/sess-stream/stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let ct = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.starts_with("text/event-stream"));
}

/// Two holders targeting the same session simultaneously: exactly one gets 409.
/// Uses explicit holder_ids because process_holder_id() is process-wide (both
/// instances would otherwise share the same ID and renew instead of conflict).
#[tokio::test]
async fn two_holders_on_same_session_one_gets_409() {
    let dir = tempfile::tempdir().unwrap();
    let storage = StorageKit::local(dir.path()).await.unwrap();
    let leases = storage.leases.clone();

    // Minimal router that stands in for the real ack route.
    async fn ok_handler() -> StatusCode {
        StatusCode::OK
    }
    let base = Router::new().route("/v1/sessions/{session_id}/ack/delivery", post(ok_handler));

    let make_router = |holder: &str| {
        let leases: Arc<dyn LeaseStore> = leases.clone();
        let holder = holder.to_string();
        base.clone()
            .layer(
                SessionOwnershipLayer::for_app("test.session-host", leases)
                    .path_param("session_id")
                    .holder_id(holder),
            )
            .layer(middleware::from_fn(inject_test_auth))
    };

    let router_a = make_router("holder-a");
    let router_b = make_router("holder-b");

    let (res_a, res_b) = tokio::join!(
        router_a.oneshot(ack_delivery_request("sess-conflict")),
        router_b.oneshot(ack_delivery_request("sess-conflict")),
    );

    let sa = res_a.unwrap().status();
    let sb = res_b.unwrap().status();
    let conflicts = [sa, sb]
        .iter()
        .filter(|&&s| s == StatusCode::CONFLICT)
        .count();
    assert_eq!(
        conflicts, 1,
        "exactly one holder must get 409 Conflict; got ({sa}, {sb})"
    );
}
