// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! Session-host identifiers and shared state handles.

use std::fmt;

use runway_app_host::EventHubHandle;
use serde::{Deserialize, Serialize};

/// Opaque id for a governed decision session (distinct from coordination operator sessions).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DecisionSessionId(String);

impl DecisionSessionId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DecisionSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Hub + app identity for a mounted session host.
#[derive(Clone)]
pub struct SessionHostState {
    pub hub: EventHubHandle,
    pub app_id: String,
}

impl SessionHostState {
    #[must_use]
    pub fn new(hub: EventHubHandle, app_id: impl Into<String>) -> Self {
        Self {
            hub,
            app_id: app_id.into(),
        }
    }
}
