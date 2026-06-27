//! Advisory presence and soft-claims.
//!
//! Presence answers "who is looking at this subject" and lets an operator raise
//! an advisory soft-claim ("I'm working on this"). Under the optimistic model a
//! soft-claim never blocks another operator from acting; it is a coordination
//! hint, not a lock. Conflict-safety lives in the decision ledger, not here.

use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::principal::OperatorPrincipal;
use crate::subject::SubjectRef;

/// One operator's presence on one subject.
#[derive(Debug, Clone, Serialize)]
pub struct PresenceEntry {
    pub session_id: Uuid,
    pub principal: OperatorPrincipal,
    pub subject: SubjectRef,
    /// Advisory soft-claim marker. Never enforced as a lock.
    pub claimed: bool,
    pub since: DateTime<Utc>,
}

/// What changed after a presence mutation, so the service can emit the right event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceChange {
    Joined,
    Updated,
}

/// In-memory presence registry. Entry identity is `(session_id, subject)`.
#[derive(Debug, Default)]
pub struct PresenceRegistry {
    inner: Mutex<Vec<PresenceEntry>>,
}

impl PresenceRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Focus a session on a subject (advisory, not claimed). Upserts the entry.
    pub fn focus(
        &self,
        session_id: Uuid,
        principal: OperatorPrincipal,
        subject: SubjectRef,
    ) -> (PresenceEntry, PresenceChange) {
        self.upsert(session_id, principal, subject, false)
    }

    /// Raise an advisory soft-claim for a session on a subject. Upserts the entry.
    pub fn claim(
        &self,
        session_id: Uuid,
        principal: OperatorPrincipal,
        subject: SubjectRef,
    ) -> (PresenceEntry, PresenceChange) {
        self.upsert(session_id, principal, subject, true)
    }

    /// Release a soft-claim (the entry stays as plain focus). Returns the entry.
    pub fn release(&self, session_id: Uuid, subject: &SubjectRef) -> Option<PresenceEntry> {
        let mut entries = self.guard();
        let entry = entries
            .iter_mut()
            .find(|entry| entry.session_id == session_id && &entry.subject == subject)?;
        entry.claimed = false;
        entry.since = Utc::now();
        Some(entry.clone())
    }

    /// Remove all presence for a session (e.g. on session close). Returns removed entries.
    pub fn leave(&self, session_id: Uuid) -> Vec<PresenceEntry> {
        let mut entries = self.guard();
        let (removed, kept): (Vec<_>, Vec<_>) = entries
            .drain(..)
            .partition(|entry| entry.session_id == session_id);
        *entries = kept;
        removed
    }

    /// Presence entries for a workspace, optionally filtered to a subject.
    pub fn list(&self, workspace_id: &str, subject: Option<&SubjectRef>) -> Vec<PresenceEntry> {
        let mut entries = self
            .guard()
            .iter()
            .filter(|entry| entry.principal.workspace_id == workspace_id)
            .filter(|entry| subject.is_none_or(|wanted| &entry.subject == wanted))
            .cloned()
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.since);
        entries
    }

    fn upsert(
        &self,
        session_id: Uuid,
        principal: OperatorPrincipal,
        subject: SubjectRef,
        claimed: bool,
    ) -> (PresenceEntry, PresenceChange) {
        let mut entries = self.guard();
        if let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.session_id == session_id && entry.subject == subject)
        {
            entry.principal = principal;
            entry.claimed = claimed;
            entry.since = Utc::now();
            (entry.clone(), PresenceChange::Updated)
        } else {
            let entry = PresenceEntry {
                session_id,
                principal,
                subject,
                claimed,
                since: Utc::now(),
            };
            entries.push(entry.clone());
            (entry, PresenceChange::Joined)
        }
    }

    fn guard(&self) -> std::sync::MutexGuard<'_, Vec<PresenceEntry>> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use application_kernel::ActorKind;

    fn principal(actor: &str) -> OperatorPrincipal {
        OperatorPrincipal::new(actor, actor, ActorKind::Human, "ws-1")
    }

    #[test]
    fn focus_then_claim_then_release() {
        let registry = PresenceRegistry::new();
        let session = Uuid::new_v4();
        let subject = SubjectRef::gate("run-1:gate-ref");

        let (_, change) = registry.focus(session, principal("alice"), subject.clone());
        assert_eq!(change, PresenceChange::Joined);

        let (entry, change) = registry.claim(session, principal("alice"), subject.clone());
        assert_eq!(change, PresenceChange::Updated);
        assert!(entry.claimed);

        let released = registry.release(session, &subject).expect("release");
        assert!(!released.claimed);
    }

    #[test]
    fn two_operators_can_focus_same_subject() {
        let registry = PresenceRegistry::new();
        let subject = SubjectRef::gate("run-1:gate-ref");
        registry.claim(Uuid::new_v4(), principal("alice"), subject.clone());
        registry.claim(Uuid::new_v4(), principal("bob"), subject.clone());
        // Optimistic: both presences coexist; no lock.
        assert_eq!(registry.list("ws-1", Some(&subject)).len(), 2);
    }

    #[test]
    fn leave_removes_all_session_presence() {
        let registry = PresenceRegistry::new();
        let session = Uuid::new_v4();
        registry.focus(session, principal("alice"), SubjectRef::gate("g1"));
        registry.focus(session, principal("alice"), SubjectRef::run("r1"));
        let removed = registry.leave(session);
        assert_eq!(removed.len(), 2);
        assert!(registry.list("ws-1", None).is_empty());
    }

    #[test]
    fn list_is_scoped_by_workspace() {
        let registry = PresenceRegistry::new();
        registry.focus(Uuid::new_v4(), principal("alice"), SubjectRef::gate("g1"));
        registry.focus(
            Uuid::new_v4(),
            OperatorPrincipal::new("carol", "carol", ActorKind::Human, "ws-2"),
            SubjectRef::gate("g1"),
        );
        assert_eq!(registry.list("ws-1", None).len(), 1);
        assert_eq!(registry.list("ws-2", None).len(), 1);
    }
}
