// ── In-memory implementations of EventLog and LeaseStore ─────────────────────
//
// Gated behind the `memory` feature (off by default). These are the
// "honest second implementors": every contract assertion that holds for the
// runway redb backend must also hold here.  The parity property tests in this
// module enforce that invariant.
//
// Feature-gate note:
//   `cargo check -p helm-event-substrate` (default features = sse only) does NOT
//   compile this file.  `cargo check -p helm-event-substrate --features memory`
//   is the gate that must stay green.  No trybuild is wired for this crate;
//   the cfg-feature gating pattern is the same as the existing `sse` module.
//
// Oracle choice (T3):
//   The parity property test uses an inline `RefLeaseModel` rather than the
//   runway-storage `RedbLeaseStore`.  RedbLeaseStore requires redb (not a
//   dep of this crate), tempfile, and async spawn_blocking plumbing — too
//   heavy to pull in as a dev-dep here.  The inline model is 50 lines of
//   pure deterministic logic that mirrors the redb arm-by-arm.  RFL-171 T9
//   can add a cross-crate oracle test once runway-storage re-exports the
//   moved traits.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    Result, SubstrateError,
    event::{EventLog, EventQuery, StoredEvent, SyncableEventLog},
    lease::{AcquireOutcome, LeaseRecord, LeaseScope, LeaseStore, RenewOutcome},
};

// ── InMemoryEventLog ──────────────────────────────────────────────────────────

struct EventLogInner {
    /// Append-only log in insertion order.
    events: Vec<StoredEvent>,
    /// Set of all ever-appended `event_id`s — O(1) duplicate detection for
    /// `append`.  Mirrors the redb OR-IGNORE semantics without an O(N) scan.
    seen_ids: HashSet<String>,
    /// Set of event_ids that have been passed to `mark_synced`.
    synced: HashSet<String>,
}

/// In-memory [`EventLog`] and [`SyncableEventLog`] backed by a plain `Vec`.
///
/// Thread-safe via an `Arc<Mutex<…>>` interior.  Idempotent on `event_id`
/// (exact same OR-IGNORE semantics as the redb backend).  Intended for tests
/// and headless composition roots; not for production use.
#[derive(Clone, Debug)]
pub struct InMemoryEventLog {
    inner: Arc<Mutex<EventLogInner>>,
}

impl std::fmt::Debug for EventLogInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventLogInner")
            .field("events_len", &self.events.len())
            .field("synced_count", &self.synced.len())
            .finish()
    }
}

impl InMemoryEventLog {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(EventLogInner {
                events: Vec::new(),
                seen_ids: HashSet::new(),
                synced: HashSet::new(),
            })),
        }
    }
}

impl Default for InMemoryEventLog {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventLog for InMemoryEventLog {
    async fn append(&self, event: StoredEvent) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| SubstrateError::Other(e.to_string()))?;
        // OR-IGNORE semantics: idempotent on event_id (matches redb behaviour).
        // `seen_ids` gives O(1) dedup instead of the O(N) linear scan.
        if guard.seen_ids.insert(event.event_id.clone()) {
            guard.events.push(event);
        }
        Ok(())
    }

    async fn query(&self, q: EventQuery) -> Result<Vec<StoredEvent>> {
        let guard = self
            .inner
            .lock()
            .map_err(|e| SubstrateError::Other(e.to_string()))?;
        Ok(apply_query(&guard.events, &q))
    }
}

#[async_trait]
impl SyncableEventLog for InMemoryEventLog {
    async fn query_unsynced(&self, q: EventQuery) -> Result<Vec<StoredEvent>> {
        let guard = self
            .inner
            .lock()
            .map_err(|e| SubstrateError::Other(e.to_string()))?;
        // Unsynced = all events minus those in the synced set.
        let unsynced: Vec<StoredEvent> = guard
            .events
            .iter()
            .filter(|e| !guard.synced.contains(&e.event_id))
            .cloned()
            .collect();
        Ok(apply_query(&unsynced, &q))
    }

    async fn mark_synced(&self, event_ids: &[String]) -> Result<()> {
        let now = Utc::now();
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| SubstrateError::Other(e.to_string()))?;
        for id in event_ids {
            // Update synced_at on the stored event record (matches redb).
            if let Some(ev) = guard.events.iter_mut().find(|e| &e.event_id == id) {
                ev.synced_at = Some(now);
            }
            guard.synced.insert(id.clone());
        }
        Ok(())
    }
}

/// Apply all filters from `q` to `events`, sort by `occurred_at`, and truncate
/// to `limit`.  Mirrors the Rust-side filtering in the redb implementation
/// (`runway-storage/src/local/event.rs`: `query_inner`).
fn apply_query(events: &[StoredEvent], q: &EventQuery) -> Vec<StoredEvent> {
    let mut result: Vec<StoredEvent> = events
        .iter()
        .filter(|e| {
            if let Some(ref org) = q.org_id
                && &e.org_id != org
            {
                return false;
            }
            if let Some(ref app) = q.app_id
                && &e.app_id != app
            {
                return false;
            }
            if let Some(ref et) = q.event_type
                && &e.event_type != et
            {
                return false;
            }
            // `since` is an exclusive lower bound: events AT or BEFORE `since`
            // are excluded.  Mirrors: `e.occurred_at <= since → return false`.
            if let Some(since) = q.since
                && e.occurred_at <= since
            {
                return false;
            }
            true
        })
        .cloned()
        .collect();

    result.sort_by_key(|e| e.occurred_at);

    if let Some(n) = q.limit {
        result.truncate(n);
    }

    result
}

// ── InMemoryLeaseStore ────────────────────────────────────────────────────────

/// In-memory [`LeaseStore`] backed by a `HashMap` keyed by `LeaseScope::key()`.
///
/// CAS semantics match the redb backend exactly (RFL-171).  Time is injected
/// via the internal `*_at` methods so that tests can control the clock without
/// touching the system wall clock.
#[derive(Clone, Debug)]
pub struct InMemoryLeaseStore {
    inner: Arc<Mutex<HashMap<String, LeaseRecord>>>,
}

impl InMemoryLeaseStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // ── Internal time-injectable methods (used by parity property tests) ──

    fn try_acquire_at(
        &self,
        scope: &LeaseScope,
        holder_id: &str,
        ttl: Duration,
        now: DateTime<Utc>,
    ) -> Result<AcquireOutcome> {
        let new_expires = now
            + chrono::Duration::from_std(ttl)
                .map_err(|e| SubstrateError::Other(e.to_string()))?;

        let mut guard = self
            .inner
            .lock()
            .map_err(|e| SubstrateError::Other(e.to_string()))?;
        let key = scope.key();

        // Clone to end the immutable borrow before the mutable insert below.
        let existing = guard.get(&key).cloned();

        let (new_rec, outcome) = match existing {
            None => {
                let rec = LeaseRecord {
                    holder_id: holder_id.to_string(),
                    expires_at: new_expires,
                };
                (Some(rec.clone()), AcquireOutcome::Acquired(rec))
            }
            Some(existing) if existing.expires_at <= now => {
                // Expired — steal regardless of previous holder.
                let rec = LeaseRecord {
                    holder_id: holder_id.to_string(),
                    expires_at: new_expires,
                };
                (Some(rec.clone()), AcquireOutcome::Acquired(rec))
            }
            Some(existing) if existing.holder_id == holder_id => {
                // Idempotent re-acquire by same holder — refresh TTL.
                let rec = LeaseRecord {
                    holder_id: holder_id.to_string(),
                    expires_at: new_expires,
                };
                (Some(rec.clone()), AcquireOutcome::Acquired(rec))
            }
            Some(existing) => (None, AcquireOutcome::HeldByOther(existing)),
        };

        if let Some(rec) = new_rec {
            guard.insert(key, rec);
        }

        Ok(outcome)
    }

    fn renew_at(
        &self,
        scope: &LeaseScope,
        holder_id: &str,
        ttl: Duration,
        now: DateTime<Utc>,
    ) -> Result<RenewOutcome> {
        let new_expires = now
            + chrono::Duration::from_std(ttl)
                .map_err(|e| SubstrateError::Other(e.to_string()))?;

        let mut guard = self
            .inner
            .lock()
            .map_err(|e| SubstrateError::Other(e.to_string()))?;
        let key = scope.key();

        // Clone to end the immutable borrow before the mutable insert below.
        let existing = guard.get(&key).cloned();

        match existing {
            // An expired same-holder lease is intentionally Lost: force the
            // holder back through try_acquire (steal path) rather than
            // silently extending a lease whose hold may have lapsed.
            // Matches the comment in the redb implementation.
            Some(ref rec) if rec.holder_id == holder_id && rec.expires_at > now => {
                let renewed = LeaseRecord {
                    holder_id: holder_id.to_string(),
                    expires_at: new_expires,
                };
                guard.insert(key, renewed.clone());
                Ok(RenewOutcome::Renewed(renewed))
            }
            other => Ok(RenewOutcome::Lost { current: other }),
        }
    }
}

impl Default for InMemoryLeaseStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LeaseStore for InMemoryLeaseStore {
    async fn try_acquire(
        &self,
        scope: &LeaseScope,
        holder_id: &str,
        ttl: Duration,
    ) -> Result<AcquireOutcome> {
        self.try_acquire_at(scope, holder_id, ttl, Utc::now())
    }

    async fn renew(
        &self,
        scope: &LeaseScope,
        holder_id: &str,
        ttl: Duration,
    ) -> Result<RenewOutcome> {
        self.renew_at(scope, holder_id, ttl, Utc::now())
    }

    async fn release(&self, scope: &LeaseScope, holder_id: &str) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| SubstrateError::Other(e.to_string()))?;
        let key = scope.key();
        // No-op if the caller is not the current holder or the lease does not
        // exist.  Matches the redb implementation.
        if let Some(rec) = guard.get(&key) {
            if rec.holder_id == holder_id {
                guard.remove(&key);
            }
        }
        Ok(())
    }

    async fn current(&self, scope: &LeaseScope) -> Result<Option<LeaseRecord>> {
        let guard = self
            .inner
            .lock()
            .map_err(|e| SubstrateError::Other(e.to_string()))?;
        // Returns the raw stored record; expiry awareness is the caller's
        // responsibility (matches redb behaviour).
        Ok(guard.get(&scope.key()).cloned())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use proptest::prelude::*;

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn now_fixed() -> DateTime<Utc> {
        Utc.timestamp_opt(1_000_000, 0).unwrap()
    }

    fn scope() -> LeaseScope {
        LeaseScope {
            org_id: "org-1".into(),
            app_id: "test-app".into(),
            session_id: "sess-abc".into(),
        }
    }

    fn long_ttl() -> Duration {
        Duration::from_secs(3600)
    }

    fn short_ttl() -> Duration {
        Duration::from_secs(1)
    }

    fn time_step() -> chrono::Duration {
        // Advances past short_ttl (2 s > 1 s) so short leases become expired.
        chrono::Duration::seconds(2)
    }

    fn stored(id: &str, org: &str, app: &str, et: &str, at: DateTime<Utc>) -> StoredEvent {
        StoredEvent {
            event_id: id.to_string(),
            org_id: org.to_string(),
            app_id: app.to_string(),
            event_type: et.to_string(),
            context_id: None,
            fact_id: None,
            payload: serde_json::Value::Null,
            occurred_at: at,
            synced_at: None,
        }
    }

    // ── EventLog unit tests ───────────────────────────────────────────────────

    #[tokio::test]
    async fn append_and_query_all() {
        let log = InMemoryEventLog::new();
        let t0 = now_fixed();
        log.append(stored("e1", "org", "app", "type-a", t0))
            .await
            .unwrap();
        log.append(stored("e2", "org", "app", "type-b", t0 + chrono::Duration::seconds(1)))
            .await
            .unwrap();
        let results = log.query(EventQuery::default()).await.unwrap();
        assert_eq!(results.len(), 2);
        // Must preserve sorted-by-occurred_at order.
        assert_eq!(results[0].event_id, "e1");
        assert_eq!(results[1].event_id, "e2");
    }

    #[tokio::test]
    async fn append_is_idempotent_on_event_id() {
        let log = InMemoryEventLog::new();
        let t0 = now_fixed();
        let ev = stored("dup", "org", "app", "t", t0);
        log.append(ev.clone()).await.unwrap();
        log.append(ev.clone()).await.unwrap();
        let results = log.query(EventQuery::default()).await.unwrap();
        assert_eq!(results.len(), 1, "duplicate event_id must be silently ignored");
    }

    #[tokio::test]
    async fn query_filters_by_org_id() {
        let log = InMemoryEventLog::new();
        let t0 = now_fixed();
        log.append(stored("e1", "org-a", "app", "t", t0))
            .await
            .unwrap();
        log.append(stored("e2", "org-b", "app", "t", t0))
            .await
            .unwrap();
        let results = log
            .query(EventQuery {
                org_id: Some("org-a".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].event_id, "e1");
    }

    #[tokio::test]
    async fn query_filters_by_app_id() {
        let log = InMemoryEventLog::new();
        let t0 = now_fixed();
        log.append(stored("e1", "org", "app-x", "t", t0))
            .await
            .unwrap();
        log.append(stored("e2", "org", "app-y", "t", t0))
            .await
            .unwrap();
        let results = log
            .query(EventQuery {
                app_id: Some("app-y".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].event_id, "e2");
    }

    #[tokio::test]
    async fn query_filters_by_event_type() {
        let log = InMemoryEventLog::new();
        let t0 = now_fixed();
        log.append(stored("e1", "org", "app", "job.started", t0))
            .await
            .unwrap();
        log.append(stored("e2", "org", "app", "job.completed", t0))
            .await
            .unwrap();
        let results = log
            .query(EventQuery {
                event_type: Some("job.started".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].event_id, "e1");
    }

    #[tokio::test]
    async fn query_filters_by_since_exclusive() {
        let log = InMemoryEventLog::new();
        let t0 = now_fixed();
        let t1 = t0 + chrono::Duration::seconds(10);
        let t2 = t0 + chrono::Duration::seconds(20);
        log.append(stored("e1", "org", "app", "t", t0))
            .await
            .unwrap();
        log.append(stored("e2", "org", "app", "t", t1))
            .await
            .unwrap();
        log.append(stored("e3", "org", "app", "t", t2))
            .await
            .unwrap();
        // since = t1: events AT or BEFORE t1 are excluded → only e3 returned.
        let results = log
            .query(EventQuery {
                since: Some(t1),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].event_id, "e3");
    }

    /// Negative: limit = Some(0) must return an empty slice, not all events.
    #[tokio::test]
    async fn query_limit_zero_returns_empty() {
        let log = InMemoryEventLog::new();
        let t0 = now_fixed();
        for i in 0u64..3 {
            log.append(stored(
                &format!("e{i}"),
                "org",
                "app",
                "t",
                t0 + chrono::Duration::seconds(i as i64),
            ))
            .await
            .unwrap();
        }
        let results = log
            .query(EventQuery {
                limit: Some(0),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(
            results.is_empty(),
            "limit=0 must return empty, not {} events",
            results.len()
        );
    }

    #[tokio::test]
    async fn query_limit_truncates_oldest_first() {
        let log = InMemoryEventLog::new();
        let t0 = now_fixed();
        for i in 0u64..5 {
            log.append(stored(
                &format!("e{i}"),
                "org",
                "app",
                "t",
                t0 + chrono::Duration::seconds(i as i64),
            ))
            .await
            .unwrap();
        }
        let results = log
            .query(EventQuery {
                limit: Some(3),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 3);
        // First 3 in occurred_at order.
        assert_eq!(results[0].event_id, "e0");
        assert_eq!(results[2].event_id, "e2");
    }

    // ── SyncableEventLog unit tests ───────────────────────────────────────────

    #[tokio::test]
    async fn query_unsynced_returns_all_initially() {
        let log = InMemoryEventLog::new();
        let t0 = now_fixed();
        log.append(stored("e1", "org", "app", "t", t0))
            .await
            .unwrap();
        log.append(stored("e2", "org", "app", "t", t0))
            .await
            .unwrap();
        let unsynced = log
            .query_unsynced(EventQuery::default())
            .await
            .unwrap();
        assert_eq!(unsynced.len(), 2);
    }

    #[tokio::test]
    async fn mark_synced_removes_from_unsynced() {
        let log = InMemoryEventLog::new();
        let t0 = now_fixed();
        log.append(stored("e1", "org", "app", "t", t0))
            .await
            .unwrap();
        log.append(stored("e2", "org", "app", "t", t0))
            .await
            .unwrap();
        log.mark_synced(&["e1".to_string()]).await.unwrap();
        let unsynced = log
            .query_unsynced(EventQuery::default())
            .await
            .unwrap();
        assert_eq!(unsynced.len(), 1);
        assert_eq!(unsynced[0].event_id, "e2");
    }

    #[tokio::test]
    async fn mark_synced_sets_synced_at_timestamp() {
        let log = InMemoryEventLog::new();
        let t0 = now_fixed();
        log.append(stored("e1", "org", "app", "t", t0))
            .await
            .unwrap();
        log.mark_synced(&["e1".to_string()]).await.unwrap();
        let all = log.query(EventQuery::default()).await.unwrap();
        assert!(
            all[0].synced_at.is_some(),
            "mark_synced must set synced_at on the stored record"
        );
    }

    #[tokio::test]
    async fn mark_synced_is_idempotent() {
        let log = InMemoryEventLog::new();
        let t0 = now_fixed();
        log.append(stored("e1", "org", "app", "t", t0))
            .await
            .unwrap();
        log.mark_synced(&["e1".to_string()]).await.unwrap();
        log.mark_synced(&["e1".to_string()]).await.unwrap();
        let unsynced = log
            .query_unsynced(EventQuery::default())
            .await
            .unwrap();
        assert!(unsynced.is_empty());
    }

    // ── LeaseStore unit tests ─────────────────────────────────────────────────

    #[test]
    fn acquire_on_empty_returns_acquired() {
        let store = InMemoryLeaseStore::new();
        let now = now_fixed();
        let outcome = store
            .try_acquire_at(&scope(), "h1", long_ttl(), now)
            .unwrap();
        assert!(matches!(outcome, AcquireOutcome::Acquired(_)));
    }

    #[test]
    fn idempotent_re_acquire_same_holder_extends_ttl() {
        let store = InMemoryLeaseStore::new();
        let now = now_fixed();
        store
            .try_acquire_at(&scope(), "h1", short_ttl(), now)
            .unwrap();
        let outcome = store
            .try_acquire_at(&scope(), "h1", long_ttl(), now)
            .unwrap();
        match outcome {
            AcquireOutcome::Acquired(rec) => {
                // Expiry must reflect the new long_ttl, not the original short_ttl.
                assert!(
                    rec.expires_at > now + chrono::Duration::seconds(100),
                    "re-acquire must refresh TTL"
                );
            }
            _ => panic!("expected Acquired"),
        }
    }

    #[test]
    fn held_by_other_returned_on_live_conflict() {
        let store = InMemoryLeaseStore::new();
        let now = now_fixed();
        store
            .try_acquire_at(&scope(), "h1", long_ttl(), now)
            .unwrap();
        let outcome = store
            .try_acquire_at(&scope(), "h2", long_ttl(), now)
            .unwrap();
        match outcome {
            AcquireOutcome::HeldByOther(rec) => {
                assert_eq!(rec.holder_id, "h1");
            }
            _ => panic!("expected HeldByOther"),
        }
    }

    #[test]
    fn steal_after_expiry_succeeds() {
        let store = InMemoryLeaseStore::new();
        let now = now_fixed();
        // h1 acquires with short TTL.
        store
            .try_acquire_at(&scope(), "h1", short_ttl(), now)
            .unwrap();
        // Advance time past TTL.
        let later = now + time_step();
        // h2 can now steal.
        let outcome = store
            .try_acquire_at(&scope(), "h2", long_ttl(), later)
            .unwrap();
        match outcome {
            AcquireOutcome::Acquired(rec) => {
                assert_eq!(rec.holder_id, "h2");
            }
            _ => panic!("expected Acquired after steal"),
        }
    }

    #[test]
    fn renew_by_holder_succeeds_while_live() {
        let store = InMemoryLeaseStore::new();
        let now = now_fixed();
        store
            .try_acquire_at(&scope(), "h1", short_ttl(), now)
            .unwrap();
        let outcome = store
            .renew_at(&scope(), "h1", long_ttl(), now)
            .unwrap();
        assert!(matches!(outcome, RenewOutcome::Renewed(_)));
    }

    #[test]
    fn renew_returns_lost_for_wrong_holder() {
        let store = InMemoryLeaseStore::new();
        let now = now_fixed();
        store
            .try_acquire_at(&scope(), "h1", long_ttl(), now)
            .unwrap();
        let outcome = store
            .renew_at(&scope(), "h2", long_ttl(), now)
            .unwrap();
        match outcome {
            RenewOutcome::Lost { current: Some(rec) } => {
                assert_eq!(rec.holder_id, "h1");
            }
            _ => panic!("expected Lost{{current: Some}} for wrong holder"),
        }
    }

    #[test]
    fn renew_returns_lost_when_expired_even_for_same_holder() {
        // An expired same-holder lease is intentionally Lost (matches redb comment).
        let store = InMemoryLeaseStore::new();
        let now = now_fixed();
        store
            .try_acquire_at(&scope(), "h1", short_ttl(), now)
            .unwrap();
        let later = now + time_step();
        let outcome = store
            .renew_at(&scope(), "h1", long_ttl(), later)
            .unwrap();
        match outcome {
            RenewOutcome::Lost { current: Some(rec) } => {
                assert_eq!(rec.holder_id, "h1");
            }
            _ => panic!("expected Lost for expired same-holder"),
        }
    }

    #[test]
    fn renew_returns_lost_with_none_when_no_lease() {
        let store = InMemoryLeaseStore::new();
        let now = now_fixed();
        let outcome = store
            .renew_at(&scope(), "h1", long_ttl(), now)
            .unwrap();
        assert!(matches!(outcome, RenewOutcome::Lost { current: None }));
    }

    #[test]
    fn release_by_holder_removes_lease() {
        let store = InMemoryLeaseStore::new();
        let scope = scope();
        let now = now_fixed();
        store
            .try_acquire_at(&scope, "h1", long_ttl(), now)
            .unwrap();
        // Sync release (we call the internal via the blocking trait path via tokio).
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(store.release(&scope, "h1")).unwrap();
        let current = rt.block_on(store.current(&scope)).unwrap();
        assert!(current.is_none(), "lease must be gone after holder releases it");
    }

    #[test]
    fn release_by_non_holder_is_noop() {
        let store = InMemoryLeaseStore::new();
        let scope = scope();
        let now = now_fixed();
        store
            .try_acquire_at(&scope, "h1", long_ttl(), now)
            .unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        // h2 tries to release h1's lease — must be a no-op.
        rt.block_on(store.release(&scope, "h2")).unwrap();
        let current = rt.block_on(store.current(&scope)).unwrap();
        assert!(
            current.is_some(),
            "lease must remain after non-holder release attempt"
        );
        assert_eq!(current.unwrap().holder_id, "h1");
    }

    #[test]
    fn current_returns_none_when_no_lease() {
        let store = InMemoryLeaseStore::new();
        let scope = scope();
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let current = rt.block_on(store.current(&scope)).unwrap();
        assert!(current.is_none());
    }

    #[test]
    fn current_returns_record_even_if_expired() {
        // `current` is expiry-agnostic (raw read) — matches redb.
        let store = InMemoryLeaseStore::new();
        let scope = scope();
        let now = now_fixed();
        store
            .try_acquire_at(&scope, "h1", short_ttl(), now)
            .unwrap();
        let later = now + time_step(); // past TTL
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        // Use `later` only for reading — the stored record is still there.
        let current = rt.block_on(store.current(&scope)).unwrap();
        assert!(
            current.is_some(),
            "current() must return the expired record (expiry-agnostic)"
        );
        // The record is expired relative to `later`.
        assert!(current.unwrap().expires_at <= later);
    }

    // ── Inline reference model for parity testing ─────────────────────────────

    /// Pure synchronous reference model of the CAS lease rules.
    ///
    /// Mirrors the arm-by-arm logic of `RedbLeaseStore` (`local/lease.rs`).
    /// Used as the oracle in the parity property test.
    struct RefLeaseModel {
        records: HashMap<String, LeaseRecord>,
    }

    impl RefLeaseModel {
        fn new() -> Self {
            Self {
                records: HashMap::new(),
            }
        }

        fn try_acquire_at(
            &mut self,
            scope: &LeaseScope,
            holder_id: &str,
            ttl: Duration,
            now: DateTime<Utc>,
        ) -> AcquireOutcome {
            let new_expires =
                now + chrono::Duration::from_std(ttl).expect("valid ttl in ref model");
            let key = scope.key();
            let existing = self.records.get(&key).cloned();

            let (new_rec, outcome) = match existing {
                None => {
                    let rec = LeaseRecord {
                        holder_id: holder_id.to_string(),
                        expires_at: new_expires,
                    };
                    (Some(rec.clone()), AcquireOutcome::Acquired(rec))
                }
                Some(ex) if ex.expires_at <= now => {
                    let rec = LeaseRecord {
                        holder_id: holder_id.to_string(),
                        expires_at: new_expires,
                    };
                    (Some(rec.clone()), AcquireOutcome::Acquired(rec))
                }
                Some(ex) if ex.holder_id == holder_id => {
                    let rec = LeaseRecord {
                        holder_id: holder_id.to_string(),
                        expires_at: new_expires,
                    };
                    (Some(rec.clone()), AcquireOutcome::Acquired(rec))
                }
                Some(ex) => (None, AcquireOutcome::HeldByOther(ex)),
            };
            if let Some(rec) = new_rec {
                self.records.insert(key, rec);
            }
            outcome
        }

        fn renew_at(
            &mut self,
            scope: &LeaseScope,
            holder_id: &str,
            ttl: Duration,
            now: DateTime<Utc>,
        ) -> RenewOutcome {
            let new_expires =
                now + chrono::Duration::from_std(ttl).expect("valid ttl in ref model");
            let key = scope.key();
            let existing = self.records.get(&key).cloned();

            match existing {
                Some(ref rec) if rec.holder_id == holder_id && rec.expires_at > now => {
                    let renewed = LeaseRecord {
                        holder_id: holder_id.to_string(),
                        expires_at: new_expires,
                    };
                    self.records.insert(key, renewed.clone());
                    RenewOutcome::Renewed(renewed)
                }
                other => RenewOutcome::Lost { current: other },
            }
        }

        fn release(&mut self, scope: &LeaseScope, holder_id: &str) {
            let key = scope.key();
            if let Some(rec) = self.records.get(&key) {
                if rec.holder_id == holder_id {
                    self.records.remove(&key);
                }
            }
        }
    }

    // ── LeaseStore parity property test ───────────────────────────────────────
    //
    // Generates random sequences of acquire/renew/release ops over 3 holders
    // and a controllable virtual clock.  Both InMemoryLeaseStore._at and
    // RefLeaseModel are exercised with identical (scope, holder, ttl, now)
    // inputs and their outcomes must match exactly.
    //
    // Oracle: inline RefLeaseModel (not RedbLeaseStore — see module-level note).

    const HOLDERS: [&str; 3] = ["h0", "h1", "h2"];
    const LONG_TTL_SECS: u64 = 3600;
    const SHORT_TTL_SECS: u64 = 1;
    const TIME_STEP_SECS: i64 = 2; // advances past SHORT_TTL

    #[derive(Debug, Clone)]
    enum LeaseOp {
        Acquire {
            holder_idx: usize,
            long_ttl: bool,
        },
        Renew {
            holder_idx: usize,
            long_ttl: bool,
        },
        Release {
            holder_idx: usize,
        },
        TimeStep,
    }

    fn arb_holder_idx() -> impl Strategy<Value = usize> {
        0usize..3
    }

    fn arb_lease_op() -> impl Strategy<Value = LeaseOp> {
        prop_oneof![
            (arb_holder_idx(), any::<bool>()).prop_map(|(h, lt)| LeaseOp::Acquire {
                holder_idx: h,
                long_ttl: lt
            }),
            (arb_holder_idx(), any::<bool>()).prop_map(|(h, lt)| LeaseOp::Renew {
                holder_idx: h,
                long_ttl: lt
            }),
            arb_holder_idx().prop_map(|h| LeaseOp::Release { holder_idx: h }),
            Just(LeaseOp::TimeStep),
        ]
    }

    fn arb_op_sequence() -> impl Strategy<Value = Vec<LeaseOp>> {
        proptest::collection::vec(arb_lease_op(), 2..25)
    }

    proptest! {
        /// Every outcome from InMemoryLeaseStore must equal the inline reference
        /// model for the same operation sequence and time.
        #[test]
        fn lease_store_parity_with_ref_model(ops in arb_op_sequence()) {
            let scope = LeaseScope {
                org_id: "parity-org".into(),
                app_id: "parity-app".into(),
                session_id: "parity-sess".into(),
            };

            let store = InMemoryLeaseStore::new();
            let mut reference = RefLeaseModel::new();
            let mut now = now_fixed();

            for op in &ops {
                match op {
                    LeaseOp::Acquire { holder_idx, long_ttl } => {
                        let holder = HOLDERS[*holder_idx];
                        let ttl = Duration::from_secs(if *long_ttl { LONG_TTL_SECS } else { SHORT_TTL_SECS });

                        let store_out = store.try_acquire_at(&scope, holder, ttl, now).unwrap();
                        let ref_out = reference.try_acquire_at(&scope, holder, ttl, now);

                        // Compare discriminant and holder_id (not expiry — ref
                        // and impl compute identically from the same inputs).
                        match (&store_out, &ref_out) {
                            (AcquireOutcome::Acquired(a), AcquireOutcome::Acquired(b)) => {
                                prop_assert_eq!(&a.holder_id, &b.holder_id, "Acquired holder mismatch");
                                prop_assert_eq!(a.expires_at, b.expires_at, "Acquired expiry mismatch");
                            }
                            (AcquireOutcome::HeldByOther(a), AcquireOutcome::HeldByOther(b)) => {
                                prop_assert_eq!(&a.holder_id, &b.holder_id, "HeldByOther holder mismatch");
                            }
                            _ => {
                                prop_assert!(false, "Acquire outcome discriminant mismatch: store={:?} ref={:?}", store_out, ref_out);
                            }
                        }
                    }
                    LeaseOp::Renew { holder_idx, long_ttl } => {
                        let holder = HOLDERS[*holder_idx];
                        let ttl = Duration::from_secs(if *long_ttl { LONG_TTL_SECS } else { SHORT_TTL_SECS });

                        let store_out = store.renew_at(&scope, holder, ttl, now).unwrap();
                        let ref_out = reference.renew_at(&scope, holder, ttl, now);

                        match (&store_out, &ref_out) {
                            (RenewOutcome::Renewed(a), RenewOutcome::Renewed(b)) => {
                                prop_assert_eq!(&a.holder_id, &b.holder_id, "Renewed holder mismatch");
                                prop_assert_eq!(a.expires_at, b.expires_at, "Renewed expiry mismatch");
                            }
                            (RenewOutcome::Lost { current: a }, RenewOutcome::Lost { current: b }) => {
                                let a_id = a.as_ref().map(|r| r.holder_id.as_str());
                                let b_id = b.as_ref().map(|r| r.holder_id.as_str());
                                prop_assert_eq!(a_id, b_id, "Lost current holder mismatch");
                            }
                            _ => {
                                prop_assert!(false, "Renew outcome discriminant mismatch: store={:?} ref={:?}", store_out, ref_out);
                            }
                        }
                    }
                    LeaseOp::Release { holder_idx } => {
                        let holder = HOLDERS[*holder_idx];
                        let rt = tokio::runtime::Builder::new_current_thread()
                            .build()
                            .unwrap();
                        rt.block_on(store.release(&scope, holder)).unwrap();
                        reference.release(&scope, holder);
                        // After release, both current() states must agree on
                        // holder (or both None).
                        let store_current = rt.block_on(store.current(&scope)).unwrap();
                        let ref_current = reference.records.get(&scope.key()).cloned();
                        let s_id = store_current.as_ref().map(|r| r.holder_id.as_str());
                        let r_id = ref_current.as_ref().map(|r| r.holder_id.as_str());
                        prop_assert_eq!(s_id, r_id, "current() holder mismatch after release");
                    }
                    LeaseOp::TimeStep => {
                        now += chrono::Duration::seconds(TIME_STEP_SECS);
                    }
                }
            }
        }
    }

    // ── EventLog property tests ───────────────────────────────────────────────
    //
    // These properties must hold for any sequence of appended events:
    //   P1: query(limit=n) ⊆ query(no limit)   (limit is a filter, not a shuffle)
    //   P2: since-filter is monotonic           (all returned events have occurred_at > since)
    //   P3: append order is preserved           (query returns events sorted by occurred_at)
    //   P4: limit ≤ requested n                 (never over-truncates)

    fn arb_nonempty_id() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9]{0,8}"
    }

    fn arb_event_at(base_ts: i64) -> impl Strategy<Value = StoredEvent> {
        (arb_nonempty_id(), 0i64..1000i64).prop_map(move |(id, offset)| {
            let t = Utc.timestamp_opt(base_ts + offset, 0).unwrap();
            stored(&id, "org", "app", "t", t)
        })
    }

    fn arb_events() -> impl Strategy<Value = Vec<StoredEvent>> {
        proptest::collection::vec(arb_event_at(1_000_000), 0..20)
    }

    /// Deduplicate a generated event list by event_id (simulates append idempotence).
    fn dedup_by_id(mut events: Vec<StoredEvent>) -> Vec<StoredEvent> {
        let mut seen = HashSet::new();
        events.retain(|e| seen.insert(e.event_id.clone()));
        events
    }

    proptest! {
        /// P1: query(limit) ⊆ query(no limit)
        #[test]
        fn event_log_limit_is_subset_of_full(events in arb_events(), n in 0usize..20) {
            let events = dedup_by_id(events);
            let full = apply_query(&events, &EventQuery::default());
            let limited = apply_query(&events, &EventQuery { limit: Some(n), ..Default::default() });
            // limited must be a prefix of full.
            prop_assert!(limited.len() <= full.len());
            for (l, f) in limited.iter().zip(full.iter()) {
                prop_assert_eq!(&l.event_id, &f.event_id, "limit must preserve order prefix");
            }
        }

        /// P2: since-filter returns only events with occurred_at > since
        #[test]
        fn event_log_since_filter_is_exclusive(events in arb_events(), since_offset in 0i64..500) {
            let events = dedup_by_id(events);
            let base = Utc.timestamp_opt(1_000_000, 0).unwrap();
            let since = base + chrono::Duration::seconds(since_offset);
            let filtered = apply_query(
                &events,
                &EventQuery { since: Some(since), ..Default::default() },
            );
            for e in &filtered {
                prop_assert!(
                    e.occurred_at > since,
                    "since is exclusive: occurred_at {:?} must be > since {:?}",
                    e.occurred_at,
                    since
                );
            }
        }

        /// P3: query without limit returns events sorted by occurred_at
        #[test]
        fn event_log_query_preserves_occurred_at_order(events in arb_events()) {
            let events = dedup_by_id(events);
            let result = apply_query(&events, &EventQuery::default());
            for w in result.windows(2) {
                prop_assert!(
                    w[0].occurred_at <= w[1].occurred_at,
                    "events must be sorted by occurred_at"
                );
            }
        }

        /// P4: query(limit=n) length ≤ n
        #[test]
        fn event_log_limit_never_exceeds_n(events in arb_events(), n in 0usize..20) {
            let events = dedup_by_id(events);
            let result = apply_query(&events, &EventQuery { limit: Some(n), ..Default::default() });
            prop_assert!(result.len() <= n);
        }
    }
}
