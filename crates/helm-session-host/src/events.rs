// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! Session-host event types on the shared Runway event hub.

use chrono::Utc;
use helm_session_contracts::{GatedDecision, SessionPush};
use runway_app_host::{EventEnvelope, EventHubHandle};
use serde_json::json;
use uuid::Uuid;

/// Client-visible push delivered over the session SSE stream.
pub const SESSION_PUSH: &str = "session.push";

/// Gate surface events (slice 4 — constants reserved now for stream filtering).
pub const SESSION_GATE_OPENED: &str = "session.gate.opened";
pub const SESSION_GATE_RESOLVED: &str = "session.gate.resolved";

const SESSION_HOST_TYPES: [&str; 3] = [SESSION_PUSH, SESSION_GATE_OPENED, SESSION_GATE_RESOLVED];

/// Whether an envelope type belongs on a decision-session stream.
#[must_use]
pub fn is_session_host_type(event_type: &str) -> bool {
    SESSION_HOST_TYPES.contains(&event_type)
}

/// Publish a [`SessionPush`] on the shared hub (sequence stamped upstream).
#[must_use]
pub fn publish_push(hub: &EventHubHandle, app_id: &str, push: &SessionPush) -> u64 {
    let Ok(payload) = serde_json::to_value(push) else {
        tracing::warn!("SessionPush failed to serialize; dropping push");
        return 0;
    };
    hub.publish(EventEnvelope {
        event_id: Uuid::new_v4(),
        sequence: 0,
        r#type: SESSION_PUSH.to_string(),
        schema_version: 1,
        occurred_at: Utc::now(),
        app_id: app_id.to_string(),
        run_id: None,
        job_id: None,
        correlation_id: Some(push.finding_id.as_str().to_string()),
        actor: None,
        payload,
    })
}

/// Publish a [`GatedDecision`] on the shared hub for one decision session.
#[must_use]
pub fn publish_gate(
    hub: &EventHubHandle,
    app_id: &str,
    session_id: &str,
    gate: &GatedDecision,
) -> u64 {
    let Ok(gate_value) = serde_json::to_value(gate) else {
        tracing::warn!("GatedDecision failed to serialize; dropping gate");
        return 0;
    };
    hub.publish(EventEnvelope {
        event_id: Uuid::new_v4(),
        sequence: 0,
        r#type: SESSION_GATE_OPENED.to_string(),
        schema_version: 1,
        occurred_at: Utc::now(),
        app_id: app_id.to_string(),
        run_id: None,
        job_id: None,
        correlation_id: Some(gate.gate_id.as_str().to_string()),
        actor: None,
        payload: json!({
            "session_context": { "session_id": session_id },
            "gate": gate_value,
        }),
    })
}
