use std::collections::{BTreeMap, HashMap};

use converge_core::{
    Agent, AgentEffect, Context, ContextKey, ConvergeResult, Engine, Fact as ConvergeFact,
    ProposedFact, TypesRunHooks,
};
use crm_kernel::{
    Actor as CrmActor, CommunicationChannel, CommunicationDirection, CommunicationRecord,
    FactRecord, Money, OpportunityCreate, OrganizationLifecycle, OrganizationUpsert, PersonUpsert,
    RecordKind, RecordRef,
};
use crm_storage::{KernelStore, StoreWriteResult};
use prio_truths::{QualifyInboundLeadEvaluator, converge_binding_for_truth};
use serde::{Deserialize, Serialize};
use tonic::Status;

use super::{
    RecordingObserver, TruthExecutionArtifacts, TruthProjection,
    common::{converge_confidence_to_bps, optional_input, optional_uuid, required_input},
    domain_event_kind_name, status_from_converge, status_from_storage,
};
use uuid::Uuid;

const COMMERCIAL_PACK_ID: &str = "prio-commercial-pack";
const WORK_PACK_ID: &str = "prio-work-pack";
const QUALIFICATION_FACT_ID: &str = "lead:qualification";
const MANUAL_REVIEW_FACT_ID: &str = "lead:qualification-pending";
const OWNER_FACT_ID: &str = "lead:owner";
const NEXT_STEP_FACT_ID: &str = "lead:next-step";
const STUB_PROVENANCE: &str = "prio.qualify-inbound-lead.rules";
const QUALIFICATION_CONFIDENCE: f64 = 0.92;
const ROUTING_CONFIDENCE: f64 = 0.98;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum LeadQualificationStatus {
    Qualified,
    Disqualified,
    ManualReviewRequired,
}

impl LeadQualificationStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Qualified => "qualified",
            Self::Disqualified => "disqualified",
            Self::ManualReviewRequired => "manual-review-required",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LeadQualificationPayload {
    status: LeadQualificationStatus,
    reason: String,
    fit_score: u16,
    authority_score: u16,
    urgency_score: u16,
    confidence_bps: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LeadRoutingPayload {
    value: String,
    rationale: String,
    confidence_bps: u16,
}

#[derive(Debug, Clone)]
struct ProjectedFactStatement {
    statement: String,
    confidence_bps: u16,
}

#[derive(Debug, Clone)]
pub struct QualifyInboundLeadInput {
    pub organization_name: String,
    pub inbound_summary: String,
    pub organization_id: Option<Uuid>,
    pub person_id: Option<Uuid>,
    pub organization_external_key: Option<String>,
    pub website: Option<String>,
    pub industry: Option<String>,
    pub contact_name: Option<String>,
    pub contact_title: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_linkedin_url: Option<String>,
    pub subject: Option<String>,
    pub opportunity_name: Option<String>,
    pub currency_code: Option<String>,
    pub opportunity_value_minor: Option<i64>,
    pub source_channel: Option<String>,
    pub confidence_bps: Option<u16>,
    pub fit_score: Option<u16>,
    pub authority_score: Option<u16>,
    pub urgency_score: Option<u16>,
    pub owner: Option<String>,
    pub next_step: Option<String>,
}

impl QualifyInboundLeadInput {
    pub fn from_map(inputs: &HashMap<String, String>) -> Result<Self, Status> {
        Ok(Self {
            organization_name: required_input(inputs, "organization_name")?.to_string(),
            inbound_summary: required_input(inputs, "inbound_summary")?.to_string(),
            organization_id: optional_uuid(inputs, "organization_id")?,
            person_id: optional_uuid(inputs, "person_id")?,
            organization_external_key: optional_input(inputs, "organization_external_key"),
            website: optional_input(inputs, "website"),
            industry: optional_input(inputs, "industry"),
            contact_name: optional_input(inputs, "contact_name"),
            contact_title: optional_input(inputs, "contact_title"),
            contact_email: optional_input(inputs, "contact_email"),
            contact_phone: optional_input(inputs, "contact_phone"),
            contact_linkedin_url: optional_input(inputs, "contact_linkedin_url"),
            subject: optional_input(inputs, "subject"),
            opportunity_name: optional_input(inputs, "opportunity_name"),
            currency_code: optional_input(inputs, "currency_code"),
            opportunity_value_minor: super::common::optional_i64(inputs, "opportunity_value_minor"),
            source_channel: optional_input(inputs, "source_channel"),
            confidence_bps: optional_input(inputs, "confidence_bps")
                .and_then(|v| v.parse::<u16>().ok()),
            fit_score: optional_input(inputs, "fit_score").and_then(|v| v.parse::<u16>().ok()),
            authority_score: optional_input(inputs, "authority_score")
                .and_then(|v| v.parse::<u16>().ok()),
            urgency_score: optional_input(inputs, "urgency_score")
                .and_then(|v| v.parse::<u16>().ok()),
            owner: optional_input(inputs, "owner"),
            next_step: optional_input(inputs, "next_step"),
        })
    }
}

pub(super) fn execute<S: KernelStore>(
    store: &S,
    runtime_stores: &crm_storage::AppRuntimeStores,
    inputs: QualifyInboundLeadInput,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    let binding = converge_binding_for_truth("qualify-inbound-lead")
        .ok_or_else(|| Status::not_found("truth not found: qualify-inbound-lead"))?;

    let mut engine = Engine::new();
    engine.register_in_pack(COMMERCIAL_PACK_ID, LeadQualificationAgent);
    engine.register_in_pack(WORK_PACK_ID, LeadRoutingAgent);

    let (result, experience_events) = super::run_engine_with_runtime(
        runtime_stores,
        &mut engine,
        &super::RuntimeContext { scope_id: inputs.organization_id.map(|id| id.to_string()).unwrap_or_else(|| "inbound".to_string()) },
        seed_context(&inputs)?,
        &binding.intent,
        std::sync::Arc::new(QualifyInboundLeadEvaluator),
    )?;

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

fn project<S: KernelStore>(
    store: &S,
    inputs: &QualifyInboundLeadInput,
    result: &ConvergeResult,
    actor: CrmActor,
) -> Result<TruthProjection, Status> {
    let organization_id = inputs.organization_id;
    let person_id = inputs.person_id;
    let organization_name = inputs.organization_name.clone();
    let inbound_summary = inputs.inbound_summary.clone();
    let organization_external_key = inputs.organization_external_key.clone();
    let website = inputs.website.clone();
    let industry = inputs.industry.clone();
    let contact_name = inputs.contact_name.clone();
    let contact_title = inputs.contact_title.clone();
    let contact_email = inputs.contact_email.clone();
    let contact_phone = inputs.contact_phone.clone();
    let contact_linkedin_url = inputs.contact_linkedin_url.clone();
    let subject = inputs.subject.clone();
    let opportunity_name = inputs.opportunity_name.clone();
    let currency_code = inputs
        .currency_code
        .clone()
        .unwrap_or_else(|| "USD".to_string());
    let opportunity_value_minor = inputs.opportunity_value_minor.unwrap_or_default();

    let qualification = qualification_payload_from_result(result)?;
    let owner = routing_payload_from_result(result, OWNER_FACT_ID)?;
    let next_step = routing_payload_from_result(result, NEXT_STEP_FACT_ID)?;
    let output_facts = projected_output_facts(&qualification, owner.as_ref(), next_step.as_ref());

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
                    owner_user_id: owner.as_ref().map(|payload| payload.value.clone()),
                    tags: vec!["inbound-lead".to_string()],
                },
                actor.clone(),
            )?;

            let person = contact_name
                .clone()
                .map(|full_name| {
                    kernel.upsert_person(
                        PersonUpsert {
                            person_id,
                            organization_id: Some(organization.id),
                            full_name,
                            title: contact_title.clone(),
                            email: contact_email.clone(),
                            phone: contact_phone.clone(),
                            linkedin_url: contact_linkedin_url.clone(),
                        },
                        actor.clone(),
                    )
                })
                .transpose()?;

            let mut related_to = vec![RecordRef {
                kind: RecordKind::Organization,
                id: organization.id,
            }];
            if let Some(person) = &person {
                related_to.push(RecordRef {
                    kind: RecordKind::Person,
                    id: person.id,
                });
            }

            let counterpart = contact_name
                .clone()
                .unwrap_or_else(|| organization.name.clone());
            let _ = kernel.record_communication(
                CommunicationRecord {
                    channel: input_channel(inputs),
                    direction: CommunicationDirection::Inbound,
                    subject: subject.clone(),
                    summary: inbound_summary.clone(),
                    counterpart,
                    related_to: related_to.clone(),
                    occurred_at: None,
                },
                actor.clone(),
            )?;

            let opportunity = qualification
                .as_ref()
                .is_some_and(|payload| payload.status == LeadQualificationStatus::Qualified)
                .then(|| {
                    kernel.create_opportunity(
                        OpportunityCreate {
                            organization_id: organization.id,
                            primary_contact_id: person.as_ref().map(|person| person.id),
                            name: opportunity_name
                                .clone()
                                .unwrap_or_else(|| format!("Inbound lead: {}", organization.name)),
                            value: Money {
                                currency_code: currency_code.clone(),
                                amount_minor: opportunity_value_minor,
                            },
                            confidence_bps: confidence_bps_for_projection(
                                inputs,
                                qualification.as_ref(),
                            ),
                            next_step: next_step.as_ref().map(|payload| payload.value.clone()),
                            expected_close_at: None,
                        },
                        actor.clone(),
                    )
                })
                .transpose()?;

            let mut fact_related_to = related_to.clone();
            if let Some(opportunity) = &opportunity {
                fact_related_to.push(RecordRef {
                    kind: RecordKind::Opportunity,
                    id: opportunity.id,
                });
            }

            let projected_facts = output_facts
                .iter()
                .map(|statement| {
                    kernel.record_fact(
                        FactRecord {
                            statement: statement.statement.clone(),
                            confidence_bps: statement.confidence_bps,
                            related_to: fact_related_to.clone(),
                            source_note_id: None,
                        },
                        actor.clone(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok((organization, person, opportunity, projected_facts))
        })
        .map_err(status_from_storage)?;

    let (organization, person, opportunity, facts) = value;
    Ok(TruthProjection {
        organization: Some(organization),
        person,
        opportunity,
        subscription: None,
        entitlements: Vec::new(),
        ledger_entries: Vec::new(),
        documents: Vec::new(),
        workflow_cases: Vec::new(),
        facts,
        domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
    })
}

// Deterministic stub agents prove the truth contract. Provider-backed replacements
// should keep the same fact IDs and payload codecs so the rest of the runtime and
// projection code does not need to change.
struct LeadQualificationAgent;

impl Agent for LeadQualificationAgent {
    fn name(&self) -> &str {
        "prio.lead-qualification"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
        ctx.has(ContextKey::Seeds)
            && !has_fact(ctx, ContextKey::Evaluations, QUALIFICATION_FACT_ID)
            && !has_fact(ctx, ContextKey::Diagnostic, MANUAL_REVIEW_FACT_ID)
    }

    fn execute(&self, ctx: &dyn converge_core::ContextView) -> AgentEffect {
        let summary = seed_value(ctx, "inbound_summary").unwrap_or_default();
        let title = seed_value(ctx, "contact_title");
        let fit = parsed_score(ctx, "fit_score").unwrap_or_else(|| heuristic_fit(&summary));
        let authority = parsed_score(ctx, "authority_score")
            .unwrap_or_else(|| heuristic_authority(title.as_deref()));
        let urgency =
            parsed_score(ctx, "urgency_score").unwrap_or_else(|| heuristic_urgency(&summary));

        let payload = if is_non_buyer_signal(&summary) {
            LeadQualificationPayload {
                status: LeadQualificationStatus::Disqualified,
                reason: "non-buyer-inbound-signal".to_string(),
                fit_score: fit,
                authority_score: authority,
                urgency_score: urgency,
                confidence_bps: converge_confidence_to_bps(QUALIFICATION_CONFIDENCE),
            }
        } else if fit >= 65 && (authority >= 50 || urgency >= 65) {
            LeadQualificationPayload {
                status: LeadQualificationStatus::Qualified,
                reason: format!(
                    "fit-and-buying-signal:fit={fit},authority={authority},urgency={urgency}"
                ),
                fit_score: fit,
                authority_score: authority,
                urgency_score: urgency,
                confidence_bps: converge_confidence_to_bps(QUALIFICATION_CONFIDENCE),
            }
        } else if fit <= 45 || (authority < 35 && urgency < 35) {
            LeadQualificationPayload {
                status: LeadQualificationStatus::Disqualified,
                reason: format!(
                    "low-fit-or-authority:fit={fit},authority={authority},urgency={urgency}"
                ),
                fit_score: fit,
                authority_score: authority,
                urgency_score: urgency,
                confidence_bps: converge_confidence_to_bps(QUALIFICATION_CONFIDENCE),
            }
        } else {
            LeadQualificationPayload {
                status: LeadQualificationStatus::ManualReviewRequired,
                reason: format!("ambiguous-fit:fit={fit},authority={authority},urgency={urgency}"),
                fit_score: fit,
                authority_score: authority,
                urgency_score: urgency,
                confidence_bps: converge_confidence_to_bps(QUALIFICATION_CONFIDENCE),
            }
        };

        let (key, id) = if payload.status == LeadQualificationStatus::ManualReviewRequired {
            (ContextKey::Diagnostic, MANUAL_REVIEW_FACT_ID.to_string())
        } else {
            (ContextKey::Evaluations, QUALIFICATION_FACT_ID.to_string())
        };

        AgentEffect::with_proposal(ProposedFact {
            key,
            id,
            content: encode_qualification_payload(&payload),
            confidence: QUALIFICATION_CONFIDENCE,
            provenance: STUB_PROVENANCE.to_string(),
        })
    }
}

struct LeadRoutingAgent;

impl Agent for LeadRoutingAgent {
    fn name(&self) -> &str {
        "prio.lead-routing"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[
            ContextKey::Seeds,
            ContextKey::Evaluations,
            ContextKey::Diagnostic,
        ]
    }

    fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
        let has_decision = has_fact(ctx, ContextKey::Evaluations, QUALIFICATION_FACT_ID)
            || has_fact(ctx, ContextKey::Diagnostic, MANUAL_REVIEW_FACT_ID);
        has_decision
            && (!has_fact(ctx, ContextKey::Strategies, OWNER_FACT_ID)
                || !has_fact(ctx, ContextKey::Strategies, NEXT_STEP_FACT_ID))
    }

    fn execute(&self, ctx: &dyn converge_core::ContextView) -> AgentEffect {
        let qualification = qualification_payload_from_view(ctx);
        let review_pending = has_fact(ctx, ContextKey::Diagnostic, MANUAL_REVIEW_FACT_ID);

        let (default_owner, default_next_step, rationale) =
            match qualification.as_ref().map(|payload| payload.status) {
                Some(LeadQualificationStatus::Qualified) => (
                    "sales-queue",
                    "schedule qualification call",
                    "qualified lead should enter the sales queue",
                ),
                Some(LeadQualificationStatus::Disqualified) => (
                    "marketing-ops",
                    "archive with explicit disqualification reason",
                    "disqualified lead should be archived with traceable reason",
                ),
                Some(LeadQualificationStatus::ManualReviewRequired) | _ if review_pending => (
                    "manual-review-queue",
                    "manual review required before qualification",
                    "qualification remained ambiguous and requires human review",
                ),
                _ => (
                    "sales-queue",
                    "review inbound lead",
                    "routing fallback applied because qualification payload was unavailable",
                ),
            };

        let owner = seed_value(ctx, "owner").unwrap_or_else(|| default_owner.to_string());
        let next_step =
            seed_value(ctx, "next_step").unwrap_or_else(|| default_next_step.to_string());
        let confidence_bps = converge_confidence_to_bps(ROUTING_CONFIDENCE);

        AgentEffect {
            facts: Vec::new(),
            proposals: vec![
                ProposedFact {
                    key: ContextKey::Strategies,
                    id: OWNER_FACT_ID.to_string(),
                    content: encode_routing_payload(&LeadRoutingPayload {
                        value: owner,
                        rationale: rationale.to_string(),
                        confidence_bps,
                    }),
                    confidence: ROUTING_CONFIDENCE,
                    provenance: STUB_PROVENANCE.to_string(),
                },
                ProposedFact {
                    key: ContextKey::Strategies,
                    id: NEXT_STEP_FACT_ID.to_string(),
                    content: encode_routing_payload(&LeadRoutingPayload {
                        value: next_step,
                        rationale: rationale.to_string(),
                        confidence_bps,
                    }),
                    confidence: ROUTING_CONFIDENCE,
                    provenance: STUB_PROVENANCE.to_string(),
                },
            ],
        }
    }
}

fn qualification_payload_from_result(
    result: &ConvergeResult,
) -> Result<Option<LeadQualificationPayload>, Status> {
    fact_content(result, ContextKey::Evaluations, QUALIFICATION_FACT_ID)
        .map(|content| decode_qualification_payload(&content))
        .transpose()
}

fn routing_payload_from_result(
    result: &ConvergeResult,
    fact_id: &str,
) -> Result<Option<LeadRoutingPayload>, Status> {
    fact_content(result, ContextKey::Strategies, fact_id)
        .map(|content| decode_routing_payload(&content))
        .transpose()
}

fn projected_output_facts(
    qualification: &Option<LeadQualificationPayload>,
    owner: Option<&LeadRoutingPayload>,
    next_step: Option<&LeadRoutingPayload>,
) -> Vec<ProjectedFactStatement> {
    let mut facts = Vec::new();

    if let Some(payload) = qualification {
        facts.push(ProjectedFactStatement {
            statement: format!(
                "lead qualification: {} ({})",
                payload.status.as_str(),
                payload.reason
            ),
            confidence_bps: payload.confidence_bps,
        });
    }

    if let Some(payload) = owner {
        facts.push(ProjectedFactStatement {
            statement: format!("lead owner: {}", payload.value),
            confidence_bps: payload.confidence_bps,
        });
    }

    if let Some(payload) = next_step {
        facts.push(ProjectedFactStatement {
            statement: format!("lead next step: {}", payload.value),
            confidence_bps: payload.confidence_bps,
        });
    }

    facts
}

fn seed_context(inputs: &QualifyInboundLeadInput) -> Result<Context, Status> {
    let mut context = Context::new();
    let mut map = BTreeMap::new();
    map.insert("organization_name", inputs.organization_name.clone());
    map.insert("inbound_summary", inputs.inbound_summary.clone());
    if let Some(id) = inputs.organization_id {
        map.insert("organization_id", id.to_string());
    }
    if let Some(id) = inputs.person_id {
        map.insert("person_id", id.to_string());
    }
    if let Some(ref val) = inputs.organization_external_key {
        map.insert("organization_external_key", val.clone());
    }
    if let Some(ref val) = inputs.website {
        map.insert("website", val.clone());
    }
    if let Some(ref val) = inputs.industry {
        map.insert("industry", val.clone());
    }
    if let Some(ref val) = inputs.contact_name {
        map.insert("contact_name", val.clone());
    }
    if let Some(ref val) = inputs.contact_title {
        map.insert("contact_title", val.clone());
    }
    if let Some(ref val) = inputs.contact_email {
        map.insert("contact_email", val.clone());
    }
    if let Some(ref val) = inputs.contact_phone {
        map.insert("contact_phone", val.clone());
    }
    if let Some(ref val) = inputs.contact_linkedin_url {
        map.insert("contact_linkedin_url", val.clone());
    }
    if let Some(ref val) = inputs.subject {
        map.insert("subject", val.clone());
    }
    if let Some(ref val) = inputs.opportunity_name {
        map.insert("opportunity_name", val.clone());
    }
    if let Some(ref val) = inputs.currency_code {
        map.insert("currency_code", val.clone());
    }
    if let Some(val) = inputs.opportunity_value_minor {
        map.insert("opportunity_value_minor", val.to_string());
    }
    if let Some(ref val) = inputs.source_channel {
        map.insert("source_channel", val.clone());
    }
    if let Some(val) = inputs.confidence_bps {
        map.insert("confidence_bps", val.to_string());
    }
    if let Some(val) = inputs.fit_score {
        map.insert("fit_score", val.to_string());
    }
    if let Some(val) = inputs.authority_score {
        map.insert("authority_score", val.to_string());
    }
    if let Some(val) = inputs.urgency_score {
        map.insert("urgency_score", val.to_string());
    }
    if let Some(ref val) = inputs.owner {
        map.insert("owner", val.clone());
    }
    if let Some(ref val) = inputs.next_step {
        map.insert("next_step", val.clone());
    }

    for (key, value) in map {
        context
            .add_fact(ConvergeFact::new(
                ContextKey::Seeds,
                format!("input:{key}"),
                value,
            ))
            .map_err(status_from_converge)?;
    }
    Ok(context)
}

fn encode_qualification_payload(payload: &LeadQualificationPayload) -> String {
    serde_json::to_string(payload).expect("qualification payload should serialize")
}

fn decode_qualification_payload(content: &str) -> Result<LeadQualificationPayload, Status> {
    serde_json::from_str(content)
        .map_err(|error| Status::internal(format!("invalid qualification payload: {error}")))
}

fn encode_routing_payload(payload: &LeadRoutingPayload) -> String {
    serde_json::to_string(payload).expect("routing payload should serialize")
}

fn decode_routing_payload(content: &str) -> Result<LeadRoutingPayload, Status> {
    serde_json::from_str(content)
        .map_err(|error| Status::internal(format!("invalid routing payload: {error}")))
}

fn qualification_payload_from_view(
    ctx: &dyn converge_core::ContextView,
) -> Option<LeadQualificationPayload> {
    fact_content_from_view(ctx, ContextKey::Evaluations, QUALIFICATION_FACT_ID)
        .and_then(|content| decode_qualification_payload(&content).ok())
}

fn fact_content(result: &ConvergeResult, key: ContextKey, fact_id: &str) -> Option<String> {
    result
        .context
        .get(key)
        .iter()
        .find(|fact| fact.id == fact_id)
        .map(|fact| fact.content.clone())
}

fn fact_content_from_view(
    ctx: &dyn converge_core::ContextView,
    key: ContextKey,
    fact_id: &str,
) -> Option<String> {
    ctx.get(key)
        .iter()
        .find(|fact| fact.id == fact_id)
        .map(|fact| fact.content.clone())
}

fn has_fact(ctx: &dyn converge_core::ContextView, key: ContextKey, fact_id: &str) -> bool {
    ctx.get(key).iter().any(|fact| fact.id == fact_id)
}

fn seed_value(ctx: &dyn converge_core::ContextView, key: &str) -> Option<String> {
    let id = format!("input:{key}");
    ctx.get(ContextKey::Seeds)
        .iter()
        .find(|fact| fact.id == id)
        .map(|fact| fact.content.clone())
}

fn parsed_score(ctx: &dyn converge_core::ContextView, key: &str) -> Option<u16> {
    seed_value(ctx, key).and_then(|value| value.parse::<u16>().ok())
}

fn heuristic_fit(summary: &str) -> u16 {
    if contains_any(
        summary,
        &["pricing", "quote", "timeline", "pilot", "implementation"],
    ) {
        80
    } else {
        50
    }
}

fn heuristic_authority(title: Option<&str>) -> u16 {
    if title.is_some_and(|title| {
        contains_any(
            title,
            &["cto", "ceo", "founder", "head", "director", "vp", "chief"],
        )
    }) {
        80
    } else {
        45
    }
}

fn heuristic_urgency(summary: &str) -> u16 {
    if contains_any(
        summary,
        &["this quarter", "urgent", "soon", "asap", "next week"],
    ) {
        75
    } else {
        50
    }
}

fn is_non_buyer_signal(summary: &str) -> bool {
    contains_any(
        summary,
        &[
            "job application",
            "looking for work",
            "student project",
            "internship",
        ],
    )
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let normalized = value.to_ascii_lowercase();
    needles.iter().any(|needle| normalized.contains(needle))
}

fn input_channel(inputs: &QualifyInboundLeadInput) -> CommunicationChannel {
    match inputs
        .source_channel
        .as_deref()
        .unwrap_or("email")
        .to_ascii_lowercase()
        .as_str()
    {
        "phone" => CommunicationChannel::Phone,
        "meeting" => CommunicationChannel::Meeting,
        "chat" => CommunicationChannel::Chat,
        "sms" => CommunicationChannel::Sms,
        _ => CommunicationChannel::Email,
    }
}

fn confidence_bps_for_projection(
    inputs: &QualifyInboundLeadInput,
    qualification: Option<&LeadQualificationPayload>,
) -> u16 {
    inputs
        .confidence_bps
        .filter(|value| *value <= 10_000)
        .unwrap_or_else(|| {
            qualification
                .map(|payload| payload.confidence_bps)
                .unwrap_or_else(|| converge_confidence_to_bps(QUALIFICATION_CONFIDENCE))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use converge_core::{ExperienceEvent, StopReason};
    use crm_kernel::ActorKind;
    use crm_storage::InMemoryKernelStore;

    fn human() -> CrmActor {
        CrmActor {
            actor_id: "user-1".to_string(),
            display_name: "Kenneth".to_string(),
            kind: ActorKind::Human,
        }
    }

    #[test]
    fn qualify_inbound_lead_executes_end_to_end() {
        let store = InMemoryKernelStore::default_local();
        let inputs = HashMap::from([
            ("organization_name".to_string(), "Acme".to_string()),
            (
                "inbound_summary".to_string(),
                "Need pricing and implementation timeline for an AI pilot next week.".to_string(),
            ),
            ("contact_name".to_string(), "Alice Doe".to_string()),
            ("contact_title".to_string(), "CTO".to_string()),
        ]);

        let execution = execute(
            &store,
            &crm_storage::AppRuntimeStores {
                context: crm_storage::AppContextStore::Memory(
                    crm_storage::InMemoryContextStore::new(),
                ),
                experience: crm_storage::AppExperienceStore::Memory(
                    crm_storage::InMemoryExperienceStoreAdapter::new(),
                ),
            },
            QualifyInboundLeadInput::from_map(&inputs).unwrap(),
            human(),
            true,
        )
        .expect("truth should execute");

        assert!(matches!(
            execution.result.stop_reason,
            StopReason::CriteriaMet { .. }
        ));
        assert!(
            execution
                .experience_events
                .iter()
                .filter(|event| matches!(event, ExperienceEvent::FactPromoted { .. }))
                .count()
                >= 3
        );

        let projection = execution.projection.expect("projection should exist");
        assert_eq!(
            projection
                .organization
                .as_ref()
                .map(|organization| organization.lifecycle),
            Some(OrganizationLifecycle::Prospect)
        );
        assert!(projection.opportunity.is_some());
        assert_eq!(projection.facts.len(), 3);
    }

    #[test]
    fn confidence_mapping_is_explicit() {
        assert_eq!(converge_confidence_to_bps(0.92), 9_200);
        assert_eq!(converge_confidence_to_bps(1.2), 10_000);
        assert_eq!(converge_confidence_to_bps(-0.1), 0);
    }

    #[test]
    fn qualify_inbound_lead_disqualifies_non_buyer_signals() {
        for summary in [
            "This is a job application for your engineering team",
            "I am working on a student project about CRM systems",
        ] {
            let store = InMemoryKernelStore::default_local();
            let inputs = HashMap::from([
                ("organization_name".to_string(), "NonBuyer Inc".to_string()),
                ("inbound_summary".to_string(), summary.to_string()),
            ]);

            let execution = execute(
                &store,
                &crm_storage::AppRuntimeStores {
                    context: crm_storage::AppContextStore::Memory(
                        crm_storage::InMemoryContextStore::new(),
                    ),
                    experience: crm_storage::AppExperienceStore::Memory(
                        crm_storage::InMemoryExperienceStoreAdapter::new(),
                    ),
                },
                QualifyInboundLeadInput::from_map(&inputs).unwrap(),
                human(),
                true,
            )
            .expect("truth should execute");

            assert!(
                matches!(execution.result.stop_reason, StopReason::CriteriaMet { .. }),
                "should converge for summary: {summary}"
            );

            let projection = execution.projection.expect("projection should exist");
            let has_disqualified_fact = projection
                .facts
                .iter()
                .any(|fact| fact.statement.contains("disqualified"));
            assert!(
                has_disqualified_fact,
                "lead:qualification should contain 'disqualified' for summary: {summary}"
            );

            let has_marketing_ops_routing = projection
                .facts
                .iter()
                .any(|fact| fact.statement.contains("marketing-ops"));
            assert!(
                has_marketing_ops_routing,
                "should route to marketing-ops for summary: {summary}"
            );
        }
    }

    #[test]
    fn qualify_inbound_lead_routes_ambiguous_leads_to_manual_review() {
        let store = InMemoryKernelStore::default_local();
        let inputs = HashMap::from([
            ("organization_name".to_string(), "Ambiguous Co".to_string()),
            (
                "inbound_summary".to_string(),
                "general inquiry about your product".to_string(),
            ),
        ]);

        let execution = execute(
            &store,
            &crm_storage::AppRuntimeStores {
                context: crm_storage::AppContextStore::Memory(
                    crm_storage::InMemoryContextStore::new(),
                ),
                experience: crm_storage::AppExperienceStore::Memory(
                    crm_storage::InMemoryExperienceStoreAdapter::new(),
                ),
            },
            QualifyInboundLeadInput::from_map(&inputs).unwrap(),
            human(),
            true,
        )
        .expect("truth should execute");

        assert!(
            execution.result.converged,
            "engine should converge even for ambiguous leads"
        );

        let has_diagnostic = execution
            .result
            .context
            .get(converge_core::ContextKey::Diagnostic)
            .iter()
            .any(|fact| fact.id == MANUAL_REVIEW_FACT_ID);

        let has_manual_review_routing = execution
            .result
            .context
            .get(converge_core::ContextKey::Strategies)
            .iter()
            .any(|fact| fact.id == OWNER_FACT_ID && fact.content.contains("manual-review-queue"));

        assert!(
            has_diagnostic || has_manual_review_routing,
            "ambiguous lead should have qualification-pending diagnostic or manual-review-queue routing"
        );
    }

    #[test]
    fn qualify_inbound_lead_without_projection_produces_no_side_effects() {
        let store = InMemoryKernelStore::default_local();
        let inputs = HashMap::from([
            ("organization_name".to_string(), "Ghost Corp".to_string()),
            (
                "inbound_summary".to_string(),
                "Need pricing and implementation timeline for an AI pilot next week.".to_string(),
            ),
            ("contact_name".to_string(), "Bob Builder".to_string()),
            ("contact_title".to_string(), "CTO".to_string()),
        ]);

        let execution = execute(
            &store,
            &crm_storage::AppRuntimeStores {
                context: crm_storage::AppContextStore::Memory(
                    crm_storage::InMemoryContextStore::new(),
                ),
                experience: crm_storage::AppExperienceStore::Memory(
                    crm_storage::InMemoryExperienceStoreAdapter::new(),
                ),
            },
            QualifyInboundLeadInput::from_map(&inputs).unwrap(),
            human(),
            false,
        )
        .expect("truth should execute");

        assert!(matches!(
            execution.result.stop_reason,
            StopReason::CriteriaMet { .. }
        ));
        assert!(
            execution.projection.is_none(),
            "projection should be None when persist_projection=false"
        );

        let organizations = store
            .read(|kernel| kernel.list_organizations())
            .expect("store read should succeed");
        assert!(
            organizations.is_empty(),
            "store should have no organizations when projection is disabled"
        );
    }

    #[test]
    fn qualify_inbound_lead_missing_required_input_returns_error() {
        let _store = InMemoryKernelStore::default_local();
        let inputs = HashMap::from([("inbound_summary".to_string(), "We need help".to_string())]);

        let result = QualifyInboundLeadInput::from_map(&inputs);

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(
            status.message().contains("organization_name"),
            "error should mention the missing field"
        );
    }

    #[test]
    fn qualify_inbound_lead_empty_summary_returns_error() {
        let _store = InMemoryKernelStore::default_local();
        let inputs = HashMap::from([
            ("organization_name".to_string(), "Empty Co".to_string()),
            ("inbound_summary".to_string(), String::new()),
        ]);

        let result = QualifyInboundLeadInput::from_map(&inputs);

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(
            status.message().contains("inbound_summary"),
            "error should mention the missing field"
        );
    }

    #[test]
    fn qualify_inbound_lead_idempotent_organization_upsert() {
        let store = InMemoryKernelStore::default_local();
        let org_id = uuid::Uuid::new_v4();
        let make_inputs = || {
            HashMap::from([
                (
                    "organization_name".to_string(),
                    "Idempotent Corp".to_string(),
                ),
                ("organization_id".to_string(), org_id.to_string()),
                (
                    "inbound_summary".to_string(),
                    "Need pricing and implementation timeline for an AI pilot next week."
                        .to_string(),
                ),
                ("contact_name".to_string(), "Jane Doe".to_string()),
                ("contact_title".to_string(), "CEO".to_string()),
            ])
        };

        let first = execute(
            &store,
            &crm_storage::AppRuntimeStores {
                context: crm_storage::AppContextStore::Memory(
                    crm_storage::InMemoryContextStore::new(),
                ),
                experience: crm_storage::AppExperienceStore::Memory(
                    crm_storage::InMemoryExperienceStoreAdapter::new(),
                ),
            },
            QualifyInboundLeadInput::from_map(&make_inputs()).unwrap(),
            human(),
            true,
        )
        .expect("first execution should succeed");
        let second = execute(
            &store,
            &crm_storage::AppRuntimeStores {
                context: crm_storage::AppContextStore::Memory(
                    crm_storage::InMemoryContextStore::new(),
                ),
                experience: crm_storage::AppExperienceStore::Memory(
                    crm_storage::InMemoryExperienceStoreAdapter::new(),
                ),
            },
            QualifyInboundLeadInput::from_map(&make_inputs()).unwrap(),
            human(),
            true,
        )
        .expect("second execution should succeed");

        assert!(first.projection.is_some());
        assert!(second.projection.is_some());

        let organizations = store
            .read(|kernel| kernel.list_organizations())
            .expect("store read should succeed");
        let matching = organizations.iter().filter(|org| org.id == org_id).count();
        assert_eq!(
            matching, 1,
            "organization should be upserted, not duplicated"
        );
    }

    #[test]
    fn qualify_inbound_lead_sequential_runs_share_store_state() {
        let store = InMemoryKernelStore::default_local();

        let inputs_a = HashMap::from([
            ("organization_name".to_string(), "Alpha Inc".to_string()),
            (
                "inbound_summary".to_string(),
                "Need pricing and implementation timeline for an AI pilot next week.".to_string(),
            ),
            ("contact_name".to_string(), "Alice Alpha".to_string()),
            ("contact_title".to_string(), "CTO".to_string()),
        ]);
        let execution_a = execute(
            &store,
            &crm_storage::AppRuntimeStores {
                context: crm_storage::AppContextStore::Memory(
                    crm_storage::InMemoryContextStore::new(),
                ),
                experience: crm_storage::AppExperienceStore::Memory(
                    crm_storage::InMemoryExperienceStoreAdapter::new(),
                ),
            },
            QualifyInboundLeadInput::from_map(&inputs_a).unwrap(),
            human(),
            true,
        )
        .expect("execution A should succeed");
        assert!(execution_a.projection.is_some());

        let inputs_b = HashMap::from([
            ("organization_name".to_string(), "Beta LLC".to_string()),
            (
                "inbound_summary".to_string(),
                "We need a quote for your platform this quarter".to_string(),
            ),
            ("contact_name".to_string(), "Bob Beta".to_string()),
            ("contact_title".to_string(), "VP Engineering".to_string()),
        ]);
        let execution_b = execute(
            &store,
            &crm_storage::AppRuntimeStores {
                context: crm_storage::AppContextStore::Memory(
                    crm_storage::InMemoryContextStore::new(),
                ),
                experience: crm_storage::AppExperienceStore::Memory(
                    crm_storage::InMemoryExperienceStoreAdapter::new(),
                ),
            },
            QualifyInboundLeadInput::from_map(&inputs_b).unwrap(),
            human(),
            true,
        )
        .expect("execution B should succeed");
        assert!(execution_b.projection.is_some());

        let organizations = store
            .read(|kernel| kernel.list_organizations())
            .expect("store read should succeed");
        assert_eq!(
            organizations.len(),
            2,
            "store should contain both organizations"
        );

        let org_names: Vec<&str> = organizations.iter().map(|o| o.name.as_str()).collect();
        assert!(org_names.contains(&"Alpha Inc"), "should contain Alpha Inc");
        assert!(org_names.contains(&"Beta LLC"), "should contain Beta LLC");

        let proj_a = execution_a.projection.unwrap();
        let proj_b = execution_b.projection.unwrap();
        let org_a_id = proj_a.organization.as_ref().unwrap().id;
        let org_b_id = proj_b.organization.as_ref().unwrap().id;

        assert_ne!(org_a_id, org_b_id, "organizations should have distinct IDs");
        assert!(
            !proj_a.facts.is_empty(),
            "org A should have projected facts"
        );
        assert!(
            !proj_b.facts.is_empty(),
            "org B should have projected facts"
        );

        let opportunities_a = store
            .read(|kernel| kernel.list_opportunities(Some(org_a_id)))
            .expect("store read should succeed");
        let opportunities_b = store
            .read(|kernel| kernel.list_opportunities(Some(org_b_id)))
            .expect("store read should succeed");
        assert!(
            !opportunities_a.is_empty(),
            "org A should have opportunities"
        );
        assert!(
            !opportunities_b.is_empty(),
            "org B should have opportunities"
        );
    }

    // -----------------------------------------------------------------------
    // Agent misbehavior tests
    // -----------------------------------------------------------------------

    /// An agent that produces nothing — simulates an LLM returning empty output.
    struct SilentAgent;
    impl Agent for SilentAgent {
        fn name(&self) -> &str {
            "prio.silent-agent"
        }
        fn dependencies(&self) -> &[ContextKey] {
            &[ContextKey::Seeds]
        }
        fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
            ctx.has(ContextKey::Seeds)
        }
        fn execute(&self, _ctx: &dyn converge_core::ContextView) -> AgentEffect {
            AgentEffect::empty()
        }
    }

    /// An agent that produces a fact with malformed JSON content.
    struct MalformedPayloadAgent;
    impl Agent for MalformedPayloadAgent {
        fn name(&self) -> &str {
            "prio.malformed-payload"
        }
        fn dependencies(&self) -> &[ContextKey] {
            &[ContextKey::Seeds]
        }
        fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
            ctx.has(ContextKey::Seeds)
                && !super::has_fact(ctx, ContextKey::Evaluations, QUALIFICATION_FACT_ID)
        }
        fn execute(&self, _ctx: &dyn converge_core::ContextView) -> AgentEffect {
            AgentEffect::with_proposal(ProposedFact {
                key: ContextKey::Evaluations,
                id: QUALIFICATION_FACT_ID.to_string(),
                content: "NOT VALID JSON {{{".to_string(),
                confidence: 0.8,
                provenance: "prio.test.malformed".to_string(),
            })
        }
    }

    /// An agent that produces extra irrelevant facts alongside its real output.
    struct NoisyQualificationAgent;
    impl Agent for NoisyQualificationAgent {
        fn name(&self) -> &str {
            "prio.noisy-qualification"
        }
        fn dependencies(&self) -> &[ContextKey] {
            &[ContextKey::Seeds]
        }
        fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
            ctx.has(ContextKey::Seeds)
                && !super::has_fact(ctx, ContextKey::Evaluations, QUALIFICATION_FACT_ID)
        }
        fn execute(&self, _ctx: &dyn converge_core::ContextView) -> AgentEffect {
            let real = ProposedFact {
                key: ContextKey::Evaluations,
                id: QUALIFICATION_FACT_ID.to_string(),
                content: serde_json::to_string(&LeadQualificationPayload {
                    status: LeadQualificationStatus::Qualified,
                    reason: "noisy-but-valid".to_string(),
                    fit_score: 90,
                    authority_score: 80,
                    urgency_score: 70,
                    confidence_bps: 9200,
                })
                .unwrap(),
                confidence: 0.92,
                provenance: "prio.test.noisy".to_string(),
            };
            let noise1 = ProposedFact {
                key: ContextKey::Signals,
                id: "irrelevant:weather-forecast".to_string(),
                content: r#"{"temperature":22,"conditions":"sunny"}"#.to_string(),
                confidence: 0.5,
                provenance: "prio.test.noise".to_string(),
            };
            let noise2 = ProposedFact {
                key: ContextKey::Signals,
                id: "irrelevant:stock-price".to_string(),
                content: r#"{"ticker":"AAPL","price":185.50}"#.to_string(),
                confidence: 0.3,
                provenance: "prio.test.noise".to_string(),
            };
            AgentEffect {
                facts: Vec::new(),
                proposals: vec![real, noise1, noise2],
            }
        }
    }

    /// An agent that produces a very low confidence proposal.
    struct LowConfidenceAgent;
    impl Agent for LowConfidenceAgent {
        fn name(&self) -> &str {
            "prio.low-confidence"
        }
        fn dependencies(&self) -> &[ContextKey] {
            &[ContextKey::Seeds]
        }
        fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
            ctx.has(ContextKey::Seeds)
                && !super::has_fact(ctx, ContextKey::Evaluations, QUALIFICATION_FACT_ID)
        }
        fn execute(&self, _ctx: &dyn converge_core::ContextView) -> AgentEffect {
            AgentEffect::with_proposal(ProposedFact {
                key: ContextKey::Evaluations,
                id: QUALIFICATION_FACT_ID.to_string(),
                content: serde_json::to_string(&LeadQualificationPayload {
                    status: LeadQualificationStatus::Qualified,
                    reason: "low-confidence-guess".to_string(),
                    fit_score: 55,
                    authority_score: 40,
                    urgency_score: 45,
                    confidence_bps: 100,
                })
                .unwrap(),
                confidence: 0.01,
                provenance: "prio.test.low-confidence".to_string(),
            })
        }
    }

    /// Helper: build a seeded engine with evaluator + observer, ready for run.
    fn build_engine_and_context(
        inputs: &HashMap<String, String>,
    ) -> (
        Engine,
        Context,
        converge_core::TypesRootIntent,
        TypesRunHooks,
    ) {
        let binding = converge_binding_for_truth("qualify-inbound-lead").unwrap();
        let engine = Engine::new();
        let observer = std::sync::Arc::new(RecordingObserver::default());
        let hooks = TypesRunHooks {
            criterion_evaluator: Some(std::sync::Arc::new(QualifyInboundLeadEvaluator)),
            event_observer: Some(observer),
        };
        let typed_input = super::QualifyInboundLeadInput::from_map(inputs).unwrap();
        let context = super::seed_context(&typed_input).unwrap();
        (engine, context, binding.intent, hooks)
    }

    fn standard_inputs() -> HashMap<String, String> {
        HashMap::from([
            ("organization_name".to_string(), "Test Corp".to_string()),
            (
                "inbound_summary".to_string(),
                "Need pricing for an AI pilot.".to_string(),
            ),
            ("contact_name".to_string(), "Jane Doe".to_string()),
            ("contact_title".to_string(), "VP Engineering".to_string()),
        ])
    }

    #[test]
    fn silent_agent_converges_without_meeting_criteria() {
        let inputs = standard_inputs();
        let (mut engine, context, intent, hooks) = build_engine_and_context(&inputs);
        engine.register_in_pack(COMMERCIAL_PACK_ID, SilentAgent);
        engine.register_in_pack(WORK_PACK_ID, LeadRoutingAgent);
        let result = engine
            .run_with_types_intent_and_hooks(context, &intent, hooks)
            .expect("engine should converge even with silent agent");

        assert!(result.converged, "engine should converge");
        assert!(
            !matches!(result.stop_reason, StopReason::CriteriaMet { .. }),
            "criteria should NOT be met when qualification agent produces nothing"
        );
        let has_qualification = result
            .context
            .get(ContextKey::Evaluations)
            .iter()
            .any(|fact| fact.id == QUALIFICATION_FACT_ID);
        assert!(
            !has_qualification,
            "no qualification fact should exist from silent agent"
        );
    }

    #[test]
    fn malformed_payload_converges_but_projection_fails_gracefully() {
        let inputs = standard_inputs();
        let (mut engine, context, intent, hooks) = build_engine_and_context(&inputs);
        engine.register_in_pack(COMMERCIAL_PACK_ID, MalformedPayloadAgent);
        engine.register_in_pack(WORK_PACK_ID, LeadRoutingAgent);
        let result = engine
            .run_with_types_intent_and_hooks(context, &intent, hooks)
            .expect("engine should converge even with malformed payload");

        assert!(result.converged, "engine should converge");

        // The fact exists in converge context but has invalid JSON
        let has_qualification = result
            .context
            .get(ContextKey::Evaluations)
            .iter()
            .any(|fact| fact.id == QUALIFICATION_FACT_ID);
        assert!(has_qualification, "malformed fact should exist in context");

        // Attempting to decode should fail
        let decode_result: Result<LeadQualificationPayload, _> = result
            .context
            .get(ContextKey::Evaluations)
            .iter()
            .find(|fact| fact.id == QUALIFICATION_FACT_ID)
            .map(|fact| serde_json::from_str(&fact.content))
            .unwrap();
        assert!(
            decode_result.is_err(),
            "malformed JSON should fail deserialization"
        );
    }

    #[test]
    fn noisy_agent_extra_facts_do_not_affect_criteria() {
        let inputs = standard_inputs();
        let (mut engine, context, intent, hooks) = build_engine_and_context(&inputs);
        engine.register_in_pack(COMMERCIAL_PACK_ID, NoisyQualificationAgent);
        engine.register_in_pack(WORK_PACK_ID, LeadRoutingAgent);
        let result = engine
            .run_with_types_intent_and_hooks(context, &intent, hooks)
            .expect("engine should converge");

        assert!(result.converged);
        assert!(
            matches!(result.stop_reason, StopReason::CriteriaMet { .. }),
            "criteria should be met despite extra irrelevant facts"
        );

        // Extra facts exist in context but don't affect the outcome
        let noise_facts: Vec<_> = result
            .context
            .get(ContextKey::Signals)
            .iter()
            .filter(|fact| fact.id.starts_with("irrelevant:"))
            .collect();
        assert!(
            !noise_facts.is_empty(),
            "irrelevant facts should be present in context"
        );

        // Real qualification should still work
        let has_qualification = result
            .context
            .get(ContextKey::Evaluations)
            .iter()
            .any(|fact| fact.id == QUALIFICATION_FACT_ID);
        assert!(has_qualification, "real qualification fact should exist");
    }

    #[test]
    fn low_confidence_agent_still_converges() {
        let inputs = standard_inputs();
        let (mut engine, context, intent, hooks) = build_engine_and_context(&inputs);
        engine.register_in_pack(COMMERCIAL_PACK_ID, LowConfidenceAgent);
        engine.register_in_pack(WORK_PACK_ID, LeadRoutingAgent);
        let result = engine
            .run_with_types_intent_and_hooks(context, &intent, hooks)
            .expect("engine should converge");

        assert!(result.converged);

        // The fact exists but with very low confidence
        let qualification_fact = result
            .context
            .get(ContextKey::Evaluations)
            .iter()
            .find(|fact| fact.id == QUALIFICATION_FACT_ID)
            .expect("qualification fact should exist");

        let payload: LeadQualificationPayload =
            serde_json::from_str(&qualification_fact.content).expect("should decode");
        assert_eq!(
            payload.confidence_bps, 100,
            "confidence should reflect the low-confidence agent's output"
        );
    }

    // -----------------------------------------------------------------------
    // HITL / retry tests
    // -----------------------------------------------------------------------

    fn test_runtime_stores() -> crm_storage::AppRuntimeStores {
        crm_storage::AppRuntimeStores::default()
    }

    fn run_qualify(
        store: &InMemoryKernelStore,
        inputs: HashMap<String, String>,
        persist: bool,
    ) -> Result<super::super::TruthExecutionArtifacts, Status> {
        execute(
            store,
            &test_runtime_stores(),
            QualifyInboundLeadInput::from_map(&inputs)?,
            human(),
            persist,
        )
    }

    #[test]
    fn blocked_truth_retried_without_new_evidence_is_idempotent() {
        let store = InMemoryKernelStore::default_local();
        let org_id = uuid::Uuid::new_v4();
        let ambiguous_inputs = HashMap::from([
            ("organization_name".to_string(), "Retry Corp".to_string()),
            ("organization_id".to_string(), org_id.to_string()),
            (
                "inbound_summary".to_string(),
                "general inquiry about your product".to_string(),
            ),
        ]);

        let first = run_qualify(&store, ambiguous_inputs.clone(), true)
            .expect("first execution should succeed");
        let first_org_count = store
            .read(|kernel| kernel.list_organizations())
            .expect("read")
            .len();

        let second = run_qualify(&store, ambiguous_inputs, true)
            .expect("second execution should succeed");
        let second_org_count = store
            .read(|kernel| kernel.list_organizations())
            .expect("read")
            .len();

        assert_eq!(
            first_org_count, second_org_count,
            "org should not duplicate on retry"
        );

        assert!(first.result.converged);
        assert!(second.result.converged);
    }

    // -----------------------------------------------------------------------
    // Property tests: LLM output variation at the parsing boundary
    // -----------------------------------------------------------------------

    mod llm_output_properties {
        use super::*;
        use proptest::prelude::*;

        // The decode boundary where LLM agent output becomes typed data.
        // These tests simulate the variety of JSON an LLM might produce.

        proptest! {
            /// LLM wraps the expected JSON in markdown code fences.
            #[test]
            fn markdown_wrapped_json_is_rejected_not_panicked(
                status in prop_oneof![
                    Just("qualified"),
                    Just("disqualified"),
                    Just("manual-review-required"),
                ],
            ) {
                let wrapped = format!(
                    "```json\n{{\"status\":\"{status}\",\"reason\":\"test\",\"fit_score\":80,\"authority_score\":70,\"urgency_score\":60,\"confidence_bps\":9000}}\n```"
                );
                let result = serde_json::from_str::<LeadQualificationPayload>(&wrapped);
                // Must not panic — should be a clean Err
                prop_assert!(result.is_err(), "markdown-wrapped JSON should fail deserialization cleanly");
            }

            /// LLM adds trailing explanation after the JSON.
            #[test]
            fn json_with_trailing_text_is_rejected(
                trailing in "[a-zA-Z]{1,100}",
            ) {
                let content = format!(
                    "{{\"status\":\"qualified\",\"reason\":\"good fit\",\"fit_score\":80,\"authority_score\":70,\"urgency_score\":60,\"confidence_bps\":9000}} {trailing}"
                );
                let result = serde_json::from_str::<LeadQualificationPayload>(&content);
                prop_assert!(result.is_err(), "JSON with trailing non-whitespace text should fail");
            }

            /// LLM returns valid JSON but with extra unexpected fields.
            #[test]
            fn extra_fields_are_ignored_by_serde(
                extra_key in "[a-z_]{1,20}",
                extra_value in "[a-zA-Z0-9 ]{0,50}",
            ) {
                let content = format!(
                    "{{\"status\":\"qualified\",\"reason\":\"test\",\"fit_score\":80,\"authority_score\":70,\"urgency_score\":60,\"confidence_bps\":9000,\"{extra_key}\":\"{extra_value}\"}}"
                );
                let result = serde_json::from_str::<LeadQualificationPayload>(&content);
                // serde with default settings ignores unknown fields
                prop_assert!(result.is_ok(), "extra fields should be silently ignored");
                let payload = result.unwrap();
                prop_assert_eq!(payload.status, LeadQualificationStatus::Qualified);
            }

            /// LLM returns a status value not in our enum.
            #[test]
            fn unknown_status_value_is_rejected(
                status in "[a-z_]{1,30}".prop_filter("must not be a known status", |s| {
                    !matches!(s.as_str(), "qualified" | "disqualified" | "manual-review-required")
                }),
            ) {
                let content = format!(
                    "{{\"status\":\"{status}\",\"reason\":\"test\",\"fit_score\":80,\"authority_score\":70,\"urgency_score\":60,\"confidence_bps\":9000}}"
                );
                let result = serde_json::from_str::<LeadQualificationPayload>(&content);
                prop_assert!(result.is_err(), "unknown status '{status}' should fail deserialization");
            }

            /// LLM returns scores as strings instead of numbers.
            #[test]
            fn string_scores_are_rejected(
                score in 0u16..=100,
            ) {
                let content = format!(
                    "{{\"status\":\"qualified\",\"reason\":\"test\",\"fit_score\":\"{score}\",\"authority_score\":70,\"urgency_score\":60,\"confidence_bps\":9000}}"
                );
                let result = serde_json::from_str::<LeadQualificationPayload>(&content);
                prop_assert!(result.is_err(), "string-typed scores should fail deserialization");
            }

            /// LLM returns negative scores.
            #[test]
            fn negative_scores_in_json_are_rejected(
                score in -1000i32..=-1,
            ) {
                let content = format!(
                    "{{\"status\":\"qualified\",\"reason\":\"test\",\"fit_score\":{score},\"authority_score\":70,\"urgency_score\":60,\"confidence_bps\":9000}}"
                );
                let result = serde_json::from_str::<LeadQualificationPayload>(&content);
                // u16 can't be negative, so serde should reject
                prop_assert!(result.is_err(), "negative score {score} should fail u16 deserialization");
            }

            /// LLM omits a required field.
            #[test]
            fn missing_required_field_is_rejected(
                omit in prop_oneof![
                    Just("status"),
                    Just("reason"),
                    Just("fit_score"),
                    Just("authority_score"),
                    Just("urgency_score"),
                    Just("confidence_bps"),
                ],
            ) {
                let mut fields = vec![
                    ("status", "\"qualified\"".to_string()),
                    ("reason", "\"test\"".to_string()),
                    ("fit_score", "80".to_string()),
                    ("authority_score", "70".to_string()),
                    ("urgency_score", "60".to_string()),
                    ("confidence_bps", "9000".to_string()),
                ];
                fields.retain(|(key, _)| *key != omit);
                let pairs: Vec<String> = fields.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect();
                let content = format!("{{{}}}", pairs.join(","));
                let result = serde_json::from_str::<LeadQualificationPayload>(&content);
                prop_assert!(result.is_err(), "omitting '{omit}' should fail deserialization");
            }

            /// LLM returns null for a required field.
            #[test]
            fn null_required_field_is_rejected(
                null_field in prop_oneof![
                    Just("status"),
                    Just("reason"),
                    Just("fit_score"),
                ],
            ) {
                let content = match null_field.as_ref() {
                    "status" => r#"{"status":null,"reason":"test","fit_score":80,"authority_score":70,"urgency_score":60,"confidence_bps":9000}"#,
                    "reason" => r#"{"status":"qualified","reason":null,"fit_score":80,"authority_score":70,"urgency_score":60,"confidence_bps":9000}"#,
                    "fit_score" => r#"{"status":"qualified","reason":"test","fit_score":null,"authority_score":70,"urgency_score":60,"confidence_bps":9000}"#,
                    _ => unreachable!(),
                };
                let result = serde_json::from_str::<LeadQualificationPayload>(content);
                prop_assert!(result.is_err(), "null for required field '{null_field}' should fail");
            }

            /// LLM returns completely random bytes.
            #[test]
            fn random_bytes_never_panic(
                garbage in prop::collection::vec(any::<u8>(), 0..500),
            ) {
                let content = String::from_utf8_lossy(&garbage);
                let result = serde_json::from_str::<LeadQualificationPayload>(&content);
                // Must not panic — either Ok or Err
                let _ = result;
            }

            /// Confidence mapping from f64 to bps is always within bounds.
            #[test]
            fn converge_confidence_to_bps_is_bounded(
                confidence in prop::num::f64::ANY,
            ) {
                use crate::truth_runtime::common::converge_confidence_to_bps;
                let bps = converge_confidence_to_bps(confidence);
                prop_assert!(bps <= 10_000, "bps {bps} exceeds 10000 for confidence {confidence}");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Security property tests: injection and boundary attacks
    // -----------------------------------------------------------------------

    mod security_properties {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// SQL injection payloads in organization names must not crash
            /// or produce unexpected behavior. When SurrealDB lands, these
            /// strings will become query parameter values — never interpolated.
            #[test]
            fn sql_injection_in_org_name_is_safely_stored(
                injection in prop_oneof![
                    Just("'; DROP TABLE organizations; --".to_string()),
                    Just("Robert'); DROP TABLE students;--".to_string()),
                    Just("1 OR 1=1".to_string()),
                    Just("UNION SELECT * FROM users".to_string()),
                    Just("' UNION SELECT password FROM admin --".to_string()),
                ],
            ) {
                let store = InMemoryKernelStore::default_local();
                let inputs = HashMap::from([
                    ("organization_name".to_string(), injection.clone()),
                    (
                        "inbound_summary".to_string(),
                        "Need pricing for an AI pilot next week.".to_string(),
                    ),
                    ("contact_name".to_string(), "Alice".to_string()),
                    ("contact_title".to_string(), "CTO".to_string()),
                ]);

                let result = run_qualify(&store, inputs, true);
                // Must not crash. The injection string is stored verbatim.
                prop_assert!(result.is_ok(), "SQL injection payload should not crash execution");
                let projection = result.unwrap().projection.unwrap();
                let org = projection.organization.unwrap();
                prop_assert_eq!(org.name, injection, "injection payload must be stored exactly as received, not modified");
            }

            /// XSS payloads in input fields must be stored verbatim — no
            /// sanitization at the kernel level. Output encoding is the
            /// responsibility of the rendering layer (Tauri/web).
            #[test]
            fn xss_payloads_in_inputs_are_stored_verbatim(
                xss in prop_oneof![
                    Just("<script>alert('xss')</script>".to_string()),
                    Just("<img src=x onerror=alert(1)>".to_string()),
                    Just("javascript:alert(document.cookie)".to_string()),
                    Just("<svg onload=alert(1)>".to_string()),
                    Just("{{constructor.constructor('return this')()}}".to_string()),
                ],
            ) {
                let store = InMemoryKernelStore::default_local();
                let inputs = HashMap::from([
                    ("organization_name".to_string(), xss.clone()),
                    (
                        "inbound_summary".to_string(),
                        "Need pricing for an AI pilot next week.".to_string(),
                    ),
                    ("contact_name".to_string(), "Alice".to_string()),
                    ("contact_title".to_string(), "CTO".to_string()),
                ]);

                let result = run_qualify(&store, inputs, true);
                prop_assert!(result.is_ok());
                let org = result.unwrap().projection.unwrap().organization.unwrap();
                prop_assert_eq!(org.name, xss, "XSS payload must be stored verbatim — output encoding is the UI's responsibility");
            }

            /// Template injection payloads (Jinja, Handlebars, etc.) in
            /// fact statements must not be evaluated.
            #[test]
            fn template_injection_in_summary_does_not_crash(
                template in prop_oneof![
                    Just("${7*7}".to_string()),
                    Just("{{7*7}}".to_string()),
                    Just("#{7*7}".to_string()),
                    Just("<%= 7*7 %>".to_string()),
                    Just("${process.env.SECRET_KEY}".to_string()),
                    Just("{{constructor.constructor('return this')()}}".to_string()),
                ],
            ) {
                let store = InMemoryKernelStore::default_local();
                let inputs = HashMap::from([
                    ("organization_name".to_string(), "Template Test Corp".to_string()),
                    ("inbound_summary".to_string(), template),
                    ("contact_name".to_string(), "Alice".to_string()),
                    ("contact_title".to_string(), "CTO".to_string()),
                ]);

                let result = run_qualify(&store, inputs, true);
                prop_assert!(result.is_ok(), "template injection should not crash execution");
            }

            /// Path traversal in string inputs must not affect system behavior.
            #[test]
            fn path_traversal_in_inputs_is_harmless(
                traversal in prop_oneof![
                    Just("../../etc/passwd".to_string()),
                    Just("..\\..\\windows\\system32\\config\\sam".to_string()),
                    Just("/dev/null".to_string()),
                    Just("file:///etc/shadow".to_string()),
                    Just("\\\\server\\share\\secret.txt".to_string()),
                ],
            ) {
                let store = InMemoryKernelStore::default_local();
                let inputs = HashMap::from([
                    ("organization_name".to_string(), traversal),
                    (
                        "inbound_summary".to_string(),
                        "Need pricing for an AI pilot.".to_string(),
                    ),
                    ("contact_name".to_string(), "Alice".to_string()),
                    ("contact_title".to_string(), "CTO".to_string()),
                ]);

                let result = run_qualify(&store, inputs, true);
                prop_assert!(result.is_ok(), "path traversal payload should not crash execution");
            }

            /// Null bytes in strings must not cause truncation or crashes.
            #[test]
            fn null_bytes_in_strings_are_harmless(
                prefix in "[a-zA-Z]{1,10}",
                suffix in "[a-zA-Z]{1,10}",
            ) {
                let poisoned = format!("{prefix}\0{suffix}");
                let store = InMemoryKernelStore::default_local();
                let inputs = HashMap::from([
                    ("organization_name".to_string(), poisoned.clone()),
                    (
                        "inbound_summary".to_string(),
                        "Need pricing for an AI pilot.".to_string(),
                    ),
                    ("contact_name".to_string(), "Alice".to_string()),
                    ("contact_title".to_string(), "CTO".to_string()),
                ]);

                let result = run_qualify(&store, inputs, true);
                prop_assert!(result.is_ok(), "null byte should not crash execution");
                let org = result.unwrap().projection.unwrap().organization.unwrap();
                prop_assert_eq!(org.name, poisoned, "null byte string must be stored without truncation");
            }

            /// Agent fact content with embedded control characters must not
            /// corrupt the converge context or crash the engine.
            #[test]
            fn control_characters_in_fact_content_are_harmless(
                control_char in prop::sample::select(vec![
                    '\x00', '\x01', '\x02', '\x03', '\x04', '\x05', '\x06', '\x07',
                    '\x08', '\x0B', '\x0C', '\x0E', '\x0F', '\x10', '\x7F',
                ]),
            ) {
                let content = format!(
                    "{{\"status\":\"qualified\",\"reason\":\"test{control_char}value\",\"fit_score\":80,\"authority_score\":70,\"urgency_score\":60,\"confidence_bps\":9000}}"
                );
                let result = serde_json::from_str::<LeadQualificationPayload>(&content);
                // serde_json accepts control chars in strings — they become part of the value.
                // The important thing is no panic.
                let _ = result;
            }

            /// Very large JSON payloads (simulating LLM verbosity) must not
            /// cause OOM — they should either parse or fail cleanly.
            #[test]
            fn oversized_reason_field_does_not_oom(
                length in 10_000usize..=100_000,
            ) {
                let reason = "x".repeat(length);
                let content = format!(
                    "{{\"status\":\"qualified\",\"reason\":\"{reason}\",\"fit_score\":80,\"authority_score\":70,\"urgency_score\":60,\"confidence_bps\":9000}}"
                );
                let result = serde_json::from_str::<LeadQualificationPayload>(&content);
                // Must parse successfully — the reason field has no length limit at the JSON level.
                prop_assert!(result.is_ok(), "large reason field should parse");
                let payload = result.unwrap();
                prop_assert_eq!(payload.reason.len(), length);
            }
        }
    }
}
