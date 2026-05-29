//! helm-truth-execution — Helm's truth dispatcher framework as a HelmModule.
//!
//! Truth bodies live with their consumer (Catalyst, atelier-showcase) and
//! register with this module at startup via [`TruthExecutionModule::register`].
//!
//! # Architecture
//!
//! ```text
//! Consumer (Catalyst / CRM showcase)
//!   └─ implements TruthBody  →  registers with TruthExecutionModule
//!
//! TruthExecutionModule (HelmModule)
//!   ├─ init():   logs registered truth count
//!   ├─ router(): mounts dispatcher routes
//!   └─ execute_truth(key, ctx):  registry lookup → body.execute(ctx)
//! ```
//!
//! # Phase 3b / 4b unblocking
//!
//! This crate is the Phase 5 extraction that `helm-governed-jobs` and
//! `helm-operator-control` were waiting for.  Phases 3b and 4b can now
//! consume `TruthBody` + `TruthExecutionContext` instead of depending on
//! `application-server`-internal types.
//!
//! # `KernelStore` generic resolution
//!
//! The original `execute_truth` was generic over `S: KernelStore`.  Because
//! `KernelStore` requires `Clone + Sized`, it is not dyn-compatible and cannot
//! be erased behind `Arc<dyn KernelStore>`.  The generic is resolved by using
//! the concrete `AppKernelStore` enum (which covers both in-memory and
//! SurrealDB variants) in [`dispatcher::TruthExecutionContext`].
//! Phases 3b/4b can revisit if a different concrete type is needed.

pub mod common;
pub mod dispatcher;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use axum::Router;
use runway_app_host::{HelmModule, HostContext};

pub use dispatcher::{
    RecordingObserver, RuntimeContext, TruthExecutionArtifacts, TruthProjection,
    domain_event_kind_name, execute_truth, run_engine_with_runtime, runtime_gate_request_id,
    status_from_converge, status_from_storage, supports_truth_execution,
};

// ── TruthBody trait ────────────────────────────────────────────────────────────

/// A single executable truth body.
///
/// Implement this trait for each truth (e.g. `score-inbound-fit`) and register
/// the implementation with [`TruthExecutionModule::register`] at application
/// startup.
///
/// # Generic plumbing
///
/// The original per-truth `execute` functions were generic over `S: KernelStore`.
/// That generic is resolved at the trait boundary by using the concrete
/// `AppKernelStore` enum in [`dispatcher::TruthExecutionContext`].  Truth
/// bodies access the store via `ctx.store`.
#[async_trait]
pub trait TruthBody: Send + Sync + 'static {
    /// The stable kebab-case key that identifies this truth (e.g. `"score-inbound-fit"`).
    fn key(&self) -> &'static str;

    /// Execute the truth body.  The dispatcher routes here based on [`Self::key`].
    async fn execute(
        &self,
        ctx: dispatcher::TruthExecutionContext,
    ) -> Result<TruthExecutionArtifacts, tonic::Status>;
}

// ── Registry ───────────────────────────────────────────────────────────────────

/// A mountable [`HelmModule`] that owns the truth-body registry and dispatcher.
///
/// Build with [`TruthExecutionModule::new`], chain [`TruthExecutionModule::register`]
/// calls for each truth body, then wrap in `Arc` before passing to the host builder.
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use helm_truth_execution::TruthExecutionModule;
///
/// let module = Arc::new(
///     TruthExecutionModule::new()
///         // .register(Arc::new(MyTruthBody))
/// );
/// ```
pub struct TruthExecutionModule {
    registry: RwLock<HashMap<&'static str, Arc<dyn TruthBody>>>,
}

impl TruthExecutionModule {
    pub fn new() -> Self {
        Self {
            registry: RwLock::new(HashMap::new()),
        }
    }

    /// Register a truth body.  Returns `self` for chaining.
    ///
    /// If two bodies share the same key the last registration wins.
    pub fn register(self, body: Arc<dyn TruthBody>) -> Self {
        self.registry
            .write()
            .expect("truth registry write lock poisoned")
            .insert(body.key(), body);
        self
    }

    /// Look up a registered body by key.
    pub fn lookup(&self, key: &str) -> Option<Arc<dyn TruthBody>> {
        self.registry
            .read()
            .expect("truth registry read lock poisoned")
            .get(key)
            .cloned()
    }

    /// Returns the number of registered truth bodies.
    pub fn registered_count(&self) -> usize {
        self.registry
            .read()
            .expect("truth registry read lock poisoned")
            .len()
    }
}

impl Default for TruthExecutionModule {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HelmModule for TruthExecutionModule {
    fn module_id(&self) -> &'static str {
        "helm.truth-execution"
    }

    async fn init(&self, _ctx: &HostContext) -> anyhow::Result<()> {
        let count = self.registered_count();
        tracing::info!(
            module = self.module_id(),
            registered_truths = count,
            "initialized"
        );
        Ok(())
    }

    /// The dispatcher does not mount its own Axum routes in Phase 5.
    ///
    /// The `/v1/truths/{key}/execute` HTTP surface belongs to `application-server`
    /// (via gRPC / its existing HTTP API) and is wired there using the original
    /// `execute_truth` call-site.  Phases 3b/4b will add an HTTP route here
    /// once the `HttpState<S>` extractor is decoupled from `application-server`.
    fn router(self: Arc<Self>) -> Router {
        Router::new()
    }
}
