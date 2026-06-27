//! The coordination service: sessions, presence, and conflict-safe gate decisions.
//!
//! This is the headless core that multiple operators share. It is transport
//! agnostic (the HTTP layer is a thin adapter over these methods) so it can be
//! unit-tested directly and driven from a CLI or automation later.

use std::sync::Arc;
use std::time::Duration;

use helm_governed_jobs::JobStreamState;
use runway_app_host::EventHubHandle;
use serde_json::json;
use uuid::Uuid;

use crate::error::CoordinationError;
use crate::events::{
    self, CoordinationPublisher, CLAIM_ACQUIRED, CLAIM_RELEASED, DECISION_CONFLICT,
    DECISION_DENIED, DECISION_RECORDED, PRESENCE_FOCUS_CHANGED, PRESENCE_JOINED, PRESENCE_LEFT,
    SESSION_CLOSED, SESSION_OPENED,
};
use crate::ledger::{
    AuthorityResolver, DecisionLedger, DecisionOutcome, GateDecisionKind, PermissiveAuthority,
};
use crate::presence::{PresenceChange, PresenceEntry, PresenceRegistry};
use crate::principal::{PrincipalClaim, PrincipalResolver, RequestActorResolver};
use crate::session::{Session, SessionRegistry, DEFAULT_SESSION_LEASE};
use crate::subject::SubjectRef;

/// Shared multi-operator coordination state and behavior.
pub struct CoordinationService {
    resolver: Arc<dyn PrincipalResolver>,
    authority: Arc<dyn AuthorityResolver>,
    sessions: SessionRegistry,
    presence: PresenceRegistry,
    ledger: DecisionLedger,
    publisher: CoordinationPublisher,
    hub: EventHubHandle,
    job_state: Option<Arc<JobStreamState>>,
    app_id: String,
}

impl CoordinationService {
    /// Construct over a hub with default identity (request-actor) and authority
    /// (permissive) and the default session lease.
    pub fn new(hub: EventHubHandle, app_id: impl Into<String>) -> Self {
        let app_id = app_id.into();
        let publisher = CoordinationPublisher::new(hub.clone(), app_id.clone());
        Self {
            resolver: Arc::new(RequestActorResolver),
            authority: Arc::new(PermissiveAuthority),
            sessions: SessionRegistry::new(DEFAULT_SESSION_LEASE),
            presence: PresenceRegistry::new(),
            ledger: DecisionLedger::new(),
            publisher,
            hub,
            job_state: None,
            app_id,
        }
    }

    #[must_use]
    pub fn with_resolver(mut self, resolver: Arc<dyn PrincipalResolver>) -> Self {
        self.resolver = resolver;
        self
    }

    #[must_use]
    pub fn with_authority(mut self, authority: Arc<dyn AuthorityResolver>) -> Self {
        self.authority = authority;
        self
    }

    #[must_use]
    pub fn with_session_lease(mut self, lease: Duration) -> Self {
        self.sessions = SessionRegistry::new(lease);
        self
    }

    /// Wire the governed-jobs state so accepted gate decisions drive real runs.
    ///
    /// Adopts the job state's hub so coordination and job events share one
    /// globally-monotonic stream (sequence stamped by the hub).
    #[must_use]
    pub fn with_job_state(mut self, job_state: Arc<JobStreamState>) -> Self {
        self.hub = job_state.hub.clone();
        self.publisher = CoordinationPublisher::new(job_state.hub.clone(), self.app_id.clone());
        self.job_state = Some(job_state);
        self
    }

    /// Whether the service can drive real gate decisions (job wiring present).
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.job_state.is_some()
    }

    #[must_use]
    pub fn hub(&self) -> EventHubHandle {
        self.hub.clone()
    }

    // ── Sessions ───────────────────────────────────────────────────────────

    pub fn open_session(&self, claim: &PrincipalClaim) -> Result<Session, CoordinationError> {
        let principal = self.resolver.resolve(claim)?;
        let session = self.sessions.open(principal.clone());
        self.publisher
            .emit(SESSION_OPENED, &principal, json!({ "session_id": session.id }));
        Ok(session)
    }

    pub fn heartbeat(&self, session_id: Uuid) -> Result<Session, CoordinationError> {
        self.sessions.heartbeat(session_id)
    }

    pub fn close_session(&self, session_id: Uuid) -> Result<(), CoordinationError> {
        let session = self
            .sessions
            .close(session_id)
            .ok_or_else(|| CoordinationError::SessionNotFound(session_id.to_string()))?;
        for entry in self.presence.leave(session_id) {
            self.publisher.emit(
                PRESENCE_LEFT,
                &entry.principal,
                json!({ "session_id": session_id, "subject": entry.subject }),
            );
        }
        self.publisher.emit(
            SESSION_CLOSED,
            &session.principal,
            json!({ "session_id": session_id }),
        );
        Ok(())
    }

    pub fn list_sessions(&self, workspace_id: &str) -> Vec<Session> {
        self.sessions.active(workspace_id)
    }

    // ── Presence ───────────────────────────────────────────────────────────

    pub fn focus(
        &self,
        session_id: Uuid,
        claim: &PrincipalClaim,
        subject: SubjectRef,
    ) -> Result<PresenceEntry, CoordinationError> {
        let principal = self.resolver.resolve(claim)?;
        self.ensure_active(session_id)?;
        let (entry, change) = self.presence.focus(session_id, principal.clone(), subject);
        let event_type = match change {
            PresenceChange::Joined => PRESENCE_JOINED,
            PresenceChange::Updated => PRESENCE_FOCUS_CHANGED,
        };
        self.publisher.emit(
            event_type,
            &principal,
            json!({ "session_id": session_id, "subject": entry.subject, "claimed": entry.claimed }),
        );
        Ok(entry)
    }

    pub fn claim(
        &self,
        session_id: Uuid,
        claim: &PrincipalClaim,
        subject: SubjectRef,
    ) -> Result<PresenceEntry, CoordinationError> {
        let principal = self.resolver.resolve(claim)?;
        self.ensure_active(session_id)?;
        let (entry, _change) = self.presence.claim(session_id, principal.clone(), subject);
        self.publisher.emit(
            CLAIM_ACQUIRED,
            &principal,
            json!({ "session_id": session_id, "subject": entry.subject }),
        );
        Ok(entry)
    }

    pub fn release(
        &self,
        session_id: Uuid,
        claim: &PrincipalClaim,
        subject: SubjectRef,
    ) -> Result<Option<PresenceEntry>, CoordinationError> {
        let principal = self.resolver.resolve(claim)?;
        let entry = self.presence.release(session_id, &subject);
        if entry.is_some() {
            self.publisher.emit(
                CLAIM_RELEASED,
                &principal,
                json!({ "session_id": session_id, "subject": subject }),
            );
        }
        Ok(entry)
    }

    pub fn list_presence(
        &self,
        workspace_id: &str,
        subject: Option<&SubjectRef>,
    ) -> Vec<PresenceEntry> {
        self.presence.list(workspace_id, subject)
    }

    // ── Gate decisions (optimistic) ─────────────────────────────────────────

    /// Resolve, authorize, record (idempotent-or-conflict), and — only on a
    /// fresh accepted decision — signal the governed-job waiter.
    pub fn decide_gate(
        &self,
        ref_id: &str,
        claim: &PrincipalClaim,
        decision: GateDecisionKind,
        note: Option<String>,
    ) -> Result<DecisionOutcome, CoordinationError> {
        let principal = self.resolver.resolve(claim)?;
        let subject = SubjectRef::gate(ref_id);

        if !self.authority.can_decide(&principal, &subject) {
            self.publisher.emit(
                DECISION_DENIED,
                &principal,
                json!({ "ref_id": ref_id, "decision": decision }),
            );
            return Err(CoordinationError::AuthorityDenied {
                actor_id: principal.actor_id,
                subject: subject.to_string(),
            });
        }

        let outcome = self.ledger.record(ref_id, principal, decision, note);
        match &outcome {
            DecisionOutcome::Recorded(record) => {
                self.publisher.emit(
                    DECISION_RECORDED,
                    &record.principal,
                    json!({
                        "ref_id": ref_id,
                        "decision": record.decision,
                        "decision_id": record.decision_id,
                    }),
                );
                if let Some(job_state) = &self.job_state {
                    job_state.signal_gate(ref_id, decision.to_gate_decision());
                }
            }
            DecisionOutcome::Idempotent(_) => {
                // Optimistic dedup: identical decision, no second side-effect.
            }
            DecisionOutcome::Conflict {
                existing,
                attempted,
                attempted_by,
            } => {
                self.publisher.emit(
                    DECISION_CONFLICT,
                    attempted_by,
                    json!({
                        "ref_id": ref_id,
                        "existing_decision": existing.decision,
                        "existing_actor": existing.principal.actor_id,
                        "attempted": attempted,
                    }),
                );
            }
        }
        Ok(outcome)
    }

    // ── Stream classification (used by the SSE layer) ───────────────────────

    /// Whether an event belongs on a workspace's coordination stream.
    #[must_use]
    pub fn stream_includes(
        event_type: &str,
        event_workspace: Option<&str>,
        workspace_id: &str,
    ) -> bool {
        if events::is_coordination_type(event_type) {
            return event_workspace == Some(workspace_id);
        }
        events::is_job_type(event_type)
    }

    fn ensure_active(&self, session_id: Uuid) -> Result<(), CoordinationError> {
        if self.sessions.is_active(session_id) {
            Ok(())
        } else {
            Err(CoordinationError::SessionNotFound(session_id.to_string()))
        }
    }
}
