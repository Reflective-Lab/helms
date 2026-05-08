use std::collections::HashMap;

use application_kernel::{
    ActivityAppend, ActivityOutcome, Actor as CrmActor, DocumentAttach, DocumentStatus, FactRecord,
    RecordKind, RecordRef, WorkflowCaseAdvance, WorkflowCaseCreate, WorkflowPriority,
    WorkflowState,
};
use application_storage::{KernelStore, StoreWriteResult};
use converge_kernel::{ContextState as Context, ConvergeResult, Engine};
use converge_optimization::Pack;
use converge_optimization::packs::lead_routing::{
    Lead as RoutingLead, LeadRoutingInput, LeadRoutingOutput, LeadRoutingPack, RoutingConfig,
    SalesRep,
};
use converge_pack::gate::{ObjectiveSpec, ProblemSpec};
use converge_pack::{AgentEffect, Context as ContextView, ContextKey, ProposedFact, Suggestor};
use serde::{Deserialize, Serialize};
use tonic::Status;
use truth_catalog::{
    PlanOutboundCampaignEvaluator,
    admission::{admit_truth_intent, default_helms_capabilities, select_formation_for_intent},
    converge_binding_for_truth,
};
use uuid::Uuid;

use super::{
    TruthExecutionArtifacts, TruthProjection,
    common::{
        converge_confidence_to_bps, has_fact_id, optional_i64, payload_from_result, required_input,
    },
    domain_event_kind_name, status_from_storage,
};

const COMMERCIAL_PACK_ID: &str = "prio-commercial-pack";
const WORK_PACK_ID: &str = "prio-work-pack";
const REVENUE_PACK_ID: &str = "prio-revenue-pack";
const ROUTING_INPUT_FACT_ID: &str = "campaign:lead-routing-input";
const CAPACITY_STATUS_FACT_ID: &str = "campaign:capacity-status";
const CAMPAIGN_PLAN_FACT_ID: &str = "campaign:plan";
const BUDGET_STATUS_FACT_ID: &str = "campaign:budget-status";
const PLAN_PROVENANCE: &str = "prio.plan-outbound-campaign.optimization";
const CAPACITY_PROVENANCE: &str = "prio.plan-outbound-campaign.capacity";
const BUDGET_PROVENANCE: &str = "prio.plan-outbound-campaign.budget";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignProspectSeed {
    lead_id: String,
    organization_id: Option<Uuid>,
    score: f64,
    territory: String,
    segment: String,
    #[serde(default)]
    required_skills: Vec<String>,
    #[serde(default)]
    estimated_value: f64,
    #[serde(default = "default_priority")]
    priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignRepSeed {
    rep_id: String,
    name: String,
    capacity: i64,
    current_load: i64,
    territories: Vec<String>,
    segments: Vec<String>,
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default = "default_performance_score")]
    performance_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CapacityStatusPayload {
    total_capacity: i64,
    total_available_capacity: i64,
    rep_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignAssignmentPayload {
    lead_id: String,
    organization_id: Option<Uuid>,
    rep_id: String,
    rep_name: String,
    fit_score: f64,
    rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignPlanPayload {
    campaign_name: String,
    summary: String,
    assignments: Vec<CampaignAssignmentPayload>,
    unassigned_leads: Vec<String>,
    average_fit_score: f64,
    confidence_bps: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BudgetStatusPayload {
    within_budget: bool,
    estimated_spend_minor: i64,
    budget_minor: i64,
    approval_required: bool,
}

#[derive(Clone)]
struct ProspectPoolAgent {
    prospects: Vec<CampaignProspectSeed>,
    reps: Vec<CampaignRepSeed>,
}

struct RepCapacityAgent {
    reps: Vec<CampaignRepSeed>,
}

struct CampaignSolverAgent {
    campaign_name: String,
    prospects: Vec<CampaignProspectSeed>,
}

struct BudgetGuardAgent {
    campaign_budget_minor: i64,
    outreach_cost_minor: i64,
}

#[derive(Debug, Clone)]
pub struct PlanOutboundCampaignInput {
    pub campaign_name: String,
    pub prospects_json: String,
    pub reps_json: String,
    pub campaign_budget_minor: i64,
    pub outreach_cost_minor: Option<i64>,
}

impl PlanOutboundCampaignInput {
    pub fn from_map(inputs: &HashMap<String, String>) -> Result<Self, Status> {
        Ok(Self {
            campaign_name: required_input(inputs, "campaign_name")?.to_string(),
            prospects_json: required_input(inputs, "prospects_json")?.to_string(),
            reps_json: required_input(inputs, "reps_json")?.to_string(),
            campaign_budget_minor: required_input(inputs, "campaign_budget_minor")?
                .parse()
                .map_err(|e| {
                    Status::invalid_argument(format!("invalid campaign_budget_minor: {e}"))
                })?,
            outreach_cost_minor: optional_i64(inputs, "outreach_cost_minor"),
        })
    }
}

pub(super) async fn execute<S: KernelStore>(
    store: &S,
    runtime_stores: &application_storage::AppRuntimeStores,
    inputs: PlanOutboundCampaignInput,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    let binding = converge_binding_for_truth("plan-outbound-campaign")
        .ok_or_else(|| Status::not_found("truth not found: plan-outbound-campaign"))?;

    let campaign_name = inputs.campaign_name.clone();
    let prospects = prospects_from_inputs(&inputs)?;
    let reps = reps_from_inputs(&inputs)?;
    let campaign_budget_minor = inputs.campaign_budget_minor;
    let outreach_cost_minor = inputs.outreach_cost_minor.unwrap_or(2_500);

    let mut engine = Engine::new();
    engine.register_suggestor_in_pack(
        COMMERCIAL_PACK_ID,
        ProspectPoolAgent {
            prospects: prospects.clone(),
            reps: reps.clone(),
        },
    );
    engine.register_suggestor_in_pack(WORK_PACK_ID, RepCapacityAgent { reps: reps.clone() });
    engine.register_suggestor_in_pack(
        COMMERCIAL_PACK_ID,
        CampaignSolverAgent {
            campaign_name,
            prospects,
        },
    );
    engine.register_suggestor_in_pack(
        REVENUE_PACK_ID,
        BudgetGuardAgent {
            campaign_budget_minor,
            outreach_cost_minor,
        },
    );

    let mut seed_ctx = seed_context()?;
    let intent = admit_truth_intent(
        "plan-outbound-campaign",
        &actor.actor_id,
        "truth:plan-outbound-campaign",
        &mut seed_ctx,
    )
    .map_err(|e| Status::internal(format!("admit intent failed: {e}")))?;
    let selection = select_formation_for_intent(&intent, &default_helms_capabilities())
        .map_err(|e| Status::internal(format!("formation selection failed: {e}")))?;
    tracing::info!(
        truth = "plan-outbound-campaign",
        primary = %selection.primary_template_id,
        alternates = ?selection.alternate_template_ids,
        "formation selected"
    );

    let (result, experience_events) = super::run_engine_with_runtime(
        runtime_stores,
        &mut engine,
        &super::RuntimeContext {
            scope_id: slug(&inputs.campaign_name),
        },
        seed_ctx,
        &binding.intent,
        std::sync::Arc::new(PlanOutboundCampaignEvaluator),
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
    })
}

#[async_trait::async_trait]
impl Suggestor for ProspectPoolAgent {
    fn name(&self) -> &str {
        "ProspectPoolAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.has(ContextKey::Seeds)
            && !has_fact_id(ctx, ContextKey::Proposals, ROUTING_INPUT_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        let routing_input = LeadRoutingInput {
            leads: self
                .prospects
                .iter()
                .map(|prospect| RoutingLead {
                    id: prospect.lead_id.clone(),
                    score: prospect.score,
                    territory: prospect.territory.clone(),
                    segment: prospect.segment.clone(),
                    required_skills: prospect.required_skills.clone(),
                    estimated_value: prospect.estimated_value,
                    priority: prospect.priority,
                })
                .collect(),
            reps: self
                .reps
                .iter()
                .map(|rep| SalesRep {
                    id: rep.rep_id.clone(),
                    name: rep.name.clone(),
                    capacity: rep.capacity,
                    current_load: rep.current_load,
                    territories: rep.territories.clone(),
                    segments: rep.segments.clone(),
                    skills: rep.skills.clone(),
                    performance_score: rep.performance_score,
                })
                .collect(),
            config: RoutingConfig::default(),
        };

        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Proposals,
                ROUTING_INPUT_FACT_ID.to_string(),
                serde_json::to_string(&routing_input).unwrap_or_default(),
                PLAN_PROVENANCE.to_string(),
            )
            .with_confidence(1.0),
        )
    }
}

#[async_trait::async_trait]
impl Suggestor for RepCapacityAgent {
    fn name(&self) -> &str {
        "RepCapacityAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.has(ContextKey::Seeds)
            && !has_fact_id(ctx, ContextKey::Signals, CAPACITY_STATUS_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        let payload = CapacityStatusPayload {
            total_capacity: self.reps.iter().map(|rep| rep.capacity).sum(),
            total_available_capacity: self
                .reps
                .iter()
                .map(|rep| (rep.capacity - rep.current_load).max(0))
                .sum(),
            rep_count: self.reps.len(),
        };
        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Signals,
                CAPACITY_STATUS_FACT_ID.to_string(),
                serde_json::to_string(&payload).unwrap_or_default(),
                CAPACITY_PROVENANCE.to_string(),
            )
            .with_confidence(1.0),
        )
    }
}

#[async_trait::async_trait]
impl Suggestor for CampaignSolverAgent {
    fn name(&self) -> &str {
        "CampaignSolverAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Proposals, ContextKey::Signals]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Proposals, ROUTING_INPUT_FACT_ID)
            && has_fact_id(ctx, ContextKey::Signals, CAPACITY_STATUS_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Strategies, CAMPAIGN_PLAN_FACT_ID)
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        let Some(input_fact) = ctx
            .get(ContextKey::Proposals)
            .iter()
            .find(|fact| fact.id() == ROUTING_INPUT_FACT_ID)
        else {
            return AgentEffect::empty();
        };
        let routing_input = match serde_json::from_str::<LeadRoutingInput>(&input_fact.content()) {
            Ok(input) => input,
            Err(error) => {
                return AgentEffect::with_proposal(
                    ProposedFact::new(
                        ContextKey::Diagnostic,
                        "campaign:plan:error",
                        error.to_string(),
                        "diagnostic",
                    )
                    .with_confidence(1.0),
                );
            }
        };

        let spec = match ProblemSpec::builder(
            format!("campaign-{}", slug(&self.campaign_name)),
            "crm.prio.ai",
        )
        .objective(ObjectiveSpec::maximize("conversion"))
        .inputs(&routing_input)
        .and_then(|builder| builder.build())
        {
            Ok(spec) => spec,
            Err(error) => {
                return AgentEffect::with_proposal(
                    ProposedFact::new(
                        ContextKey::Diagnostic,
                        "campaign:plan:error",
                        error.to_string(),
                        "diagnostic",
                    )
                    .with_confidence(1.0),
                );
            }
        };

        let pack = LeadRoutingPack;
        let solved = match pack.solve(&spec) {
            Ok(result) => result,
            Err(error) => {
                return AgentEffect::with_proposal(
                    ProposedFact::new(
                        ContextKey::Diagnostic,
                        "campaign:plan:error",
                        error.to_string(),
                        "diagnostic",
                    )
                    .with_confidence(1.0),
                );
            }
        };
        let output = match solved.plan.plan_as::<LeadRoutingOutput>() {
            Ok(output) => output,
            Err(error) => {
                return AgentEffect::with_proposal(
                    ProposedFact::new(
                        ContextKey::Diagnostic,
                        "campaign:plan:error",
                        error.to_string(),
                        "diagnostic",
                    )
                    .with_confidence(1.0),
                );
            }
        };
        let plan_payload = CampaignPlanPayload {
            campaign_name: self.campaign_name.clone(),
            summary: output.summary(),
            assignments: output
                .assignments
                .iter()
                .map(|assignment| CampaignAssignmentPayload {
                    lead_id: assignment.lead_id.clone(),
                    organization_id: self
                        .prospects
                        .iter()
                        .find(|prospect| prospect.lead_id == assignment.lead_id)
                        .and_then(|prospect| prospect.organization_id),
                    rep_id: assignment.rep_id.clone(),
                    rep_name: assignment.rep_name.clone(),
                    fit_score: assignment.fit_score,
                    rationale: assignment.scoring_rationale.explanation.clone(),
                })
                .collect(),
            unassigned_leads: output
                .unassigned
                .iter()
                .map(|lead| lead.lead_id.clone())
                .collect(),
            average_fit_score: output.stats.average_fit_score,
            confidence_bps: converge_confidence_to_bps(solved.plan.confidence()),
        };

        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Strategies,
                CAMPAIGN_PLAN_FACT_ID.to_string(),
                serde_json::to_string(&plan_payload).unwrap_or_default(),
                PLAN_PROVENANCE.to_string(),
            )
            .with_confidence(solved.plan.confidence()),
        )
    }
}

#[async_trait::async_trait]
impl Suggestor for BudgetGuardAgent {
    fn name(&self) -> &str {
        "BudgetGuardAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Strategies]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Strategies, CAMPAIGN_PLAN_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Evaluations, BUDGET_STATUS_FACT_ID)
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        let Some(plan_fact) = ctx
            .get(ContextKey::Strategies)
            .iter()
            .find(|fact| fact.id() == CAMPAIGN_PLAN_FACT_ID)
        else {
            return AgentEffect::empty();
        };
        let plan = match serde_json::from_str::<CampaignPlanPayload>(&plan_fact.content()) {
            Ok(plan) => plan,
            Err(error) => {
                return AgentEffect::with_proposal(
                    ProposedFact::new(
                        ContextKey::Diagnostic,
                        "campaign:plan:error",
                        error.to_string(),
                        "diagnostic",
                    )
                    .with_confidence(1.0),
                );
            }
        };

        let estimated_spend_minor = plan.assignments.len() as i64 * self.outreach_cost_minor;
        let within_budget = estimated_spend_minor <= self.campaign_budget_minor;
        let payload = BudgetStatusPayload {
            within_budget,
            estimated_spend_minor,
            budget_minor: self.campaign_budget_minor,
            approval_required: !within_budget,
        };
        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Evaluations,
                BUDGET_STATUS_FACT_ID.to_string(),
                serde_json::to_string(&payload).unwrap_or_default(),
                BUDGET_PROVENANCE.to_string(),
            )
            .with_confidence(1.0),
        )
    }
}

fn project<S: KernelStore>(
    store: &S,
    inputs: &PlanOutboundCampaignInput,
    result: &ConvergeResult,
    actor: CrmActor,
) -> Result<TruthProjection, Status> {
    let campaign_name = inputs.campaign_name.clone();
    let plan = campaign_plan_from_result(result)?;
    let budget = budget_status_from_result(result)?;
    let related_to = related_record_refs(&plan.assignments);
    if related_to.is_empty() {
        return Err(Status::invalid_argument(
            "campaign projection requires organization_id on at least one prospect",
        ));
    }

    let StoreWriteResult { value, events } = store
        .write_with_events(|kernel| {
            let workflow_case = kernel.create_workflow_case(
                WorkflowCaseCreate {
                    title: format!("Outbound campaign: {campaign_name}"),
                    priority: WorkflowPriority::High,
                    owner_user_id: None,
                    related_to: related_to.clone(),
                },
                actor.clone(),
            )?;

            let workflow_case = if budget.approval_required {
                kernel.advance_workflow_case(
                    WorkflowCaseAdvance {
                        workflow_case_id: workflow_case.id,
                        state: WorkflowState::AwaitingApproval,
                    },
                    actor.clone(),
                )?
            } else {
                workflow_case
            };

            let mut document_related_to = related_to.clone();
            document_related_to.push(RecordRef {
                kind: RecordKind::WorkflowCase,
                id: workflow_case.id,
            });
            let document = kernel.attach_document(
                DocumentAttach {
                    title: format!("Campaign plan: {campaign_name}"),
                    media_type: "application/json".to_string(),
                    uri: format!(
                        "converge://truths/plan-outbound-campaign/{}/plan.json",
                        slug(&campaign_name)
                    ),
                    status: DocumentStatus::Draft,
                    related_to: document_related_to.clone(),
                },
                actor.clone(),
            )?;

            for assignment in &plan.assignments {
                let mut activity_related_to = assignment
                    .organization_id
                    .map(|organization_id| {
                        vec![RecordRef {
                            kind: RecordKind::Organization,
                            id: organization_id,
                        }]
                    })
                    .unwrap_or_default();
                activity_related_to.push(RecordRef {
                    kind: RecordKind::WorkflowCase,
                    id: workflow_case.id,
                });
                let _ = kernel.append_activity(
                    ActivityAppend {
                        subject: format!("Outbound assignment for {}", assignment.lead_id),
                        details: format!(
                            "Assigned to {} ({}) with fit {:.1}: {}",
                            assignment.rep_name,
                            assignment.rep_id,
                            assignment.fit_score,
                            assignment.rationale
                        ),
                        related_to: activity_related_to,
                        outcome: ActivityOutcome::Waiting,
                        occurred_at: None,
                        next_action_due_at: None,
                    },
                    actor.clone(),
                )?;
            }

            let plan_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "Campaign plan {} assigns {} leads with average fit {:.1}",
                        campaign_name,
                        plan.assignments.len(),
                        plan.average_fit_score
                    ),
                    confidence_bps: plan.confidence_bps,
                    related_to: document_related_to.clone(),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            let budget_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "Campaign spend estimate {} against budget {} ({})",
                        budget.estimated_spend_minor,
                        budget.budget_minor,
                        if budget.within_budget {
                            "within-budget"
                        } else {
                            "approval-required"
                        }
                    ),
                    confidence_bps: 10_000,
                    related_to: document_related_to,
                    source_note_id: None,
                },
                actor,
            )?;

            Ok((workflow_case, document, vec![plan_fact, budget_fact]))
        })
        .map_err(status_from_storage)?;

    let (workflow_case, document, facts) = value;
    Ok(TruthProjection {
        organization: None,
        person: None,
        opportunity: None,
        subscription: None,
        entitlements: Vec::new(),
        ledger_entries: Vec::new(),
        documents: vec![document],
        workflow_cases: vec![workflow_case],
        facts,
        domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
    })
}

fn seed_context() -> Result<Context, Status> {
    let mut context = Context::new();
    context
        .add_input(
            ContextKey::Seeds,
            "plan-outbound-campaign:seed",
            "campaign-seed",
        )
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(context)
}

fn prospects_from_inputs(
    inputs: &PlanOutboundCampaignInput,
) -> Result<Vec<CampaignProspectSeed>, Status> {
    serde_json::from_str(&inputs.prospects_json)
        .map_err(|error| Status::invalid_argument(format!("invalid prospects_json: {error}")))
}

fn reps_from_inputs(inputs: &PlanOutboundCampaignInput) -> Result<Vec<CampaignRepSeed>, Status> {
    serde_json::from_str(&inputs.reps_json)
        .map_err(|error| Status::invalid_argument(format!("invalid reps_json: {error}")))
}

fn campaign_plan_from_result(result: &ConvergeResult) -> Result<CampaignPlanPayload, Status> {
    payload_from_result(result, ContextKey::Strategies, CAMPAIGN_PLAN_FACT_ID)
}

fn budget_status_from_result(result: &ConvergeResult) -> Result<BudgetStatusPayload, Status> {
    payload_from_result(result, ContextKey::Evaluations, BUDGET_STATUS_FACT_ID)
}

fn related_record_refs(assignments: &[CampaignAssignmentPayload]) -> Vec<RecordRef> {
    let mut refs = assignments
        .iter()
        .filter_map(|assignment| assignment.organization_id)
        .map(|organization_id| RecordRef {
            kind: RecordKind::Organization,
            id: organization_id,
        })
        .collect::<Vec<_>>();
    refs.sort_by_key(|reference| reference.id);
    refs.dedup_by_key(|reference| reference.id);
    refs
}

fn slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !slug.is_empty() && !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    if slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "campaign".to_string()
    } else {
        slug
    }
}

fn default_priority() -> i32 {
    5
}

fn default_performance_score() -> f64 {
    50.0
}

#[cfg(test)]
mod tests {
    use super::*;

    use application_kernel::Actor;
    use application_kernel::{OrganizationLifecycle, OrganizationUpsert};
    use application_storage::InMemoryKernelStore;

    #[tokio::test]
    async fn plan_outbound_campaign_executes_end_to_end() {
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
        let west_org_id = store
            .write(|kernel| {
                kernel
                    .upsert_organization(
                        OrganizationUpsert {
                            organization_id: None,
                            name: "West Prospect".to_string(),
                            external_key: None,
                            website: None,
                            industry: None,
                            lifecycle: OrganizationLifecycle::Prospect,
                            owner_user_id: None,
                            tags: vec!["campaign".to_string()],
                        },
                        actor.clone(),
                    )
                    .map(|organization| organization.id)
            })
            .expect("west prospect seed");
        let east_org_id = store
            .write(|kernel| {
                kernel
                    .upsert_organization(
                        OrganizationUpsert {
                            organization_id: None,
                            name: "East Prospect".to_string(),
                            external_key: None,
                            website: None,
                            industry: None,
                            lifecycle: OrganizationLifecycle::Prospect,
                            owner_user_id: None,
                            tags: vec!["campaign".to_string()],
                        },
                        actor.clone(),
                    )
                    .map(|organization| organization.id)
            })
            .expect("east prospect seed");
        let inputs = PlanOutboundCampaignInput {
            campaign_name: "Q2 outbound".to_string(),
            prospects_json: serde_json::json!([
                {
                    "lead_id": "lead-1",
                    "organization_id": west_org_id,
                    "score": 88.0,
                    "territory": "west",
                    "segment": "enterprise",
                    "required_skills": ["cloud"],
                    "estimated_value": 120000.0,
                    "priority": 1
                },
                {
                    "lead_id": "lead-2",
                    "organization_id": east_org_id,
                    "score": 72.0,
                    "territory": "east",
                    "segment": "smb",
                    "required_skills": [],
                    "estimated_value": 25000.0,
                    "priority": 3
                }
            ])
            .to_string(),
            reps_json: serde_json::json!([
                {
                    "rep_id": "rep-1",
                    "name": "Alice",
                    "capacity": 5,
                    "current_load": 1,
                    "territories": ["west"],
                    "segments": ["enterprise"],
                    "skills": ["cloud"],
                    "performance_score": 92.0
                },
                {
                    "rep_id": "rep-2",
                    "name": "Bob",
                    "capacity": 5,
                    "current_load": 2,
                    "territories": ["east", "west"],
                    "segments": ["smb", "enterprise"],
                    "skills": [],
                    "performance_score": 80.0
                }
            ])
            .to_string(),
            campaign_budget_minor: 6000,
            outreach_cost_minor: Some(2500),
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
        assert_eq!(projection.workflow_cases.len(), 1);
        assert_eq!(projection.documents.len(), 1);
        assert_eq!(projection.facts.len(), 2);
    }
}
