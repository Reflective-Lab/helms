//! helm-governed-jobs — governed job stream as a mountable HelmModule.
//!
//! # Phase 4b — real implementation
//!
//! The `/v1/jobs/{key}/stream` SSE route now lives here.  The three blockers
//! from earlier phases have been resolved:
//!
//! 1. `crate::truth_runtime::execute_truth` → `helm_truth_execution::dispatcher::execute_truth`
//! 2. `crate::realtime::RealtimeHub` → local `hub` module (verbatim copy;
//!    `runway_app_host::EventHubHandle` lacks the sequence/replay semantics needed
//!    for per-run SSE delivery — full upstream merge is a future phase)
//! 3. `crate::http_api::HttpState<S>` → self-contained `JobStreamState`
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
//! empty truth registry and in-memory stores.  Routes built with this default
//! state will return `501 Not Implemented` for every truth key (no truths are
//! registered).  Use `GovernedJobsModule::with_state(state)` when real wiring
//! is needed.
//!
//! This preserves the zero-arg constructor contract that `quorum-server` and
//! `atlas-server` rely on.

#![allow(clippy::result_large_err)]

mod hub;
mod job_stream;

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use runway_app_host::{HelmModule, HostContext};

pub use job_stream::{GateDecision, JobGateWaiter, JobStreamState};

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
}

impl Default for GovernedJobsModule {
    fn default() -> Self {
        Self::new()
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
}
