// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! Contract tests — session-host module reports Live state after setup.
//!
//! The full RunwayAppHost mount + SSE route integration test was separated here
//! to break a circular CI dependency: helms CI checks out runtime-runway main,
//! but runtime-runway's `RunwayAppHost::mount()` expects its own locally-defined
//! `HelmModule` until PR #15 lands. Moving the RunwayAppHost test into
//! runtime-runway avoids the type mismatch. (RFL-128)

use helm_session_host::{SessionHostModuleState, mount_session_host};
use runway_app_host::EventHub;
use runway_storage::StorageKit;

#[tokio::test]
async fn session_host_module_reports_live_after_setup() {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage = StorageKit::local(dir.path()).await.expect("local storage");
    let hub = EventHub::with_capacity(256);
    let module = mount_session_host(hub.handle(), "test.session-host", storage.leases.clone());
    assert_eq!(module.module_state(), SessionHostModuleState::Live);
}
