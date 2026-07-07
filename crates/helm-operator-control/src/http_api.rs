//! HTTP routes for the operator-control workbench surface.
//!
//! Extracted from `application-server/src/http_api.rs`.
//! Only routes under `/v1/workbench/operator-control/` are included here;
//! all other workbench and CRM routes remain in application-server until a
//! later phase.

use std::fmt;
use std::sync::Arc;

use crate::OperatorControlReadinessFeed;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use helm_module_contracts::operator_preview::OperatorControlPreview;
use helm_module_contracts::operator_receipts::OperatorControlError;
use serde::Serialize;

// ── State ────────────────────────────────────────────────────────────────────

/// Focused state struct for operator-control routes.
///
/// Holds only what these routes need: an optional live readiness feed.
/// `OperatorApp<S>` (previously held here) was dead weight — the preview
/// methods implement live-feed logic directly (RFL-154 T5a).
#[derive(Clone)]
pub struct OperatorControlState {
    readiness_feed: Option<Arc<dyn OperatorControlReadinessFeed>>,
}

impl OperatorControlState {
    pub fn new() -> Self {
        Self {
            readiness_feed: None,
        }
    }

    pub fn with_readiness_feed(mut self, feed: Arc<dyn OperatorControlReadinessFeed>) -> Self {
        self.readiness_feed = Some(feed);
        self
    }

    pub fn operator_control_preview(&self) -> Result<OperatorControlPreview, OperatorStateError> {
        let mut previews = self.live_previews()?;
        if !previews.is_empty() {
            return Ok(previews.remove(0));
        }

        Err(OperatorStateError::NotAvailable)
    }

    pub fn operator_control_previews(
        &self,
    ) -> Result<Vec<OperatorControlPreview>, OperatorStateError> {
        self.live_previews()
    }

    fn live_previews(&self) -> Result<Vec<OperatorControlPreview>, OperatorStateError> {
        let Some(feed) = &self.readiness_feed else {
            return Ok(Vec::new());
        };

        let snapshots = feed.previews().map_err(OperatorStateError::Feed)?;
        Ok(snapshots.into_iter().map(Into::into).collect())
    }
}

impl Default for OperatorControlState {
    fn default() -> Self {
        Self::new()
    }
}

// ── State-layer error ─────────────────────────────────────────────────────────

/// Error returned by [`OperatorControlState`] preview methods.
///
/// - [`OperatorStateError::Feed`] wraps a validation error propagated from the
///   live readiness feed's [`OperatorControlReadinessFeed::previews`] call.
/// - [`OperatorStateError::NotAvailable`] signals that no live preview is
///   available from the configured feed (feed absent or returned empty).
#[derive(Debug)]
pub enum OperatorStateError {
    Feed(OperatorControlError),
    NotAvailable,
}

impl fmt::Display for OperatorStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Feed(e) => e.fmt(f),
            Self::NotAvailable => {
                f.write_str("operator-control preview requires an injected live readiness feed")
            }
        }
    }
}

impl std::error::Error for OperatorStateError {}

// ── Router ───────────────────────────────────────────────────────────────────

/// Returns the Axum router for operator-control routes.
///
/// Paths exposed:
/// - `GET /v1/workbench/operator-control/preview`  — first injected live preview
/// - `GET /v1/workbench/operator-control/previews` — injected live preview list
pub fn router(state: Arc<OperatorControlState>) -> Router {
    Router::new()
        .route(
            "/v1/workbench/operator-control/preview",
            get(workbench_operator_control_preview),
        )
        .route(
            "/v1/workbench/operator-control/previews",
            get(workbench_operator_control_previews),
        )
        .with_state(state)
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn workbench_operator_control_preview(
    State(state): State<Arc<OperatorControlState>>,
) -> Result<Json<OperatorControlPreview>, ApiError> {
    state
        .operator_control_preview()
        .map(Json)
        .map_err(api_error_from_operator_state)
}

async fn workbench_operator_control_previews(
    State(state): State<Arc<OperatorControlState>>,
) -> Result<Json<Vec<OperatorControlPreview>>, ApiError> {
    state
        .operator_control_previews()
        .map(Json)
        .map_err(api_error_from_operator_state)
}

// ── Error handling ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ErrorPayload {
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorPayload {
                error: self.message,
            }),
        )
            .into_response()
    }
}

/// Maps the operator-control state-layer error to an HTTP [`ApiError`].
///
/// `NotAvailable` → 404; feed validation errors → via
/// [`api_error_from_operator_control`].
fn api_error_from_operator_state(error: OperatorStateError) -> ApiError {
    match error {
        OperatorStateError::NotAvailable => {
            ApiError::new(StatusCode::NOT_FOUND, error.to_string())
        }
        OperatorStateError::Feed(e) => api_error_from_operator_control(e),
    }
}

/// Maps a contracts-layer [`OperatorControlError`] to an HTTP [`ApiError`].
///
/// All 7 variants are reachable through the feed path (the feed validates
/// packets and ledger entries before returning them). Validation failures →
/// 400 Bad Request. `DomainActionAuthorityRequested` is a construction-time
/// invariant violation; it maps to 400 as well because the caller supplied
/// an invalid packet.
fn api_error_from_operator_control(error: OperatorControlError) -> ApiError {
    match error {
        OperatorControlError::EmptyField { field } => {
            ApiError::new(StatusCode::BAD_REQUEST, format!("`{field}` must not be empty"))
        }
        OperatorControlError::EmptyBacklink => ApiError::new(
            StatusCode::BAD_REQUEST,
            "backlink ids must not contain empty values",
        ),
        OperatorControlError::InvalidBasisPoints { field, value } => ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("`{field}` must be between 0 and 10000, got `{value}`"),
        ),
        OperatorControlError::InvalidRange { field, min, max } => ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("`{field}` must have min < max, got `{min}`..`{max}`"),
        ),
        OperatorControlError::InvalidCount { field, value } => ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("`{field}` must be greater than zero, got `{value}`"),
        ),
        OperatorControlError::InvalidSha256 { field, value } => ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("`{field}` must be a sha256 hash, got `{value}`"),
        ),
        OperatorControlError::DomainActionAuthorityRequested => ApiError::new(
            StatusCode::BAD_REQUEST,
            "job readiness packets must not authorize domain action",
        ),
    }
}
