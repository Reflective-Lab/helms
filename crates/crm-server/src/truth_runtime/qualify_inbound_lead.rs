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

pub(super) fn execute<S: KernelStore>(
    store: &S,
    inputs: HashMap<String, String>,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    let binding = converge_binding_for_truth("qualify-inbound-lead")
        .ok_or_else(|| Status::not_found("truth not found: qualify-inbound-lead"))?;

    required_input(&inputs, "organization_name")?;
    required_input(&inputs, "inbound_summary")?;

    let mut engine = Engine::new();
    engine.register_in_pack(COMMERCIAL_PACK_ID, LeadQualificationAgent);
    engine.register_in_pack(WORK_PACK_ID, LeadRoutingAgent);

    let observer = std::sync::Arc::new(RecordingObserver::default());
    let result = engine
        .run_with_types_intent_and_hooks(
            seed_context(&inputs)?,
            &binding.intent,
            TypesRunHooks {
                criterion_evaluator: Some(std::sync::Arc::new(QualifyInboundLeadEvaluator)),
                event_observer: Some(observer.clone()),
            },
        )
        .map_err(status_from_converge)?;

    let projection = if persist_projection {
        Some(project(store, &inputs, &result, actor)?)
    } else {
        None
    };

    Ok(TruthExecutionArtifacts {
        result,
        experience_events: observer.snapshot(),
        projection,
    })
}

fn project<S: KernelStore>(
    store: &S,
    inputs: &HashMap<String, String>,
    result: &ConvergeResult,
    actor: CrmActor,
) -> Result<TruthProjection, Status> {
    let organization_id = optional_uuid(inputs, "organization_id")?;
    let person_id = optional_uuid(inputs, "person_id")?;
    let organization_name = required_input(inputs, "organization_name")?.to_string();
    let inbound_summary = required_input(inputs, "inbound_summary")?.to_string();
    let organization_external_key = optional_input(inputs, "organization_external_key");
    let website = optional_input(inputs, "website");
    let industry = optional_input(inputs, "industry");
    let contact_name = optional_input(inputs, "contact_name");
    let contact_title = optional_input(inputs, "contact_title");
    let contact_email = optional_input(inputs, "contact_email");
    let contact_phone = optional_input(inputs, "contact_phone");
    let contact_linkedin_url = optional_input(inputs, "contact_linkedin_url");
    let subject = optional_input(inputs, "subject");
    let opportunity_name = optional_input(inputs, "opportunity_name");
    let currency_code =
        optional_input(inputs, "currency_code").unwrap_or_else(|| "USD".to_string());
    let opportunity_value_minor =
        optional_i64(inputs, "opportunity_value_minor").unwrap_or_default();

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

fn seed_context(inputs: &HashMap<String, String>) -> Result<Context, Status> {
    let mut context = Context::new();
    for (key, value) in BTreeMap::from_iter(inputs.iter().map(|(key, value)| (key, value))) {
        context
            .add_fact(ConvergeFact::new(
                ContextKey::Seeds,
                format!("input:{key}"),
                value.to_string(),
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

fn input_channel(inputs: &HashMap<String, String>) -> CommunicationChannel {
    match optional_input(inputs, "source_channel")
        .unwrap_or_else(|| "email".to_string())
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

fn optional_i64(inputs: &HashMap<String, String>, key: &str) -> Option<i64> {
    optional_input(inputs, key).and_then(|value| value.parse::<i64>().ok())
}

fn confidence_bps_for_projection(
    inputs: &HashMap<String, String>,
    qualification: Option<&LeadQualificationPayload>,
) -> u16 {
    optional_input(inputs, "confidence_bps")
        .and_then(|value| value.parse::<u16>().ok())
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

        let execution = execute(&store, inputs, human(), true).expect("truth should execute");

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
}
