use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KernelError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("record not found: {kind} {id}")]
    NotFound { kind: &'static str, id: String },
    #[error("invariant violated: {0}")]
    Invariant(String),
    #[error("conflict: {0}")]
    Conflict(String),
}

pub type KernelResult<T> = Result<T, KernelError>;
