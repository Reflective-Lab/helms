// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! In-memory session state mirrored into [`helm_client::ClientHelm`] for projection.

use std::collections::HashMap;
use std::sync::Mutex;

use director_contracts::{DirectorIntent, GateVerdict};
use helm_client::ClientHelm;
use helm_session_contracts::{FindingId, GatedDecision, ParticipantId, SessionPush};

use crate::delivery::DeliveryTable;

/// Per-session mirror of client-side coordination state.
#[derive(Default)]
struct SessionRecord {
    helm: ClientHelm,
    version: u64,
    live: bool,
}

impl SessionRecord {
    fn apply_push(&mut self, push: SessionPush, version: u64) {
        self.version = version;
        self.live = true;
        let _ = self.helm.handle_push(push);
    }

    fn apply_gate(&mut self, gate: GatedDecision, version: u64) {
        self.version = version;
        self.live = true;
        self.helm.handle_gate(gate);
    }
}

/// Tracks decision-session state for server-side director projection.
#[derive(Default)]
pub struct SessionStore {
    sessions: HashMap<String, SessionRecord>,
    last_active_session: Option<String>,
    delivery: DeliveryTable,
}

impl SessionStore {
    pub fn apply_push(&mut self, push: SessionPush, version: u64) {
        let session_id = push.session_context.session_id.clone();
        self.sessions
            .entry(session_id.clone())
            .or_default()
            .apply_push(push, version);
        self.last_active_session = Some(session_id);
    }

    pub fn apply_gate(&mut self, session_id: &str, gate: GatedDecision, version: u64) {
        self.sessions
            .entry(session_id.to_string())
            .or_default()
            .apply_gate(gate, version);
        self.last_active_session = Some(session_id.to_string());
    }

    /// Apply a typed director intent to the session mirror (dev / mobile submit path).
    pub fn apply_director_intent(
        &mut self,
        session_id: &str,
        intent: &DirectorIntent,
    ) -> Option<u64> {
        let record = self.sessions.get_mut(session_id)?;
        if !record.live {
            return None;
        }
        match intent {
            DirectorIntent::RespondGate { gate_id, verdict } => {
                let response = serde_json::json!({
                    "verdict": match verdict {
                        GateVerdict::Approve => "approve",
                        GateVerdict::Reject => "reject",
                    }
                });
                record.helm.respond_to_gate(gate_id, response);
                record.version = record.version.saturating_add(1);
                Some(record.version)
            }
            _ => None,
        }
    }

    pub fn helm_and_version(&self, session_id: &str) -> Option<(&ClientHelm, u64)> {
        self.sessions.get(session_id).and_then(|record| {
            if record.live {
                Some((&record.helm, record.version))
            } else {
                None
            }
        })
    }

    pub fn last_active_session(&self) -> Option<&str> {
        self.last_active_session.as_deref()
    }

    // ── Delivery tracking ────────────────────────────────────────────────

    pub fn record_delivery(
        &mut self,
        session_id: &str,
        participant_id: ParticipantId,
        finding_id: FindingId,
        push: SessionPush,
        version: u64,
    ) {
        self.delivery
            .record(session_id, participant_id, finding_id, push, version);
    }

    pub fn apply_delivery_ack(
        &mut self,
        session_id: &str,
        participant_id: &ParticipantId,
        finding_id: &FindingId,
        now_ms: u64,
    ) -> bool {
        self.delivery
            .ack_delivery(session_id, participant_id, finding_id, now_ms)
    }

    pub fn apply_completion_ack(
        &mut self,
        session_id: &str,
        participant_id: &ParticipantId,
        finding_id: &FindingId,
        produced_output: bool,
        now_ms: u64,
    ) -> bool {
        self.delivery
            .ack_completion(session_id, participant_id, finding_id, produced_output, now_ms)
    }

    pub fn unacked_pushes_for_replay(
        &self,
        session_id: &str,
        participant_id: &ParticipantId,
        max_version: u64,
    ) -> Vec<SessionPush> {
        self.delivery
            .unacked_for_replay(session_id, participant_id, max_version)
            .into_iter()
            .map(|(_, _, push)| push)
            .collect()
    }
}

/// Thread-safe wrapper around [`SessionStore`].
#[derive(Default)]
pub struct SharedSessionStore(Mutex<SessionStore>);

impl SharedSessionStore {
    #[must_use]
    pub fn new() -> Self {
        Self(Mutex::new(SessionStore::default()))
    }

    pub fn with_store<R>(&self, f: impl FnOnce(&SessionStore) -> R) -> Option<R> {
        let guard = self.0.lock().ok()?;
        Some(f(&guard))
    }

    pub fn mutate<R>(&self, f: impl FnOnce(&mut SessionStore) -> R) -> Option<R> {
        let mut guard = self.0.lock().ok()?;
        Some(f(&mut guard))
    }
}
