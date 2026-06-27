//! HTTP routes for the operator-control workbench surface.
//!
//! Extracted from `application-server/src/http_api.rs`.
//! Only routes under `/v1/workbench/operator-control/` are included here;
//! all other workbench and CRM routes remain in application-server until a
//! later phase.

use std::sync::Arc;

use crate::{OperatorControlPreview, OperatorControlReadinessFeed};
use application_storage::{AppConfig, InMemoryKernelStore, KernelStore};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use workbench_backend::{OperatorApp, OperatorAppError};

// ── State ────────────────────────────────────────────────────────────────────

/// Focused state struct for operator-control routes.
///
/// Holds only what these two routes need: an `OperatorApp<S>`. The broader
/// `HttpState<S>` in application-server carries billing, runtime stores, and
/// other concerns that do not belong to operator-control.
#[derive(Clone)]
pub struct OperatorControlState<S = InMemoryKernelStore> {
    pub operator: OperatorApp<S>,
    readiness_feed: Option<Arc<dyn OperatorControlReadinessFeed>>,
}

impl<S> OperatorControlState<S>
where
    S: KernelStore + Clone,
{
    pub fn new(config: AppConfig, store: S) -> Self {
        Self {
            operator: OperatorApp::new(config, store),
            readiness_feed: None,
        }
    }

    pub fn with_readiness_feed(mut self, feed: Arc<dyn OperatorControlReadinessFeed>) -> Self {
        self.readiness_feed = Some(feed);
        self
    }

    pub fn operator_control_preview(&self) -> Result<OperatorControlPreview, OperatorAppError> {
        let mut previews = self.live_previews()?;
        if !previews.is_empty() {
            return Ok(previews.remove(0));
        }

        Err(OperatorAppError::OperatorControl(
            "operator-control preview requires an injected live readiness feed".to_string(),
        ))
    }

    pub fn operator_control_previews(
        &self,
    ) -> Result<Vec<OperatorControlPreview>, OperatorAppError> {
        self.live_previews()
    }

    fn live_previews(&self) -> Result<Vec<OperatorControlPreview>, OperatorAppError> {
        let Some(feed) = &self.readiness_feed else {
            return Ok(Vec::new());
        };

        let snapshots = feed
            .previews()
            .map_err(|error| OperatorAppError::OperatorControl(error.to_string()))?;

        Ok(snapshots.into_iter().map(Into::into).collect())
    }
}

// ── Router ───────────────────────────────────────────────────────────────────

/// Returns the Axum router for operator-control routes.
///
/// Paths exposed:
/// - `GET /v1/workbench/operator-control/preview`  — first injected live preview
/// - `GET /v1/workbench/operator-control/previews` — injected live preview list
pub fn router<S>(state: Arc<OperatorControlState<S>>) -> Router
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    Router::new()
        .route(
            "/v1/workbench/operator-control/preview",
            get(workbench_operator_control_preview::<S>),
        )
        .route(
            "/v1/workbench/operator-control/previews",
            get(workbench_operator_control_previews::<S>),
        )
        .with_state(state)
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn workbench_operator_control_preview<S>(
    State(state): State<Arc<OperatorControlState<S>>>,
) -> Result<Json<OperatorControlPreview>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    state
        .operator_control_preview()
        .map(Json)
        .map_err(api_error_from_operator)
}

async fn workbench_operator_control_previews<S>(
    State(state): State<Arc<OperatorControlState<S>>>,
) -> Result<Json<Vec<OperatorControlPreview>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    state
        .operator_control_previews()
        .map(Json)
        .map_err(api_error_from_operator)
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

fn api_error_from_operator(error: OperatorAppError) -> ApiError {
    match error {
        OperatorAppError::Storage(error) => api_error_from_storage(error),
        OperatorAppError::TruthNotFound(key) => {
            ApiError::new(StatusCode::NOT_FOUND, format!("truth not found: {key}"))
        }
        OperatorAppError::MissingInput(field) => ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("missing required input: {field}"),
        ),
        OperatorAppError::InvalidUuid { field, value } => ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("invalid uuid for {field}: {value}"),
        ),
        OperatorAppError::InvalidInteger { field, value } => ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("invalid integer for {field}: {value}"),
        ),
        OperatorAppError::Validation(message)
        | OperatorAppError::OperatorControl(message)
        | OperatorAppError::UnsupportedTruth(message) => {
            ApiError::new(StatusCode::BAD_REQUEST, message)
        }
    }
}

fn api_error_from_storage(error: application_storage::StorageError) -> ApiError {
    match error {
        application_storage::StorageError::LockPoisoned => {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "storage lock poisoned")
        }
        application_storage::StorageError::Kernel(error) => api_error_from_kernel(error),
        application_storage::StorageError::ConnectionFailed { backend, message } => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("{backend} connection failed: {message}"),
        ),
        application_storage::StorageError::SerializationFailed { message } => {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, message)
        }
        application_storage::StorageError::Timeout { operation } => {
            ApiError::new(StatusCode::GATEWAY_TIMEOUT, operation)
        }
        application_storage::StorageError::RuntimeStore { message } => {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, message)
        }
    }
}

fn api_error_from_kernel(error: application_kernel::KernelError) -> ApiError {
    match error {
        application_kernel::KernelError::Validation(message) => {
            ApiError::new(StatusCode::BAD_REQUEST, message)
        }
        application_kernel::KernelError::NotFound { kind, id } => {
            ApiError::new(StatusCode::NOT_FOUND, format!("{kind} not found: {id}"))
        }
        application_kernel::KernelError::Invariant(message) => {
            ApiError::new(StatusCode::PRECONDITION_FAILED, message)
        }
        application_kernel::KernelError::Conflict(message) => {
            ApiError::new(StatusCode::CONFLICT, message)
        }
    }
}
