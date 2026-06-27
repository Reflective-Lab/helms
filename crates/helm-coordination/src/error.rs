//! Coordination error type and its HTTP mapping.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::ledger::GateDecisionKind;
use crate::principal::OperatorPrincipal;

/// Errors surfaced by the coordination service.
#[derive(Debug, thiserror::Error)]
pub enum CoordinationError {
    /// The request did not carry enough identity to resolve a principal.
    #[error("missing operator identity: {0}")]
    MissingIdentity(String),

    /// The principal is not authorized to decide on this subject.
    #[error("operator {actor_id} is not authorized to decide {subject}")]
    AuthorityDenied { actor_id: String, subject: String },

    /// A divergent decision already exists for this gate (optimistic conflict).
    #[error(
        "decision conflict on {ref_id}: already {existing:?} by {existing_actor}, attempted {attempted:?}"
    )]
    DecisionConflict {
        ref_id: String,
        existing: GateDecisionKind,
        existing_actor: String,
        attempted: GateDecisionKind,
        attempted_by: OperatorPrincipal,
    },

    /// The referenced session does not exist or has expired.
    #[error("session not found: {0}")]
    SessionNotFound(String),

    /// A malformed request value.
    #[error("invalid request: {0}")]
    BadRequest(String),
}

impl CoordinationError {
    #[must_use]
    pub fn status(&self) -> StatusCode {
        match self {
            Self::MissingIdentity(_) => StatusCode::UNAUTHORIZED,
            Self::AuthorityDenied { .. } => StatusCode::FORBIDDEN,
            Self::DecisionConflict { .. } => StatusCode::CONFLICT,
            Self::SessionNotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorPayload {
    error: String,
}

impl IntoResponse for CoordinationError {
    fn into_response(self) -> Response {
        (
            self.status(),
            Json(ErrorPayload {
                error: self.to_string(),
            }),
        )
            .into_response()
    }
}
