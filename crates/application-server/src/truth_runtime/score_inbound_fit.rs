use std::collections::HashMap;
use std::io::Write;

use application_kernel::{
    Actor as CrmActor, FactRecord, OrganizationLifecycle, OrganizationUpsert, RecordKind, RecordRef,
};
use application_storage::{KernelStore, StoreWriteResult};
use converge_analytics::batch::{
    TemporalFeatureConfig, TemporalFeatures, extract_temporal_features, temporal_to_feature_vector,
};
use converge_analytics::engine::FeatureVector;
use converge_analytics::model::{ModelConfig, run_batch_inference};
use converge_kernel::{ContextState as Context, ConvergeResult, Engine};
use converge_pack::{AgentEffect, Context as ContextView, ContextKey, Suggestor};
use serde::{Deserialize, Serialize};
use tempfile::Builder;
use tonic::Status;
use truth_catalog::{
    ScoreInboundFitEvaluator,
    admission::{admit_truth_intent, default_helms_capabilities, select_formation_for_intent},
    converge_binding_for_truth,
};
use uuid::Uuid;

use super::{
    TruthExecutionArtifacts, TruthProjection,
    common::{has_fact_id, optional_input, optional_uuid, payload_from_result, required_input},
    domain_event_kind_name, status_from_storage,
};

const REVENUE_PACK_ID: &str = "prio-revenue-pack";
const COMMERCIAL_PACK_ID: &str = "prio-commercial-pack";
const FEATURE_FACT_ID: &str = "lead:behavioral-features";
const FIT_SCORE_FACT_ID: &str = "lead:fit-score";
const FIT_EVIDENCE_FACT_ID: &str = "lead:fit-evidence";
const ANALYTICS_PROVENANCE: &str = "prio.score-inbound-fit.analytics";
const SCORING_PROVENANCE: &str = "prio.score-inbound-fit.model";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageEventSeed {
    visitor_id: String,
    timestamp: i64,
    event_type: String,
    page: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BehavioralFeaturesPayload {
    temporal: TemporalFeatures,
    vector: FeatureVector,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FitEvidencePayload {
    event_count: u32,
    unique_pages: u32,
    burst_score: u32,
    mean_delta_s: f64,
    type_entropy: f64,
    night_ratio: f64,
    burn_signal: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FitScorePayload {
    score_bps: u16,
    confidence_bps: u16,
    label: String,
    rationale: String,
}

#[derive(Debug, Clone)]
pub struct ScoreInboundFitInput {
    pub organization_name: String,
    pub visitor_id: String,
    pub usage_events_json: String,
    pub organization_id: Option<Uuid>,
    pub organization_external_key: Option<String>,
    pub website: Option<String>,
    pub industry: Option<String>,
}

impl ScoreInboundFitInput {
    pub fn from_map(inputs: &HashMap<String, String>) -> Result<Self, Status> {
        Ok(Self {
            organization_name: required_input(inputs, "organization_name")?.to_string(),
            visitor_id: required_input(inputs, "visitor_id")?.to_string(),
            usage_events_json: required_input(inputs, "usage_events_json")?.to_string(),
            organization_id: optional_uuid(inputs, "organization_id")?,
            organization_external_key: optional_input(inputs, "organization_external_key"),
            website: optional_input(inputs, "website"),
            industry: optional_input(inputs, "industry"),
        })
    }
}

pub(super) async fn execute<S: KernelStore>(
    store: &S,
    runtime_stores: &application_storage::AppRuntimeStores,
    inputs: ScoreInboundFitInput,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    let binding = converge_binding_for_truth("score-inbound-fit")
        .ok_or_else(|| Status::not_found("truth not found: score-inbound-fit"))?;

    let organization_name = inputs.organization_name.clone();
    let visitor_id = inputs.visitor_id.clone();
    let usage_events = usage_events_from_inputs(&inputs)?;

    let mut engine = Engine::new();
    engine.register_suggestor_in_pack(
        REVENUE_PACK_ID,
        BehavioralFeatureAgent {
            visitor_id,
            usage_events,
        },
    );
    engine.register_suggestor_in_pack(COMMERCIAL_PACK_ID, FitScoringAgent);

    let mut seed_ctx = seed_context(&organization_name)?;
    let intent = admit_truth_intent(
        "score-inbound-fit",
        &actor.actor_id,
        "truth:score-inbound-fit",
        &mut seed_ctx,
    )
    .map_err(|e| Status::internal(format!("admit intent failed: {e}")))?;
    let selection = select_formation_for_intent(&intent, &default_helms_capabilities())
        .map_err(|e| Status::internal(format!("formation selection failed: {e}")))?;
    tracing::info!(
        truth = "score-inbound-fit",
        primary = %selection.primary_template_id,
        alternates = ?selection.alternate_template_ids,
        "formation selected"
    );

    let runtime_ctx = super::RuntimeContext {
        scope_id: inputs
            .organization_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "inbound".to_string()),
    };
    let (result, experience_events) = super::run_engine_with_runtime(
        runtime_stores,
        &mut engine,
        &runtime_ctx,
        seed_ctx,
        &binding.intent,
        std::sync::Arc::new(ScoreInboundFitEvaluator),
    )
    .await?;

    let projection = if persist_projection {
        Some(project(store, &inputs, &result, actor)?)
    } else {
        None
    };

    Ok(TruthExecutionArtifacts {
        result,
        experience_events,
        projection,
        runtime_scope_id: runtime_ctx.scope_id,
    })
}

fn project<S: KernelStore>(
    store: &S,
    inputs: &ScoreInboundFitInput,
    result: &ConvergeResult,
    actor: CrmActor,
) -> Result<TruthProjection, Status> {
    let organization_name = inputs.organization_name.clone();
    let organization_id = inputs.organization_id;
    let organization_external_key = inputs.organization_external_key.clone();
    let website = inputs.website.clone();
    let industry = inputs.industry.clone();
    let fit_score = fit_score_payload_from_result(result)?;
    let evidence = fit_evidence_payload_from_result(result)?;

    let StoreWriteResult { value, events } = store
        .write_with_events(|kernel| {
            let organization = kernel.upsert_organization(
                OrganizationUpsert {
                    organization_id,
                    name: organization_name.clone(),
                    external_key: organization_external_key.clone(),
                    website: website.clone(),
                    industry: industry.clone(),
                    lifecycle: OrganizationLifecycle::Prospect,
                    owner_user_id: None,
                    tags: vec!["inbound-fit-scored".to_string()],
                },
                actor.clone(),
            )?;

            let related_to = vec![RecordRef {
                kind: RecordKind::Organization,
                id: organization.id,
            }];

            let score_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "Inbound fit score {} bps ({}) with rationale: {}",
                        fit_score.score_bps, fit_score.label, fit_score.rationale
                    ),
                    confidence_bps: fit_score.confidence_bps,
                    related_to: related_to.clone(),
                    source_note_id: None,
                },
                actor.clone(),
            )?;

            let evidence_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "Behavioral evidence: {} events, {} pages, burst {}, mean delta {:.0}s, entropy {:.2}, night ratio {:.2}",
                        evidence.event_count,
                        evidence.unique_pages,
                        evidence.burst_score,
                        evidence.mean_delta_s,
                        evidence.type_entropy,
                        evidence.night_ratio
                    ),
                    confidence_bps: fit_score.confidence_bps,
                    related_to,
                    source_note_id: None,
                },
                actor,
            )?;

            Ok((organization, vec![score_fact, evidence_fact]))
        })
        .map_err(status_from_storage)?;

    let (organization, facts) = value;
    Ok(TruthProjection {
        organization: Some(organization),
        person: None,
        opportunity: None,
        subscription: None,
        entitlements: Vec::new(),
        ledger_entries: Vec::new(),
        documents: Vec::new(),
        workflow_cases: Vec::new(),
        facts,
        domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
    })
}

struct BehavioralFeatureAgent {
    visitor_id: String,
    usage_events: Vec<UsageEventSeed>,
}

#[async_trait::async_trait]
impl Suggestor for BehavioralFeatureAgent {
    fn name(&self) -> &str {
        "BehavioralFeatureAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.has(ContextKey::Seeds) && !has_fact_id(ctx, ContextKey::Signals, FEATURE_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        let payload = match extract_behavioral_features(&self.visitor_id, &self.usage_events) {
            Ok(payload) => payload,
            Err(error) => {
                return AgentEffect::with_proposal(
                    crate::truth_runtime::common::proposed_text_fact(
                        ContextKey::Diagnostic,
                        "lead:fit-score:error",
                        error.to_string(),
                        "diagnostic",
                    )
                    .with_confidence(1.0),
                );
            }
        };
        let content = match serde_json::to_string(&payload) {
            Ok(content) => content,
            Err(error) => {
                return AgentEffect::with_proposal(
                    crate::truth_runtime::common::proposed_text_fact(
                        ContextKey::Diagnostic,
                        "lead:fit-score:error",
                        error.to_string(),
                        "diagnostic",
                    )
                    .with_confidence(1.0),
                );
            }
        };

        AgentEffect::with_proposal(
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Signals,
                FEATURE_FACT_ID,
                content,
                ANALYTICS_PROVENANCE,
            )
            .with_confidence(1.0),
        )
    }
}

struct FitScoringAgent;

#[async_trait::async_trait]
impl Suggestor for FitScoringAgent {
    fn name(&self) -> &str {
        "FitScoringAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Signals]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Signals, FEATURE_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Evaluations, FIT_SCORE_FACT_ID)
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        let Some(feature_fact) = ctx
            .get(ContextKey::Signals)
            .iter()
            .find(|fact| fact.id() == FEATURE_FACT_ID)
        else {
            return AgentEffect::empty();
        };
        let payload = match serde_json::from_str::<BehavioralFeaturesPayload>(
            &feature_fact.text().unwrap_or_default(),
        ) {
            Ok(payload) => payload,
            Err(error) => {
                return AgentEffect::with_proposal(
                    crate::truth_runtime::common::proposed_text_fact(
                        ContextKey::Diagnostic,
                        "lead:fit-score:error",
                        error.to_string(),
                        "diagnostic",
                    )
                    .with_confidence(1.0),
                );
            }
        };

        let burn_signal = bootstrap_burn_signal(&payload.vector);
        let score_bps = bootstrap_fit_score(&payload.temporal);
        let confidence_bps = fit_confidence_bps(&payload.temporal);
        let score_payload = FitScorePayload {
            score_bps,
            confidence_bps,
            label: fit_label(score_bps).to_string(),
            rationale: format!(
                "{} events across {} pages with burst {} and entropy {:.2}",
                payload.temporal.event_count,
                payload.temporal.unique_categories,
                payload.temporal.burst_score,
                payload.temporal.type_entropy
            ),
        };
        let evidence_payload = FitEvidencePayload {
            event_count: payload.temporal.event_count,
            unique_pages: payload.temporal.unique_categories,
            burst_score: payload.temporal.burst_score,
            mean_delta_s: payload.temporal.mean_delta_s,
            type_entropy: payload.temporal.type_entropy,
            night_ratio: payload.temporal.night_ratio,
            burn_signal,
        };

        let mut builder = AgentEffect::builder();
        builder.push(
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Evaluations,
                FIT_SCORE_FACT_ID,
                serde_json::to_string(&score_payload).unwrap_or_default(),
                SCORING_PROVENANCE,
            )
            .with_confidence(f64::from(confidence_bps) / 10_000.0),
        );
        builder.push(
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Signals,
                FIT_EVIDENCE_FACT_ID,
                serde_json::to_string(&evidence_payload).unwrap_or_default(),
                ANALYTICS_PROVENANCE,
            )
            .with_confidence(1.0),
        );
        builder.build()
    }
}

fn seed_context(organization_name: &str) -> Result<Context, Status> {
    let mut context = Context::new();
    context
        .add_input(
            ContextKey::Seeds,
            "score-inbound-fit:seed",
            organization_name,
        )
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(context)
}

fn usage_events_from_inputs(inputs: &ScoreInboundFitInput) -> Result<Vec<UsageEventSeed>, Status> {
    let events = serde_json::from_str::<Vec<UsageEventSeed>>(&inputs.usage_events_json)
        .map_err(|error| Status::invalid_argument(format!("invalid usage_events_json: {error}")))?;
    if events.is_empty() {
        return Err(Status::invalid_argument(
            "usage_events_json must not be empty",
        ));
    }
    Ok(events)
}

fn extract_behavioral_features(
    visitor_id: &str,
    usage_events: &[UsageEventSeed],
) -> anyhow::Result<BehavioralFeaturesPayload> {
    let mut file = Builder::new().suffix(".csv").tempfile()?;
    writeln!(file, "visitor_id,timestamp,event_type,page")?;
    for event in usage_events {
        writeln!(
            file,
            "{},{},{},{}",
            event.visitor_id, event.timestamp, event.event_type, event.page
        )?;
    }
    file.flush()?;

    let config = TemporalFeatureConfig {
        entity_column: "visitor_id".to_string(),
        timestamp_column: "timestamp".to_string(),
        type_column: "event_type".to_string(),
        category_column: "page".to_string(),
        burst_threshold_seconds: 90,
    };
    let feature_rows = extract_temporal_features(file.path().to_string_lossy().as_ref(), &config)?;
    let temporal = feature_rows
        .into_iter()
        .find(|row| row.entity_id == visitor_id)
        .or_else(|| {
            usage_events.first().and_then(|first| {
                if first.visitor_id == visitor_id {
                    None
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| TemporalFeatures {
            entity_id: visitor_id.to_string(),
            event_count: 0,
            mean_delta_s: 0.0,
            min_delta_s: 0.0,
            std_delta_s: 0.0,
            burst_score: 0,
            type_entropy: 0.0,
            unique_categories: 0,
            night_ratio: 0.0,
        });
    let vector = temporal_to_feature_vector(std::slice::from_ref(&temporal))?;
    Ok(BehavioralFeaturesPayload { temporal, vector })
}

fn fit_score_payload_from_result(result: &ConvergeResult) -> Result<FitScorePayload, Status> {
    payload_from_result(result, ContextKey::Evaluations, FIT_SCORE_FACT_ID)
}

fn fit_evidence_payload_from_result(result: &ConvergeResult) -> Result<FitEvidencePayload, Status> {
    payload_from_result(result, ContextKey::Signals, FIT_EVIDENCE_FACT_ID)
}

fn bootstrap_burn_signal(vector: &FeatureVector) -> Option<f32> {
    let config = ModelConfig::new(8, 16, 1);
    run_batch_inference(&config, vector)
        .ok()
        .and_then(|values| values.into_iter().next())
}

fn bootstrap_fit_score(features: &TemporalFeatures) -> u16 {
    let event_component = (features.event_count.min(24) as f64 / 24.0) * 3_500.0;
    let burst_component = (features.burst_score.min(8) as f64 / 8.0) * 1_500.0;
    let depth_component = (features.unique_categories.min(10) as f64 / 10.0) * 1_600.0;
    let return_component = if features.mean_delta_s > 0.0 {
        ((259_200.0 - features.mean_delta_s).clamp(0.0, 259_200.0) / 259_200.0) * 1_600.0
    } else {
        600.0
    };
    let entropy_component = (features.type_entropy.clamp(0.0, 2.5) / 2.5) * 1_200.0;
    let night_penalty = features.night_ratio.clamp(0.0, 1.0) * 900.0;
    (1_200.0
        + event_component
        + burst_component
        + depth_component
        + return_component
        + entropy_component
        - night_penalty)
        .round()
        .clamp(0.0, 10_000.0) as u16
}

fn fit_confidence_bps(features: &TemporalFeatures) -> u16 {
    let density = (features.event_count.min(20) as f64 / 20.0) * 4_000.0;
    let diversity = (features.unique_categories.min(8) as f64 / 8.0) * 2_500.0;
    let cadence = if features.mean_delta_s > 0.0 {
        ((86_400.0 - features.mean_delta_s).clamp(0.0, 86_400.0) / 86_400.0) * 2_000.0
    } else {
        750.0
    };
    let entropy = (features.type_entropy.clamp(0.0, 2.5) / 2.5) * 1_500.0;
    (density + diversity + cadence + entropy)
        .round()
        .clamp(2_500.0, 9_500.0) as u16
}

fn fit_label(score_bps: u16) -> &'static str {
    match score_bps {
        8_000..=10_000 => "high-fit",
        5_500..=7_999 => "medium-fit",
        _ => "low-fit",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use application_kernel::Actor;
    use application_storage::InMemoryKernelStore;

    #[tokio::test]
    async fn score_inbound_fit_executes_end_to_end() {
        let store = InMemoryKernelStore::default_local();
        let runtime_stores = application_storage::AppRuntimeStores {
            context: application_storage::AppContextStore::Memory(
                application_storage::InMemoryContextStore::new(),
            ),
            experience: application_storage::AppExperienceStore::Memory(
                application_storage::InMemoryExperienceStoreAdapter::new(),
            ),
        };
        let actor = Actor::system();
        let inputs = ScoreInboundFitInput {
            organization_name: "Aprio Labs".to_string(),
            visitor_id: "visitor-123".to_string(),
            usage_events_json: serde_json::json!([
                {
                    "visitor_id": "visitor-123",
                    "timestamp": 1_710_000_000_i64,
                    "event_type": "page_view",
                    "page": "/pricing"
                },
                {
                    "visitor_id": "visitor-123",
                    "timestamp": 1_710_000_120_i64,
                    "event_type": "page_view",
                    "page": "/case-studies"
                },
                {
                    "visitor_id": "visitor-123",
                    "timestamp": 1_710_086_400_i64,
                    "event_type": "page_view",
                    "page": "/contact"
                }
            ])
            .to_string(),
            organization_id: None,
            organization_external_key: None,
            website: None,
            industry: None,
        };

        let execution = execute(&store, &runtime_stores, inputs, actor, true)
            .await
            .expect("truth should execute");
        assert!(execution.result.converged);
        assert!(
            execution
                .result
                .criteria_outcomes
                .iter()
                .all(|outcome| matches!(
                    outcome.result,
                    converge_kernel::CriterionResult::Met { .. }
                ))
        );
        assert!(execution.experience_events.iter().any(|event| matches!(
            event.kind(),
            converge_kernel::ExperienceEventKind::FactPromoted
        )));

        let projection = execution.projection.expect("projection should persist");
        assert!(projection.organization.is_some());
        assert_eq!(projection.facts.len(), 2);
    }
}
