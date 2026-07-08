//! Event hub: in-process broadcast channel with optional durable EventLog backing.
//!
//! [`EventHub`] owns the broadcast channel and replay buffer. Callers hold
//! cheap clones of [`EventHubHandle`] to publish and subscribe.
//!
//! Moved verbatim from `runway-app-host/src/realtime.rs` (RFL-171 T2).
//! Imports retargeted: envelope/cursor/subscription/EventLog/StoredEvent now
//! come from `crate::event`; errors from `crate::SubstrateError`.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use crate::event::{
    EventCursor, EventEnvelope, EventLog, EventQuery, EventSubscription, StoredEvent,
};

const HUB_CAPACITY: usize = 512;

#[derive(Clone)]
struct EventLogBacking {
    log: Arc<dyn EventLog>,
    org_id: String,
    app_id: String,
}

/// In-process event hub: broadcast channel with a bounded replay buffer and an
/// optional durable [`EventLog`] backing.
///
/// Callers do not hold `EventHub` directly after construction — they call
/// [`EventHub::handle`] to get a cheap [`EventHubHandle`] that can be cloned
/// freely across threads.  `EventHub` itself must be kept alive as long as the
/// hub is in use (it owns the broadcast sender).
pub struct EventHub {
    sender: broadcast::Sender<EventEnvelope>,
    replay: Arc<Mutex<VecDeque<EventEnvelope>>>,
    capacity: usize,
    next_sequence: Option<Arc<AtomicU64>>,
    backing: Option<EventLogBacking>,
}

/// Cheap clone of [`EventHub`] for publishing and subscribing.
///
/// Obtain one via [`EventHub::handle`].  All clones share the same broadcast
/// channel, replay buffer, and sequence counter.
#[derive(Clone)]
pub struct EventHubHandle {
    sender: broadcast::Sender<EventEnvelope>,
    replay: Arc<Mutex<VecDeque<EventEnvelope>>>,
    capacity: usize,
    next_sequence: Option<Arc<AtomicU64>>,
    backing: Option<EventLogBacking>,
}

impl EventHub {
    /// Create a hub with the default in-memory capacity (`512`).
    pub fn new() -> Self {
        Self::with_capacity(HUB_CAPACITY)
    }

    /// Create a hub with a custom replay buffer capacity.
    ///
    /// `capacity` bounds both the in-process replay buffer and the broadcast
    /// channel.  When the buffer is full the oldest event is evicted before the
    /// new one is inserted.  `capacity` is clamped to `1` to avoid a
    /// zero-sized broadcast channel.
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self {
            sender,
            replay: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
            next_sequence: Some(Arc::new(AtomicU64::new(0))),
            backing: None,
        }
    }

    /// Hub backed by durable [`EventLog`] storage. Replay survives process restart;
    /// sequence numbers continue from the log high-water mark.
    pub async fn with_event_log(
        log: Arc<dyn EventLog>,
        org_id: impl Into<String>,
        app_id: impl Into<String>,
    ) -> Self {
        Self::with_event_log_capacity(HUB_CAPACITY, log, org_id, app_id).await
    }

    /// Hub backed by a durable [`EventLog`] with a custom capacity.
    ///
    /// Like [`EventHub::with_event_log`] but lets the caller control the replay
    /// buffer / broadcast channel size.
    pub async fn with_event_log_capacity(
        capacity: usize,
        log: Arc<dyn EventLog>,
        org_id: impl Into<String>,
        app_id: impl Into<String>,
    ) -> Self {
        let org_id = org_id.into();
        let app_id = app_id.into();
        let backing = EventLogBacking {
            log,
            org_id,
            app_id,
        };
        let high_water = load_max_sequence(&backing).await;
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self {
            sender,
            replay: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
            next_sequence: Some(Arc::new(AtomicU64::new(high_water))),
            backing: Some(backing),
        }
    }

    /// Return a cloneable handle to this hub.
    pub fn handle(&self) -> EventHubHandle {
        EventHubHandle {
            sender: self.sender.clone(),
            replay: Arc::clone(&self.replay),
            capacity: self.capacity,
            next_sequence: self.next_sequence.clone(),
            backing: self.backing.clone(),
        }
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

impl EventHubHandle {
    /// Publish an event to all current subscribers and append it to the replay buffer.
    ///
    /// When the hub is backed by [`EventLog`], assigns the next monotonic sequence
    /// and appends to durable storage. If the buffer is at capacity the oldest
    /// event is evicted first.
    pub fn publish(&self, mut env: EventEnvelope) -> u64 {
        let assigned = if let Some(seq) = &self.next_sequence {
            let n = seq.fetch_add(1, Ordering::SeqCst) + 1;
            env.sequence = n;
            n
        } else {
            env.sequence
        };
        {
            let mut replay = self.replay.lock().expect("replay buffer lock poisoned");
            if replay.len() >= self.capacity {
                replay.pop_front();
            }
            replay.push_back(env.clone());
        }
        if let Some(backing) = &self.backing {
            let stored = envelope_to_stored(backing, &env);
            let log = backing.log.clone();
            tokio::spawn(async move {
                if let Err(e) = log.append(stored).await {
                    tracing::warn!(error = %e, "failed to persist event to EventLog");
                }
            });
        }
        let _ = self.sender.send(env);
        assigned
    }

    /// Subscribe to the live broadcast channel without any replay catch-up.
    ///
    /// Backwards-compatible zero-arg form; callers that need catch-up should
    /// use [`Self::subscribe_with_cursor`] instead.
    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.sender.subscribe()
    }

    /// Subscribe with cursor-based catch-up.
    ///
    /// Returns a snapshot of replay events that satisfy the cursor filters,
    /// then a live broadcast receiver starting from the moment of the call.
    /// Replay events with `sequence <= cursor.last_sequence` are excluded.
    ///
    /// When durable storage is wired, replay merges the in-memory buffer with
    /// events from [`EventLog`], falling back to the buffer alone on query failure.
    pub async fn subscribe_with_cursor(&self, cursor: EventCursor) -> EventSubscription {
        // Subscribe to the live channel first so we don't miss events published
        // between snapshotting replay and the caller draining it.
        let receiver = self.sender.subscribe();
        let replay = self.collect_replay(&cursor).await;
        EventSubscription { replay, receiver }
    }

    /// Return the number of active [`broadcast::Receiver`]s attached to this hub.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    async fn collect_replay(&self, cursor: &EventCursor) -> Vec<EventEnvelope> {
        let mut replay = self.replay_from_buffer(cursor);
        if let Some(backing) = &self.backing {
            match replay_from_log(backing, cursor).await {
                Ok(log_events) => {
                    for ev in log_events {
                        if !replay.iter().any(|e| e.event_id == ev.event_id) {
                            replay.push(ev);
                        }
                    }
                    replay.sort_by_key(|e| e.sequence);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "EventLog replay failed; using in-memory buffer");
                }
            }
        }
        replay
    }

    fn replay_from_buffer(&self, cursor: &EventCursor) -> Vec<EventEnvelope> {
        let buf = self.replay.lock().expect("replay buffer lock poisoned");
        buf.iter()
            .filter(|env| matches_cursor(env, cursor))
            .cloned()
            .collect()
    }
}

fn envelope_to_stored(backing: &EventLogBacking, env: &EventEnvelope) -> StoredEvent {
    StoredEvent {
        event_id: env.event_id.to_string(),
        org_id: backing.org_id.clone(),
        app_id: backing.app_id.clone(),
        event_type: env.r#type.clone(),
        context_id: env.run_id.clone(),
        fact_id: env.job_id.clone(),
        payload: serde_json::to_value(env).expect("EventEnvelope serialises"),
        occurred_at: env.occurred_at,
        synced_at: None,
    }
}

fn stored_to_envelope(stored: &StoredEvent) -> Option<EventEnvelope> {
    serde_json::from_value(stored.payload.clone()).ok()
}

async fn load_max_sequence(backing: &EventLogBacking) -> u64 {
    let q = EventQuery {
        org_id: Some(backing.org_id.clone()),
        app_id: Some(backing.app_id.clone()),
        ..Default::default()
    };
    match backing.log.query(q).await {
        Ok(events) => events
            .iter()
            .filter_map(stored_to_envelope)
            .map(|e| e.sequence)
            .max()
            .unwrap_or(0),
        Err(e) => {
            tracing::warn!(error = %e, "failed to load EventLog sequence high-water mark");
            0
        }
    }
}

async fn replay_from_log(
    backing: &EventLogBacking,
    cursor: &EventCursor,
) -> crate::Result<Vec<EventEnvelope>> {
    let q = EventQuery {
        org_id: Some(backing.org_id.clone()),
        app_id: Some(backing.app_id.clone()),
        ..Default::default()
    };
    let stored = backing.log.query(q).await?;
    let mut replay: Vec<EventEnvelope> = stored
        .iter()
        .filter_map(stored_to_envelope)
        .filter(|env| matches_cursor(env, cursor))
        .collect();
    replay.sort_by_key(|e| e.sequence);
    Ok(replay)
}

fn matches_cursor(env: &EventEnvelope, cursor: &EventCursor) -> bool {
    if let Some(last) = cursor.last_sequence
        && env.sequence <= last
    {
        return false;
    }
    if let Some(ref rid) = cursor.run_id
        && env.run_id.as_deref() != Some(rid.as_str())
    {
        return false;
    }
    if let Some(ref jid) = cursor.job_id
        && env.job_id.as_deref() != Some(jid.as_str())
    {
        return false;
    }
    true
}

#[cfg(test)]
mod hub_tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn sample(seq: u64, ty: &str) -> EventEnvelope {
        sample_env(seq, ty, None, None)
    }

    fn sample_env(seq: u64, ty: &str, run_id: Option<&str>, job_id: Option<&str>) -> EventEnvelope {
        EventEnvelope {
            event_id: Uuid::new_v4(),
            sequence: seq,
            r#type: ty.into(),
            schema_version: 1,
            occurred_at: Utc::now(),
            app_id: "test".into(),
            run_id: run_id.map(String::from),
            job_id: job_id.map(String::from),
            correlation_id: None,
            actor: None,
            payload: serde_json::Value::Null,
        }
    }

    #[tokio::test]
    async fn in_memory_hub_owns_and_stamps_sequence() {
        let hub = EventHub::with_capacity(8);
        let h = hub.handle();
        let mut rx = h.subscribe();
        h.publish(sample(0, "a"));
        h.publish(sample(0, "b"));
        assert_eq!(rx.recv().await.unwrap().sequence, 1);
        assert_eq!(rx.recv().await.unwrap().sequence, 2);
    }

    #[tokio::test]
    async fn handle_delivers_to_subscriber() {
        let hub = EventHub::new();
        let h = hub.handle();
        let mut rx = h.subscribe();

        h.publish(sample(1, "foo"));
        let got = rx.recv().await.unwrap();
        assert_eq!(got.sequence, 1);
    }

    #[tokio::test]
    async fn publish_without_subscribers_is_silent() {
        let hub = EventHub::new();
        let h = hub.handle();
        h.publish(sample(1, "foo"));
        assert_eq!(h.subscriber_count(), 0);
    }

    // ── Replay buffer tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn replay_buffer_catches_up_late_subscriber() {
        let hub = EventHub::with_capacity(8);
        let h = hub.handle();
        // Publish 3 events before subscribing.
        for seq in 1..=3 {
            h.publish(sample_env(seq, "job.started", Some("run-1"), None));
        }
        let sub = h.subscribe_with_cursor(EventCursor::default()).await;
        assert_eq!(sub.replay.len(), 3);
        assert_eq!(sub.replay[0].sequence, 1);
    }

    #[tokio::test]
    async fn replay_buffer_filters_by_run_id() {
        let hub = EventHub::with_capacity(8);
        let h = hub.handle();
        h.publish(sample_env(1, "job.started", Some("run-1"), Some("job-A")));
        h.publish(sample_env(2, "job.started", Some("run-2"), Some("job-B")));
        h.publish(sample_env(3, "job.completed", Some("run-1"), Some("job-A")));
        let cursor = EventCursor {
            last_sequence: None,
            run_id: Some("run-1".into()),
            job_id: None,
        };
        let sub = h.subscribe_with_cursor(cursor).await;
        assert_eq!(sub.replay.len(), 2); // only run-1 events
        assert!(
            sub.replay
                .iter()
                .all(|e| e.run_id.as_deref() == Some("run-1"))
        );
    }

    #[tokio::test]
    async fn replay_buffer_trims_when_full() {
        let hub = EventHub::with_capacity(2);
        let h = hub.handle();
        for seq in 1..=5 {
            h.publish(sample_env(seq, "job.started", None, None));
        }
        let sub = h.subscribe_with_cursor(EventCursor::default()).await;
        assert_eq!(sub.replay.len(), 2);
        assert_eq!(sub.replay[0].sequence, 4); // earliest retained
        assert_eq!(sub.replay[1].sequence, 5);
    }

    #[tokio::test]
    async fn subscribe_with_cursor_after_sequence_skips_replay() {
        let hub = EventHub::with_capacity(8);
        let h = hub.handle();
        h.publish(sample_env(1, "job.started", None, None));
        h.publish(sample_env(2, "job.completed", None, None));
        let cursor = EventCursor {
            last_sequence: Some(2),
            run_id: None,
            job_id: None,
        };
        let sub = h.subscribe_with_cursor(cursor).await;
        assert_eq!(sub.replay.len(), 0); // no events after sequence 2
    }

    #[tokio::test]
    async fn replay_buffer_filters_by_job_id() {
        let hub = EventHub::with_capacity(8);
        let h = hub.handle();
        h.publish(sample_env(1, "job.started", Some("run-1"), Some("job-A")));
        h.publish(sample_env(2, "job.started", Some("run-2"), Some("job-B")));
        h.publish(sample_env(3, "job.completed", Some("run-1"), Some("job-A")));
        let cursor = EventCursor {
            last_sequence: None,
            run_id: None,
            job_id: Some("job-A".into()),
        };
        let sub = h.subscribe_with_cursor(cursor).await;
        assert_eq!(sub.replay.len(), 2); // only job-A events
        assert!(
            sub.replay
                .iter()
                .all(|e| e.job_id.as_deref() == Some("job-A"))
        );
    }

    #[tokio::test]
    async fn subscribe_with_cursor_default_returns_all_replay() {
        let hub = EventHub::with_capacity(8);
        let h = hub.handle();
        for seq in 1..=4 {
            h.publish(sample_env(seq, "tick", None, None));
        }
        let sub = h.subscribe_with_cursor(EventCursor::default()).await;
        assert_eq!(sub.replay.len(), 4);
    }

    // ── Durable EventLog integration ─────────────────────────────────────

    mod durable {
        use super::*;

        /// Minimal in-process [`EventLog`] stub for hub durable-storage tests.
        ///
        /// Replaces `runway_storage::StorageKit` (which lives outside this crate)
        /// with a plain `Vec` behind a `Mutex`. Correct contract: append → stored,
        /// query returns a filtered copy. No persistence across process restarts
        /// (that is the point of the durable tests — they use hub2 to re-hydrate
        /// from the same `Arc<dyn EventLog>` instance, not from disk).
        struct TestEventLog {
            inner: Arc<Mutex<Vec<StoredEvent>>>,
        }

        impl TestEventLog {
            fn new() -> Arc<dyn EventLog> {
                Arc::new(Self {
                    inner: Arc::new(Mutex::new(Vec::new())),
                })
            }
        }

        #[async_trait::async_trait]
        impl EventLog for TestEventLog {
            async fn append(&self, event: StoredEvent) -> crate::Result<()> {
                self.inner.lock().unwrap().push(event);
                Ok(())
            }

            async fn query(&self, q: EventQuery) -> crate::Result<Vec<StoredEvent>> {
                let events = self.inner.lock().unwrap();
                let result = events
                    .iter()
                    .filter(|e| {
                        if let Some(ref org) = q.org_id {
                            if &e.org_id != org {
                                return false;
                            }
                        }
                        if let Some(ref app) = q.app_id {
                            if &e.app_id != app {
                                return false;
                            }
                        }
                        true
                    })
                    .cloned()
                    .collect();
                Ok(result)
            }
        }

        struct TestStorage {
            events: Arc<dyn EventLog>,
        }

        async fn storage_kit() -> TestStorage {
            TestStorage {
                events: TestEventLog::new(),
            }
        }

        fn durable_envelope(ty: &str, run_id: Option<&str>, job_id: Option<&str>) -> EventEnvelope {
            EventEnvelope {
                event_id: Uuid::new_v4(),
                sequence: 0,
                r#type: ty.into(),
                schema_version: 1,
                occurred_at: Utc::now(),
                app_id: "durable-app".into(),
                run_id: run_id.map(String::from),
                job_id: job_id.map(String::from),
                correlation_id: None,
                actor: None,
                payload: serde_json::Value::Null,
            }
        }

        async fn wait_for_persist() {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        #[tokio::test]
        async fn publish_appends_to_event_log() {
            let storage = storage_kit().await;
            let hub =
                EventHub::with_event_log(storage.events.clone(), "test-org", "durable-app").await;
            let h = hub.handle();
            h.publish(durable_envelope("job.started", Some("run-1"), None));
            wait_for_persist().await;

            let stored = storage
                .events
                .query(EventQuery {
                    org_id: Some("test-org".into()),
                    app_id: Some("durable-app".into()),
                    ..Default::default()
                })
                .await
                .expect("query");
            assert_eq!(stored.len(), 1);
            let env = stored_to_envelope(&stored[0]).expect("roundtrip");
            assert_eq!(env.sequence, 1);
            assert_eq!(env.r#type, "job.started");
        }

        #[tokio::test]
        async fn event_survives_hub_restart_via_event_log() {
            let storage = storage_kit().await;
            let hub1 =
                EventHub::with_event_log(storage.events.clone(), "test-org", "durable-app").await;
            let h1 = hub1.handle();
            h1.publish(durable_envelope("job.started", Some("run-1"), None));
            h1.publish(durable_envelope("job.completed", Some("run-1"), None));
            wait_for_persist().await;

            // Simulate process restart: fresh hub, same durable log, empty buffer.
            let hub2 =
                EventHub::with_event_log(storage.events.clone(), "test-org", "durable-app").await;
            let h2 = hub2.handle();
            let sub = h2.subscribe_with_cursor(EventCursor::default()).await;
            assert_eq!(sub.replay.len(), 2);
            assert_eq!(sub.replay[0].sequence, 1);
            assert_eq!(sub.replay[1].sequence, 2);
            assert_eq!(sub.replay[0].r#type, "job.started");
            assert_eq!(sub.replay[1].r#type, "job.completed");
        }

        #[tokio::test]
        async fn cursor_replay_from_event_log_after_sequence() {
            let storage = storage_kit().await;
            let hub1 =
                EventHub::with_event_log(storage.events.clone(), "test-org", "durable-app").await;
            let h1 = hub1.handle();
            h1.publish(durable_envelope("tick", None, None));
            h1.publish(durable_envelope("tick", None, None));
            h1.publish(durable_envelope("tick", None, None));
            wait_for_persist().await;

            let hub2 =
                EventHub::with_event_log(storage.events.clone(), "test-org", "durable-app").await;
            let h2 = hub2.handle();
            let cursor = EventCursor {
                last_sequence: Some(1),
                run_id: None,
                job_id: None,
            };
            let sub = h2.subscribe_with_cursor(cursor).await;
            assert_eq!(sub.replay.len(), 2);
            assert_eq!(sub.replay[0].sequence, 2);
            assert_eq!(sub.replay[1].sequence, 3);
        }

        #[tokio::test]
        async fn sequence_continues_after_restart() {
            let storage = storage_kit().await;
            let hub1 =
                EventHub::with_event_log(storage.events.clone(), "test-org", "durable-app").await;
            hub1.handle().publish(durable_envelope("tick", None, None));
            wait_for_persist().await;

            let hub2 =
                EventHub::with_event_log(storage.events.clone(), "test-org", "durable-app").await;
            let h2 = hub2.handle();
            h2.publish(durable_envelope("tick", None, None));
            wait_for_persist().await;

            let sub = h2.subscribe_with_cursor(EventCursor::default()).await;
            assert_eq!(sub.replay.len(), 2);
            assert_eq!(sub.replay[1].sequence, 2);
        }

        /// Negative: a StoredEvent whose payload cannot be deserialized back into
        /// EventEnvelope must be silently skipped — not panicked — in both
        /// `load_max_sequence` (hub init) and `replay_from_log` (subscribe).
        ///
        /// This exercises the `stored_to_envelope` → `filter_map(...).ok()` path.
        #[tokio::test]
        async fn malformed_stored_payload_is_skipped_not_panicked() {
            // Seed the log with one good event and one event whose payload is
            // a JSON object that does NOT round-trip to EventEnvelope (missing
            // required fields like `event_id`, `type`, etc.).
            let storage = storage_kit().await;
            // Good event — appended first through the hub so it has a real payload.
            let hub1 =
                EventHub::with_event_log(storage.events.clone(), "test-org", "durable-app").await;
            let h1 = hub1.handle();
            h1.publish(durable_envelope("tick", None, None));
            wait_for_persist().await;

            // Malformed event — inject directly into the log, bypassing the hub.
            storage
                .events
                .append(StoredEvent {
                    event_id: "malformed-1".into(),
                    org_id: "test-org".into(),
                    app_id: "durable-app".into(),
                    event_type: "tick".into(),
                    context_id: None,
                    fact_id: None,
                    // payload cannot deserialise to EventEnvelope
                    payload: serde_json::json!({"garbage": true, "sequence": "not-a-number"}),
                    occurred_at: Utc::now(),
                    synced_at: None,
                })
                .await
                .unwrap();

            // Creating a second hub must not panic even though one stored payload
            // is unparseable (load_max_sequence silently filters it).
            let hub2 =
                EventHub::with_event_log(storage.events.clone(), "test-org", "durable-app").await;
            let h2 = hub2.handle();

            // subscribe_with_cursor must not panic, and must return only the
            // one well-formed event.
            let sub = h2.subscribe_with_cursor(EventCursor::default()).await;
            assert_eq!(
                sub.replay.len(),
                1,
                "malformed stored payload must be skipped, not panicked; only the good event should replay"
            );
            assert_eq!(sub.replay[0].r#type, "tick");
        }
    }

    // ── Negative: subscribe-after-eviction cursor catch-up ──────────────────

    /// A cursor with `last_sequence` pointing into the evicted range must
    /// return only the events still in the buffer — not panic and not return
    /// zero events as if everything were consumed.
    ///
    /// Scenario: capacity=2, publish 5 events (seqs 1-5, buffer retains [4, 5]).
    /// Subscribe with cursor `last_sequence = Some(2)` (asked for events after
    /// seq 2).  Events 3, 4, 5 satisfy the cursor but event 3 was evicted;
    /// only events 4 and 5 are in the buffer.
    #[tokio::test]
    async fn subscribe_after_eviction_cursor_returns_remaining_buffer() {
        let hub = EventHub::with_capacity(2);
        let h = hub.handle();
        for _i in 0..5 {
            h.publish(sample_env(0, "tick", None, None));
        }
        // Buffer now holds sequences 4 and 5 (1, 2, 3 evicted).
        let cursor = EventCursor {
            last_sequence: Some(2), // want events after seq 2
            run_id: None,
            job_id: None,
        };
        let sub = h.subscribe_with_cursor(cursor).await;
        // Events 3 was evicted; buffer only has 4 and 5.
        assert_eq!(
            sub.replay.len(),
            2,
            "evicted events are not available; only buffer-resident events are replayed"
        );
        assert_eq!(sub.replay[0].sequence, 4, "first retained event must be sequence 4");
        assert_eq!(sub.replay[1].sequence, 5, "second retained event must be sequence 5");
    }

    // ── Soak test ─────────────────────────────────────────────────────────────

    /// Soak: publish/subscribe cycles through EventHub backed by InMemoryEventLog.
    ///
    /// Assertions:
    /// - All sequences assigned by `publish` are strictly monotone with no gaps (1..=N).
    /// - The in-memory replay buffer is bounded: after N > CAPACITY publishes the buffer
    ///   contains exactly CAPACITY events.
    /// - Buffer end-state replay for a fresh cursor gives the last CAPACITY events in order.
    /// - A "restarted" hub2 (empty buffer, same durable log) replays ALL N events in order.
    ///
    /// Run with `SOAK_ITERS=10000 cargo test -p helm-event-substrate --features memory,sse -- --include-ignored soak`.
    #[cfg(feature = "memory")]
    #[tokio::test]
    #[ignore]
    async fn soak_publish_subscribe_hub_with_backing() {
        use crate::memory::InMemoryEventLog;
        use std::sync::Arc;

        let soak_iters: u64 = std::env::var("SOAK_ITERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100_000);

        const CAPACITY: usize = 512;

        let log = Arc::new(InMemoryEventLog::new());
        let hub = EventHub::with_event_log_capacity(
            CAPACITY,
            log.clone() as Arc<dyn EventLog>,
            "soak-org",
            "soak-app",
        )
        .await;
        let h = hub.handle();

        let start = std::time::Instant::now();

        // Publish all events; record the sequence returned by each publish call.
        let mut sequences: Vec<u64> = Vec::with_capacity(soak_iters as usize);
        for i in 0..soak_iters {
            let env = EventEnvelope {
                event_id: uuid::Uuid::new_v4(),
                sequence: 0, // assigned by hub
                r#type: "soak.tick".into(),
                schema_version: 1,
                occurred_at: Utc::now(),
                app_id: "soak-app".into(),
                run_id: None,
                job_id: Some(format!("job-{i}")),
                correlation_id: None,
                actor: None,
                payload: serde_json::Value::Null,
            };
            sequences.push(h.publish(env));
        }

        // ① Monotone + no-gaps: each publish must return exactly i+1.
        for (i, &seq) in sequences.iter().enumerate() {
            assert_eq!(
                seq,
                (i as u64) + 1,
                "sequence must be strictly monotone with no gaps at position {i}"
            );
        }

        // ② Buffer bounded — after N > CAPACITY publishes the buffer has exactly CAPACITY entries.
        let expected_buf = (soak_iters as usize).min(CAPACITY);
        let buf_sub = h.subscribe_with_cursor(EventCursor::default()).await;
        assert_eq!(
            buf_sub.replay.len(),
            expected_buf,
            "replay buffer must be bounded to CAPACITY"
        );

        // ③ Buffer end-state: last CAPACITY events in strictly increasing sequence order.
        let first_retained = soak_iters - expected_buf as u64 + 1;
        for (i, env) in buf_sub.replay.iter().enumerate() {
            assert_eq!(
                env.sequence,
                first_retained + i as u64,
                "buffer replay must equal in-order publication (position {i})"
            );
        }

        // ④ Wait for all async log-persist tasks spawned inside publish to settle.
        // Poll until the log holds exactly soak_iters events (or 10 s timeout).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let count = log
                .query(EventQuery {
                    org_id: Some("soak-org".into()),
                    app_id: Some("soak-app".into()),
                    ..Default::default()
                })
                .await
                .unwrap()
                .len();
            if count >= soak_iters as usize {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for log to reach {soak_iters} events; got {count}"
            );
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        // ⑤ Fresh hub (simulated restart): empty buffer, same durable log.
        let hub2 = EventHub::with_event_log_capacity(
            CAPACITY,
            log.clone() as Arc<dyn EventLog>,
            "soak-org",
            "soak-app",
        )
        .await;
        let h2 = hub2.handle();
        let full_sub = h2.subscribe_with_cursor(EventCursor::default()).await;

        assert_eq!(
            full_sub.replay.len(),
            soak_iters as usize,
            "fresh hub must replay ALL events from durable log"
        );
        for (i, env) in full_sub.replay.iter().enumerate() {
            assert_eq!(
                env.sequence,
                (i as u64) + 1,
                "durable replay must be in publication order (position {i})"
            );
        }

        let elapsed = start.elapsed();
        // eprintln so the result appears in `cargo test -- --nocapture` output.
        eprintln!(
            "\nSOAK RESULT: iterations={soak_iters} elapsed={elapsed:?} ({:.0} iter/s)",
            soak_iters as f64 / elapsed.as_secs_f64()
        );
    }
}
