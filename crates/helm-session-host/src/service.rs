// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! Session-host service — publishes wire types on the shared hub.

use director_contracts::DirectorIntent;
use director_contracts::DirectorSnapshot;
use helm_client::DomainPresenter;
use helm_session_contracts::{FindingId, GatedDecision, ParticipantId, SessionPush, UrgencyIntent};
use runway_app_host::{EventEnvelope, EventHubHandle};

use crate::events::{is_session_host_type, publish_gate, publish_push};
use crate::presenter::QuorumDomainPresenter;
use crate::store::SharedSessionStore;
use crate::types::SessionHostState;

/// Core session-host behavior: publish `SessionPush` envelopes and classify stream events.
pub struct SessionHostService {
    state: SessionHostState,
    store: SharedSessionStore,
}

impl SessionHostService {
    #[must_use]
    pub fn new(state: SessionHostState) -> Self {
        Self {
            state,
            store: SharedSessionStore::new(),
        }
    }

    #[must_use]
    pub fn from_hub(hub: EventHubHandle, app_id: impl Into<String>) -> Self {
        Self::new(SessionHostState::new(hub, app_id))
    }

    #[must_use]
    pub fn hub(&self) -> EventHubHandle {
        self.state.hub.clone()
    }

    #[must_use]
    pub fn app_id(&self) -> &str {
        &self.state.app_id
    }

    /// Emit a client-visible push on the shared hub and update live director state.
    #[must_use]
    pub fn publish_push(&self, push: SessionPush) -> u64 {
        let version = publish_push(&self.state.hub, &self.state.app_id, &push);
        if version > 0 {
            let _ = self.store.mutate(|store| store.apply_push(push, version));
        }
        version
    }

    /// Surface a HITL gate on the shared hub and update live director state.
    #[must_use]
    pub fn publish_gate(&self, session_id: &str, gate: GatedDecision) -> u64 {
        let version = publish_gate(&self.state.hub, &self.state.app_id, session_id, &gate);
        if version > 0 {
            let _ = self
                .store
                .mutate(|store| store.apply_gate(session_id, gate, version));
        }
        version
    }

    /// Publish a [`SessionPush`] to the shared hub and record delivery for each targeted
    /// participant (Disruptive and Preemptive only). Use this instead of `publish_push`
    /// when the coordinator knows which participants should receive and act on the push.
    #[must_use]
    pub fn publish_push_to(&self, push: SessionPush, participants: &[ParticipantId]) -> u64 {
        let version = publish_push(&self.state.hub, &self.state.app_id, &push);
        if version > 0 {
            if is_tracked_urgency(push.urgency_intent) {
                let _ = self.store.mutate(|store| {
                    for participant_id in participants {
                        store.record_delivery(
                            &push.session_context.session_id,
                            participant_id.clone(),
                            push.finding_id.clone(),
                            push.clone(),
                            version,
                        );
                    }
                });
            }
            let _ = self.store.mutate(|store| store.apply_push(push, version));
        }
        version
    }

    /// Re-publish a [`SessionPush`] for replay WITHOUT writing a new delivery record.
    /// Used at SSE subscribe time to re-deliver unacked Disruptive/Preemptive pushes.
    #[must_use]
    pub fn republish_push(&self, push: SessionPush) -> u64 {
        publish_push(&self.state.hub, &self.state.app_id, &push)
    }

    /// Mark delivery acked for a participant/finding pair.
    /// Returns `true` if the record exists, `false` if not found.
    #[must_use]
    pub fn apply_delivery_ack(
        &self,
        session_id: &str,
        participant_id: &ParticipantId,
        finding_id: &FindingId,
        now_ms: u64,
    ) -> bool {
        self.store
            .mutate(|store| {
                store.apply_delivery_ack(session_id, participant_id, finding_id, now_ms)
            })
            .unwrap_or(false)
    }

    /// Mark completion acked for a participant/finding pair.
    /// Returns `true` if the record exists, `false` if not found.
    #[must_use]
    pub fn apply_completion_ack(
        &self,
        session_id: &str,
        participant_id: &ParticipantId,
        finding_id: &FindingId,
        produced_output: bool,
        now_ms: u64,
    ) -> bool {
        self.store
            .mutate(|store| {
                store.apply_completion_ack(
                    session_id,
                    participant_id,
                    finding_id,
                    produced_output,
                    now_ms,
                )
            })
            .unwrap_or(false)
    }

    /// Return pushes that were delivered to `participant_id` at or before `max_version`
    /// but have not yet been delivery-acked. Used for pull-replay at subscribe time.
    #[must_use]
    pub fn unacked_for_replay(
        &self,
        session_id: &str,
        participant_id: &ParticipantId,
        max_version: u64,
    ) -> Vec<SessionPush> {
        self.store
            .with_store(|store| {
                store.unacked_pushes_for_replay(session_id, participant_id, max_version)
            })
            .unwrap_or_default()
    }

    /// Project live session state into a versioned [`DirectorSnapshot`].
    #[must_use]
    pub fn director_snapshot(
        &self,
        session_id: &str,
        presenter: &dyn DomainPresenter,
    ) -> Option<DirectorSnapshot> {
        self.store.with_store(|store| {
            let (helm, version) = store.helm_and_version(session_id)?;
            Some(helm.director_snapshot(version, presenter))
        })?
    }

    /// Project the most recently updated session, if any.
    #[must_use]
    pub fn director_snapshot_active(
        &self,
        presenter: &dyn DomainPresenter,
    ) -> Option<DirectorSnapshot> {
        self.store.with_store(|store| {
            let session_id = store.last_active_session()?;
            let (helm, version) = store.helm_and_version(session_id)?;
            Some(helm.director_snapshot(version, presenter))
        })?
    }

    /// Quorum-default projection using [`QuorumDomainPresenter`].
    #[must_use]
    pub fn quorum_director_snapshot(&self, session_id: &str) -> Option<DirectorSnapshot> {
        self.director_snapshot(session_id, &QuorumDomainPresenter)
    }

    /// Quorum-default projection for the active session.
    #[must_use]
    pub fn quorum_director_snapshot_active(&self) -> Option<DirectorSnapshot> {
        self.director_snapshot_active(&QuorumDomainPresenter)
    }

    /// Apply a typed director intent against live session mirror state.
    #[must_use]
    pub fn apply_director_intent(&self, session_id: &str, intent: &DirectorIntent) -> Option<u64> {
        self.store
            .mutate(|store| store.apply_director_intent(session_id, intent))
            .flatten()
    }

    /// Whether an envelope belongs on a session-scoped SSE stream.
    #[must_use]
    pub fn stream_includes(env: &EventEnvelope, session_id: &str) -> bool {
        if !is_session_host_type(&env.r#type) {
            return false;
        }
        env.payload
            .get("session_context")
            .and_then(|ctx| ctx.get("session_id"))
            .and_then(|value| value.as_str())
            == Some(session_id)
    }
}

fn is_tracked_urgency(urgency: UrgencyIntent) -> bool {
    matches!(
        urgency,
        UrgencyIntent::Disruptive | UrgencyIntent::Preemptive
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use director_contracts::DirectorPrompt;
    use helm_session_contracts::{FindingId, GateCondition, GateId, SessionContext, UrgencyIntent};

    fn sample_push(session_id: &str) -> SessionPush {
        SessionPush {
            finding_id: FindingId::from_string("find-1"),
            urgency_intent: UrgencyIntent::Advisory,
            payload: serde_json::json!({"note": "hello"}),
            session_context: SessionContext {
                session_id: session_id.to_string(),
                phase: "hypothesis".into(),
                cycle: 1,
                timestamp_ms: 1,
            },
        }
    }

    #[tokio::test]
    async fn publish_push_is_visible_on_matching_session_stream_filter() {
        let hub = runway_app_host::EventHub::with_capacity(8);
        let service = SessionHostService::from_hub(hub.handle(), "test.session-host");
        let _ = service.publish_push(sample_push("sess-a"));
        let _ = service.publish_push(sample_push("sess-b"));

        let sub = service
            .hub()
            .subscribe_with_cursor(runway_app_host::EventCursor::default())
            .await;
        let matching: Vec<_> = sub
            .replay
            .iter()
            .filter(|env| SessionHostService::stream_includes(env, "sess-a"))
            .collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].r#type, "session.push");
    }

    #[test]
    fn publish_push_updates_quorum_director_snapshot_version() {
        let hub = runway_app_host::EventHub::with_capacity(8);
        let service = SessionHostService::from_hub(hub.handle(), "test.session-host");
        let version = service.publish_push(SessionPush {
            finding_id: FindingId::from_string("find-live"),
            urgency_intent: UrgencyIntent::Advisory,
            payload: serde_json::json!({"objective": "Evaluate Vendor X's security claims"}),
            session_context: SessionContext {
                session_id: "procurement-security-review".into(),
                phase: "decision".into(),
                cycle: 3,
                timestamp_ms: 1,
            },
        });
        assert_eq!(version, 1);

        let snapshot = service
            .quorum_director_snapshot("procurement-security-review")
            .expect("live snapshot after push");
        assert_eq!(snapshot.version, 1);
        assert!(snapshot.frame.now.is_some());
        assert!(snapshot.frame.prompt.is_none());
    }

    #[test]
    fn publish_gate_takes_precedence_in_director_snapshot() {
        let hub = runway_app_host::EventHub::with_capacity(8);
        let service = SessionHostService::from_hub(hub.handle(), "test.session-host");
        let session_id = "procurement-security-review";
        let _ = service.publish_push(sample_push(session_id));
        let gate_version = service.publish_gate(
            session_id,
            GatedDecision {
                gate_id: GateId::from_string("gate:procurement-security-approval"),
                condition: GateCondition::AnyParticipant,
                payload: serde_json::json!({
                    "reason": "Legal approval required",
                    "consequence": "Formation cannot claim success until resolved"
                }),
                deadline: None,
            },
        );
        assert_eq!(gate_version, 2);

        let snapshot = service
            .quorum_director_snapshot(session_id)
            .expect("live snapshot after gate");
        assert_eq!(snapshot.version, 2);
        assert!(matches!(
            snapshot.frame.prompt,
            Some(DirectorPrompt::Gate(_))
        ));
    }
}
