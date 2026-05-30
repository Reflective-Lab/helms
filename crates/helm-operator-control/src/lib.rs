//! helm-operator-control — Helm's operator-control surface as a mountable HelmModule.
//!
//! # Scope
//!
//! Wraps the operator-control HTTP routes under `/v1/workbench/operator-control/`
//! and the showcase pipeline routes under `/v1/pipeline/showcase/` into a HelmModule
//! for runway-app-host. Underlying packet types (`JobReadinessPacket`,
//! `OperatorControlPreview`, `FuzzyReadinessTrace`, and the full portfolio of
//! `*_packet()` constructors) stay in `prio-agent-ops` and `workbench-backend`
//! — this crate CONSUMES, never moves them.
//!
//! # Routes exposed
//!
//! - `GET /v1/workbench/operator-control/preview` — single preview (Tally escrow-release)
//! - `GET /v1/workbench/operator-control/previews` — portfolio preview list (6 packets:
//!   Tally, Quorum, Fathom, Warden, Plumb, Atlas)
//!
//! Pipeline routes (mounted when truths are registered via `with_truths`):
//! - `POST /v1/pipeline/showcase/run` — run showcase pipeline
//! - `GET  /v1/pipeline/showcase/status` — get current pipeline status
//! - `POST /v1/pipeline/showcase/reset` — reset pipeline state
//!
//! # Re-extraction notes (Phase 3a / Phase 3b)
//!
//! Phase 3a re-extracted the operator-control routes against helms main `5f8d6b6`,
//! picking up the full operator-control packet portfolio and `FuzzyReadinessTrace`.
//!
//! Phase 3b adds `pipeline.rs`: the showcase pipeline coordinator now lives here
//! instead of `application-server`. The `truth_runtime::execute_truth` dependency
//! is replaced by `helm_truth_execution::dispatcher::execute_truth` — truth bodies
//! are supplied via `OperatorControlModule::with_truths(...)`.
//!
//! # What does NOT belong here
//!
//! - `job_stream.rs` core run loop — deferred to Phase 4b (see helm-governed-jobs)
//! - SSE realtime streaming — coupled to application-server's RealtimeHub (Phase 4b)

#![allow(clippy::result_large_err)]

mod http_api;
pub mod pipeline;

use std::sync::Arc;

use application_storage::{AppConfig, InMemoryKernelStore, KernelStore};
use async_trait::async_trait;
use axum::Router;
use helm_truth_execution::TruthExecutionModule;
use runway_app_host::{HelmModule, HostContext};

pub use http_api::OperatorControlState;
pub use pipeline::PipelineRouteState;

// Re-export types that downstream apps (Phase 8 Quorum, etc.) will consume
// without needing to depend on prio-agent-ops directly.
pub use prio_agent_ops::{
    AdapterReceiptStatus, EvidenceReadinessStatus, FuzzyDefuzzifiedScore, FuzzyMembership,
    FuzzyReadinessTrace, FuzzyRuleActivation, JobEvidenceStatus, JobReadinessPacket,
    JobReadinessPacketInput, JobVerdict, OperatorControlError, OperatorLedgerEntry,
    OperatorLedgerRecordKind, ReceiptFamily,
};

// ── Module ────────────────────────────────────────────────────────────────────

/// A `HelmModule` that mounts the operator-control workbench routes and (optionally)
/// the showcase pipeline routes.
///
/// The generic parameter `S` is the `KernelStore` implementation. For most
/// Runway-hosted deployments this will be `InMemoryKernelStore` (the default)
/// or a remote-backed store wired up at startup via
/// [`OperatorControlModule::with_store`].
///
/// # Constructors
///
/// - [`OperatorControlModule::new`] — zero-arg default for existing consumers (e.g.
///   quorum-server). Pipeline routes exist but return "not implemented" because no
///   truth bodies are registered.
/// - [`OperatorControlModule::with_store`] — explicit store, still no truth registry.
/// - [`OperatorControlModule::with_truths`] — full constructor for callers that want
///   the pipeline to actually dispatch truths (e.g. atlas-integration, future apps).
pub struct OperatorControlModule<S = InMemoryKernelStore> {
    state: Arc<OperatorControlState<S>>,
    pipeline: Arc<PipelineRouteState>,
}

impl OperatorControlModule<InMemoryKernelStore> {
    /// Construct using the default in-memory kernel store and an empty truth registry.
    ///
    /// Suitable for development, demos, and existing consumers (e.g. quorum-server)
    /// that do not need pipeline truth dispatch. Pipeline routes will respond with
    /// `501 Not Implemented` for each truth key until bodies are registered.
    pub fn new(config: AppConfig) -> Self {
        let store = InMemoryKernelStore::default_local();
        Self {
            state: Arc::new(OperatorControlState::new(config, store)),
            pipeline: Arc::new(PipelineRouteState::new()),
        }
    }

    /// Construct with an in-memory store **and** a populated truth registry.
    ///
    /// Use this constructor when the caller has registered truth bodies (e.g.
    /// `score-inbound-fit`, `qualify-inbound-lead`, `schedule-strategic-meetings`)
    /// and wants the pipeline routes to actually dispatch through them.
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use application_storage::AppConfig;
    /// use helm_operator_control::OperatorControlModule;
    /// use helm_truth_execution::TruthExecutionModule;
    ///
    /// let truths = Arc::new(
    ///     TruthExecutionModule::new()
    ///         // .register(Arc::new(MyTruthBody))
    /// );
    /// let module = OperatorControlModule::with_truths(AppConfig::from_env(), truths);
    /// ```
    pub fn with_truths(config: AppConfig, truths: Arc<TruthExecutionModule>) -> Self {
        let store = InMemoryKernelStore::default_local();
        Self {
            state: Arc::new(OperatorControlState::new(config, store)),
            pipeline: Arc::new(PipelineRouteState::with_truths(truths)),
        }
    }
}

impl<S> OperatorControlModule<S>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    /// Construct with an explicit store and an empty truth registry.
    pub fn with_store(config: AppConfig, store: S) -> Self {
        Self {
            state: Arc::new(OperatorControlState::new(config, store)),
            pipeline: Arc::new(PipelineRouteState::new()),
        }
    }
}

#[async_trait]
impl<S> HelmModule for OperatorControlModule<S>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    fn module_id(&self) -> &'static str {
        "helm.operator-control"
    }

    async fn init(&self, _ctx: &HostContext) -> anyhow::Result<()> {
        let registered = self.pipeline.truths.registered_count();
        tracing::info!(
            module = self.module_id(),
            registered_truths = registered,
            "initialized"
        );
        Ok(())
    }

    fn router(self: Arc<Self>) -> Router {
        let operator_routes = http_api::router(self.state.clone());
        let pipeline_routes = pipeline::pipeline_router(self.pipeline.clone());
        operator_routes.merge(pipeline_routes)
    }
}
