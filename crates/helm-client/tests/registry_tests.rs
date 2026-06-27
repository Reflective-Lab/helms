use helm_client::formation::SeedContext;
use helm_client::registry::{LoopKind, LoopRegistry, LoopState};

fn seed(desc: &str) -> SeedContext {
    SeedContext {
        facts: vec![],
        description: desc.into(),
    }
}

#[test]
fn new_registry_is_empty() {
    let r = LoopRegistry::new();
    assert!(r.running_entry().is_none());
    assert!(r.entries().is_empty());
}

#[test]
fn spawn_creates_running_entry() {
    let mut r = LoopRegistry::new();
    let id = r.spawn("personal-synthesis".into(), seed("think about hypothesis X"));
    assert!(r.running_entry().is_some());
    let entry = r.get(&id).unwrap();
    assert!(matches!(entry.state, LoopState::Running));
}

#[test]
fn at_most_one_running_at_a_time() {
    let mut r = LoopRegistry::new();
    let _ = r.spawn("synthesis".into(), seed("context a"));
    let result = r.try_spawn_sequential("synthesis".into(), seed("context b"));
    assert!(result.is_err());
}

#[test]
fn server_handle_does_not_block_local_slot() {
    let mut r = LoopRegistry::new();
    let _ = r.spawn("synthesis".into(), seed("primary"));
    let id2 = r.spawn_server_handle(
        "srv-formation-1".into(),
        "dd-analysis".into(),
        seed("dd context"),
    );
    assert!(r.get(&id2).is_some());
    assert_eq!(r.entries().len(), 2);
    let running = r.running_entry().expect("a local formation is running");
    assert!(matches!(running.kind, LoopKind::Local));
    assert!(r
        .try_spawn_sequential("synthesis".into(), seed("context b"))
        .is_err());
}

#[test]
fn server_handle_alone_has_no_local_running() {
    let mut r = LoopRegistry::new();
    let _ = r.spawn_server_handle(
        "srv-1".into(),
        "dd-analysis".into(),
        seed("dd context"),
    );
    assert!(r.running_entry().is_none());
    assert!(r
        .try_spawn_sequential("synthesis".into(), seed("local work"))
        .is_ok());
}

#[test]
fn pause_running_entry() {
    let mut r = LoopRegistry::new();
    let id = r.spawn("synthesis".into(), seed("initial"));
    r.pause(&id, seed("injected from server")).unwrap();
    let entry = r.get(&id).unwrap();
    assert!(matches!(entry.state, LoopState::Paused { .. }));
    assert!(r.running_entry().is_none());
}

#[test]
fn resume_paused_entry() {
    let mut r = LoopRegistry::new();
    let id = r.spawn("synthesis".into(), seed("initial"));
    r.pause(&id, seed("injected")).unwrap();
    r.resume(&id).unwrap();
    let entry = r.get(&id).unwrap();
    assert!(matches!(entry.state, LoopState::Running));
    assert!(r.running_entry().is_some());
}

#[test]
fn complete_entry_stays_in_registry() {
    let mut r = LoopRegistry::new();
    let id = r.spawn("synthesis".into(), seed("task"));
    r.complete(&id, vec![serde_json::json!({"result": "done"})])
        .unwrap();
    assert!(r.running_entry().is_none());
    let entry = r.get(&id).unwrap();
    assert!(matches!(entry.state, LoopState::Completed(_)));
    assert_eq!(r.entries().len(), 1);
}

#[test]
fn pause_nonexistent_loop_returns_error() {
    let mut r = LoopRegistry::new();
    let fake_id = helm_client::ids::LoopId::new();
    let result = r.pause(&fake_id, seed("ctx"));
    assert!(result.is_err());
}
