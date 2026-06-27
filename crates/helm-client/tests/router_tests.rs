use helm_client::formation::SeedContext;
use helm_client::ids::LoopId;
use helm_client::router::{RoutingDecision, SeverityRouter};
use helm_session_contracts::urgency::UrgencyIntent;

fn seed(desc: &str) -> SeedContext {
    SeedContext {
        facts: vec![],
        description: desc.into(),
    }
}

fn router() -> SeverityRouter {
    SeverityRouter::new()
}

#[test]
fn no_loop_informational_spawns_new() {
    let decision = router().decide(UrgencyIntent::Informational, None, seed("ctx"));
    assert!(matches!(decision, RoutingDecision::SpawnNew { .. }));
}

#[test]
fn no_loop_advisory_spawns_new() {
    let decision = router().decide(UrgencyIntent::Advisory, None, seed("ctx"));
    assert!(matches!(decision, RoutingDecision::SpawnNew { .. }));
}

#[test]
fn no_loop_disruptive_spawns_new() {
    let decision = router().decide(UrgencyIntent::Disruptive, None, seed("ctx"));
    assert!(matches!(decision, RoutingDecision::SpawnNew { .. }));
}

#[test]
fn no_loop_preemptive_spawns_new() {
    let decision = router().decide(UrgencyIntent::Preemptive, None, seed("ctx"));
    assert!(matches!(decision, RoutingDecision::SpawnNew { .. }));
}

#[test]
fn active_loop_informational_queues_and_notifies() {
    let id = LoopId::new();
    let decision = router().decide(UrgencyIntent::Informational, Some(&id), seed("ctx"));
    assert!(matches!(decision, RoutingDecision::QueueAndNotify { .. }));
}

#[test]
fn active_loop_advisory_queues_and_notifies() {
    let id = LoopId::new();
    let decision = router().decide(UrgencyIntent::Advisory, Some(&id), seed("ctx"));
    assert!(matches!(decision, RoutingDecision::QueueAndNotify { .. }));
}

#[test]
fn active_loop_disruptive_offloads_to_server() {
    let id = LoopId::new();
    let decision = router().decide(UrgencyIntent::Disruptive, Some(&id), seed("ctx"));
    assert!(matches!(decision, RoutingDecision::OffloadToServer { .. }));
}

#[test]
fn active_loop_preemptive_pauses_and_injects() {
    let id = LoopId::new();
    let decision = router().decide(UrgencyIntent::Preemptive, Some(&id), seed("ctx"));
    match decision {
        RoutingDecision::PauseAndInject { loop_id_to_pause, .. } => {
            assert_eq!(loop_id_to_pause.as_str(), id.as_str());
        }
        other => panic!("expected PauseAndInject, got {other:?}"),
    }
}

#[test]
fn preemptive_with_active_loop_carries_seed_context() {
    let id = LoopId::new();
    let ctx = seed("server contradiction context");
    let decision = router().decide(UrgencyIntent::Preemptive, Some(&id), ctx);
    match decision {
        RoutingDecision::PauseAndInject { injected_context, .. } => {
            assert_eq!(injected_context.description, "server contradiction context");
        }
        other => panic!("expected PauseAndInject, got {other:?}"),
    }
}
