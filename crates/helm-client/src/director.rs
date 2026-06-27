// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use crate::formation::LocalFormationIntent;
use director_contracts::{
    BlockingState, ContextLevel, DirectorFrame, DirectorIntent, DirectorPrompt, DirectorSnapshot,
    GatePrompt, GateVerdict, NowTask, PrimaryAction, SecondaryAction, WaitingFor,
};
use helm_session_contracts::gate::GatedDecision;

/// Human copy for a gate, read by the app from the gate's opaque payload.
pub struct GateCopy {
    pub reason: String,
    pub consequence: String,
}

/// `helm-client` is domain-agnostic — session payloads are opaque. The per-app
/// FFI implements this to supply the human words for a frame. `helm-client` owns
/// the frame STRUCTURE and lifecycle; the app owns the WORDS.
pub trait DomainPresenter {
    fn now_task(&self, intent: &LocalFormationIntent) -> NowTask;
    fn gate_copy(&self, gate: &GatedDecision) -> GateCopy;
    fn idle_title(&self) -> String;
}

/// Plain inputs the projector reads from `ClientHelm` state, kept separate so
/// `project` is pure and unit-testable.
pub struct ProjectionInputs<'a> {
    pub running_intent: Option<&'a LocalFormationIntent>,
    pub pending_gate: Option<&'a GatedDecision>,
}

/// First projection: an unresolved HITL gate is the scene; else the running local
/// formation; else idle. Refined against real fixtures during mobile M3A.
#[must_use]
pub fn project(
    version: u64,
    inputs: ProjectionInputs<'_>,
    presenter: &dyn DomainPresenter,
) -> DirectorSnapshot {
    if let Some(gate) = inputs.pending_gate {
        let copy = presenter.gate_copy(gate);
        let frame = DirectorFrame {
            frame_id: gate.gate_id.as_str().to_string(),
            title: copy.reason.clone(),
            subtitle: None,
            now: None,
            waiting_for: WaitingFor::Server,
            primary: PrimaryAction {
                label: "Approve".into(),
                intent: DirectorIntent::RespondGate {
                    gate_id: gate.gate_id.clone(),
                    verdict: GateVerdict::Approve,
                },
            },
            secondary: vec![SecondaryAction {
                label: "Reject".into(),
                intent: DirectorIntent::RespondGate {
                    gate_id: gate.gate_id.clone(),
                    verdict: GateVerdict::Reject,
                },
            }],
            prompt: Some(DirectorPrompt::Gate(GatePrompt {
                gate_id: gate.gate_id.clone(),
                reason: copy.reason,
                consequence: copy.consequence,
                deadline_ms: gate.deadline,
                condition: gate.condition.clone(),
            })),
            presence: vec![],
            context_trail: vec![ContextLevel::Task],
            blocking: BlockingState::BlocksFormation,
        };
        return DirectorSnapshot { version, frame };
    }

    if let Some(intent) = inputs.running_intent {
        let now = presenter.now_task(intent);
        let frame = DirectorFrame {
            frame_id: "now".into(),
            title: now.objective.clone(),
            subtitle: None,
            now: Some(now),
            waiting_for: WaitingFor::Nobody,
            primary: PrimaryAction {
                label: "Open".into(),
                intent: DirectorIntent::OpenTask {
                    frame_id: "now".into(),
                },
            },
            secondary: vec![],
            prompt: None,
            presence: vec![],
            context_trail: vec![ContextLevel::Task],
            blocking: BlockingState::NotBlocking,
        };
        return DirectorSnapshot { version, frame };
    }

    DirectorSnapshot {
        version,
        frame: DirectorFrame {
            frame_id: "idle".into(),
            title: presenter.idle_title(),
            subtitle: None,
            now: None,
            waiting_for: WaitingFor::Nobody,
            primary: PrimaryAction {
                label: "Refresh".into(),
                intent: DirectorIntent::RequestContext {
                    level: ContextLevel::Session,
                },
            },
            secondary: vec![],
            prompt: None,
            presence: vec![],
            context_trail: vec![ContextLevel::Task],
            blocking: BlockingState::NotBlocking,
        },
    }
}
