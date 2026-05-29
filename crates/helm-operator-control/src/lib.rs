//! helm-operator-control — Helm's operator-control surface as a mountable HelmModule.
//!
//! # Scope
//!
//! Wraps the two operator-control HTTP routes under `/v1/workbench/operator-control/`
//! into a HelmModule for runway-app-host. Underlying packet types (`JobReadinessPacket`,
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
//! # Re-extraction notes (Phase 3a)
//!
//! The earlier extraction on `feat/helm-operator-control` (`42f67af`) predated 8+
//! commits on helms main that added the full portfolio of operator-control packets
//! (`quorum_adaptive_inquiry_packet`, `fathom_temporal_evidence_packet`,
//! `warden_compliance_packet`, `plumb_execution_drift_packet`, `atlas_integration_packet`)
//! and the `FuzzyReadinessTrace` / `FuzzyDefuzzifiedScore` types. This re-extraction
//! is against main `5f8d6b6` and picks up all of those additions via the
//! `workbench-backend` dep. The HTTP surface (2 routes) is unchanged; the response
//! bodies are richer.
//!
//! # What does NOT belong here
//!
//! - `pipeline.rs` — depends on `truth_runtime::execute_truth` (deferred to Phase 3b)
//! - `job_stream.rs` core run loop — same dep (deferred to Phase 4b, see helm-governed-jobs)
//! - Any `truth_runtime`-dependent code

#![allow(clippy::result_large_err)]

mod http_api;

use std::sync::Arc;

use application_storage::{AppConfig, InMemoryKernelStore, KernelStore};
use async_trait::async_trait;
use axum::Router;
use runway_app_host::{HelmModule, HostContext};

pub use http_api::OperatorControlState;

// Re-export types that downstream apps (Phase 8 Quorum, etc.) will consume
// without needing to depend on prio-agent-ops directly.
pub use prio_agent_ops::{
    AdapterReceiptStatus, EvidenceReadinessStatus, FuzzyDefuzzifiedScore, FuzzyMembership,
    FuzzyReadinessTrace, FuzzyRuleActivation, JobEvidenceStatus, JobReadinessPacket,
    JobReadinessPacketInput, JobVerdict, OperatorControlError, OperatorLedgerEntry,
    OperatorLedgerRecordKind, ReceiptFamily,
};

// ── Module ────────────────────────────────────────────────────────────────────

/// A `HelmModule` that mounts the operator-control workbench routes.
///
/// The generic parameter `S` is the `KernelStore` implementation. For most
/// Runway-hosted deployments this will be `InMemoryKernelStore` (the default)
/// or a remote-backed store wired up at startup via
/// [`OperatorControlModule::with_store`].
pub struct OperatorControlModule<S = InMemoryKernelStore> {
    state: Arc<OperatorControlState<S>>,
}

impl OperatorControlModule<InMemoryKernelStore> {
    /// Construct using the default in-memory kernel store (suitable for
    /// development, demos, and integration tests).
    pub fn new(config: AppConfig) -> Self {
        let store = InMemoryKernelStore::default_local();
        Self {
            state: Arc::new(OperatorControlState::new(config, store)),
        }
    }
}

impl<S> OperatorControlModule<S>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    /// Construct with an explicit store (for production use or custom test fixtures).
    pub fn with_store(config: AppConfig, store: S) -> Self {
        Self {
            state: Arc::new(OperatorControlState::new(config, store)),
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
        // No realtime hub wiring needed for the current operator-control routes —
        // they are read-only previews that do not publish SSE events.
        // When pipeline.rs is extracted (later phase), the hub from ctx.realtime
        // will be wired here.
        tracing::info!(module = self.module_id(), "initialized");
        Ok(())
    }

    fn router(self: Arc<Self>) -> Router {
        http_api::router(self.state.clone())
    }
}
