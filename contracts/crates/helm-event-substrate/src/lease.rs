// ── Lease / session-ownership contracts ─────────────────────────────────────
// Moved verbatim from runway-storage/src/traits/lease.rs (RFL-171).
// Error type retargeted from runway_storage::traits::Error → crate::SubstrateError.

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::Result;

/// Triple that identifies a unique lease. Serialized as
/// `format!("{org_id}|{app_id}|{session_id}")` for storage keys.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LeaseScope {
    pub org_id: String,
    pub app_id: String,
    pub session_id: String,
}

impl LeaseScope {
    /// Stable string key for backend storage. Implementations MUST use this
    /// for both redb keys and Firestore document IDs to keep contract-suite
    /// assertions consistent.
    ///
    /// `RP-NO-LEASE-WITHOUT-FENCING-V1` — v1 lease contract is admission-time
    /// correctness only. There is no write-side fencing; a paused process that
    /// wakes after TTL steal can still write through `DocumentStore`.
    pub fn key(&self) -> String {
        format!("{}|{}|{}", self.org_id, self.app_id, self.session_id)
    }
}

/// Persistent state of a lease.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LeaseRecord {
    pub holder_id: String,
    pub expires_at: DateTime<Utc>,
}

/// Outcome of a `try_acquire` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcquireOutcome {
    /// Caller now holds the lease (new acquire, idempotent re-acquire by same
    /// holder, or steal after expiry).
    Acquired(LeaseRecord),
    /// Another holder owns an unexpired lease.
    HeldByOther(LeaseRecord),
}

/// Outcome of a `renew` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenewOutcome {
    /// Renewal succeeded; `expires_at` was advanced.
    Renewed(LeaseRecord),
    /// Caller is no longer the holder. `current` is `Some` when another holder
    /// has taken over, `None` when the lease was released or never existed.
    Lost { current: Option<LeaseRecord> },
}

/// Atomic compare-and-swap leases keyed by `(org_id, app_id, session_id)`.
///
/// Local impl: redb single-`WriteTransaction` read-modify-write.
/// Remote impl: Firestore `runTransaction` on `_runway_leases/{scope_key}`.
///
/// v1 is admission-time correctness only. There is no write-side fencing —
/// a paused process that wakes after TTL steal can still write through
/// `DocumentStore`. See `RP-NO-LEASE-WITHOUT-FENCING-V1`.
#[async_trait]
pub trait LeaseStore: Send + Sync {
    /// Try to acquire (or re-acquire / steal-after-expiry) the lease.
    async fn try_acquire(
        &self,
        scope: &LeaseScope,
        holder_id: &str,
        ttl: Duration,
    ) -> Result<AcquireOutcome>;

    /// Renew an existing lease iff `holder_id` matches the current record.
    async fn renew(
        &self,
        scope: &LeaseScope,
        holder_id: &str,
        ttl: Duration,
    ) -> Result<RenewOutcome>;

    /// Release the lease. No-op if the caller is not the current holder or
    /// the lease does not exist.
    async fn release(&self, scope: &LeaseScope, holder_id: &str) -> Result<()>;

    /// Read the current record without modifying it.
    async fn current(&self, scope: &LeaseScope) -> Result<Option<LeaseRecord>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_key_is_pipe_delimited() {
        let scope = LeaseScope {
            org_id: "org-1".into(),
            app_id: "quorum".into(),
            session_id: "inq-abc".into(),
        };
        assert_eq!(scope.key(), "org-1|quorum|inq-abc");
    }

    #[test]
    fn lease_record_serde_roundtrip() {
        let now = Utc::now();
        let rec = LeaseRecord {
            holder_id: "rev-1:uuid-x".into(),
            expires_at: now,
        };
        let json = serde_json::to_string(&rec).expect("serialize");
        let back: LeaseRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.holder_id, rec.holder_id);
        assert_eq!(back.expires_at, rec.expires_at);
    }
}
