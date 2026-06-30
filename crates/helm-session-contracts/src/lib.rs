// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

pub mod finding;
pub mod gate;
pub mod push;
pub mod urgency;

pub use finding::{CoordinatorFinding, FindingId, FindingType};
pub use gate::{GateCondition, GateId, GatedDecision};
pub use push::{SessionContext, SessionPush};
pub use urgency::UrgencyIntent;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urgency_intent_round_trips() {
        let variants = [
            UrgencyIntent::Informational,
            UrgencyIntent::Advisory,
            UrgencyIntent::Disruptive,
            UrgencyIntent::Preemptive,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let back: UrgencyIntent = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{v:?}"), format!("{back:?}"));
        }
    }

    #[test]
    fn coordinator_finding_serializes_opaque_payload() {
        let finding = CoordinatorFinding {
            finding_id: FindingId::new(),
            finding_type: FindingType::HighConvictionDissent,
            payload: serde_json::json!({"hypothesis_id": "h-1", "dissent_count": 3}),
            urgency_intent: UrgencyIntent::Preemptive,
            requires_human: false,
            target_participants: vec!["alice".into(), "bob".into()],
        };
        let json = serde_json::to_string(&finding).unwrap();
        let back: CoordinatorFinding<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.urgency_intent, finding.urgency_intent);
        assert_eq!(back.target_participants.len(), 2);
    }

    #[test]
    fn gated_decision_with_no_deadline() {
        let gate = GatedDecision {
            gate_id: GateId::new(),
            condition: GateCondition::AnyParticipant,
            payload: serde_json::json!({}),
            deadline: None,
        };
        let json = serde_json::to_string(&gate).unwrap();
        let back: GatedDecision = serde_json::from_str(&json).unwrap();
        assert!(back.deadline.is_none());
    }

    #[test]
    fn gate_condition_quorum_of_roles_round_trips() {
        let cond = GateCondition::QuorumOfRoles {
            roles: vec!["facilitator".into(), "lead".into()],
        };
        let json = serde_json::to_string(&cond).unwrap();
        let back: GateCondition = serde_json::from_str(&json).unwrap();
        match back {
            GateCondition::QuorumOfRoles { roles } => assert_eq!(roles.len(), 2),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn session_push_preserves_opaque_payload() {
        let push = SessionPush {
            finding_id: FindingId::new(),
            urgency_intent: UrgencyIntent::Disruptive,
            payload: serde_json::json!({"msg": "contradiction detected"}),
            session_context: SessionContext {
                session_id: "sess-1".into(),
                phase: "hypothesis".into(),
                cycle: 3,
                timestamp_ms: 1_700_000_000_000,
            },
        };
        let json = serde_json::to_string(&push).unwrap();
        let back: SessionPush = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_context.cycle, 3);
        assert_eq!(back.urgency_intent, UrgencyIntent::Disruptive);
    }
}
