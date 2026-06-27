// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use crate::finding::FindingId;
use crate::urgency::UrgencyIntent;
use serde::{Deserialize, Serialize};

/// Session-level context appended by Helms before routing a push.
/// Helms owns this; the domain coordinator does not set it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: String,
    pub phase: String,
    pub cycle: u32,
    pub timestamp_ms: u64,
}

/// What Client Helm receives via SSE from the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPush {
    pub finding_id: FindingId,
    /// Passed through unchanged from the CoordinatorFinding.
    pub urgency_intent: UrgencyIntent,
    /// Domain payload — opaque. The client app renders it; Client Helm routes it.
    pub payload: serde_json::Value,
    pub session_context: SessionContext,
}
