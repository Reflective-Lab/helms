// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! `helm.session-host` as a mountable `HelmModule`.

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use helm_module_contracts::{HelmModuleReadiness, HelmModuleState, HelmModuleStatus};
use runway_app_host::{HelmModule, HostContext, ModuleState};

use crate::http;
use crate::service::SessionHostService;

const MODULE_ID: &str = "helm.session-host";

/// Server-side Session Helm — routes findings to participants via SSE.
pub struct SessionHostModule {
    service: Arc<SessionHostService>,
}

impl SessionHostModule {
    #[must_use]
    pub fn new(service: Arc<SessionHostService>) -> Self {
        Self { service }
    }

    #[must_use]
    pub fn service(&self) -> Arc<SessionHostService> {
        self.service.clone()
    }

    #[must_use]
    pub fn module_state(&self) -> HelmModuleState {
        HelmModuleState::Live
    }

    #[must_use]
    pub fn readiness_status(&self) -> HelmModuleStatus {
        HelmModuleStatus::new(
            MODULE_ID,
            self.module_state(),
            "session-host SSE push surface is wired to EventHubHandle",
        )
        .with_live_requirements(["event_hub"])
    }
}

impl HelmModuleReadiness for SessionHostModule {
    fn module_state(&self) -> HelmModuleState {
        SessionHostModule::module_state(self)
    }

    fn readiness_status(&self) -> HelmModuleStatus {
        SessionHostModule::readiness_status(self)
    }
}

#[async_trait]
impl HelmModule for SessionHostModule {
    fn module_id(&self) -> &'static str {
        MODULE_ID
    }

    async fn init(&self, _ctx: &HostContext) -> anyhow::Result<()> {
        tracing::info!(module = MODULE_ID, "initialized");
        Ok(())
    }

    fn router(self: Arc<Self>) -> Router {
        http::router(self.service.clone())
    }

    fn module_state(&self) -> ModuleState {
        ModuleState::Live
    }
}
