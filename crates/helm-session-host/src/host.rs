// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! App-host wiring for the session-host module.

use std::sync::Arc;

use runway_app_host::EventHubHandle;

use crate::{SessionHostModule, SessionHostService};

/// Build a [`SessionHostModule`] over the shared hub.
#[must_use]
pub fn mount_session_host(
    hub: EventHubHandle,
    app_id: impl Into<String>,
) -> Arc<SessionHostModule> {
    Arc::new(SessionHostModule::new(Arc::new(
        SessionHostService::from_hub(hub, app_id),
    )))
}
