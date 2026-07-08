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
        if !guard.events.iter().any(|e| e.event_id == event.event_id) {
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

