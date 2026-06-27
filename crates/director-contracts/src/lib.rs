// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

pub mod action;
pub mod context;
pub mod frame;
pub mod prompt;

pub use action::{DirectorIntent, GateVerdict, PrimaryAction, ReviewStance, SecondaryAction};
pub use context::{ContextLevel, PresenceHint};
pub use frame::{BlockingState, DirectorFrame, DirectorSnapshot, NowTask, WaitingFor};
pub use prompt::{Choice, DirectorPrompt, GatePrompt, JudgmentPrompt, ReviewPrompt};

#[cfg(test)]
mod tests {
    use super::*;
    use helm_session_contracts::gate::{GateCondition, GateId};

    fn gate_frame() -> DirectorFrame {
        DirectorFrame {
            frame_id: "f-1".into(),
            title: "Legal approval required".into(),
            subtitle: None,
            now: None,
            waiting_for: WaitingFor::Server,
            primary: PrimaryAction {
                label: "Approve".into(),
                intent: DirectorIntent::RespondGate {
                    gate_id: GateId::from_string("g-1"),
                    verdict: GateVerdict::Approve,
                },
            },
            secondary: vec![SecondaryAction {
                label: "Reject".into(),
                intent: DirectorIntent::RespondGate {
                    gate_id: GateId::from_string("g-1"),
                    verdict: GateVerdict::Reject,
                },
            }],
            prompt: Some(DirectorPrompt::Gate(GatePrompt {
                gate_id: GateId::from_string("g-1"),
                reason: "Approve revised liability wording".into(),
                consequence: "Formation cannot claim success until resolved".into(),
                deadline_ms: Some(1_700_000_000_000),
                condition: GateCondition::AnyParticipant,
            })),
            presence: vec![],
            context_trail: vec![ContextLevel::Task, ContextLevel::Session],
            blocking: BlockingState::BlocksFormation,
        }
    }

    #[test]
    fn director_snapshot_round_trips_and_keeps_version() {
        let snap = DirectorSnapshot { version: 42, frame: gate_frame() };
        let json = serde_json::to_string(&snap).unwrap();
        let back: DirectorSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, 42);
        assert!(matches!(back.frame.prompt, Some(DirectorPrompt::Gate(_))));
        assert!(matches!(back.frame.blocking, BlockingState::BlocksFormation));
    }

    #[test]
    fn gate_verdict_has_only_contract_backed_variants() {
        // Guards the "no UI-only verdict" rule at the type level.
        for v in [GateVerdict::Approve, GateVerdict::Reject] {
            let s = serde_json::to_string(&v).unwrap();
            let back: GateVerdict = serde_json::from_str(&s).unwrap();
            assert_eq!(v, back);
        }
        // "later"/"defer" is intentionally NOT a variant; adding it requires a
        // Helms gate-contract change first.
        assert!(serde_json::from_str::<GateVerdict>("\"later\"").is_err());
    }

    #[test]
    fn director_intent_round_trips() {
        let intent = DirectorIntent::RequestContext { level: ContextLevel::Formation };
        let json = serde_json::to_string(&intent).unwrap();
        let back: DirectorIntent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            back,
            DirectorIntent::RequestContext { level: ContextLevel::Formation }
        ));
    }
}
