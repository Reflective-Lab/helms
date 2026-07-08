//! Contract types for the showcase pipeline injection boundary.
//!
//! `ShowcasePipelineInput` is the typed value crossing from seed-IO
//! (Parquet, fixtures, JSON) into the operator-control spine.
//! `ShowcaseSeedSource` is the injection trait the mounting app implements;
//! this keeps the spine crate (`helm-operator-control`) free of heavy IO
//! dependencies such as `polars` / Parquet (RFL-154 T5b).
//!
//! # Mounting-app responsibility
//!
//! The mounting app (or seed-IO layer) implements [`ShowcaseSeedSource`] and
//! supplies it via `PipelineRouteState::with_seed_source`. A Parquet-based
//! reference implementation lives in `crates/seed-gen/src/showcase_seed.rs`.
//!
//! Mirrors the `OperatorControlReadinessFeed` injection pattern from
//! `helm-operator-control::lib`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ── Input ──────────────────────────────────────────────────────────────────────

/// Fully-typed input to the showcase pipeline.
///
/// Produced by the mounting app (e.g. via [`ShowcaseSeedSource`]) and handed
/// to `PipelineRouteState`. All fields map 1-to-1 to truth input keys.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShowcasePipelineInput {
    pub prospect_name: String,
    pub visitor_id: String,
    /// JSON-serialised array of behavioural events.
    pub usage_events_json: String,
    pub inbound_summary: String,
    pub meeting_count: u32,
    pub window_start: String,
    pub window_end: String,
    /// Optional JSON-serialised calendar slot array.
    pub calendar_slots_json: Option<String>,
    pub industry: Option<String>,
    pub website: Option<String>,
    pub contact_name: Option<String>,
    pub contact_title: Option<String>,
    pub contact_email: Option<String>,
}

// ── Error ──────────────────────────────────────────────────────────────────────

/// Typed errors from seed-source loading.
///
/// Variants carry structured context through typed enum discriminants and
/// named fields. `StorageError` and `ParseError` include `detail: String` for
/// foreign IO messages where a richer type is not available (RFL-129
/// typed-contract rule).
#[derive(Debug, Clone)]
pub enum SeedSourceError {
    /// The requested prospect was not found in the seed dataset.
    ProspectNotFound { prospect_id: String },
    /// The underlying seed dataset could not be opened or read.
    StorageError { detail: String },
    /// Data within the dataset was invalid or could not be parsed.
    ParseError { detail: String },
}

impl std::fmt::Display for SeedSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeedSourceError::ProspectNotFound { prospect_id } => {
                write!(f, "prospect '{prospect_id}' not found in seed dataset")
            }
            SeedSourceError::StorageError { detail } => {
                write!(f, "seed storage error: {detail}")
            }
            SeedSourceError::ParseError { detail } => {
                write!(f, "seed parse error: {detail}")
            }
        }
    }
}

impl std::error::Error for SeedSourceError {}

// ── Trait ──────────────────────────────────────────────────────────────────────

/// Injection contract for showcase-pipeline seed data.
///
/// The mounting app implements this trait to supply [`ShowcasePipelineInput`]
/// to the `run_pipeline` HTTP handler. This decouples the spine from IO
/// concerns (Parquet, file system, remote APIs). Implementations live in the
/// app or seed-IO layer; the spine only holds the `dyn ShowcaseSeedSource`
/// pointer.
///
/// # Pattern
///
/// Mirrors [`helm_operator_control::OperatorControlReadinessFeed`]: both are
/// trait-object injection points stored in route state, wired at mount time.
#[async_trait]
pub trait ShowcaseSeedSource: Send + Sync + 'static {
    /// Load the full pipeline input for a given prospect identifier.
    ///
    /// The prospect identifier comes from the `prospect_id` field of the
    /// `POST /v1/pipeline/showcase/run` request body; it defaults to
    /// `"prospect-001"` when omitted.
    async fn load(&self, prospect_id: &str) -> Result<ShowcasePipelineInput, SeedSourceError>;
}
