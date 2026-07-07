//! helm-operator-control — Helm's operator-control surface as a mountable HelmModule.
//!
//! # Scope
//!
//! Wraps the operator-control HTTP routes under `/v1/workbench/operator-control/`
//! and the showcase pipeline routes under `/v1/pipeline/showcase/` into a HelmModule
//! for runway-app-host. Downstream apps should import operator-control packet
//! and ledger contracts from `helm-module-contracts`, not from this crate.
//!
//! # Routes exposed
//!
//! - `GET /v1/workbench/operator-control/preview` — first injected live preview
//! - `GET /v1/workbench/operator-control/previews` — injected live preview list
//!
//! Pipeline routes (mounted when truths are registered via `with_truths`):
//! - `POST /v1/pipeline/showcase/run` — run showcase pipeline
//! - `GET  /v1/pipeline/showcase/status` — get current pipeline status
//! - `POST /v1/pipeline/showcase/reset` — reset pipeline state
//!
//! # Re-extraction notes (Phase 3a / Phase 3b)
//!
//! Phase 3a re-extracted the operator-control routes against helms main `5f8d6b6`.
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
//!
//! # Vocabulary
//!
//! All operator-control receipt and preview types live in `helm-module-contracts`
//! (`operator_receipts` and `operator_preview` submodules). Consumers must import
//! from there directly; receipts/preview vocabulary is never re-exported from this
//! crate. Pipeline API types (`ShowcaseSeedSource`, `ShowcasePipelineInput`,
//! `SeedSourceError`) are re-exported from `pipeline` because they appear in its
//! own public API signatures (RFL-154 T5a seam cut).

#![allow(clippy::result_large_err)]

mod http_api;
pub mod pipeline;

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use helm_module_contracts::operator_preview::OperatorControlPreview;
use helm_module_contracts::operator_receipts::{
    JobReadinessPacket, OperatorControlError, OperatorLedgerEntry,
};
use helm_module_contracts::{
    HelmModule, HelmModuleReadiness, HelmModuleState, HelmModuleStatus, ModuleState,
};
use helm_truth_execution::TruthExecutionModule;

pub use http_api::{OperatorControlState, OperatorStateError};
pub use pipeline::PipelineRouteState;

// ── Module ────────────────────────────────────────────────────────────────────

/// A `HelmModule` that mounts the operator-control workbench routes and (optionally)
/// the showcase pipeline routes.
///
/// # Constructors
///
/// - [`OperatorControlModule::new`] — zero-arg default. Pipeline routes exist but
///   return "not implemented" because no truth bodies are registered.
/// - [`OperatorControlModule::with_truths`] — full constructor for callers that want
///   the pipeline to actually dispatch truths.
pub struct OperatorControlModule {
    state: Arc<OperatorControlState>,
    pipeline: Arc<PipelineRouteState>,
    live_evidence: Option<LiveReadinessEvidence>,
    readiness_feed: Option<Arc<dyn OperatorControlReadinessFeed>>,
}

const LIVE_REQUIREMENTS: [&str; 4] = [
    "process_receipt",
    "integrity_proof",
    "adapter_receipt",
    "axiom_report",
];
const READINESS_FEED_REQUIREMENT: &str = "readiness_feed";

/// App-supplied live operator-control packet and ledger chain.
///
/// Apps should build these snapshots from app-owned process receipts, integrity
/// proofs, adapter receipts, and Axiom reports. Helm stores and renders the
/// packet/ledger shape; it does not read app domain state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveOperatorControlSnapshot {
    pub packet: JobReadinessPacket,
    pub ledger_entries: Vec<OperatorLedgerEntry>,
}

impl LiveOperatorControlSnapshot {
    pub fn new(packet: JobReadinessPacket, ledger_entries: Vec<OperatorLedgerEntry>) -> Self {
        Self {
            packet,
            ledger_entries,
        }
    }
}

impl From<LiveOperatorControlSnapshot> for OperatorControlPreview {
    fn from(snapshot: LiveOperatorControlSnapshot) -> Self {
        OperatorControlPreview::live_app_feed(snapshot.packet, snapshot.ledger_entries)
    }
}

/// Provider contract for live operator-control readiness.
///
/// The provider is app-owned. Helm calls it to obtain already-derived
/// packet/ledger snapshots and the evidence-completeness marker needed for RR's
/// module liveness check. Implementations must not expose raw app transcripts,
/// entitlement state, deployment authority, or domain write authority.
pub trait OperatorControlReadinessFeed: Send + Sync + 'static {
    fn live_evidence(&self) -> LiveReadinessEvidence;

    fn previews(&self) -> Result<Vec<LiveOperatorControlSnapshot>, OperatorControlError>;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LiveReadinessEvidence {
    pub process_receipt: bool,
    pub integrity_proof: bool,
    pub adapter_receipt: bool,
    pub axiom_report: bool,
}

impl LiveReadinessEvidence {
    pub const fn complete() -> Self {
        Self {
            process_receipt: true,
            integrity_proof: true,
            adapter_receipt: true,
            axiom_report: true,
        }
    }

    pub const fn is_complete(self) -> bool {
        self.process_receipt && self.integrity_proof && self.adapter_receipt && self.axiom_report
    }

    fn missing_requirements(self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if !self.process_receipt {
            missing.push("process_receipt");
        }
        if !self.integrity_proof {
            missing.push("integrity_proof");
        }
        if !self.adapter_receipt {
            missing.push("adapter_receipt");
        }
        if !self.axiom_report {
            missing.push("axiom_report");
        }
        missing
    }
}

impl OperatorControlModule {
    /// Construct with a default state and an empty truth registry.
    ///
    /// Suitable for development, demos, and existing consumers that do not need
    /// pipeline truth dispatch. Pipeline routes will respond with `501 Not
    /// Implemented` for each truth key until bodies are registered.
    pub fn new() -> Self {
        Self {
            state: Arc::new(OperatorControlState::new()),
            pipeline: Arc::new(PipelineRouteState::new()),
            live_evidence: None,
            readiness_feed: None,
        }
    }

    /// Construct with a populated truth registry.
    ///
    /// Use this constructor when the caller has registered truth bodies (e.g.
    /// `score-inbound-fit`, `qualify-inbound-lead`, `schedule-strategic-meetings`)
    /// and wants the pipeline routes to actually dispatch through them.
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use helm_operator_control::OperatorControlModule;
    /// use helm_truth_execution::TruthExecutionModule;
    ///
    /// let truths = Arc::new(
    ///     TruthExecutionModule::new()
    ///         // .register(Arc::new(MyTruthBody))
    /// );
    /// let module = OperatorControlModule::with_truths(truths);
    /// ```
    pub fn with_truths(truths: Arc<TruthExecutionModule>) -> Self {
        Self {
            state: Arc::new(OperatorControlState::new()),
            pipeline: Arc::new(PipelineRouteState::with_truths(truths)),
            live_evidence: None,
            readiness_feed: None,
        }
    }

    /// Mark the operator-control module as backed by live app evidence.
    ///
    /// This is an explicit opt-in because registered truth bodies alone do not
    /// prove live app readiness. Apps must provide the four evidence signals
    /// before a Runtime Runway verifier can treat this module as live.
    pub fn with_live_readiness_evidence(mut self, evidence: LiveReadinessEvidence) -> Self {
        self.live_evidence = Some(evidence);
        self
    }

    /// Attach an app-owned live readiness feed.
    ///
    /// This is the H-01 contract apps should use to feed real
    /// `JobReadinessPacket` / `OperatorLedgerEntry` snapshots into
    /// `helm.operator-control`. The module reports `live` only when the feed
    /// evidence is complete and the feed returns at least one snapshot.
    pub fn with_live_readiness_feed(mut self, feed: Arc<dyn OperatorControlReadinessFeed>) -> Self {
        self.state = Arc::new((*self.state).clone().with_readiness_feed(feed.clone()));
        self.readiness_feed = Some(feed);
        self
    }

    pub fn module_state(&self) -> HelmModuleState {
        if self.current_live_evidence().is_complete() && self.readiness_feed_ready() {
            HelmModuleState::Live
        } else {
            HelmModuleState::ShellDefault
        }
    }

    pub fn readiness_status(&self) -> HelmModuleStatus {
        let registered_truths = self.pipeline.truths.registered_count();
        let missing = self.missing_live_requirements();
        let requirements = self.live_requirements();
        let state = self.module_state();
        let reason = match state {
            HelmModuleState::Live => "live app evidence is wired; readiness remains advisory only",
            HelmModuleState::ShellDefault => {
                "default/static operator-control shell; live app evidence is not fully wired"
            }
        };

        HelmModuleStatus::new(self.module_id(), state, reason)
            .with_registered_truths(registered_truths)
            .with_live_requirements(requirements)
            .with_missing_live_requirements(missing)
    }

    fn current_live_evidence(&self) -> LiveReadinessEvidence {
        self.readiness_feed.as_ref().map_or_else(
            || self.live_evidence.unwrap_or_default(),
            |feed| feed.live_evidence(),
        )
    }

    fn readiness_feed_ready(&self) -> bool {
        self.readiness_feed.as_ref().is_none_or(|feed| {
            feed.previews()
                .map(|previews| !previews.is_empty())
                .unwrap_or(false)
        })
    }

    fn live_requirements(&self) -> Vec<&'static str> {
        let mut requirements = LIVE_REQUIREMENTS.to_vec();
        if self.readiness_feed.is_some() {
            requirements.push(READINESS_FEED_REQUIREMENT);
        }
        requirements
    }

    fn missing_live_requirements(&self) -> Vec<&'static str> {
        let mut missing = self.current_live_evidence().missing_requirements();
        if self.readiness_feed.is_some() && !self.readiness_feed_ready() {
            missing.push(READINESS_FEED_REQUIREMENT);
        }
        missing
    }
}

impl Default for OperatorControlModule {
    fn default() -> Self {
        Self::new()
    }
}

impl HelmModuleReadiness for OperatorControlModule {
    fn module_state(&self) -> HelmModuleState {
        OperatorControlModule::module_state(self)
    }

    fn readiness_status(&self) -> HelmModuleStatus {
        OperatorControlModule::readiness_status(self)
    }
}

#[async_trait]
impl HelmModule for OperatorControlModule {
    fn module_id(&self) -> &'static str {
        "helm.operator-control"
    }

    async fn init(&self) -> anyhow::Result<()> {
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

    fn module_state(&self) -> ModuleState {
        match <Self as HelmModuleReadiness>::module_state(self) {
            HelmModuleState::ShellDefault => ModuleState::Shell,
            HelmModuleState::Live => ModuleState::Live,
        }
    }
}
