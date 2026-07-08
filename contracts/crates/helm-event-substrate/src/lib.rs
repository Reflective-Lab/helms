//! Bedrock-owned substrate injection contracts.
//!
//! This crate defines the abstract event-ledger and lease/session-ownership
//! contracts that Helms modules depend on. Concrete implementations live
//! outside this crate and are injected at construction time:
//!
//! - **runway-app-host** — production implementor (redb local + Firestore
//!   remote for [`EventLog`] / [`SyncableEventLog`]; redb + Firestore for
//!   [`LeaseStore`]).
//! - **in-memory** — lightweight second implementor, gated behind the
//!   `memory` feature (filled in T3). Required to keep the contract-suite
//!   property tests honest: any runtime assertion that passes for redb must
//!   also pass for in-memory.
//!
//! ## Feature flags
//!
//! | Feature  | Default | Description                                                                        |
//! |----------|---------|------------------------------------------------------------------------------------|
//! | `sse`    | yes     | Pulls in `axum`, `tokio-stream`, `async-stream`, and `futures` for the SSE surface (T2). |
//! | `memory` | no      | In-memory [`EventLog`] + [`LeaseStore`] implementations (T3).                     |
//!
//! ## Lineage
//!
//! - **RP-LAYERING** — this crate is the Seam A boundary described in the
//!   helms architecture. Types here are shared contracts, not application logic.
//! - **RFL-171** — extraction task that moved these types from
//!   `runway-storage` and `runway-app-host` into Bedrock-owned contracts.

pub mod event;
pub mod hub;
pub mod lease;

/// SSE transport: axum router, frame encoder, replay-then-live stream combinator.
///
/// Gated behind the `sse` feature (default on). Requires `axum`, `tokio-stream`,
/// `async-stream`, and `futures`.
#[cfg(feature = "sse")]
pub mod sse;

pub use event::{EventCursor, EventEnvelope, EventLog, EventQuery, EventSubscription, StoredEvent, SyncableEventLog};
pub use hub::{EventHub, EventHubHandle};
pub use lease::{AcquireOutcome, LeaseRecord, LeaseScope, LeaseStore, RenewOutcome};

/// Canonical error type for all substrate contract trait implementations.
///
/// Renamed from `runway_storage::traits::Error` (RFL-171). Runway-storage
/// re-exports this type so existing callers keep compiling without source edits.
#[derive(Debug, thiserror::Error)]
pub enum SubstrateError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("serialisation error: {0}")]
    Serialisation(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("{0}")]
    Other(String),
}

/// Convenience alias used throughout the substrate contract traits.
pub type Result<T> = std::result::Result<T, SubstrateError>;
