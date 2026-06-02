use std::sync::Arc;

use async_trait::async_trait;
use helm_truth_execution::{TruthBody, TruthExecutionArtifacts, TruthExecutionModule};
use runway_app_host::HelmModule;

// ── Stub truth body ────────────────────────────────────────────────────────────

struct StubTruth;

#[async_trait]
impl TruthBody for StubTruth {
    fn key(&self) -> &'static str {
        "test.stub"
    }

    async fn execute(
        &self,
        _ctx: helm_truth_execution::dispatcher::TruthExecutionContext,
    ) -> Result<TruthExecutionArtifacts, tonic::Status> {
        // Construct a minimal artifact.  ConvergeResult has no Default impl
        // in the test environment, so we verify the registry path separately
        // in `registered_truth_is_dispatchable` without executing.
        Err(tonic::Status::unimplemented(
            "stub — not callable in unit tests",
        ))
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[test]
fn module_id_is_stable() {
    let m = TruthExecutionModule::new();
    assert_eq!(m.module_id(), "helm.truth-execution");
}

#[test]
fn empty_module_has_zero_registered_truths() {
    let m = TruthExecutionModule::new();
    assert_eq!(m.registered_count(), 0);
}

#[test]
fn registered_truth_is_found_by_key() {
    let m = TruthExecutionModule::new().register(Arc::new(StubTruth));
    assert!(m.lookup("test.stub").is_some());
}

#[test]
fn unregistered_key_returns_none() {
    let m = TruthExecutionModule::new().register(Arc::new(StubTruth));
    assert!(m.lookup("nonexistent").is_none());
}

#[test]
fn registered_count_reflects_registrations() {
    let m = TruthExecutionModule::new().register(Arc::new(StubTruth));
    assert_eq!(m.registered_count(), 1);
}

#[test]
fn supports_truth_execution_matches_registry() {
    let m = TruthExecutionModule::new().register(Arc::new(StubTruth));
    assert!(helm_truth_execution::supports_truth_execution(
        &m,
        "test.stub"
    ));
    assert!(!helm_truth_execution::supports_truth_execution(
        &m,
        "score-inbound-fit"
    ));
}
