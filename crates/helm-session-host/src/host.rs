// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! App-host wiring for the session-host module.

use std::sync::Arc;

use runway_app_host::EventHubHandle;
use runway_storage::LeaseStore;

use crate::{SessionHostModule, SessionHostService};

/// Build a [`SessionHostModule`] over the shared hub and lease store.
///
/// The module applies [`runway_app_host::SessionOwnershipLayer`] to all mutating
/// session routes. GET / HEAD / OPTIONS pass through unconditionally.
/// Mutating routes require `AuthContext` in request extensions (provided by the
/// upstream `AuthLayer` on `RunwayAppHost`) — without it the layer returns 400.
#[must_use]
pub fn mount_session_host(
    hub: EventHubHandle,
    app_id: impl Into<String>,
    leases: Arc<dyn LeaseStore>,
) -> Arc<SessionHostModule> {
    Arc::new(SessionHostModule::new(
        Arc::new(SessionHostService::from_hub(hub, app_id)),
        leases,
    ))
}
