// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use helm_session_contracts::gate::{GateId, GatedDecision};
use std::collections::HashMap;

/// A pending gate response ready for submission to the server admission boundary.
#[derive(Debug, Clone)]
pub struct PendingGateResponse {
    pub gate_id: String,
    pub response: serde_json::Value,
}

/// Read-only view of a pending gate for FFI / UI.
#[derive(Debug, Clone)]
pub struct GatedDecisionView {
    pub gate_id: String,
    pub condition_label: String,
    pub deadline_ms: Option<u64>,
}

/// Tracks HITL gate events received from the server.
///
/// Gates are held until the user responds. The response is submitted
/// by the native layer via the server admission boundary.
pub struct GatedDecisionSurface {
    pending: HashMap<String, GatedDecision>,
}

impl GatedDecisionSurface {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Add a gate received from the server.
    pub fn add_gate(&mut self, gate: GatedDecision) {
        self.pending.insert(gate.gate_id.as_str().to_string(), gate);
    }

    /// Record a user response. Returns the pending response or None if gate unknown.
    pub fn respond(
        &mut self,
        gate_id: &GateId,
        response: serde_json::Value,
    ) -> Option<PendingGateResponse> {
        self.pending
            .remove(gate_id.as_str())
            .map(|_| PendingGateResponse {
                gate_id: gate_id.as_str().to_string(),
                response,
            })
    }

    /// All gates awaiting user response.
    #[must_use]
    pub fn pending_gates(&self) -> Vec<&GatedDecision> {
        self.pending.values().collect()
    }

    /// Read-only views for FFI / UI.
    #[must_use]
    pub fn gate_views(&self) -> Vec<GatedDecisionView> {
        use helm_session_contracts::gate::GateCondition;
        self.pending
            .values()
            .map(|g| GatedDecisionView {
                gate_id: g.gate_id.as_str().to_string(),
                condition_label: match &g.condition {
                    GateCondition::QuorumOfRoles { roles } => {
                        format!("quorum of: {}", roles.join(", "))
                    }
                    GateCondition::SpecificAuthority { actor_id } => {
                        format!("authority: {actor_id}")
                    }
                    GateCondition::AnyParticipant => "any participant".into(),
                    GateCondition::Unanimous => "unanimous".into(),
                },
                deadline_ms: g.deadline,
            })
            .collect()
    }

    /// True if the gate's deadline has passed at the given timestamp (ms).
    #[must_use]
    pub fn is_deadline_expired(&self, gate: &GatedDecision, now_ms: u64) -> bool {
        gate.deadline.is_some_and(|deadline| now_ms > deadline)
    }
}

impl Default for GatedDecisionSurface {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helm_session_contracts::gate::{GateCondition, GateId, GatedDecision};

    fn make_gate(deadline: Option<u64>) -> GatedDecision {
        GatedDecision {
            gate_id: GateId::new(),
            condition: GateCondition::AnyParticipant,
            payload: serde_json::json!({}),
            deadline,
        }
    }

    #[test]
    fn add_and_list_pending_gates() {
        let mut s = GatedDecisionSurface::new();
        s.add_gate(make_gate(None));
        assert_eq!(s.pending_gates().len(), 1);
    }

    #[test]
    fn respond_to_gate_removes_it() {
        let mut s = GatedDecisionSurface::new();
        let gate = make_gate(None);
        let gate_id = gate.gate_id.clone();
        s.add_gate(gate);
        let response = s.respond(&gate_id, serde_json::json!({"approved": true}));
        assert!(response.is_some());
        assert!(s.pending_gates().is_empty());
    }

    #[test]
    fn respond_to_unknown_gate_returns_none() {
        let mut s = GatedDecisionSurface::new();
        let result = s.respond(&GateId::new(), serde_json::json!({}));
        assert!(result.is_none());
    }

    #[test]
    fn deadline_not_expired_when_now_is_before() {
        let s = GatedDecisionSurface::new();
        let gate = make_gate(Some(2_000_000_000_000));
        assert!(!s.is_deadline_expired(&gate, 1_000_000_000_000));
    }

    #[test]
    fn deadline_expired_when_now_is_after() {
        let s = GatedDecisionSurface::new();
        let gate = make_gate(Some(1_000_000_000_000));
        assert!(s.is_deadline_expired(&gate, 2_000_000_000_000));
    }

    #[test]
    fn no_deadline_never_expires() {
        let s = GatedDecisionSurface::new();
        let gate = make_gate(None);
        assert!(!s.is_deadline_expired(&gate, u64::MAX));
    }
}
