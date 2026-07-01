use director_contracts::{BlockingState, DirectorPrompt, NowTask};
use helm_client::client::{ClientHelm, ClientHelmAction};
use helm_client::director::{DomainPresenter, GateCopy};
use helm_client::formation::{FormationOutput, LocalFormationIntent, SeedContext};
use helm_session_contracts::{
    finding::FindingId,
    gate::{GateCondition, GateId, GatedDecision},
    push::{SessionContext, SessionPush},
    urgency::UrgencyIntent,
};

fn ctx() -> SessionContext {
    SessionContext {
        session_id: "sess-1".into(),
        phase: "hypothesis".into(),
        cycle: 1,
        timestamp_ms: 0,
    }
}

fn push(urgency: UrgencyIntent) -> SessionPush {
    SessionPush {
        finding_id: FindingId::new(),
        urgency_intent: urgency,
        payload: serde_json::json!({"msg": "test"}),
        session_context: ctx(),
    }
}

#[test]
fn first_push_spawns_formation() {
    let mut helm = ClientHelm::new();
    let action = helm.handle_push(push(UrgencyIntent::Informational));
    assert!(matches!(action, ClientHelmAction::SpawnFormation { .. }));
}

#[test]
fn preemptive_push_while_running_pauses_active() {
    let mut helm = ClientHelm::new();
    let spawn_action = helm.handle_push(push(UrgencyIntent::Informational));
    let loop_id = match spawn_action {
        ClientHelmAction::SpawnFormation { loop_id, .. } => loop_id,
        other => panic!("expected SpawnFormation, got {other:?}"),
    };
    helm.formation_started(&loop_id);
    let action = helm.handle_push(push(UrgencyIntent::Preemptive));
    match action {
        ClientHelmAction::PauseAndInject { paused_id, .. } => {
            assert_eq!(paused_id.as_str(), loop_id.as_str());
        }
        other => panic!("expected PauseAndInject, got {other:?}"),
    }
}

#[test]
fn disruptive_push_while_running_offloads_to_server() {
    let mut helm = ClientHelm::new();
    let spawn_action = helm.handle_push(push(UrgencyIntent::Informational));
    let loop_id = match spawn_action {
        ClientHelmAction::SpawnFormation { loop_id, .. } => loop_id,
        _ => panic!(),
    };
    helm.formation_started(&loop_id);
    let action = helm.handle_push(push(UrgencyIntent::Disruptive));
    assert!(matches!(
        action,
        ClientHelmAction::RequestServerFormation { .. }
    ));
}

#[test]
fn server_formation_started_records_handle_without_blocking_local() {
    let mut helm = ClientHelm::new();
    let spawn_action = helm.handle_push(push(UrgencyIntent::Informational));
    let local_id = match spawn_action {
        ClientHelmAction::SpawnFormation { loop_id, .. } => loop_id,
        _ => panic!(),
    };
    helm.formation_started(&local_id);
    let _ = helm.handle_push(push(UrgencyIntent::Disruptive));
    let handle_id = helm.server_formation_started(
        "srv-formation-1".into(),
        "dd-analysis".into(),
        SeedContext {
            facts: vec![],
            description: "dd".into(),
        },
    );
    assert_ne!(handle_id.as_str(), local_id.as_str());
    let expired = helm.tick(u64::MAX);
    assert!(!expired.iter().any(|id| id.as_str() == handle_id.as_str()));
}

#[test]
fn advisory_push_while_running_notifies() {
    let mut helm = ClientHelm::new();
    let spawn_action = helm.handle_push(push(UrgencyIntent::Informational));
    let loop_id = match spawn_action {
        ClientHelmAction::SpawnFormation { loop_id, .. } => loop_id,
        _ => panic!(),
    };
    helm.formation_started(&loop_id);
    let action = helm.handle_push(push(UrgencyIntent::Advisory));
    assert!(matches!(action, ClientHelmAction::Notify { .. }));
}

#[test]
fn formation_completed_queues_temperature_for_submission() {
    let mut helm = ClientHelm::new();
    let spawn_action = helm.handle_push(push(UrgencyIntent::Informational));
    let loop_id = match spawn_action {
        ClientHelmAction::SpawnFormation { loop_id, .. } => loop_id,
        _ => panic!(),
    };
    helm.formation_started(&loop_id);
    helm.formation_completed(
        &loop_id,
        FormationOutput {
            proposals: vec![serde_json::json!({"conclusion": "agree"})],
            temperature: Some(helm_client::formation::TemperatureReading {
                position: "agree".into(),
                conviction: "high".into(),
                subject_ref: "quorum://hypothesis/h-1".into(),
            }),
        },
        None,
    );
    let submissions = helm.drain_submissions();
    assert!(!submissions.is_empty());
}

#[test]
fn gate_surfaces_as_pending() {
    let mut helm = ClientHelm::new();
    let gate = GatedDecision {
        gate_id: GateId::new(),
        condition: GateCondition::AnyParticipant,
        payload: serde_json::json!({}),
        deadline: None,
    };
    helm.handle_gate(gate);
    assert_eq!(helm.pending_gates().len(), 1);
}

#[test]
fn gate_response_produces_pending_submission() {
    let mut helm = ClientHelm::new();
    let gate = GatedDecision {
        gate_id: GateId::new(),
        condition: GateCondition::AnyParticipant,
        payload: serde_json::json!({}),
        deadline: None,
    };
    let gate_id = gate.gate_id.clone();
    helm.handle_gate(gate);
    helm.respond_to_gate(&gate_id, serde_json::json!({"approved": true}));
    assert!(helm.pending_gates().is_empty());
    let submissions = helm.drain_submissions();
    assert!(!submissions.is_empty());
}

#[test]
fn formation_exceeding_wall_clock_budget_is_failed() {
    let mut helm = ClientHelm::with_budget_ms(1_000);
    let action = helm.handle_push(push(UrgencyIntent::Informational));
    let loop_id = match action {
        ClientHelmAction::SpawnFormation { loop_id, .. } => loop_id,
        other => panic!("expected SpawnFormation, got {other:?}"),
    };
    let expired = helm.tick(2_000);
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].as_str(), loop_id.as_str());
    assert!(helm.tick(9_999).is_empty());
}

#[test]
fn completed_formation_is_not_budget_failed() {
    let mut helm = ClientHelm::with_budget_ms(1_000);
    let action = helm.handle_push(push(UrgencyIntent::Informational));
    let loop_id = match action {
        ClientHelmAction::SpawnFormation { loop_id, .. } => loop_id,
        _ => panic!(),
    };
    helm.formation_completed(
        &loop_id,
        FormationOutput {
            proposals: vec![],
            temperature: None,
        },
        None,
    );
    assert!(helm.tick(u64::MAX).is_empty());
}

struct TestPresenter;

impl DomainPresenter for TestPresenter {
    fn now_task(&self, intent: &LocalFormationIntent) -> NowTask {
        NowTask {
            objective: intent.description.clone(),
            needed_from_user: None,
            estimated_minutes: Some(2),
        }
    }

    fn gate_copy(&self, _gate: &GatedDecision) -> GateCopy {
        GateCopy {
            reason: "Approve the revised wording".into(),
            consequence: "Formation stays blocked until resolved".into(),
        }
    }

    fn idle_title(&self) -> String {
        "Nothing needs you right now".into()
    }
}

#[test]
fn idle_helm_projects_idle_frame() {
    let helm = ClientHelm::new();
    let snap = helm.director_snapshot(7, &TestPresenter);
    assert_eq!(snap.version, 7);
    assert!(snap.frame.prompt.is_none());
    assert!(matches!(snap.frame.blocking, BlockingState::NotBlocking));
}

#[test]
fn pending_gate_becomes_the_scene() {
    let mut helm = ClientHelm::new();
    helm.handle_gate(GatedDecision {
        gate_id: GateId::from_string("g-1"),
        condition: GateCondition::AnyParticipant,
        payload: serde_json::json!({}),
        deadline: Some(1_700_000_000_000),
    });
    let snap = helm.director_snapshot(9, &TestPresenter);
    assert_eq!(snap.version, 9);
    assert!(matches!(snap.frame.prompt, Some(DirectorPrompt::Gate(_))));
    assert!(matches!(
        snap.frame.blocking,
        BlockingState::BlocksFormation
    ));
}
