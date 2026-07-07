//! Pipeline coordinator — chains truth executions with output→input mapping.
//!
//! The showcase pipeline: score-inbound-fit → qualify-inbound-lead → schedule-strategic-meetings
//!
//! Each step is a distinct convergence run. The coordinator:
//! 1. Reads seed data via an injected [`ShowcaseSeedSource`] (app/seed-IO layer)
//! 2. Executes step N
//! 3. Extracts relevant outputs from step N's projection
//! 4. Maps them to step N+1's inputs
//! 5. Returns per-step results for live visibility
//!
//! Extracted from `application-server/src/pipeline.rs` in Phase 3b.
//! The key difference from the original: `run_showcase_pipeline` now takes a
//! `&TruthExecutionModule` (registry) and an `AppKernelStore` (concrete) rather
//! than being generic over `S: KernelStore`. This is required by the
//! `TruthExecutionContext` type in `helm-truth-execution`, which resolves the
//! `KernelStore` generic with the concrete `AppKernelStore` enum so that truth
//! bodies can be trait-object-safe.
//!
//! # Seed data injection (RFL-154 T5b)
//!
//! The former Parquet seed-loaders (`load_prospect_events_from_seed`,
//! `load_prospect_context_from_seed`) have been removed from this crate to
//! eliminate the `polars` dependency from the spine. The mounting app supplies
//! a [`ShowcaseSeedSource`] implementation via
//! [`PipelineRouteState::with_seed_source`]. A Parquet-based reference
//! implementation is in `crates/seed-gen/src/showcase_seed.rs`.
//!
//! When no seed source is wired the `POST /v1/pipeline/showcase/run` endpoint
//! returns `501 Not Implemented`, mirroring the existing behaviour for
//! unregistered truth bodies.
//!
//! # HTTP surface
//!
//! - `POST /v1/pipeline/showcase/run`    — synchronous pipeline run; returns `PipelineResult`
//! - `GET  /v1/pipeline/showcase/status` — last result, 204 if none
//! - `POST /v1/pipeline/showcase/reset`  — clear last result
//!
//! SSE streaming (`/v1/pipeline/showcase/stream`) and the approvals endpoints are NOT
//! included here — they depend on `RealtimeHub` and `JobStreamState`, which are coupled
//! to `application-server` internals (Phase 4b).

use std::collections::HashMap;
use std::sync::Arc;

use application_kernel::Actor as CrmActor;
use application_storage::{AppKernelStore, AppRuntimeStores, InMemoryKernelStore};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use helm_truth_execution::{
    TruthExecutionArtifacts, TruthExecutionModule,
    dispatcher::{TruthExecutionContext, execute_truth},
};

// Re-export the public contract types so callers can import from this module.
pub use helm_module_contracts::showcase_pipeline::{
    SeedSourceError, ShowcasePipelineInput, ShowcaseSeedSource,
};

// ── Pipeline Types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PipelineResult {
    pub steps: Vec<PipelineStepResult>,
    pub status: PipelineStatus,
    pub prospect_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipelineStepResult {
    pub truth_key: String,
    pub status: StepStatus,
    pub cycles: Option<u32>,
    pub stop_reason: Option<String>,
    pub fact_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PipelineStatus {
    Completed,
    BlockedAtStep { step: usize, reason: String },
    Failed { step: usize, error: String },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum StepStatus {
    Completed,
    Blocked { reason: String },
    Skipped,
    Failed { error: String },
}

// ── HTTP Route State ────────────────────────────────────────────────

/// State for the pipeline HTTP routes.
///
/// Holds the truth registry, a concrete kernel store, runtime stores, the
/// last pipeline result for the `/status` endpoint, and an optional
/// app-supplied seed source.
///
/// Construct with [`PipelineRouteState::new`] (empty registry, in-memory
/// stores, no seed source) or the builder methods for fuller configuration.
#[derive(Clone)]
pub struct PipelineRouteState {
    pub truths: Arc<TruthExecutionModule>,
    pub store: AppKernelStore,
    pub runtime_stores: AppRuntimeStores,
    pub current_result: Arc<RwLock<Option<PipelineResult>>>,
    /// App-supplied seed source. `None` → `run_pipeline` returns 501.
    seed_source: Option<Arc<dyn ShowcaseSeedSource>>,
}

impl PipelineRouteState {
    /// Default state with empty truth registry, in-memory stores, and no seed source.
    ///
    /// Pipeline execution will return `501 Not Implemented` for each truth key
    /// and for the seed-load step until sources are wired via builder methods.
    pub fn new() -> Self {
        Self {
            truths: Arc::new(TruthExecutionModule::new()),
            store: AppKernelStore::Memory(InMemoryKernelStore::default_local()),
            runtime_stores: AppRuntimeStores::default(),
            current_result: Arc::new(RwLock::new(None)),
            seed_source: None,
        }
    }

    /// State with a populated truth registry (in-memory stores, no seed source).
    pub fn with_truths(truths: Arc<TruthExecutionModule>) -> Self {
        Self {
            truths,
            store: AppKernelStore::Memory(InMemoryKernelStore::default_local()),
            runtime_stores: AppRuntimeStores::default(),
            current_result: Arc::new(RwLock::new(None)),
            seed_source: None,
        }
    }

    /// Wire an app-supplied seed source for the `run_pipeline` handler.
    ///
    /// The source is called with the `prospect_id` from the HTTP request body
    /// (defaulting to `"prospect-001"`). Without a source the handler returns
    /// `501 Not Implemented`.
    pub fn with_seed_source(mut self, source: Arc<dyn ShowcaseSeedSource>) -> Self {
        self.seed_source = Some(source);
        self
    }
}

impl Default for PipelineRouteState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Router ──────────────────────────────────────────────────────────

/// Returns the Axum router for pipeline routes.
///
/// Paths exposed:
/// - `POST /v1/pipeline/showcase/run`    — synchronous pipeline run
/// - `GET  /v1/pipeline/showcase/status` — last result (204 if none)
/// - `POST /v1/pipeline/showcase/reset`  — clear last result
pub fn pipeline_router(state: Arc<PipelineRouteState>) -> Router {
    Router::new()
        .route("/v1/pipeline/showcase/run", post(run_pipeline))
        .route("/v1/pipeline/showcase/status", get(pipeline_status))
        .route("/v1/pipeline/showcase/reset", post(reset_pipeline))
        .with_state(state)
}

// ── Request / Response Types ────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RunPipelineRequest {
    pub prospect_id: Option<String>,
    pub prospect_name: Option<String>,
    pub inbound_summary: Option<String>,
}

// ── Handlers ────────────────────────────────────────────────────────

async fn run_pipeline(
    State(state): State<Arc<PipelineRouteState>>,
    Json(request): Json<RunPipelineRequest>,
) -> Result<Json<PipelineResult>, (StatusCode, String)> {
    let prospect_id = request.prospect_id.unwrap_or_else(|| "prospect-001".into());

    let seed_source = state.seed_source.as_ref().ok_or_else(|| {
        (
            StatusCode::NOT_IMPLEMENTED,
            "no seed source is configured; mount this module with .with_seed_source(...)".into(),
        )
    })?;

    let mut input = seed_source
        .load(&prospect_id)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if let Some(name) = request.prospect_name {
        input.prospect_name = name;
    }
    if let Some(summary) = request.inbound_summary {
        input.inbound_summary = summary;
    }

    let actor = CrmActor::system();
    let result = run_showcase_pipeline(
        &state.truths,
        state.store.clone(),
        state.runtime_stores.clone(),
        input,
        actor,
    )
    .await;

    let mut current = state.current_result.write().await;
    *current = Some(result.clone());

    Ok(Json(result))
}

async fn pipeline_status(State(state): State<Arc<PipelineRouteState>>) -> impl IntoResponse {
    let result = state.current_result.read().await;
    match result.as_ref() {
        Some(r) => Json(serde_json::to_value(r).unwrap_or_default()).into_response(),
        None => (StatusCode::NO_CONTENT, "no pipeline has run yet").into_response(),
    }
}

async fn reset_pipeline(State(state): State<Arc<PipelineRouteState>>) -> impl IntoResponse {
    let mut result = state.current_result.write().await;
    *result = None;
    (StatusCode::OK, "pipeline state reset")
}

// ── Pipeline Execution ──────────────────────────────────────────────

pub async fn run_showcase_pipeline(
    truths: &TruthExecutionModule,
    store: AppKernelStore,
    runtime_stores: AppRuntimeStores,
    input: ShowcasePipelineInput,
    actor: CrmActor,
) -> PipelineResult {
    let mut steps = Vec::new();
    let prospect_name = input.prospect_name.clone();

    // ── Step 1: Score inbound fit ───────────────────────────────────

    let score_inputs = build_score_inputs(&input);
    let score_result = execute_truth(
        truths,
        "score-inbound-fit",
        TruthExecutionContext {
            store: store.clone(),
            runtime_stores: runtime_stores.clone(),
            inputs: score_inputs,
            actor: actor.clone(),
            persist_projection: true,
        },
    )
    .await;

    let (score_artifacts, fit_score_bps) = match score_result {
        Ok(artifacts) => {
            let fit_score = extract_fit_score(&artifacts);
            let step = step_result_from_artifacts("score-inbound-fit", &artifacts);
            let blocked = matches!(step.status, StepStatus::Blocked { .. });
            steps.push(step);
            if blocked {
                return PipelineResult {
                    steps,
                    status: PipelineStatus::BlockedAtStep {
                        step: 0,
                        reason: "score-inbound-fit blocked for review".into(),
                    },
                    prospect_name,
                };
            }
            (artifacts, fit_score)
        }
        Err(e) => {
            steps.push(PipelineStepResult {
                truth_key: "score-inbound-fit".into(),
                status: StepStatus::Failed {
                    error: e.message().to_string(),
                },
                cycles: None,
                stop_reason: None,
                fact_count: None,
            });
            return PipelineResult {
                steps,
                status: PipelineStatus::Failed {
                    step: 0,
                    error: e.message().to_string(),
                },
                prospect_name,
            };
        }
    };

    // ── Step 2: Qualify inbound lead ────────────────────────────────

    let org_id = score_artifacts
        .projection
        .as_ref()
        .and_then(|p| p.organization.as_ref())
        .map(|org| org.id.to_string());

    let qualify_inputs = build_qualify_inputs(&input, org_id.as_deref(), fit_score_bps);
    let qualify_result = execute_truth(
        truths,
        "qualify-inbound-lead",
        TruthExecutionContext {
            store: store.clone(),
            runtime_stores: runtime_stores.clone(),
            inputs: qualify_inputs,
            actor: actor.clone(),
            persist_projection: true,
        },
    )
    .await;

    let _qualify_artifacts = match qualify_result {
        Ok(artifacts) => {
            let step = step_result_from_artifacts("qualify-inbound-lead", &artifacts);
            let blocked = matches!(step.status, StepStatus::Blocked { .. });
            steps.push(step);
            if blocked {
                return PipelineResult {
                    steps,
                    status: PipelineStatus::BlockedAtStep {
                        step: 1,
                        reason: "qualify-inbound-lead blocked for review".into(),
                    },
                    prospect_name,
                };
            }
            artifacts
        }
        Err(e) => {
            steps.push(PipelineStepResult {
                truth_key: "qualify-inbound-lead".into(),
                status: StepStatus::Failed {
                    error: e.message().to_string(),
                },
                cycles: None,
                stop_reason: None,
                fact_count: None,
            });
            return PipelineResult {
                steps,
                status: PipelineStatus::Failed {
                    step: 1,
                    error: e.message().to_string(),
                },
                prospect_name,
            };
        }
    };

    // ── Step 3: Schedule strategic meetings ─────────────────────────

    let schedule_inputs = build_schedule_inputs(&input, org_id.as_deref(), fit_score_bps);
    let schedule_result = execute_truth(
        truths,
        "schedule-strategic-meetings",
        TruthExecutionContext {
            store,
            runtime_stores,
            inputs: schedule_inputs,
            actor,
            persist_projection: true,
        },
    )
    .await;

    match schedule_result {
        Ok(artifacts) => {
            let step = step_result_from_artifacts("schedule-strategic-meetings", &artifacts);
            let blocked = matches!(step.status, StepStatus::Blocked { .. });
            steps.push(step);
            if blocked {
                return PipelineResult {
                    steps,
                    status: PipelineStatus::BlockedAtStep {
                        step: 2,
                        reason: "schedule-strategic-meetings blocked for confirmation".into(),
                    },
                    prospect_name,
                };
            }
        }
        Err(e) => {
            steps.push(PipelineStepResult {
                truth_key: "schedule-strategic-meetings".into(),
                status: StepStatus::Failed {
                    error: e.message().to_string(),
                },
                cycles: None,
                stop_reason: None,
                fact_count: None,
            });
            return PipelineResult {
                steps,
                status: PipelineStatus::Failed {
                    step: 2,
                    error: e.message().to_string(),
                },
                prospect_name,
            };
        }
    }

    PipelineResult {
        steps,
        status: PipelineStatus::Completed,
        prospect_name,
    }
}

// ── Input Builders ──────────────────────────────────────────────────

fn build_score_inputs(input: &ShowcasePipelineInput) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("organization_name".into(), input.prospect_name.clone());
    m.insert("visitor_id".into(), input.visitor_id.clone());
    m.insert("usage_events_json".into(), input.usage_events_json.clone());
    if let Some(ref industry) = input.industry {
        m.insert("industry".into(), industry.clone());
    }
    if let Some(ref website) = input.website {
        m.insert("website".into(), website.clone());
    }
    m
}

fn build_qualify_inputs(
    input: &ShowcasePipelineInput,
    org_id: Option<&str>,
    fit_score_bps: Option<u16>,
) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("organization_name".into(), input.prospect_name.clone());
    m.insert("inbound_summary".into(), input.inbound_summary.clone());
    if let Some(org_id) = org_id {
        m.insert("organization_id".into(), org_id.to_string());
    }
    if let Some(fit) = fit_score_bps {
        // Convert bps (0-10000) to 0-100 scale for qualify
        m.insert("fit_score".into(), (fit / 100).to_string());
    }
    if let Some(ref industry) = input.industry {
        m.insert("industry".into(), industry.clone());
    }
    if let Some(ref website) = input.website {
        m.insert("website".into(), website.clone());
    }
    if let Some(ref name) = input.contact_name {
        m.insert("contact_name".into(), name.clone());
    }
    if let Some(ref title) = input.contact_title {
        m.insert("contact_title".into(), title.clone());
    }
    if let Some(ref email) = input.contact_email {
        m.insert("contact_email".into(), email.clone());
    }
    m
}

fn build_schedule_inputs(
    input: &ShowcasePipelineInput,
    org_id: Option<&str>,
    fit_score_bps: Option<u16>,
) -> HashMap<String, String> {
    let prospect_seed = serde_json::json!([{
        "organization_id": org_id,
        "name": input.prospect_name,
        "contact_name": input.contact_name,
        "contact_email": input.contact_email,
        "fit_score_bps": fit_score_bps,
        "pipeline_stage": "qualified",
        "last_contact_days_ago": 0,
        "estimated_value": null,
        "territory": null,
        "segment": input.industry,
        "tags": ["pipeline-showcase"]
    }]);

    let mut m = HashMap::new();
    m.insert(
        "intent_text".into(),
        format!(
            "Book {} meetings with qualified prospects",
            input.meeting_count
        ),
    );
    m.insert("requested_count".into(), input.meeting_count.to_string());
    m.insert("window_start".into(), input.window_start.clone());
    m.insert("window_end".into(), input.window_end.clone());
    m.insert("prospects_json".into(), prospect_seed.to_string());
    if let Some(ref slots) = input.calendar_slots_json {
        m.insert("calendar_slots_json".into(), slots.clone());
    }
    m
}

// ── Output Extraction ───────────────────────────────────────────────

fn extract_fit_score(artifacts: &TruthExecutionArtifacts) -> Option<u16> {
    let facts = artifacts.projection.as_ref()?.facts.as_slice();
    for fact in facts {
        if fact.statement.contains("fit score") || fact.statement.contains("Inbound fit score") {
            // Extract bps from statement like "Inbound fit score 7500 bps (high-fit)..."
            let words: Vec<&str> = fact.statement.split_whitespace().collect();
            for (i, w) in words.iter().enumerate() {
                if *w == "bps"
                    && i > 0
                    && let Ok(score) = words[i - 1].parse::<u16>()
                {
                    return Some(score);
                }
            }
            // Fallback: use confidence_bps as proxy
            return Some(fact.confidence_bps);
        }
    }
    None
}

fn step_result_from_artifacts(
    truth_key: &str,
    artifacts: &TruthExecutionArtifacts,
) -> PipelineStepResult {
    let stop_reason = format!("{:?}", artifacts.result.stop_reason);
    let is_blocked = stop_reason.contains("Blocked") || stop_reason.contains("HumanIntervention");

    PipelineStepResult {
        truth_key: truth_key.into(),
        status: if is_blocked {
            StepStatus::Blocked {
                reason: stop_reason.clone(),
            }
        } else {
            StepStatus::Completed
        },
        cycles: Some(artifacts.result.cycles),
        stop_reason: Some(stop_reason),
        fact_count: Some(artifacts.result.integrity.fact_count),
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;

    /// Minimal stub that always returns a fixed `ShowcasePipelineInput`.
    struct StubSeedSource {
        prospect_name: String,
    }

    #[async_trait]
    impl ShowcaseSeedSource for StubSeedSource {
        async fn load(&self, prospect_id: &str) -> Result<ShowcasePipelineInput, SeedSourceError> {
            Ok(ShowcasePipelineInput {
                prospect_name: self.prospect_name.clone(),
                visitor_id: prospect_id.into(),
                usage_events_json: "[]".into(),
                inbound_summary: format!("Stub inbound for {prospect_id}"),
                meeting_count: 1,
                window_start: "2026-04-21".into(),
                window_end: "2026-04-25".into(),
                calendar_slots_json: None,
                industry: None,
                website: None,
                contact_name: None,
                contact_title: None,
                contact_email: None,
            })
        }
    }

    /// Stub that always returns a `ProspectNotFound` error.
    struct MissingSeedSource;

    #[async_trait]
    impl ShowcaseSeedSource for MissingSeedSource {
        async fn load(&self, prospect_id: &str) -> Result<ShowcasePipelineInput, SeedSourceError> {
            Err(SeedSourceError::ProspectNotFound {
                prospect_id: prospect_id.into(),
            })
        }
    }

    #[tokio::test]
    async fn stub_seed_source_supplies_pipeline_input_via_injection() {
        let stub = Arc::new(StubSeedSource {
            prospect_name: "Acme Corp".into(),
        });
        let state = PipelineRouteState::new().with_seed_source(stub);

        let input = state
            .seed_source
            .as_ref()
            .expect("seed source wired in")
            .load("prospect-001")
            .await
            .expect("stub always succeeds");

        assert_eq!(input.prospect_name, "Acme Corp");
        assert_eq!(input.visitor_id, "prospect-001");
        assert_eq!(input.meeting_count, 1);
    }

    #[tokio::test]
    async fn pipeline_state_without_seed_source_has_none() {
        let state = PipelineRouteState::new();
        assert!(state.seed_source.is_none(), "no source wired by default");
    }

    #[tokio::test]
    async fn missing_seed_source_error_carries_typed_prospect_id() {
        let stub = Arc::new(MissingSeedSource);
        let state = PipelineRouteState::new().with_seed_source(stub);

        let err = state
            .seed_source
            .as_ref()
            .unwrap()
            .load("prospect-999")
            .await
            .expect_err("missing source returns error");

        assert!(
            matches!(err, SeedSourceError::ProspectNotFound { ref prospect_id } if prospect_id == "prospect-999"),
            "error variant carries prospect id"
        );
        assert!(
            err.to_string().contains("prospect-999"),
            "Display includes prospect id: {err}"
        );
    }
}
