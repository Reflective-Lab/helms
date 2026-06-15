//! helm-governed-jobs — governed job stream as a mountable HelmModule.
//!
//! # Phase 4b redo — uses runway_app_host::EventHubHandle
//!
//! The `/v1/jobs/{key}/stream` SSE route lives here. Phase 1.6 landed the
//! missing pieces on runway-app-host (replay buffer + cursor subscribe +
//! `EventEnvelope.job_id`), so there is no local hub copy. No `hub.rs`.
//!
//! # Routes mounted
//!
//! - `POST /v1/jobs/{key}/stream` — SSE-streamed job execution with full HITL
//!   gate support (pre-gate execute → gate.paused → oneshot waiter →
//!   post-gate execute → job.completed).
//!
//! # Zero-arg constructor
//!
//! `GovernedJobsModule::new()` constructs a default `JobStreamState` with an
//! empty truth registry and a freestanding in-memory `EventHub`. Routes built
//! with this default state will return `501 Not Implemented` for every truth
//! key (no truths are registered). Use `GovernedJobsModule::with_state(state)`
//! when real wiring is needed.
//!
//! This preserves the zero-arg constructor contract that `quorum-server` and
//! `atlas-server` rely on.

#![allow(clippy::result_large_err)]

mod job_stream;

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use helm_module_contracts::{HelmModuleReadiness, HelmModuleState, HelmModuleStatus};
use runway_app_host::{HelmModule, HostContext, ModuleState};

pub use helm_module_contracts::{
    HelmModuleReadiness as GovernedJobsModuleReadiness, HelmModuleState as GovernedJobsModuleState,
    HelmModuleStatus as GovernedJobsModuleStatus,
};
pub use job_stream::{GateDecision, JobGateWaiter, JobRunTask, JobStreamState, run_job_task};

// ── Module ────────────────────────────────────────────────────────────────────

/// A `HelmModule` that mounts the governed job stream.
///
/// Build with `GovernedJobsModule::new()` for zero-arg default (501 for all
/// truth keys) or `GovernedJobsModule::with_state(state)` for real wiring.
pub struct GovernedJobsModule {
    state: Arc<JobStreamState>,
}

impl GovernedJobsModule {
    /// Zero-arg constructor — preserves the contract for quorum-server and
    /// atlas-server.  Routes return 501 until truth bodies are registered.
    pub fn new() -> Self {
        Self {
            state: Arc::new(JobStreamState::default()),
        }
    }

    /// Constructor for callers that want real job execution wiring.
    pub fn with_state(state: JobStreamState) -> Self {
        Self {
            state: Arc::new(state),
        }
    }

    /// Expose the inner state for callers that need gate-waiter access
    /// (e.g. operator-control approval handler).
    pub fn state(&self) -> Arc<JobStreamState> {
        self.state.clone()
    }

    pub fn module_state(&self) -> HelmModuleState {
        if self.state.truths.registered_count() == 0 {
            HelmModuleState::ShellDefault
        } else {
            HelmModuleState::Live
        }
    }

    pub fn readiness_status(&self) -> HelmModuleStatus {
        let registered_truths = self.state.truths.registered_count();
        let state = self.module_state();
        let reason = match state {
            HelmModuleState::Live => "truth registry is populated for governed job execution",
            HelmModuleState::ShellDefault => {
                "default governed-jobs shell; no truth bodies are registered"
            }
        };
        let missing = if registered_truths == 0 {
            vec!["truth_registry"]
        } else {
            Vec::new()
        };

        HelmModuleStatus::new(self.module_id(), state, reason)
            .with_registered_truths(registered_truths)
            .with_live_requirements(["truth_registry"])
            .with_missing_live_requirements(missing)
    }
}

impl Default for GovernedJobsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl HelmModuleReadiness for GovernedJobsModule {
    fn module_state(&self) -> HelmModuleState {
        GovernedJobsModule::module_state(self)
    }

    fn readiness_status(&self) -> HelmModuleStatus {
        GovernedJobsModule::readiness_status(self)
    }
}

#[async_trait]
impl HelmModule for GovernedJobsModule {
    fn module_id(&self) -> &'static str {
        "helm.governed-jobs"
    }

    async fn init(&self, _ctx: &HostContext) -> anyhow::Result<()> {
        let registered = self.state.truths.registered_count();
        tracing::info!(
            module = self.module_id(),
            registered_truths = registered,
            "initialized"
        );
        Ok(())
    }

    fn router(self: Arc<Self>) -> Router {
        job_stream::router(self.state.clone())
    }

    fn module_state(&self) -> ModuleState {
        match <Self as HelmModuleReadiness>::module_state(self) {
            HelmModuleState::ShellDefault => ModuleState::Shell,
            HelmModuleState::Live => ModuleState::Live,
        }
    }
}
