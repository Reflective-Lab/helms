use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{RwLock, broadcast};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeActorType {
    Human,
    Agent,
    System,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeActor {
    #[serde(rename = "type")]
    pub actor_type: RealtimeActorType,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeEvent {
    pub event_id: String,
    pub sequence: u64,
    #[serde(rename = "type")]
    pub event_type: String,
    pub schema_version: u16,
    pub occurred_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<RealtimeActor>,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct RealtimeEventInput {
    pub event_type: String,
    pub app_id: Option<String>,
    pub run_id: Option<String>,
    pub job_id: Option<String>,
    pub correlation_id: Option<String>,
    pub actor: Option<RealtimeActor>,
    pub payload: Value,
}

#[derive(Debug, Clone, Default)]
pub struct RealtimeCursor {
    pub since_sequence: Option<u64>,
    pub last_event_id: Option<String>,
}

pub struct RealtimeSubscription {
    pub replay: Vec<RealtimeEvent>,
    pub live: broadcast::Receiver<RealtimeEvent>,
}

pub trait RealtimeEventLog: Send + Sync {
    fn append(&self, event: &RealtimeEvent) -> Result<(), String>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryRealtimeEventLog {
    events: Arc<Mutex<Vec<RealtimeEvent>>>,
}

impl InMemoryRealtimeEventLog {
    pub fn snapshot(&self) -> Vec<RealtimeEvent> {
        self.events
            .lock()
            .expect("realtime event log lock poisoned")
            .clone()
    }
}

impl RealtimeEventLog for InMemoryRealtimeEventLog {
    fn append(&self, event: &RealtimeEvent) -> Result<(), String> {
        self.events
            .lock()
            .map_err(|_| "realtime event log lock poisoned".to_string())?
            .push(event.clone());
        Ok(())
    }
}

#[derive(Clone)]
pub struct RealtimeHub {
    inner: Arc<RealtimeHubInner>,
}

struct RealtimeHubInner {
    tx: broadcast::Sender<RealtimeEvent>,
    replay: RwLock<VecDeque<RealtimeEvent>>,
    event_log: Arc<dyn RealtimeEventLog>,
    next_sequence: AtomicU64,
    replay_capacity: usize,
}

impl RealtimeHub {
    pub fn new(replay_capacity: usize) -> Self {
        Self::with_event_log(
            replay_capacity,
            Arc::new(InMemoryRealtimeEventLog::default()),
        )
    }

    pub fn with_event_log(replay_capacity: usize, event_log: Arc<dyn RealtimeEventLog>) -> Self {
        let (tx, _) = broadcast::channel(replay_capacity.max(1));
        Self {
            inner: Arc::new(RealtimeHubInner {
                tx,
                replay: RwLock::new(VecDeque::with_capacity(replay_capacity)),
                event_log,
                next_sequence: AtomicU64::new(1),
                replay_capacity,
            }),
        }
    }

    pub async fn publish(&self, input: RealtimeEventInput) -> RealtimeEvent {
        let sequence = self.inner.next_sequence.fetch_add(1, Ordering::SeqCst);
        let event = RealtimeEvent {
            event_id: sequence.to_string(),
            sequence,
            event_type: input.event_type,
            schema_version: 1,
            occurred_at: Utc::now(),
            app_id: input.app_id,
            run_id: input.run_id,
            job_id: input.job_id,
            correlation_id: input.correlation_id,
            actor: input.actor,
            payload: input.payload,
        };

        if let Err(error) = self.inner.event_log.append(&event) {
            tracing::warn!(error = %error, event_id = %event.event_id, "failed to append realtime event");
        }

        {
            let mut replay = self.inner.replay.write().await;
            if self.inner.replay_capacity > 0 {
                while replay.len() >= self.inner.replay_capacity {
                    replay.pop_front();
                }
                replay.push_back(event.clone());
            }
        }

        let _ = self.inner.tx.send(event.clone());
        event
    }

    pub async fn subscribe(&self, cursor: RealtimeCursor) -> RealtimeSubscription {
        let live = self.inner.tx.subscribe();
        let replay = self.replay_after(&cursor).await;
        RealtimeSubscription { replay, live }
    }

    async fn replay_after(&self, cursor: &RealtimeCursor) -> Vec<RealtimeEvent> {
        let replay = self.inner.replay.read().await;
        let after_sequence = cursor
            .since_sequence
            .or_else(|| {
                cursor
                    .last_event_id
                    .as_deref()
                    .and_then(parse_event_sequence)
            })
            .or_else(|| {
                cursor.last_event_id.as_ref().and_then(|last_event_id| {
                    replay
                        .iter()
                        .find(|event| event.event_id == *last_event_id)
                        .map(|event| event.sequence)
                })
            });

        let Some(after_sequence) = after_sequence else {
            return Vec::new();
        };

        replay
            .iter()
            .filter(|event| event.sequence > after_sequence)
            .cloned()
            .collect()
    }
}

fn parse_event_sequence(value: &str) -> Option<u64> {
    value.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replays_events_after_sequence_cursor() {
        let hub = RealtimeHub::new(8);
        hub.publish(RealtimeEventInput {
            event_type: "test.first".into(),
            app_id: Some("test".into()),
            run_id: None,
            job_id: None,
            correlation_id: None,
            actor: None,
            payload: serde_json::json!({ "n": 1 }),
        })
        .await;
        hub.publish(RealtimeEventInput {
            event_type: "test.second".into(),
            app_id: Some("test".into()),
            run_id: None,
            job_id: None,
            correlation_id: None,
            actor: None,
            payload: serde_json::json!({ "n": 2 }),
        })
        .await;

        let subscription = hub
            .subscribe(RealtimeCursor {
                since_sequence: Some(1),
                last_event_id: None,
            })
            .await;

        assert_eq!(subscription.replay.len(), 1);
        assert_eq!(subscription.replay[0].event_type, "test.second");
    }

    #[tokio::test]
    async fn new_subscribers_only_receive_live_events_without_cursor() {
        let hub = RealtimeHub::new(8);
        hub.publish(RealtimeEventInput {
            event_type: "test.existing".into(),
            app_id: Some("test".into()),
            run_id: None,
            job_id: None,
            correlation_id: None,
            actor: None,
            payload: serde_json::json!({ "n": 1 }),
        })
        .await;

        let subscription = hub.subscribe(RealtimeCursor::default()).await;

        assert!(subscription.replay.is_empty());
    }

    #[tokio::test]
    async fn appends_published_events_to_event_log() {
        let event_log = InMemoryRealtimeEventLog::default();
        let hub = RealtimeHub::with_event_log(8, std::sync::Arc::new(event_log.clone()));

        hub.publish(RealtimeEventInput {
            event_type: "test.logged".into(),
            app_id: Some("test".into()),
            run_id: Some("run-1".into()),
            job_id: Some("job-1".into()),
            correlation_id: None,
            actor: None,
            payload: serde_json::json!({ "n": 1 }),
        })
        .await;

        let events = event_log.snapshot();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "test.logged");
        assert_eq!(events[0].sequence, 1);
    }
}
