//! Typed error for truth body execution.
//!
//! [`TruthExecutionError`] replaces `tonic::Status` on the public
//! [`crate::TruthBody::execute`] surface.  All variants carry a `message`
//! string that preserves the exact human-readable text the old `Status`
//! constructors produced.
//!
//! # gRPC transport mapping
//!
//! Enable the `grpc` cargo feature to activate
//! `impl From<TruthExecutionError> for tonic::Status`.  The mapping is
//! one-to-one with the Status codes used before this change, so gRPC
//! consumers see zero behavior difference.

use thiserror::Error;

/// Error returned by [`crate::TruthBody::execute`] and the dispatcher helpers.
///
/// Variants mirror the semantic failure classes that truth bodies and the
/// dispatcher infrastructure can produce.  The names intentionally avoid
/// gRPC terminology; the mapping to transport codes lives behind the `grpc`
/// feature.
#[derive(Debug, Error)]
pub enum TruthExecutionError {
    /// A required input value is missing or syntactically invalid.
    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },

    /// A referenced entity was not found in the kernel store.
    #[error("not found: {message}")]
    NotFound { message: String },

    /// A precondition for the operation was not met (invariant violation,
    /// missing fact, invalid resume, empty provenance, etc.).
    #[error("failed precondition: {message}")]
    FailedPrecondition { message: String },

    /// An internal error occurred (lock poisoned, serialization failed,
    /// converge agent failed, runtime store failure, invalid payload JSON, etc.).
    #[error("internal error: {message}")]
    Internal { message: String },

    /// A conflicting entity already exists in the kernel store.
    #[error("already exists: {message}")]
    AlreadyExists { message: String },

    /// A converge budget (cycle limit, etc.) was exhausted.
    #[error("resource exhausted: {message}")]
    ResourceExhausted { message: String },

    /// A concurrent fact conflict was detected during converge.
    #[error("aborted: {message}")]
    Aborted { message: String },

    /// A converge context snapshot is corrupted or unreadable.
    #[error("data loss: {message}")]
    DataLoss { message: String },

    /// A backing service (database, etc.) is unavailable.
    #[error("unavailable: {message}")]
    Unavailable { message: String },

    /// An operation timed out.
    #[error("deadline exceeded: {message}")]
    DeadlineExceeded { message: String },

    /// This truth key has no registered body.
    #[error("unimplemented: {message}")]
    Unimplemented { message: String },
}

impl TruthExecutionError {
    /// Returns the inner message string without the variant prefix.
    ///
    /// Callers that previously used `tonic::Status::message()` can switch to
    /// this method for a drop-in replacement.
    pub fn message(&self) -> &str {
        match self {
            Self::InvalidArgument { message }
            | Self::NotFound { message }
            | Self::FailedPrecondition { message }
            | Self::Internal { message }
            | Self::AlreadyExists { message }
            | Self::ResourceExhausted { message }
            | Self::Aborted { message }
            | Self::DataLoss { message }
            | Self::Unavailable { message }
            | Self::DeadlineExceeded { message }
            | Self::Unimplemented { message } => message,
        }
    }
}

// ── gRPC transport mapping (feature-gated) ─────────────────────────────────────

#[cfg(feature = "grpc")]
impl From<TruthExecutionError> for tonic::Status {
    /// Maps each variant to the identical `tonic::Status` code that was
    /// produced before RFL-176.  Behavior-preserving for gRPC consumers.
    fn from(e: TruthExecutionError) -> Self {
        match e {
            TruthExecutionError::InvalidArgument { message } => Self::invalid_argument(message),
            TruthExecutionError::NotFound { message } => Self::not_found(message),
            TruthExecutionError::FailedPrecondition { message } => {
                Self::failed_precondition(message)
            }
            TruthExecutionError::Internal { message } => Self::internal(message),
            TruthExecutionError::AlreadyExists { message } => Self::already_exists(message),
            TruthExecutionError::ResourceExhausted { message } => {
                Self::resource_exhausted(message)
            }
            TruthExecutionError::Aborted { message } => Self::aborted(message),
            TruthExecutionError::DataLoss { message } => Self::data_loss(message),
            TruthExecutionError::Unavailable { message } => Self::unavailable(message),
            TruthExecutionError::DeadlineExceeded { message } => Self::deadline_exceeded(message),
            TruthExecutionError::Unimplemented { message } => Self::unimplemented(message),
        }
    }
}
