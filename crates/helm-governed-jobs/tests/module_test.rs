use std::sync::Arc;

use helm_governed_jobs::GovernedJobsModule;
use runway_app_host::HelmModule;

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
