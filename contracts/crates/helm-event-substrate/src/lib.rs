//! Bedrock-owned substrate injection contracts — the Seam A boundary.
//!
//! This crate defines the abstract event-ledger, lease/session-ownership, and
//! SSE-streaming contracts that Helms modules depend on.  Concrete
//! implementations are injected at construction time:
//!
//! - **runway-app-host** — production implementor (redb local + Firestore
//!   remote for [`EventLog`] / [`SyncableEventLog`]; redb + Firestore for
//!   [`LeaseStore`]).  `runway-storage` re-exports these traits for callers
//!   that pin to the old import path.
//! - **in-memory** — lightweight second implementor, gated behind the
//!   `memory` feature (off by default).  Required to keep the contract-suite
//!   property tests honest: any runtime assertion that passes for the redb
//!   backend must also pass here.
//!
//! ## What lives here vs. what stays app-side
//!
//! **Moved into this crate (Seam A):**
//! - [`EventLog`] / [`SyncableEventLog`] — append-only event ledger traits.
//! - [`LeaseStore`] + [`LeaseScope`] / [`LeaseRecord`] / [`AcquireOutcome`] /
//!   [`RenewOutcome`] — CAS session-ownership contract.
//! - [`EventHub`] / [`EventHubHandle`] — in-process broadcast channel with
//!   replay buffer and optional durable backing.
//! - [`sse`] module — axum SSE router, frame encoder, replay-then-live stream
//!   combinator (default feature `sse`).
//! - [`SubstrateError`] — canonical error type; `runway-storage` re-exports it.
//!
//! **Stays app-side (not moved):**
//! - `SessionOwnershipLayer` — axum middleware bound to `runway_auth::AuthContext`
//!   and `tower`.  Awaiting `OrgIdentity` neutralization (RFL-178) before it can
//!   cross the repo boundary.
//! - `StorageKit` (documents / vectors / objects), `RunwayAppHost` builder,
//!   manifest types — remain in `runway-app-host`.
//!
//! ## Feature flags
//!
//! | Feature  | Default | Description                                                                        |
//! |----------|---------|------------------------------------------------------------------------------------|
//! | `sse`    | yes     | Pulls in `axum`, `tokio-stream`, `async-stream`, and `futures` for the SSE surface. |
//! | `memory` | no      | In-memory [`EventLog`] + [`LeaseStore`] implementations for tests and headless composition roots. |
//!
//! ## Lineage
//!
//! - **RP-LAYERING** — this crate is the Seam A boundary described in the
//!   helms architecture.  Types here are shared contracts, not application logic.
//! - **RFL-171** — extraction task that moved these types from
//!   `runway-storage` and `runway-app-host` into Bedrock-owned contracts.
//! - **RFL-178** — follow-on task to neutralize `OrgIdentity` / `AuthContext`
//!   so `SessionOwnershipLayer` can also cross the boundary.  Until then,
//!   the middleware stays in `runway-app-host`.

pub mod event;
pub mod hub;
pub mod lease;

/// SSE transport: axum router, frame encoder, replay-then-live stream combinator.
///
/// Gated behind the `sse` feature (default on). Requires `axum`, `tokio-stream`,
/// `async-stream`, and `futures`.
#[cfg(feature = "sse")]
pub mod sse;

/// In-memory implementations of [`EventLog`] / [`SyncableEventLog`] and
/// [`LeaseStore`].
///
/// Gated behind the `memory` feature (off by default).  These are the
/// "honest second implementors" for tests and headless composition roots.
/// Any property that holds for the runway redb backend must also hold here —
/// the parity property tests in [`memory`] enforce that contract.
///
/// ## Feature gate
///
/// Enable with `--features memory`.  Without this feature neither
/// [`InMemoryEventLog`] nor [`InMemoryLeaseStore`] are compiled.
#[cfg(feature = "memory")]
pub mod memory;

#[cfg(feature = "memory")]
pub use memory::{InMemoryEventLog, InMemoryLeaseStore};

pub use event::{
    EventCursor, EventEnvelope, EventLog, EventQuery, EventSubscription, StoredEvent,
    SyncableEventLog,
};
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
