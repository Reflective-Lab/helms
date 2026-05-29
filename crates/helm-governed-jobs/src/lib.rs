//! helm-governed-jobs — governed job stream as a mountable HelmModule.
//!
//! # Current status: SHELL — truth_runtime still blocks full stream extraction
//!
//! This crate exposes a stub `GovernedJobsModule` with a placeholder
//! `/v1/jobs/status` route. The real `/v1/jobs/{key}/stream` SSE route from
//! `application-server/src/job_stream.rs` cannot be moved here yet because it
//! has three hard dependencies on `application-server`-internal modules.
//!
//! # Re-extraction notes (Phase 3a)
//!
//! The original `feat/helm-governed-jobs` (committed before helms main `5f8d6b6`)
//! was already a shell for the same reason — `truth_runtime` was the blocker
//! then and remains so now. Re-running this extraction against current main does
//! NOT make the `job_stream.rs` route any easier to extract: the same three
//! blockers are present in the codebase, and no commits since the original
//! attempt have changed those dep relationships.
//!
//! This is a clean re-do on a fresh branch (`feat/helm-modules-rextract`)
//! replacing the stale shell with equivalent content anchored to main `5f8d6b6`.
//!
//! # Blockers for full stream extraction (Phase 4b)
//!
//! All three are transitive from `job_stream.rs` to `truth_runtime`:
//!
//! 1. **`crate::truth_runtime::{TruthExecutionArtifacts, execute_truth,
//!    supports_truth_execution}`** — the primary blocker. This is a large
//!    sub-module tree inside `application-server/src/truth_runtime/` that
//!    drives actual Converge job execution. It transitively depends on
//!    `converge-core`, `converge-kernel`, `converge-pack`, `organism-pack`,
//!    and the full Converge runtime. Moving it out of `application-server` is
//!    Phase 5 work.
//!
//! 2. **`crate::realtime::{RealtimeCursor, RealtimeEvent, RealtimeEventInput,
//!    RealtimeHub}`** — the legacy in-process hub used to publish SSE events
//!    during a job run. Replaceable with `runway_app_host::EventHubHandle` once
//!    `truth_runtime` moves, but not independently extractable because the hub
//!    is parametrised over `TruthExecutionArtifacts`.
//!
//! 3. **`crate::http_api::HttpState<S>`** — the Axum state wrapper used by the
//!    `stream_job` extractor. It embeds `AppRuntimeStores` (needed by
//!    `execute_truth`) and the job-gate `Mutex` map. Replaceable with a locally
//!    owned `JobStreamState<S>` once points 1 and 2 are resolved.
//!
//! # What will change in Phase 4b
//!
//! 1. Copy `job_stream.rs` into `src/job_stream.rs`.
//! 2. Replace `RealtimeHub` with `runway_app_host::EventHubHandle`.
//! 3. Replace `HttpState<S>` extractor with a locally owned `JobStreamState<S>`
//!    that receives the store via `init()` / `GovernedJobsModule::with_store`.
//! 4. Wire approval events via `ctx.realtime.subscribe()` in `init()`.
//! 5. Add `truth-catalog` and `application-storage` to `[dependencies]`.
//!
//! # Routes currently mounted (stub)
//!
//! - `GET /v1/jobs/status` — returns 503 with a human-readable deferral message.
//!
//! # Routes deferred to Phase 4b
//!
//! - `POST /v1/jobs/{key}/stream` — full SSE-streamed job execution with
//!   gate/approval support. Blocked on `truth_runtime` extraction.

#![allow(clippy::result_large_err)]

mod stub;

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use runway_app_host::{HelmModule, HostContext};

// ── State ─────────────────────────────────────────────────────────────────────

/// Minimal state held by `GovernedJobsModule`.
///
/// In Phase 4b this will be replaced by the full `JobStreamState` extracted
/// from `application-server/src/job_stream.rs`, which holds a gate-waiter map
/// and an `EventHubHandle` for publishing job lifecycle events.
#[derive(Clone, Default)]
pub struct GovernedJobsState {
    // Phase 4b: EventHubHandle for publishing job lifecycle events.
    // hub: Option<runway_app_host::EventHubHandle>,
}

// ── Module ────────────────────────────────────────────────────────────────────

/// A `HelmModule` stub for the governed job stream.
///
/// Currently mounts a `/v1/jobs/status` stub route that returns 503 with a
/// "not yet wired" body. The real `/v1/jobs/{key}/stream` SSE route will
/// replace it in Phase 4b once `truth_runtime` is extracted from
/// `application-server`.
pub struct GovernedJobsModule {
    state: Arc<GovernedJobsState>,
}

impl GovernedJobsModule {
    pub fn new() -> Self {
        Self {
            state: Arc::new(GovernedJobsState::default()),
        }
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
        // Phase 4b: store ctx.realtime handle and subscribe to approval.* events.
        tracing::info!(module = self.module_id(), "initialized (stub — Phase 4b pending)");
        Ok(())
    }

    fn router(self: Arc<Self>) -> Router {
        stub::router(self.state.clone())
    }
}
