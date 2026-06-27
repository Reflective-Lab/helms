//! `helm.coordination` as a mountable `HelmModule`.
//!
//! Mirrors the readiness/mount contract of `helm-governed-jobs`. The module is
//! `Live` only when it is wired to governed-jobs state (so it can drive real
//! gate decisions); otherwise it is a default shell.

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use helm_module_contracts::{HelmModuleReadiness, HelmModuleState, HelmModuleStatus};
use runway_app_host::{HelmModule, HostContext, ModuleState};

use crate::http;
use crate::service::CoordinationService;

const MODULE_ID: &str = "helm.coordination";
const LIVE_REQUIREMENT: &str = "job_state";

/// Mounts the operator-coordination surface under `/v1/coordination/`.
pub struct CoordinationModule {
    service: Arc<CoordinationService>,
}

impl CoordinationModule {
    /// Wrap an already-built coordination service.
    pub fn new(service: Arc<CoordinationService>) -> Self {
        Self { service }
    }

    #[must_use]
    pub fn service(&self) -> Arc<CoordinationService> {
        self.service.clone()
    }

    #[must_use]
    pub fn module_state(&self) -> HelmModuleState {
        if self.service.is_live() {
            HelmModuleState::Live
        } else {
            HelmModuleState::ShellDefault
        }
    }

    #[must_use]
    pub fn readiness_status(&self) -> HelmModuleStatus {
        let state = self.module_state();
        let (reason, missing): (&str, Vec<&str>) = match state {
            HelmModuleState::Live => (
                "coordination is wired to governed-jobs state for gate decisions",
                Vec::new(),
            ),
            HelmModuleState::ShellDefault => (
                "coordination shell; presence and sessions work but gate decisions are not wired to a job runner",
                vec![LIVE_REQUIREMENT],
            ),
        };
        HelmModuleStatus::new(MODULE_ID, state, reason)
            .with_live_requirements([LIVE_REQUIREMENT])
            .with_missing_live_requirements(missing)
    }
}

impl HelmModuleReadiness for CoordinationModule {
    fn module_state(&self) -> HelmModuleState {
        CoordinationModule::module_state(self)
    }

    fn readiness_status(&self) -> HelmModuleStatus {
        CoordinationModule::readiness_status(self)
    }
}

#[async_trait]
impl HelmModule for CoordinationModule {
    fn module_id(&self) -> &'static str {
        MODULE_ID
    }

    async fn init(&self, _ctx: &HostContext) -> anyhow::Result<()> {
        tracing::info!(
            module = MODULE_ID,
            live = self.service.is_live(),
            "initialized"
        );
        Ok(())
    }

    fn router(self: Arc<Self>) -> Router {
        http::router(self.service.clone())
    }

    fn module_state(&self) -> ModuleState {
        match <Self as HelmModuleReadiness>::module_state(self) {
            HelmModuleState::ShellDefault => ModuleState::Shell,
            HelmModuleState::Live => ModuleState::Live,
        }
    }
}
