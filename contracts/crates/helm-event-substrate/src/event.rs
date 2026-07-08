// ── Event ledger traits ──────────────────────────────────────────────────────
// Moved verbatim from runway-storage/src/traits/event.rs (RFL-171).
// Error type retargeted from runway_storage::traits::Error → crate::SubstrateError.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::Result;

// ── StoredEvent / EventQuery / EventLog / SyncableEventLog ──────────────────

/// An ExperienceEvent as stored in the log. Append-only — never updated or deleted.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredEvent {
    pub event_id: String,
    pub org_id: String,
    pub app_id: String,
    pub event_type: String,
    pub context_id: Option<String>,
    pub fact_id: Option<String>,
    pub payload: Value,
    pub occurred_at: DateTime<Utc>,
    /// Set by [`SyncableEventLog::mark_synced`] to the timestamp at which the
    /// event was confirmed synced to the remote backend. `None` while unsynced.
    /// Only the local redb backend writes this field; remote backends leave it
    /// `None` because events are written directly to the remote store.
    #[serde(default)]
    pub synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
pub struct EventQuery {
    pub org_id: Option<String>,
    pub app_id: Option<String>,
    pub event_type: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

/// Append-only event ledger. The ExperienceStore from the Converge architecture.
///
/// Local impl:  redb (survives restarts, feeds sync engine)
/// Remote impl: Firestore events subcollection + BigQuery streaming insert
///
/// Sync-engine-specific operations (`mark_synced`, querying for unsynced
/// events) live on [`SyncableEventLog`], which only the local impl implements.
#[async_trait]
pub trait EventLog: Send + Sync {
    async fn append(&self, event: StoredEvent) -> Result<()>;
    async fn query(&self, q: EventQuery) -> Result<Vec<StoredEvent>>;
}

/// Local-only extension of `EventLog` for the sync engine. Remote backends do
/// not implement this; the type system enforces that mark_synced/query_unsynced
/// cannot be called on a remote log.
#[async_trait]
pub trait SyncableEventLog: EventLog {
    /// Return events matching `q` that have NOT yet been marked synced.
    async fn query_unsynced(&self, q: EventQuery) -> Result<Vec<StoredEvent>>;

    /// Mark events as synced.
    async fn mark_synced(&self, event_ids: &[String]) -> Result<()>;
}

// ── Pure event-stream types ──────────────────────────────────────────────────
// Moved verbatim from runway-app-host/src/realtime.rs:11-49 (RFL-171).
// These are backend-neutral value types; EventHub (T2) remains in realtime.rs
// until it moves in the next task.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventEnvelope {
    pub event_id: Uuid,
    pub sequence: u64,
    #[serde(rename = "type")]
    pub r#type: String,
    pub schema_version: u32,
    pub occurred_at: DateTime<Utc>,
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    pub payload: serde_json::Value,
}

/// Cursor identifying a point in the event stream. Used for SSE catch-up.
#[derive(Debug, Clone, Default)]
pub struct EventCursor {
    /// Last sequence number the caller has already consumed. `subscribe_with_cursor`
    /// returns events with `sequence > last_sequence` as replay, then live events.
    pub last_sequence: Option<u64>,
    /// Optional filter: only events matching this `run_id`.
    pub run_id: Option<String>,
    /// Optional filter: only events matching this `job_id`.
    pub job_id: Option<String>,
}

/// Returned by [`EventHubHandle::subscribe_with_cursor`].
pub struct EventSubscription {
    /// Buffered events to replay before the live stream starts.
    pub replay: Vec<EventEnvelope>,
    /// Live stream from this point forward.
    pub receiver: broadcast::Receiver<EventEnvelope>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_roundtrips_through_json() {
        let env = EventEnvelope {
            event_id: Uuid::nil(),
            sequence: 7,
            r#type: "job.started".into(),
            schema_version: 1,
            occurred_at: DateTime::parse_from_rfc3339("2026-05-28T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            app_id: "catalyst".into(),
            run_id: Some("run-1".into()),
            job_id: None,
            correlation_id: None,
            actor: Some("user:alice".into()),
            payload: serde_json::json!({"key": "value"}),
        };
        let s = serde_json::to_string(&env).unwrap();
        let back: EventEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(env.event_id, back.event_id);
        assert_eq!(env.sequence, back.sequence);
        assert_eq!(env.r#type, back.r#type);
        assert!(
            !s.contains("correlation_id"),
            "None fields should be omitted"
        );
        assert!(
            !s.contains("job_id"),
            "None job_id should be omitted from JSON"
        );
    }

    #[test]
    fn envelope_job_id_roundtrips_through_json() {
        let env = EventEnvelope {
            event_id: Uuid::nil(),
            sequence: 8,
            r#type: "job.completed".into(),
            schema_version: 1,
            occurred_at: DateTime::parse_from_rfc3339("2026-05-28T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            app_id: "catalyst".into(),
            run_id: Some("run-1".into()),
            job_id: Some("my-job".into()),
            correlation_id: None,
            actor: None,
            payload: serde_json::Value::Null,
        };
        let s = serde_json::to_string(&env).unwrap();
        let back: EventEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(back.job_id.as_deref(), Some("my-job"));
        assert!(s.contains("\"job_id\":\"my-job\""));
    }

    #[test]
    fn envelope_without_job_id_field_deserializes_to_none() {
        // Simulate a legacy producer that doesn't emit job_id.
        let json = r#"{"event_id":"00000000-0000-0000-0000-000000000000","sequence":1,"type":"tick","schema_version":1,"occurred_at":"2026-05-28T12:00:00Z","app_id":"old-producer","payload":null}"#;
        let env: EventEnvelope = serde_json::from_str(json).unwrap();
        assert_eq!(env.job_id, None);
    }
}
