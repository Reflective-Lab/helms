//! Coordination event publishing onto the shared Runtime Runway event hub.
//!
//! Coordination events ride the same [`EventHubHandle`] as governed-job events,
//! so a single SSE stream can show operators each other's presence and decisions
//! alongside live job progress. Every event carries the deciding/acting
//! principal (stamped into `EventEnvelope.actor`) and a `workspace_id` in the
//! payload so the stream can scope by workspace.

use chrono::Utc;
use runway_app_host::{EventEnvelope, EventHubHandle};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::principal::OperatorPrincipal;

// Coordination event types (kept flat to match the platform event vocabulary).
pub const SESSION_OPENED: &str = "session.opened";
pub const SESSION_CLOSED: &str = "session.closed";
pub const PRESENCE_JOINED: &str = "presence.joined";
pub const PRESENCE_LEFT: &str = "presence.left";
pub const PRESENCE_FOCUS_CHANGED: &str = "presence.focus_changed";
pub const CLAIM_ACQUIRED: &str = "claim.acquired";
pub const CLAIM_RELEASED: &str = "claim.released";
pub const DECISION_RECORDED: &str = "decision.recorded";
pub const DECISION_CONFLICT: &str = "decision.conflict";
pub const DECISION_DENIED: &str = "decision.denied";

const COORDINATION_TYPES: [&str; 10] = [
    SESSION_OPENED,
    SESSION_CLOSED,
    PRESENCE_JOINED,
    PRESENCE_LEFT,
    PRESENCE_FOCUS_CHANGED,
    CLAIM_ACQUIRED,
    CLAIM_RELEASED,
    DECISION_RECORDED,
    DECISION_CONFLICT,
    DECISION_DENIED,
];

/// Whether an event type is a coordination event.
#[must_use]
pub fn is_coordination_type(event_type: &str) -> bool {
    COORDINATION_TYPES.contains(&event_type)
}

/// Whether an event type is an attributed governed-job/gate event worth relaying
/// to the coordination stream.
#[must_use]
pub fn is_job_type(event_type: &str) -> bool {
    event_type.starts_with("job.") || event_type.starts_with("gate.")
}

/// Publishes coordination events on the shared hub (sequence stamped upstream).
#[derive(Clone)]
pub struct CoordinationPublisher {
    hub: EventHubHandle,
    app_id: String,
}

impl CoordinationPublisher {
    pub fn new(hub: EventHubHandle, app_id: impl Into<String>) -> Self {
        Self {
            hub,
            app_id: app_id.into(),
        }
    }

    /// Emit a coordination event attributed to `principal`, merging
    /// `workspace_id` and the principal summary into the payload.
    pub fn emit(&self, event_type: &str, principal: &OperatorPrincipal, mut payload: Value) {
        if let Value::Object(map) = &mut payload {
            map.entry("workspace_id")
                .or_insert_with(|| json!(principal.workspace_id));
            map.entry("principal").or_insert_with(|| {
                json!({
                    "actor_id": principal.actor_id,
                    "display_name": principal.display_name,
                    "kind": principal.kind,
                })
            });
        }
        self.hub.publish(EventEnvelope {
            event_id: Uuid::new_v4(),
            sequence: 0,
            r#type: event_type.to_string(),
            schema_version: 1,
            occurred_at: Utc::now(),
            app_id: self.app_id.clone(),
            run_id: None,
            job_id: None,
            correlation_id: None,
            actor: Some(principal.actor_tag()),
            payload,
        });
    }
}
