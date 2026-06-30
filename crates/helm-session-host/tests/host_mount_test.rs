// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! Runway app-host mount test — session-host live on one host.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use helm_session_host::{SessionHostModuleState, mount_session_host};
use runway_app_host::{
    AppExecutionPacket, MountKind, MountedModule, RouteOwner, RouteRegistration, RunwayAppHost,
};
use runway_storage::StorageKit;
use tower::ServiceExt;

fn host_packet() -> AppExecutionPacket {
    AppExecutionPacket::new(
        "test.session-host",
        "Session Host Mount Test",
        "Pins live session-host SSE on RunwayAppHost",
        "",
    )
    .with_mounted_module(MountedModule {
        module_id: "helm.session-host".into(),
        mount_kind: MountKind::Mounted,
        routes: vec![RouteRegistration {
            method: "GET".into(),
            path: "/v1/sessions/{session_id}/stream".into(),
            owner: RouteOwner::HelmModule,
        }],
    })
}

#[tokio::test]
async fn runway_host_mounts_live_session_host_stream() {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage = StorageKit::local(dir.path()).await.expect("local storage");

    let hub = runway_app_host::EventHub::with_capacity(256);
    let module = mount_session_host(hub.handle(), "test.session-host");

    assert_eq!(module.module_state(), SessionHostModuleState::Live);

    let router = RunwayAppHost::builder(host_packet())
        .with_storage(storage)
        .mount(module)
        .build()
        .await
        .expect("host builds")
        .into_router();

    let response = router
        .oneshot(
            Request::builder()
                .uri("/v1/sessions/sess-mount-1/stream")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let ct = response.headers().get("content-type").unwrap();
    assert!(ct.to_str().unwrap().starts_with("text/event-stream"));
}
