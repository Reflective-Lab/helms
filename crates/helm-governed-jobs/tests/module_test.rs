use std::sync::Arc;

use helm_governed_jobs::{GovernedJobsModule, GovernedJobsModuleState};
use runway_app_host::{HelmModule, ModuleState};
use serde_json::json;

#[test]
fn module_id_is_stable() {
    let m = Arc::new(GovernedJobsModule::new());
    assert_eq!(m.module_id(), "helm.governed-jobs");
}

#[test]
fn default_and_new_produce_equivalent_modules() {
    let a = GovernedJobsModule::new();
    let b = GovernedJobsModule::default();
    assert_eq!(a.module_id(), b.module_id());
}

#[test]
fn module_exposes_stub_router() {
    let m = Arc::new(GovernedJobsModule::new());
    // Calling router() consumes the Arc — verify it doesn't panic.
    let _router = m.router();
}

#[test]
fn default_module_reports_shell_default() {
    let m = GovernedJobsModule::new();
    let status = m.readiness_status();

    assert_eq!(m.module_state(), GovernedJobsModuleState::ShellDefault);
    assert_eq!(
        <GovernedJobsModule as HelmModule>::module_state(&m),
        ModuleState::Shell
    );
    assert_eq!(status.state, GovernedJobsModuleState::ShellDefault);
    assert_eq!(status.registered_truths, Some(0));
    assert_eq!(status.missing_live_requirements, vec!["truth_registry"]);
}

#[test]
fn readiness_status_serializes_shell_default_for_rr_verifier() {
    let m = GovernedJobsModule::new();
    let value = serde_json::to_value(m.readiness_status()).expect("status serializes");

    assert_eq!(
        value,
        json!({
            "module_id": "helm.governed-jobs",
            "state": "shell-default",
            "reason": "default governed-jobs shell; no truth bodies are registered",
            "registered_truths": 0,
            "live_requirements": ["truth_registry"],
            "missing_live_requirements": ["truth_registry"]
        })
    );
}
