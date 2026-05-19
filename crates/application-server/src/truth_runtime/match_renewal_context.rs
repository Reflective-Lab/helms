use std::collections::HashMap;
use std::sync::Arc;

use application_kernel::{
    AccountSummary, Actor as CrmActor, DocumentAttach, DocumentStatus, FactRecord, RecordKind,
    RecordRef, WorkflowCaseAdvance, WorkflowCaseCreate, WorkflowPriority, WorkflowState,
};
use application_storage::{KernelStore, StorageError, StoreWriteResult};
use converge_kernel::{ContextState as Context, ConvergeResult, Engine};
use converge_knowledge::{KnowledgeBase, KnowledgeEntry, SearchOptions};
use converge_pack::{AgentEffect, Context as ContextView, ContextKey, Suggestor};
use serde::{Deserialize, Serialize};
use tonic::Status;
use truth_catalog::{
    MatchRenewalContextEvaluator,
    admission::{admit_truth_intent, default_helms_capabilities, select_formation_for_intent},
    converge_binding_for_truth,
};
use uuid::Uuid;

use super::{
    TruthExecutionArtifacts, TruthProjection,
    common::{block_on_async, has_fact_id, optional_uuid, payload_from_result, required_uuid},
    domain_event_kind_name, status_from_storage,
};

const RELATIONSHIP_PACK_ID: &str = "prio-relationship-pack";
const COMMERCIAL_PACK_ID: &str = "prio-commercial-pack";
const WORK_PACK_ID: &str = "prio-work-pack";
const KNOWLEDGE_PACK_ID: &str = "knowledge";
const CONTEXT_INDEX_FACT_ID: &str = "renewal:context-indexed";
const RENEWAL_BRIEF_FACT_ID: &str = "renewal:brief";
const RENEWAL_TERMS_FACT_ID: &str = "renewal:terms";
const KNOWLEDGE_PROVENANCE: &str = "prio.match-renewal-context.knowledge";
const BRIEF_PROVENANCE: &str = "prio.match-renewal-context.brief";
const TERMS_PROVENANCE: &str = "prio.match-renewal-context.terms";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RenewalIndexPayload {
    organization_id: Uuid,
    entry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RenewalSignalPayload {
    signal_id: String,
    query: String,
    title: String,
    summary: String,
    source: Option<String>,
    similarity_bps: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RenewalBriefPayload {
    summary: String,
    strengths: Vec<String>,
    risks: Vec<String>,
    opportunities: Vec<String>,
    talking_points: Vec<String>,
    confidence_bps: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RenewalTermsPayload {
    recommendation: String,
    rationale: String,
    approval_required: bool,
    confidence_bps: u16,
}

#[derive(Clone)]
struct ContextGathererAgent<S: KernelStore> {
    store: S,
    organization_id: Uuid,
    knowledge_base: Arc<KnowledgeBase>,
}

#[derive(Clone)]
struct RenewalSignalAgent {
    knowledge_base: Arc<KnowledgeBase>,
}

struct NegotiationBriefAgent;

struct RenewalTermsAgent;

#[derive(Debug, Clone)]
pub struct MatchRenewalContextInput {
    pub organization_id: Uuid,
    pub opportunity_id: Option<Uuid>,
}

impl MatchRenewalContextInput {
    pub fn from_map(inputs: &HashMap<String, String>) -> Result<Self, Status> {
        Ok(Self {
            organization_id: required_uuid(inputs, "organization_id")?,
            opportunity_id: optional_uuid(inputs, "opportunity_id")?,
        })
    }
}

pub(super) async fn execute<S: KernelStore>(
    store: &S,
    runtime_stores: &application_storage::AppRuntimeStores,
    inputs: MatchRenewalContextInput,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    let binding = converge_binding_for_truth("match-renewal-context")
        .ok_or_else(|| Status::not_found("truth not found: match-renewal-context"))?;

    let organization_id = inputs.organization_id;
    let scratch = tempfile::tempdir().map_err(|error| {
        Status::internal(format!("failed to create renewal scratch dir: {error}"))
    })?;
    let kb_path = scratch.path().join("renewal.kb");
    let kb_path_owned = kb_path.clone();
    let knowledge_base = Arc::new(
        block_on_async(async move { KnowledgeBase::open(kb_path_owned).await })
            .map_err(|error| Status::internal(format!("failed to open knowledge base: {error}")))?,
    );

    let mut engine = Engine::new();
    engine.register_suggestor_in_pack(
        RELATIONSHIP_PACK_ID,
        ContextGathererAgent {
            store: store.clone(),
            organization_id,
            knowledge_base: knowledge_base.clone(),
        },
    );
    engine.register_suggestor_in_pack(
        KNOWLEDGE_PACK_ID,
        RenewalSignalAgent {
            knowledge_base: knowledge_base.clone(),
        },
    );
    engine.register_suggestor_in_pack(WORK_PACK_ID, NegotiationBriefAgent);
    engine.register_suggestor_in_pack(COMMERCIAL_PACK_ID, RenewalTermsAgent);

    let mut seed_ctx = seed_context(organization_id)?;
    let intent = admit_truth_intent(
        "match-renewal-context",
        &actor.actor_id,
        "truth:match-renewal-context",
        &mut seed_ctx,
    )
    .map_err(|e| Status::internal(format!("admit intent failed: {e}")))?;
    let selection = select_formation_for_intent(&intent, &default_helms_capabilities())
        .map_err(|e| Status::internal(format!("formation selection failed: {e}")))?;
    tracing::info!(
        truth = "match-renewal-context",
        primary = %selection.primary_template_id,
        alternates = ?selection.alternate_template_ids,
        "formation selected"
    );

    let runtime_ctx = super::RuntimeContext {
        scope_id: inputs.organization_id.to_string(),
    };
    let (result, experience_events) = super::run_engine_with_runtime(
        runtime_stores,
        &mut engine,
        &runtime_ctx,
        seed_ctx,
        &binding.intent,
        std::sync::Arc::new(MatchRenewalContextEvaluator),
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

#[async_trait::async_trait]
impl<S: KernelStore> Suggestor for ContextGathererAgent<S> {
    fn name(&self) -> &str {
        "ContextGathererAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.has(ContextKey::Seeds) && !has_fact_id(ctx, ContextKey::Signals, CONTEXT_INDEX_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        let summary = match account_summary_from_store(&self.store, self.organization_id) {
            Ok(summary) => summary,
            Err(error) => {
                return AgentEffect::with_proposal(
                    crate::truth_runtime::common::proposed_text_fact(
                        ContextKey::Diagnostic,
                        "renewal:context:error",
                        error,
                        KNOWLEDGE_PROVENANCE,
                    )
                    .with_confidence(1.0),
                );
            }
        };
        let entries = knowledge_entries_from_summary(&summary);
        let entry_count = entries.len();
        let knowledge_base = self.knowledge_base.clone();
        let entries_for_ingest = entries.clone();
        if let Err(error) =
            block_on_async(async move { knowledge_base.add_entries(entries_for_ingest).await })
        {
            return AgentEffect::with_proposal(
                crate::truth_runtime::common::proposed_text_fact(
                    ContextKey::Diagnostic,
                    "renewal:context:error",
                    error.to_string(),
                    KNOWLEDGE_PROVENANCE,
                )
                .with_confidence(1.0),
            );
        }

        AgentEffect::with_proposal(
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Signals,
                CONTEXT_INDEX_FACT_ID,
                serde_json::to_string(&RenewalIndexPayload {
                    organization_id: self.organization_id,
                    entry_count,
                })
                .unwrap_or_default(),
                KNOWLEDGE_PROVENANCE,
            )
            .with_confidence(1.0),
        )
    }
}

#[async_trait::async_trait]
impl Suggestor for RenewalSignalAgent {
    fn name(&self) -> &str {
        "RenewalSignalAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Signals]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Signals, CONTEXT_INDEX_FACT_ID)
            && !ctx
                .get(ContextKey::Signals)
                .iter()
                .any(|fact| fact.id().starts_with("renewal:signal:"))
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        let queries = [
            (
                "competitive-evaluation",
                "competitive evaluation competitor dissatisfaction",
            ),
            ("support-incident", "incident outage severity escalation"),
            (
                "expansion-interest",
                "expansion growth upgrade usage increase",
            ),
            (
                "stakeholder-change",
                "stakeholder change executive sponsor champion",
            ),
        ];

        let mut builder = AgentEffect::builder();
        for (signal_id, query) in queries {
            let options = SearchOptions::new(1)
                .with_min_similarity(0.0)
                .with_diversity(0.2)
                .hybrid(0.35);
            let knowledge_base = self.knowledge_base.clone();
            let query = query.to_string();
            let query_for_search = query.clone();
            let results = match block_on_async(async move {
                knowledge_base.search(&query_for_search, options).await
            }) {
                Ok(results) => results,
                Err(error) => {
                    builder.push(
                        crate::truth_runtime::common::proposed_text_fact(
                            ContextKey::Diagnostic,
                            format!("renewal:signal:error:{signal_id}"),
                            error.to_string(),
                            KNOWLEDGE_PROVENANCE,
                        )
                        .with_confidence(1.0),
                    );
                    continue;
                }
            };
            let Some(result) = results.first() else {
                continue;
            };
            let payload = RenewalSignalPayload {
                signal_id: signal_id.to_string(),
                query: query.to_string(),
                title: result.entry.title.clone(),
                summary: summarize(&result.entry.content, 180),
                source: result.entry.source.clone(),
                similarity_bps: (result.similarity.clamp(0.0, 1.0) * 10_000.0).round() as u16,
            };
            builder.push(
                crate::truth_runtime::common::proposed_text_fact(
                    ContextKey::Signals,
                    format!("renewal:signal:{signal_id}"),
                    serde_json::to_string(&payload).unwrap_or_default(),
                    KNOWLEDGE_PROVENANCE,
                )
                .with_confidence(result.similarity as f64),
            );
        }
        builder.build()
    }
}

#[async_trait::async_trait]
impl Suggestor for NegotiationBriefAgent {
    fn name(&self) -> &str {
        "NegotiationBriefAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Signals]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.get(ContextKey::Signals)
            .iter()
            .any(|fact| fact.id().starts_with("renewal:signal:"))
            && !has_fact_id(ctx, ContextKey::Strategies, RENEWAL_BRIEF_FACT_ID)
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        let signals = renewal_signals_from_context(ctx);
        if signals.is_empty() {
            return AgentEffect::empty();
        }

        let risks = signals
            .iter()
            .filter(|signal| {
                signal.signal_id.contains("competitive")
                    || signal.signal_id.contains("incident")
                    || signal.signal_id.contains("stakeholder")
            })
            .map(|signal| format!("{}: {}", signal.signal_id, signal.summary))
            .collect::<Vec<_>>();
        let opportunities = signals
            .iter()
            .filter(|signal| signal.signal_id.contains("expansion"))
            .map(|signal| signal.summary.clone())
            .collect::<Vec<_>>();
        let strengths = if risks.is_empty() {
            vec!["Account context is stable with no major negative retrieval signals.".to_string()]
        } else {
            vec![
                "Retrieved account context is dense enough to support a guided renewal discussion."
                    .to_string(),
            ]
        };
        let talking_points = signals
            .iter()
            .map(|signal| {
                format!(
                    "Discuss {} with evidence from {}",
                    signal.signal_id, signal.title
                )
            })
            .collect::<Vec<_>>();
        let summary = format!(
            "Renewal brief built from {} retrieved signals.",
            signals.len()
        );
        let confidence_bps = (6_500 + (signals.len().min(4) as u16 * 700)).min(9_200);
        let payload = RenewalBriefPayload {
            summary,
            strengths,
            risks,
            opportunities,
            talking_points,
            confidence_bps,
        };

        AgentEffect::with_proposal(
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Strategies,
                RENEWAL_BRIEF_FACT_ID,
                serde_json::to_string(&payload).unwrap_or_default(),
                BRIEF_PROVENANCE,
            )
            .with_confidence(f64::from(confidence_bps) / 10_000.0),
        )
    }
}

#[async_trait::async_trait]
impl Suggestor for RenewalTermsAgent {
    fn name(&self) -> &str {
        "RenewalTermsAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Strategies]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Strategies, RENEWAL_BRIEF_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Strategies, RENEWAL_TERMS_FACT_ID)
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        let Some(brief_fact) = ctx
            .get(ContextKey::Strategies)
            .iter()
            .find(|fact| fact.id() == RENEWAL_BRIEF_FACT_ID)
        else {
            return AgentEffect::empty();
        };
        let brief = match serde_json::from_str::<RenewalBriefPayload>(
            &brief_fact.text().unwrap_or_default(),
        ) {
            Ok(brief) => brief,
            Err(error) => {
                return AgentEffect::with_proposal(
                    crate::truth_runtime::common::proposed_text_fact(
                        ContextKey::Diagnostic,
                        "renewal:terms:error",
                        error.to_string(),
                        TERMS_PROVENANCE,
                    )
                    .with_confidence(1.0),
                );
            }
        };

        let approval_required = brief
            .risks
            .iter()
            .any(|risk| risk.contains("competitive") || risk.contains("incident"));
        let recommendation = if !brief.opportunities.is_empty() && !approval_required {
            "propose expansion-oriented renewal".to_string()
        } else if approval_required {
            "prepare standard renewal with explicit mitigation and approval".to_string()
        } else {
            "propose standard renewal".to_string()
        };
        let payload = RenewalTermsPayload {
            rationale: brief.summary.clone(),
            approval_required,
            confidence_bps: if approval_required { 6_200 } else { 8_100 },
            recommendation,
        };

        AgentEffect::with_proposal(
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Strategies,
                RENEWAL_TERMS_FACT_ID,
                serde_json::to_string(&payload).unwrap_or_default(),
                TERMS_PROVENANCE,
            )
            .with_confidence(f64::from(payload.confidence_bps) / 10_000.0),
        )
    }
}

fn project<S: KernelStore>(
    store: &S,
    inputs: &MatchRenewalContextInput,
    result: &ConvergeResult,
    actor: CrmActor,
) -> Result<TruthProjection, Status> {
    let organization_id = inputs.organization_id;
    let opportunity_id = inputs.opportunity_id;
    let _brief = renewal_brief_from_result(result)?;
    let terms = renewal_terms_from_result(result)?;
    let signals = renewal_signals_from_result(result)?;
    let organization = account_summary_from_store(store, organization_id)
        .map_err(Status::failed_precondition)?
        .organization;

    let mut related_to = vec![RecordRef {
        kind: RecordKind::Organization,
        id: organization.id,
    }];
    if let Some(opportunity_id) = opportunity_id {
        related_to.push(RecordRef {
            kind: RecordKind::Opportunity,
            id: opportunity_id,
        });
    }

    let StoreWriteResult { value, events } = store
        .write_with_events(|kernel| {
            let document = kernel.attach_document(
                DocumentAttach {
                    title: format!("Renewal brief: {}", organization.name),
                    media_type: "text/markdown".to_string(),
                    uri: format!(
                        "converge://truths/match-renewal-context/{}/brief.md",
                        organization.id
                    ),
                    status: DocumentStatus::Draft,
                    related_to: related_to.clone(),
                },
                actor.clone(),
            )?;

            let mut projected_facts = Vec::new();
            for signal in &signals {
                projected_facts.push(kernel.record_fact(
                    FactRecord {
                        statement: format!(
                            "Renewal signal {} from {}: {}",
                            signal.signal_id, signal.title, signal.summary
                        ),
                        confidence_bps: signal.similarity_bps,
                        related_to: related_to.clone(),
                        source_note_id: None,
                    },
                    actor.clone(),
                )?);
            }
            projected_facts.push(kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "Renewal terms recommendation: {} ({})",
                        terms.recommendation, terms.rationale
                    ),
                    confidence_bps: terms.confidence_bps,
                    related_to: related_to.clone(),
                    source_note_id: None,
                },
                actor.clone(),
            )?);

            let workflow_case = if terms.approval_required {
                let case = kernel.create_workflow_case(
                    WorkflowCaseCreate {
                        title: format!("Renewal approval: {}", organization.name),
                        priority: WorkflowPriority::High,
                        owner_user_id: None,
                        related_to: related_to.clone(),
                    },
                    actor.clone(),
                )?;
                Some(kernel.advance_workflow_case(
                    WorkflowCaseAdvance {
                        workflow_case_id: case.id,
                        state: WorkflowState::AwaitingApproval,
                    },
                    actor,
                )?)
            } else {
                None
            };

            Ok((document, projected_facts, workflow_case))
        })
        .map_err(status_from_storage)?;

    let (document, facts, workflow_case) = value;
    Ok(TruthProjection {
        organization: Some(organization),
        person: None,
        opportunity: None,
        subscription: None,
        entitlements: Vec::new(),
        ledger_entries: Vec::new(),
        documents: vec![document],
        workflow_cases: workflow_case.into_iter().collect(),
        facts,
        domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
    })
}

fn seed_context(organization_id: Uuid) -> Result<Context, Status> {
    let mut context = Context::new();
    context
        .add_input(
            ContextKey::Seeds,
            "match-renewal-context:seed",
            organization_id.to_string(),
        )
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(context)
}

fn account_summary_from_store<S: KernelStore>(
    store: &S,
    organization_id: Uuid,
) -> Result<AccountSummary, String> {
    match store.read(|kernel| kernel.get_account_summary(organization_id, 50)) {
        Ok(Ok(summary)) => Ok(summary),
        Ok(Err(error)) => Err(error.to_string()),
        Err(StorageError::LockPoisoned) => Err("storage lock poisoned".to_string()),
        Err(StorageError::Kernel(error)) => Err(error.to_string()),
        Err(StorageError::ConnectionFailed { message, .. }) => Err(message),
        Err(StorageError::SerializationFailed { message }) => Err(message),
        Err(StorageError::Timeout { operation }) => Err(operation),
        Err(StorageError::RuntimeStore { message }) => Err(message),
    }
}

fn knowledge_entries_from_summary(summary: &AccountSummary) -> Vec<KnowledgeEntry> {
    let mut entries = Vec::new();
    entries.push(
        KnowledgeEntry::new(
            format!("Account {}", summary.organization.name),
            format!(
                "Industry: {:?}. Website: {:?}. Tags: {}",
                summary.organization.industry,
                summary.organization.website,
                summary.organization.tags.join(", ")
            ),
        )
        .with_category("organization")
        .with_tags(["organization", "renewal"])
        .with_source(format!("crm://organization/{}", summary.organization.id)),
    );
    entries.extend(summary.documents.iter().map(|document| {
        KnowledgeEntry::new(&document.title, format!("Document URI {}", document.uri))
            .with_category("document")
            .with_tags(["document", "renewal"])
            .with_source(format!("crm://document/{}", document.id))
    }));
    entries.extend(summary.facts.iter().map(|fact| {
        KnowledgeEntry::new("CRM fact", &fact.statement)
            .with_category("fact")
            .with_tags(["fact", "renewal"])
            .with_source(format!("crm://fact/{}", fact.id))
    }));
    entries.extend(summary.recent_timeline.iter().map(|entry| {
        KnowledgeEntry::new(&entry.headline, &entry.body)
            .with_category("timeline")
            .with_tags(["timeline", "renewal"])
            .with_source(format!("crm://timeline/{}", entry.id))
    }));
    entries
}

fn renewal_brief_from_result(result: &ConvergeResult) -> Result<RenewalBriefPayload, Status> {
    payload_from_result(result, ContextKey::Strategies, RENEWAL_BRIEF_FACT_ID)
}

fn renewal_terms_from_result(result: &ConvergeResult) -> Result<RenewalTermsPayload, Status> {
    payload_from_result(result, ContextKey::Strategies, RENEWAL_TERMS_FACT_ID)
}

fn renewal_signals_from_result(
    result: &ConvergeResult,
) -> Result<Vec<RenewalSignalPayload>, Status> {
    let signals = result
        .context
        .get(ContextKey::Signals)
        .iter()
        .filter(|fact| fact.id().starts_with("renewal:signal:"))
        .map(|fact| {
            serde_json::from_str::<RenewalSignalPayload>(&fact.text().unwrap_or_default()).map_err(
                |error| {
                    Status::internal(format!(
                        "invalid renewal signal payload {}: {error}",
                        fact.id()
                    ))
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(signals)
}

fn renewal_signals_from_context(ctx: &dyn ContextView) -> Vec<RenewalSignalPayload> {
    ctx.get(ContextKey::Signals)
        .iter()
        .filter(|fact| fact.id().starts_with("renewal:signal:"))
        .filter_map(|fact| {
            serde_json::from_str::<RenewalSignalPayload>(&fact.text().unwrap_or_default()).ok()
        })
        .collect()
}

fn summarize(content: &str, max_len: usize) -> String {
    let content = content.trim();
    if content.len() <= max_len {
        content.to_string()
    } else {
        format!("{}...", &content[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use application_kernel::{
        Actor, FactRecord, OrganizationLifecycle, OrganizationUpsert, RecordKind, RecordRef,
    };
    use application_storage::InMemoryKernelStore;

    #[tokio::test]
    async fn match_renewal_context_executes_end_to_end() {
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
        let organization_id = store
            .write(|kernel| {
                let organization = kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Acme Renewals".to_string(),
                        external_key: None,
                        website: Some("https://acme.example".to_string()),
                        industry: Some("Software".to_string()),
                        lifecycle: OrganizationLifecycle::Active,
                        owner_user_id: None,
                        tags: vec!["renewal".to_string()],
                    },
                    actor.clone(),
                )?;
                let related_to = vec![RecordRef {
                    kind: RecordKind::Organization,
                    id: organization.id,
                }];
                let _ = kernel.record_fact(
                    FactRecord {
                        statement: "Customer mentioned competitor evaluation in a QBR.".to_string(),
                        confidence_bps: 8_400,
                        related_to: related_to.clone(),
                        source_note_id: None,
                    },
                    actor.clone(),
                )?;
                let _ = kernel.attach_document(
                    DocumentAttach {
                        title: "Q3 incident review".to_string(),
                        media_type: "text/plain".to_string(),
                        uri: "converge://docs/q3-incident-review.txt".to_string(),
                        status: DocumentStatus::Verified,
                        related_to,
                    },
                    actor.clone(),
                )?;
                Ok(organization.id)
            })
            .expect("seed organization");

        let inputs = MatchRenewalContextInput {
            organization_id,
            opportunity_id: None,
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

        let projection = execution.projection.expect("projection should persist");
        assert!(projection.organization.is_some());
        assert_eq!(projection.documents.len(), 1);
        assert!(!projection.facts.is_empty());
    }
}
