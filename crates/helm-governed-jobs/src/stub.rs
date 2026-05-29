//! Stub routes for Phase 3a.
//!
//! These will be replaced by the real job-stream routes in Phase 4b once
//! `truth_runtime` is extracted from `application-server`.
//!
//! See `lib.rs` for the full rationale.

use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;

use crate::GovernedJobsState;

pub fn router(_state: Arc<GovernedJobsState>) -> Router {
    Router::new().route("/v1/jobs/status", get(jobs_status_stub))
}

/// Temporary stub — returns 503 until Phase 4b wires the real stream route.
///
/// The real route is `POST /v1/jobs/{key}/stream`. It is not mounted here
/// because it depends on `truth_runtime::execute_truth` from
/// `application-server`, which has not been extracted yet (Phase 5 work).
async fn jobs_status_stub() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "helm-governed-jobs: /v1/jobs/{key}/stream not yet wired (Phase 4b pending — \
         requires truth_runtime extraction from application-server)",
    )
}
