use std::sync::Arc;

use application_storage::{AppConfig, InMemoryKernelStore};
use helm_operator_control::OperatorControlModule;
use runway_app_host::HelmModule;

fn test_config() -> AppConfig {
    AppConfig::default()
}

#[test]
fn module_id_is_stable() {
    let m: Arc<OperatorControlModule<InMemoryKernelStore>> =
        Arc::new(OperatorControlModule::new(test_config()));
    assert_eq!(m.module_id(), "helm.operator-control");
}

#[test]
fn module_exposes_router() {
    let m: Arc<OperatorControlModule<InMemoryKernelStore>> =
        Arc::new(OperatorControlModule::new(test_config()));
    // Calling router() consumes the Arc — just verify it doesn't panic.
    let _router = m.router();
}
