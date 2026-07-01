// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

use crate::budget::WallClockGuard;
use crate::director::{self, DomainPresenter, ProjectionInputs};
use crate::formation::{FormationOutput, SeedContext};
use crate::gate_surface::{GatedDecisionSurface, GatedDecisionView, PendingGateResponse};
use crate::ids::LoopId;
use crate::registry::{LoopEntry, LoopRegistry};
use crate::router::{RoutingDecision, SeverityRouter};
use crate::temperature::{PendingSubmission, TemperatureQueue, TemperatureSignal};
use director_contracts::DirectorSnapshot;
use helm_session_contracts::{
    gate::{GateId, GatedDecision},
    push::SessionPush,
    urgency::UrgencyIntent,
};
use uuid::Uuid;

/// Action that Client Helm instructs the native layer to execute.
#[derive(Debug)]
pub enum ClientHelmAction {
    /// B — spawn a new **local** formation. `loop_id` is already registered as a
    /// `Local` Running entry. (At most one local formation runs at a time.)
    SpawnFormation {
        loop_id: LoopId,
        seed_context: SeedContext,
    },
    /// B — a local formation is already running: ask the **server** to spawn a
    /// sub-formation with these seeds (the "DD job while I wait"). The native
    /// layer issues the server request; when the server returns an id it calls
    /// `server_formation_started` to register the tracked `ServerHandle`. This
    /// runs on the server, so it is NOT wall-clock budgeted on device.
    RequestServerFormation { seed_context: SeedContext },
    /// C — suspend the active formation; the native layer spawns a fresh
    /// formation seeded with its accumulated context + injected_context.
    /// Not a Converge Engine::resume.
    PauseAndInject {
        paused_id: LoopId,
        injected_context: SeedContext,
    },
    /// Surface a notification without interrupting the active formation.
    Notify {
        urgency: UrgencyIntent,
        message: String,
    },
    /// Nothing required from the native layer right now.
    NoAction,
}

/// Pending submission item — temperature signal or gate response.
#[derive(Debug)]
pub enum ClientSubmission {
    Temperature(PendingSubmission),
    GateResponse(PendingGateResponse),
}

/// The headless Client Helm coordinator.
///
/// Synchronous and pure — no network, no async, no Converge deps.
/// The native layer (iOS, Android, Desktop) owns the SSE connection,
/// feeds events in via `handle_push` / `handle_gate`, executes the
/// returned `ClientHelmAction`, and calls back with lifecycle events.
pub struct ClientHelm {
    registry: LoopRegistry,
    router: SeverityRouter,
    temperature_queue: TemperatureQueue,
    gate_surface: GatedDecisionSurface,
    gate_responses: Vec<PendingGateResponse>,
    budget: WallClockGuard,
    default_budget_ms: u64,
}

/// Default on-device formation wall-clock budget: 5 minutes.
/// Converge does not enforce `time_limit`; Client Helm bounds formations itself.
const DEFAULT_FORMATION_BUDGET_MS: u64 = 5 * 60 * 1_000;

impl ClientHelm {
    #[must_use]
    pub fn new() -> Self {
        Self::with_budget_ms(DEFAULT_FORMATION_BUDGET_MS)
    }

    /// Construct with an explicit per-formation wall-clock budget (ms).
    #[must_use]
    pub fn with_budget_ms(default_budget_ms: u64) -> Self {
        Self {
            registry: LoopRegistry::new(),
            router: SeverityRouter::new(),
            temperature_queue: TemperatureQueue::new(),
            gate_surface: GatedDecisionSurface::new(),
            gate_responses: Vec::new(),
            budget: WallClockGuard::new(),
            default_budget_ms,
        }
    }

    // ── Inbound events from native layer ──────────────────────────────────

    /// Call when an SSE SessionPush arrives from the server.
    #[must_use]
    pub fn handle_push(&mut self, push: SessionPush) -> ClientHelmAction {
        let active_id = self.registry.running_entry().map(|e| e.loop_id.clone());
        let started_at_ms = push.session_context.timestamp_ms;
        let seed = SeedContext {
            facts: vec![push.payload.clone()],
            description: push_objective_description(&push),
        };
        let decision = self
            .router
            .decide(push.urgency_intent, active_id.as_ref(), seed);
        let action = self.apply_routing_decision(decision);
        if let ClientHelmAction::SpawnFormation { loop_id, .. } = &action {
            self.budget
                .arm(loop_id, started_at_ms, self.default_budget_ms);
        }
        action
    }

    /// Call when a GatedDecision event arrives from the server.
    pub fn handle_gate(&mut self, gate: GatedDecision) {
        self.gate_surface.add_gate(gate);
    }

    /// Call when the native layer has started a spawned local formation.
    pub fn formation_started(&mut self, loop_id: &LoopId) {
        let _ = loop_id;
    }

    /// Call when the server has accepted an offloaded sub-formation and assigned an id.
    #[must_use]
    pub fn server_formation_started(
        &mut self,
        server_formation_id: String,
        formation_type: String,
        seed_context: SeedContext,
    ) -> LoopId {
        self.registry
            .spawn_server_handle(server_formation_id, formation_type, seed_context)
    }

    /// Call when a local formation completes. Queues temperature + proposals.
    pub fn formation_completed(
        &mut self,
        loop_id: &LoopId,
        output: FormationOutput,
        triggered_by: Option<helm_session_contracts::FindingId>,
    ) {
        let _ = self.registry.complete(loop_id, output.proposals.clone());
        self.budget.disarm(loop_id);
        if let Some(temp) = output.temperature {
            self.temperature_queue.enqueue(
                TemperatureSignal {
                    position: temp.position,
                    conviction: temp.conviction,
                    subject_ref: temp.subject_ref,
                },
                Uuid::new_v4().to_string(),
                triggered_by,
            );
        }
    }

    /// Call when the user responds to a gate. Removes gate from surface.
    pub fn respond_to_gate(&mut self, gate_id: &GateId, response: serde_json::Value) {
        if let Some(gate_response) = self.gate_surface.respond(gate_id, response) {
            self.gate_responses.push(gate_response);
        }
    }

    /// Periodic tick from the native layer. Returns loop ids whose wall-clock
    /// budget elapsed; those loops are marked `Failed`.
    #[must_use]
    pub fn tick(&mut self, now_ms: u64) -> Vec<LoopId> {
        let expired = self.budget.expired(now_ms);
        for id in &expired {
            let _ = self.registry.fail(id, "wall-clock budget exhausted".into());
        }
        expired
    }

    // ── State inspection for native UI ───────────────────────────────────

    /// Drain all pending submissions (temperature signals + gate responses).
    #[must_use]
    pub fn drain_submissions(&mut self) -> Vec<ClientSubmission> {
        let mut out = Vec::new();
        for t in self.temperature_queue.drain() {
            out.push(ClientSubmission::Temperature(t));
        }
        for g in self.gate_responses.drain(..) {
            out.push(ClientSubmission::GateResponse(g));
        }
        out
    }

    #[must_use]
    pub fn registry_state(&self) -> Vec<&LoopEntry> {
        self.registry.entries()
    }

    #[must_use]
    pub fn pending_gates(&self) -> Vec<GatedDecisionView> {
        self.gate_surface.gate_views()
    }

    /// Project current session/gate/loop state into a versioned `DirectorFrame`.
    #[must_use]
    pub fn director_snapshot(
        &self,
        version: u64,
        presenter: &dyn DomainPresenter,
    ) -> DirectorSnapshot {
        let running_intent = self.registry.running_entry().map(|e| &e.intent);
        let pending_gate = self.gate_surface.pending_gates().into_iter().next();
        director::project(
            version,
            ProjectionInputs {
                running_intent,
                pending_gate,
            },
            presenter,
        )
    }

    // ── Internal ─────────────────────────────────────────────────────────

    fn apply_routing_decision(&mut self, decision: RoutingDecision) -> ClientHelmAction {
        match decision {
            RoutingDecision::SpawnNew { seed_context } => {
                let loop_id = self
                    .registry
                    .spawn(seed_context.description.clone(), seed_context.clone());
                ClientHelmAction::SpawnFormation {
                    loop_id,
                    seed_context,
                }
            }
            RoutingDecision::OffloadToServer { seed_context } => {
                ClientHelmAction::RequestServerFormation { seed_context }
            }
            RoutingDecision::QueueAndNotify {
                urgency,
                seed_context: _,
            } => ClientHelmAction::Notify {
                urgency,
                message: format!("Server update: {urgency:?}"),
            },
            RoutingDecision::PauseAndInject {
                loop_id_to_pause,
                injected_context,
            } => {
                let _ = self
                    .registry
                    .pause(&loop_id_to_pause, injected_context.clone());
                ClientHelmAction::PauseAndInject {
                    paused_id: loop_id_to_pause,
                    injected_context,
                }
            }
        }
    }
}

/// Prefer domain copy from push payload; fall back to urgency label.
#[must_use]
pub fn push_objective_description(push: &SessionPush) -> String {
    push.payload
        .get("objective")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("server push: {:?}", push.urgency_intent))
}

impl Default for ClientHelm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod triggered_by_tests {
    use super::*;
    use crate::formation::{FormationOutput, TemperatureReading};
    use helm_session_contracts::finding::FindingId;
    use helm_session_contracts::push::{SessionContext, SessionPush};
    use helm_session_contracts::urgency::UrgencyIntent;

    fn preemptive_push(finding_id: FindingId) -> SessionPush {
        SessionPush {
            finding_id,
            urgency_intent: UrgencyIntent::Preemptive,
            payload: serde_json::json!({}),
            session_context: SessionContext {
                session_id: "sess-tb".into(),
                phase: "test".into(),
                cycle: 1,
                timestamp_ms: 1,
            },
        }
    }

    fn output_with_temperature() -> FormationOutput {
        FormationOutput {
            proposals: vec![],
            temperature: Some(TemperatureReading {
                position: "agree".into(),
                conviction: "high".into(),
                subject_ref: "quorum://hypothesis/h-tb".into(),
            }),
        }
    }

    #[test]
    fn triggered_by_round_trip_through_drain_submissions() {
        let mut helm = ClientHelm::new();
        let fid = FindingId::from_string("f-trigger");
        let push = preemptive_push(fid.clone());

        // With no running formation, Preemptive → SpawnFormation
        let action = helm.handle_push(push);
        let loop_id = match action {
            ClientHelmAction::SpawnFormation { loop_id, .. } => loop_id,
            other => panic!("expected SpawnFormation, got {other:?}"),
        };

        // Complete the formation with a temperature reading and triggered_by
        helm.formation_completed(&loop_id, output_with_temperature(), Some(fid.clone()));

        // drain_submissions must surface a Temperature submission with triggered_by set
        let submissions = helm.drain_submissions();
        assert_eq!(submissions.len(), 1);
        match &submissions[0] {
            ClientSubmission::Temperature(pending) => {
                assert_eq!(
                    pending.triggered_by.as_ref().map(|f| f.as_str()),
                    Some("f-trigger"),
                    "triggered_by should match the finding that triggered the formation"
                );
            }
            other => panic!("expected Temperature submission, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod push_description_tests {
    use super::push_objective_description;
    use helm_session_contracts::finding::FindingId;
    use helm_session_contracts::push::{SessionContext, SessionPush};
    use helm_session_contracts::urgency::UrgencyIntent;

    #[test]
    fn objective_from_payload_wins_over_urgency_fallback() {
        let push = SessionPush {
            finding_id: FindingId::from_string("find-1"),
            urgency_intent: UrgencyIntent::Advisory,
            payload: serde_json::json!({"objective": "LIVE: Track A push received"}),
            session_context: SessionContext {
                session_id: "sess".into(),
                phase: "decision".into(),
                cycle: 1,
                timestamp_ms: 1,
            },
        };
        assert_eq!(
            push_objective_description(&push),
            "LIVE: Track A push received"
        );
    }

    #[test]
    fn empty_objective_falls_back_to_urgency_label() {
        let push = SessionPush {
            finding_id: FindingId::from_string("find-1"),
            urgency_intent: UrgencyIntent::Advisory,
            payload: serde_json::json!({}),
            session_context: SessionContext {
                session_id: "sess".into(),
                phase: "decision".into(),
                cycle: 1,
                timestamp_ms: 1,
            },
        };
        assert_eq!(push_objective_description(&push), "server push: Advisory");
    }
}
