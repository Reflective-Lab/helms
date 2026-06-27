//! Operator session registry.
//!
//! A session represents one connected operator. Sessions are heartbeat-leased:
//! a session that has not been seen within the lease window is considered stale
//! and swept. State is in-memory for the first increment (durable coordination
//! state is a documented follow-up).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::error::CoordinationError;
use crate::principal::OperatorPrincipal;

/// Default heartbeat lease: a session unseen for longer than this is swept.
pub const DEFAULT_SESSION_LEASE: Duration = Duration::from_secs(300);

/// A connected operator session.
#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub id: Uuid,
    pub principal: OperatorPrincipal,
    pub opened_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

impl Session {
    fn is_stale(&self, now: DateTime<Utc>, lease: Duration) -> bool {
        let lease = chrono::Duration::from_std(lease).unwrap_or_else(|_| chrono::Duration::zero());
        now - self.last_seen > lease
    }
}

/// In-memory, workspace-scoped session registry.
#[derive(Debug)]
pub struct SessionRegistry {
    inner: Mutex<HashMap<Uuid, Session>>,
    lease: Duration,
}

impl SessionRegistry {
    #[must_use]
    pub fn new(lease: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            lease,
        }
    }

    /// Open a new session for a resolved principal.
    pub fn open(&self, principal: OperatorPrincipal) -> Session {
        let now = Utc::now();
        let session = Session {
            id: Uuid::new_v4(),
            principal,
            opened_at: now,
            last_seen: now,
        };
        self.guard().insert(session.id, session.clone());
        session
    }

    /// Refresh a session's lease. Errors if the session is unknown or expired.
    pub fn heartbeat(&self, id: Uuid) -> Result<Session, CoordinationError> {
        let now = Utc::now();
        let mut sessions = self.guard();
        match sessions.get_mut(&id) {
            Some(session) if !session.is_stale(now, self.lease) => {
                session.last_seen = now;
                Ok(session.clone())
            }
            Some(_) => {
                sessions.remove(&id);
                Err(CoordinationError::SessionNotFound(id.to_string()))
            }
            None => Err(CoordinationError::SessionNotFound(id.to_string())),
        }
    }

    /// Close a session explicitly. Returns the closed session, if any.
    pub fn close(&self, id: Uuid) -> Option<Session> {
        self.guard().remove(&id)
    }

    /// Whether a session is present and not stale.
    pub fn is_active(&self, id: Uuid) -> bool {
        let now = Utc::now();
        self.guard()
            .get(&id)
            .is_some_and(|session| !session.is_stale(now, self.lease))
    }

    /// Active (non-stale) sessions for a workspace. Sweeps stale sessions first.
    pub fn active(&self, workspace_id: &str) -> Vec<Session> {
        self.sweep();
        let mut sessions = self
            .guard()
            .values()
            .filter(|session| session.principal.workspace_id == workspace_id)
            .cloned()
            .collect::<Vec<_>>();
        sessions.sort_by_key(|session| session.opened_at);
        sessions
    }

    /// Remove every stale session and return the removed set.
    pub fn sweep(&self) -> Vec<Session> {
        let now = Utc::now();
        let mut sessions = self.guard();
        let stale = sessions
            .values()
            .filter(|session| session.is_stale(now, self.lease))
            .map(|session| session.id)
            .collect::<Vec<_>>();
        stale
            .into_iter()
            .filter_map(|id| sessions.remove(&id))
            .collect()
    }

    fn guard(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, Session>> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_SESSION_LEASE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use application_kernel::ActorKind;

    fn principal(actor: &str, workspace: &str) -> OperatorPrincipal {
        OperatorPrincipal::new(actor, actor, ActorKind::Human, workspace)
    }

    #[test]
    fn open_then_heartbeat_then_close() {
        let registry = SessionRegistry::new(Duration::from_secs(60));
        let session = registry.open(principal("alice", "ws-1"));
        assert!(registry.is_active(session.id));
        let refreshed = registry.heartbeat(session.id).expect("heartbeat");
        assert!(refreshed.last_seen >= session.last_seen);
        assert!(registry.close(session.id).is_some());
        assert!(!registry.is_active(session.id));
    }

    #[test]
    fn active_is_scoped_by_workspace() {
        let registry = SessionRegistry::new(Duration::from_secs(60));
        registry.open(principal("alice", "ws-1"));
        registry.open(principal("bob", "ws-1"));
        registry.open(principal("carol", "ws-2"));
        assert_eq!(registry.active("ws-1").len(), 2);
        assert_eq!(registry.active("ws-2").len(), 1);
    }

    #[test]
    fn stale_sessions_are_swept_and_rejected() {
        let registry = SessionRegistry::new(Duration::from_millis(0));
        let session = registry.open(principal("alice", "ws-1"));
        // With a zero lease, any elapsed time makes the session stale.
        std::thread::sleep(Duration::from_millis(5));
        assert!(matches!(
            registry.heartbeat(session.id),
            Err(CoordinationError::SessionNotFound(_))
        ));
        assert!(registry.active("ws-1").is_empty());
    }
}
