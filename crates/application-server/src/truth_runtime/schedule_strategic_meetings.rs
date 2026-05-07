use std::collections::HashMap;

use application_kernel::{
    ActivityAppend, ActivityOutcome, Actor as CrmActor, FactRecord, OrganizationLifecycle,
    OrganizationUpsert, RecordKind, RecordRef,
};
use application_storage::{KernelStore, StoreWriteResult};
use converge_kernel::{ContextState as Context, ConvergeResult, Engine};
use converge_pack::{AgentEffect, Context as ContextView, ContextKey, ProposedFact, Suggestor};
use serde::{Deserialize, Serialize};
use tonic::Status;
use truth_catalog::{ScheduleStrategicMeetingsEvaluator, converge_binding_for_truth};
use uuid::Uuid;

use super::{
    TruthExecutionArtifacts, TruthProjection,
    common::{
        converge_confidence_to_bps, has_fact_id, optional_input, payload_from_result,
        required_input,
    },
    domain_event_kind_name, status_from_storage,
};

const RELATIONSHIP_PACK_ID: &str = "prio-relationship-pack";
const COMMERCIAL_PACK_ID: &str = "prio-commercial-pack";
const WORK_PACK_ID: &str = "prio-work-pack";

const RANKED_CANDIDATES_FACT_ID: &str = "meeting:ranked-candidates";
const AVAILABILITY_FACT_ID: &str = "meeting:availability";
const SLATE_FACT_ID: &str = "meeting:slate";
const CONFIRMATION_FACT_ID: &str = "meeting:human-confirmation-required";

const RANKING_PROVENANCE: &str = "prio.schedule-strategic-meetings.ranker";
const SCHEDULING_PROVENANCE: &str = "prio.schedule-strategic-meetings.scheduler";

// --- Input ---

#[derive(Debug, Clone)]
pub struct ScheduleStrategicMeetingsInput {
    pub intent_text: String,
    pub requested_count: u32,
    pub window_start: String,
    pub window_end: String,
    pub prospects_json: String,
    pub strategy_context_json: Option<String>,
    pub calendar_slots_json: Option<String>,
    pub actor_name: Option<String>,
}

impl ScheduleStrategicMeetingsInput {
    pub fn from_map(inputs: &HashMap<String, String>) -> Result<Self, Status> {
        Ok(Self {
            intent_text: required_input(inputs, "intent_text")?.to_string(),
            requested_count: required_input(inputs, "requested_count")?
                .parse::<u32>()
                .map_err(|e| Status::invalid_argument(format!("invalid requested_count: {e}")))?,
            window_start: required_input(inputs, "window_start")?.to_string(),
            window_end: required_input(inputs, "window_end")?.to_string(),
            prospects_json: required_input(inputs, "prospects_json")?.to_string(),
            strategy_context_json: optional_input(inputs, "strategy_context_json"),
            calendar_slots_json: optional_input(inputs, "calendar_slots_json"),
            actor_name: optional_input(inputs, "actor_name"),
        })
    }
}

// --- Domain types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProspectSeed {
    organization_id: Option<Uuid>,
    name: String,
    contact_name: Option<String>,
    contact_email: Option<String>,
    fit_score_bps: Option<u16>,
    pipeline_stage: Option<String>,
    last_contact_days_ago: Option<u32>,
    estimated_value: Option<f64>,
    territory: Option<String>,
    segment: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StrategyContext {
    target_segments: Vec<String>,
    priority_territories: Vec<String>,
    focus_tags: Vec<String>,
    campaign_id: Option<String>,
}

impl Default for StrategyContext {
    fn default() -> Self {
        Self {
            target_segments: Vec::new(),
            priority_territories: Vec::new(),
            focus_tags: Vec::new(),
            campaign_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CalendarSlot {
    start: String,
    end: String,
    preference: SlotPreference,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SlotPreference {
    Preferred,
    Available,
    LastResort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RankedCandidate {
    rank: u32,
    prospect: ProspectSeed,
    strategy_score_bps: u16,
    readiness_score_bps: u16,
    combined_score_bps: u16,
    reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MeetingProposal {
    rank: u32,
    prospect_name: String,
    organization_id: Option<Uuid>,
    proposed_slot: Option<CalendarSlot>,
    strategy_alignment: String,
    readiness_signal: String,
    suggested_agenda: String,
    combined_score_bps: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MeetingSlatePayload {
    intent: String,
    window: String,
    proposals: Vec<MeetingProposal>,
    candidates_considered: u32,
    candidates_ranked: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AlignmentEvidence {
    prospect_name: String,
    fit_score_bps: u16,
    strategy_score_bps: u16,
    signals: Vec<String>,
}

// --- Execution ---

pub(super) async fn execute<S: KernelStore>(
    store: &S,
    runtime_stores: &application_storage::AppRuntimeStores,
    inputs: ScheduleStrategicMeetingsInput,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    let binding = converge_binding_for_truth("schedule-strategic-meetings")
        .ok_or_else(|| Status::not_found("truth not found: schedule-strategic-meetings"))?;

    let prospects = parse_prospects(&inputs)?;
    let strategy = parse_strategy(&inputs)?;
    let calendar_slots = parse_calendar_slots(&inputs)?;
    let requested_count = inputs.requested_count;

    let mut engine = Engine::new();
    engine.register_suggestor_in_pack(
        RELATIONSHIP_PACK_ID,
        CandidateRankerAgent {
            prospects,
            strategy: strategy.clone(),
            requested_count,
        },
    );
    engine.register_suggestor_in_pack(
        COMMERCIAL_PACK_ID,
        AvailabilityResolverAgent { calendar_slots },
    );
    engine.register_suggestor_in_pack(
        WORK_PACK_ID,
        SlateProposerAgent {
            intent: inputs.intent_text.clone(),
            window_start: inputs.window_start.clone(),
            window_end: inputs.window_end.clone(),
            requested_count,
        },
    );

    let scope_id = format!("meeting-schedule-{}", Uuid::new_v4().simple());
    let (result, experience_events) = super::run_engine_with_runtime(
        runtime_stores,
        &mut engine,
        &super::RuntimeContext { scope_id },
        seed_context(&inputs)?,
        &binding.intent,
        std::sync::Arc::new(ScheduleStrategicMeetingsEvaluator),
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

// --- Agents ---

struct CandidateRankerAgent {
    prospects: Vec<ProspectSeed>,
    strategy: StrategyContext,
    requested_count: u32,
}

#[async_trait::async_trait]
impl Suggestor for CandidateRankerAgent {
    fn name(&self) -> &str {
        "CandidateRankerAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.has(ContextKey::Seeds)
            && !has_fact_id(ctx, ContextKey::Proposals, RANKED_CANDIDATES_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        let mut ranked: Vec<RankedCandidate> = self
            .prospects
            .iter()
            .map(|prospect| {
                let strategy_score = compute_strategy_score(prospect, &self.strategy);
                let readiness_score = compute_readiness_score(prospect);
                let combined =
                    ((u32::from(strategy_score) * 6 + u32::from(readiness_score) * 4) / 10) as u16;
                let reasoning = build_ranking_reasoning(prospect, &self.strategy);

                RankedCandidate {
                    rank: 0,
                    prospect: prospect.clone(),
                    strategy_score_bps: strategy_score,
                    readiness_score_bps: readiness_score,
                    combined_score_bps: combined,
                    reasoning,
                }
            })
            .collect();

        ranked.sort_by(|a, b| b.combined_score_bps.cmp(&a.combined_score_bps));
        for (index, candidate) in ranked.iter_mut().enumerate() {
            candidate.rank = (index + 1) as u32;
        }
        ranked.truncate(self.requested_count as usize * 2);

        let content = serde_json::to_string(&ranked).unwrap_or_default();

        let mut builder = AgentEffect::builder();
        builder.push(
            ProposedFact::new(
                ContextKey::Proposals,
                RANKED_CANDIDATES_FACT_ID,
                content,
                RANKING_PROVENANCE,
            )
            .with_confidence(0.85),
        );

        for candidate in &ranked {
            builder.push(
                ProposedFact::new(
                    ContextKey::Signals,
                    format!("meeting:alignment:{}", slug(&candidate.prospect.name)),
                    serde_json::to_string(&AlignmentEvidence {
                        prospect_name: candidate.prospect.name.clone(),
                        fit_score_bps: candidate.prospect.fit_score_bps.unwrap_or(0),
                        strategy_score_bps: candidate.strategy_score_bps,
                        signals: collect_alignment_signals(candidate, &self.strategy),
                    })
                    .unwrap_or_default(),
                    RANKING_PROVENANCE,
                )
                .with_confidence(f64::from(candidate.combined_score_bps) / 10_000.0),
            );
        }

        builder.build()
    }
}

struct AvailabilityResolverAgent {
    calendar_slots: Vec<CalendarSlot>,
}

#[async_trait::async_trait]
impl Suggestor for AvailabilityResolverAgent {
    fn name(&self) -> &str {
        "AvailabilityResolverAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.has(ContextKey::Seeds) && !has_fact_id(ctx, ContextKey::Signals, AVAILABILITY_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        let content = serde_json::to_string(&self.calendar_slots).unwrap_or_default();

        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Signals,
                AVAILABILITY_FACT_ID,
                content,
                SCHEDULING_PROVENANCE,
            )
            .with_confidence(1.0),
        )
    }
}

struct SlateProposerAgent {
    intent: String,
    window_start: String,
    window_end: String,
    requested_count: u32,
}

#[async_trait::async_trait]
impl Suggestor for SlateProposerAgent {
    fn name(&self) -> &str {
        "SlateProposerAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Proposals, ContextKey::Signals]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Proposals, RANKED_CANDIDATES_FACT_ID)
            && has_fact_id(ctx, ContextKey::Signals, AVAILABILITY_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Strategies, SLATE_FACT_ID)
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        let candidates_fact = ctx
            .get(ContextKey::Proposals)
            .iter()
            .find(|f| f.id() == RANKED_CANDIDATES_FACT_ID)
            .cloned();
        let availability_fact = ctx
            .get(ContextKey::Signals)
            .iter()
            .find(|f| f.id() == AVAILABILITY_FACT_ID)
            .cloned();

        let candidates: Vec<RankedCandidate> = candidates_fact
            .and_then(|f| serde_json::from_str(&f.content()).ok())
            .unwrap_or_default();
        let slots: Vec<CalendarSlot> = availability_fact
            .and_then(|f| serde_json::from_str(&f.content()).ok())
            .unwrap_or_default();

        let candidates_considered = candidates.len() as u32;
        let take = self.requested_count as usize;

        let proposals: Vec<MeetingProposal> = candidates
            .into_iter()
            .take(take)
            .enumerate()
            .map(|(index, candidate)| {
                let slot = slots.get(index).cloned();
                MeetingProposal {
                    rank: (index + 1) as u32,
                    prospect_name: candidate.prospect.name.clone(),
                    organization_id: candidate.prospect.organization_id,
                    proposed_slot: slot,
                    strategy_alignment: candidate.reasoning.clone(),
                    readiness_signal: format!(
                        "readiness {} bps (stage: {}, last contact: {} days ago)",
                        candidate.readiness_score_bps,
                        candidate
                            .prospect
                            .pipeline_stage
                            .as_deref()
                            .unwrap_or("unknown"),
                        candidate.prospect.last_contact_days_ago.unwrap_or(0)
                    ),
                    suggested_agenda: format!(
                        "Explore {} fit for {}",
                        candidate.prospect.segment.as_deref().unwrap_or("general"),
                        candidate.prospect.name
                    ),
                    combined_score_bps: candidate.combined_score_bps,
                }
            })
            .collect();

        let slate = MeetingSlatePayload {
            intent: self.intent.clone(),
            window: format!("{} to {}", self.window_start, self.window_end),
            candidates_considered,
            candidates_ranked: proposals.len() as u32,
            proposals,
        };

        let mut builder = AgentEffect::builder();
        builder.push(
            ProposedFact::new(
                ContextKey::Strategies,
                SLATE_FACT_ID,
                serde_json::to_string(&slate).unwrap_or_default(),
                SCHEDULING_PROVENANCE,
            )
            .with_confidence(0.9),
        );

        builder.push(
            ProposedFact::new(
                ContextKey::Evaluations,
                CONFIRMATION_FACT_ID,
                serde_json::to_string(&serde_json::json!({
                    "reason": "meeting proposals require human confirmation before booking",
                    "proposal_count": slate.candidates_ranked,
                }))
                .unwrap_or_default(),
                SCHEDULING_PROVENANCE,
            )
            .with_confidence(1.0),
        );

        builder.build()
    }
}

// --- Scoring ---

fn compute_strategy_score(prospect: &ProspectSeed, strategy: &StrategyContext) -> u16 {
    let mut score: u32 = 2_000;

    let fit = u32::from(prospect.fit_score_bps.unwrap_or(0));
    score += fit * 3 / 10;

    if let Some(ref segment) = prospect.segment {
        if strategy.target_segments.iter().any(|s| s == segment) {
            score += 1_500;
        }
    }

    if let Some(ref territory) = prospect.territory {
        if strategy.priority_territories.iter().any(|t| t == territory) {
            score += 1_000;
        }
    }

    let tag_matches = prospect
        .tags
        .iter()
        .filter(|tag| strategy.focus_tags.iter().any(|f| f == *tag))
        .count() as u32;
    score += (tag_matches * 500).min(1_500);

    score.min(10_000) as u16
}

fn compute_readiness_score(prospect: &ProspectSeed) -> u16 {
    let mut score: u32 = 2_000;

    match prospect.pipeline_stage.as_deref() {
        Some("qualified") => score += 3_000,
        Some("discovery") => score += 2_500,
        Some("proposal") | Some("negotiation") => score += 3_500,
        Some("contacted") => score += 1_500,
        _ => score += 500,
    }

    match prospect.last_contact_days_ago {
        Some(0..=3) => score += 2_000,
        Some(4..=7) => score += 1_500,
        Some(8..=14) => score += 1_000,
        Some(15..=30) => score += 500,
        _ => {}
    }

    if let Some(value) = prospect.estimated_value {
        let value_component = ((value / 100_000.0).clamp(0.0, 1.0) * 2_000.0) as u32;
        score += value_component;
    }

    score.min(10_000) as u16
}

fn build_ranking_reasoning(prospect: &ProspectSeed, strategy: &StrategyContext) -> String {
    let mut reasons = Vec::new();

    if let Some(score) = prospect.fit_score_bps {
        if score >= 7_000 {
            reasons.push(format!("high fit score ({score} bps)"));
        } else if score >= 4_000 {
            reasons.push(format!("moderate fit score ({score} bps)"));
        }
    }

    if let Some(ref segment) = prospect.segment {
        if strategy.target_segments.iter().any(|s| s == segment) {
            reasons.push(format!("in target segment '{segment}'"));
        }
    }

    if let Some(ref territory) = prospect.territory {
        if strategy.priority_territories.iter().any(|t| t == territory) {
            reasons.push(format!("in priority territory '{territory}'"));
        }
    }

    match prospect.pipeline_stage.as_deref() {
        Some("qualified") | Some("discovery") => {
            reasons.push("pipeline stage shows active engagement".to_string());
        }
        Some("proposal") | Some("negotiation") => {
            reasons.push("advanced pipeline stage — close to decision".to_string());
        }
        _ => {}
    }

    if let Some(days) = prospect.last_contact_days_ago {
        if days <= 7 {
            reasons.push(format!("recent contact ({days} days ago)"));
        }
    }

    if reasons.is_empty() {
        "baseline candidate — no strong signals".to_string()
    } else {
        reasons.join("; ")
    }
}

fn collect_alignment_signals(
    candidate: &RankedCandidate,
    strategy: &StrategyContext,
) -> Vec<String> {
    let mut signals = Vec::new();

    if let Some(score) = candidate.prospect.fit_score_bps {
        signals.push(format!("fit-score:{score}"));
    }

    if let Some(ref segment) = candidate.prospect.segment {
        if strategy.target_segments.iter().any(|s| s == segment) {
            signals.push(format!("target-segment:{segment}"));
        }
    }

    if let Some(ref territory) = candidate.prospect.territory {
        if strategy.priority_territories.iter().any(|t| t == territory) {
            signals.push(format!("priority-territory:{territory}"));
        }
    }

    if let Some(ref stage) = candidate.prospect.pipeline_stage {
        signals.push(format!("pipeline:{stage}"));
    }

    signals
}

// --- Seed context ---

fn seed_context(inputs: &ScheduleStrategicMeetingsInput) -> Result<Context, Status> {
    let mut context = Context::new();
    context
        .add_input(
            ContextKey::Seeds,
            "schedule-strategic-meetings:intent",
            &inputs.intent_text,
        )
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(context)
}

// --- Parse inputs ---

fn parse_prospects(inputs: &ScheduleStrategicMeetingsInput) -> Result<Vec<ProspectSeed>, Status> {
    let prospects = serde_json::from_str::<Vec<ProspectSeed>>(&inputs.prospects_json)
        .map_err(|e| Status::invalid_argument(format!("invalid prospects_json: {e}")))?;
    if prospects.is_empty() {
        return Err(Status::invalid_argument("prospects_json must not be empty"));
    }
    Ok(prospects)
}

fn parse_strategy(inputs: &ScheduleStrategicMeetingsInput) -> Result<StrategyContext, Status> {
    match &inputs.strategy_context_json {
        Some(json) => serde_json::from_str(json)
            .map_err(|e| Status::invalid_argument(format!("invalid strategy_context_json: {e}"))),
        None => Ok(StrategyContext::default()),
    }
}

fn parse_calendar_slots(
    inputs: &ScheduleStrategicMeetingsInput,
) -> Result<Vec<CalendarSlot>, Status> {
    match &inputs.calendar_slots_json {
        Some(json) => serde_json::from_str(json)
            .map_err(|e| Status::invalid_argument(format!("invalid calendar_slots_json: {e}"))),
        None => Ok(Vec::new()),
    }
}

// --- Projection ---

fn project<S: KernelStore>(
    store: &S,
    inputs: &ScheduleStrategicMeetingsInput,
    result: &ConvergeResult,
    actor: CrmActor,
) -> Result<TruthProjection, Status> {
    let slate: MeetingSlatePayload =
        payload_from_result(result, ContextKey::Strategies, SLATE_FACT_ID)?;

    let StoreWriteResult {
        value: facts,
        events,
    } = store
        .write_with_events(|kernel| {
            let mut facts = Vec::new();
            let mut org_refs = Vec::new();

            for proposal in &slate.proposals {
                let org = kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: proposal.organization_id,
                        name: proposal.prospect_name.clone(),
                        external_key: None,
                        website: None,
                        industry: None,
                        lifecycle: OrganizationLifecycle::Prospect,
                        owner_user_id: None,
                        tags: vec!["meeting-proposed".to_string()],
                    },
                    actor.clone(),
                )?;
                org_refs.push(RecordRef {
                    kind: RecordKind::Organization,
                    id: org.id,
                });
            }

            let slate_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "Meeting slate proposed: {} meetings from {} candidates for '{}'",
                        slate.proposals.len(),
                        slate.candidates_considered,
                        slate.intent,
                    ),
                    confidence_bps: 8_500,
                    related_to: org_refs.clone(),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            facts.push(slate_fact);

            for (index, proposal) in slate.proposals.iter().enumerate() {
                let related_to = vec![org_refs[index].clone()];

                let alignment_fact = kernel.record_fact(
                    FactRecord {
                        statement: format!(
                            "Strategy alignment for {}: {} (combined score {} bps)",
                            proposal.prospect_name,
                            proposal.strategy_alignment,
                            proposal.combined_score_bps,
                        ),
                        confidence_bps: converge_confidence_to_bps(
                            f64::from(proposal.combined_score_bps) / 10_000.0,
                        ),
                        related_to: related_to.clone(),
                        source_note_id: None,
                    },
                    actor.clone(),
                )?;
                facts.push(alignment_fact);

                kernel.append_activity(
                    ActivityAppend {
                        subject: format!(
                            "Proposed meeting: {} (rank #{})",
                            proposal.prospect_name, proposal.rank
                        ),
                        details: proposal.suggested_agenda.clone(),
                        related_to,
                        outcome: ActivityOutcome::Waiting,
                        occurred_at: None,
                        next_action_due_at: None,
                    },
                    actor.clone(),
                )?;
            }

            Ok(facts)
        })
        .map_err(status_from_storage)?;

    Ok(TruthProjection {
        organization: None,
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
    slug
}

#[cfg(test)]
mod tests {
    use super::*;

    use application_kernel::Actor;
    use application_storage::InMemoryKernelStore;

    fn test_runtime_stores() -> application_storage::AppRuntimeStores {
        application_storage::AppRuntimeStores {
            context: application_storage::AppContextStore::Memory(
                application_storage::InMemoryContextStore::new(),
            ),
            experience: application_storage::AppExperienceStore::Memory(
                application_storage::InMemoryExperienceStoreAdapter::new(),
            ),
        }
    }

    #[tokio::test]
    async fn schedule_strategic_meetings_executes_end_to_end() {
        let store = InMemoryKernelStore::default_local();
        let runtime_stores = test_runtime_stores();
        let actor = Actor::system();

        let inputs = ScheduleStrategicMeetingsInput {
            intent_text: "Book three meetings this week with potential customers".to_string(),
            requested_count: 3,
            window_start: "2026-04-06".to_string(),
            window_end: "2026-04-10".to_string(),
            prospects_json: serde_json::json!([
                {
                    "name": "Acme Corp",
                    "organization_id": null,
                    "contact_name": "Alex Chen",
                    "contact_email": "alex@acme.com",
                    "fit_score_bps": 8200,
                    "pipeline_stage": "qualified",
                    "last_contact_days_ago": 3,
                    "estimated_value": 75000.0,
                    "territory": "nordics",
                    "segment": "mid-market",
                    "tags": ["ai-interested", "fast-mover"]
                },
                {
                    "name": "Globex Industries",
                    "organization_id": null,
                    "contact_name": "Jordan Lee",
                    "contact_email": "jordan@globex.com",
                    "fit_score_bps": 5500,
                    "pipeline_stage": "discovery",
                    "last_contact_days_ago": 12,
                    "estimated_value": 120000.0,
                    "territory": "nordics",
                    "segment": "enterprise",
                    "tags": ["compliance-focus"]
                },
                {
                    "name": "Initech",
                    "organization_id": null,
                    "contact_name": "Sam Rivera",
                    "contact_email": "sam@initech.io",
                    "fit_score_bps": 7100,
                    "pipeline_stage": "contacted",
                    "last_contact_days_ago": 1,
                    "estimated_value": 45000.0,
                    "territory": "dach",
                    "segment": "mid-market",
                    "tags": ["ai-interested"]
                },
                {
                    "name": "Umbrella Ltd",
                    "organization_id": null,
                    "contact_name": "Pat Kim",
                    "contact_email": "pat@umbrella.co",
                    "fit_score_bps": 3200,
                    "pipeline_stage": "new",
                    "last_contact_days_ago": 45,
                    "estimated_value": 20000.0,
                    "territory": "uk",
                    "segment": "smb",
                    "tags": []
                }
            ])
            .to_string(),
            strategy_context_json: Some(
                serde_json::json!({
                    "target_segments": ["mid-market", "enterprise"],
                    "priority_territories": ["nordics", "dach"],
                    "focus_tags": ["ai-interested", "fast-mover"],
                    "campaign_id": null
                })
                .to_string(),
            ),
            calendar_slots_json: Some(
                serde_json::json!([
                    { "start": "2026-04-07T09:00:00Z", "end": "2026-04-07T10:00:00Z", "preference": "preferred" },
                    { "start": "2026-04-08T14:00:00Z", "end": "2026-04-08T15:00:00Z", "preference": "available" },
                    { "start": "2026-04-09T11:00:00Z", "end": "2026-04-09T12:00:00Z", "preference": "preferred" }
                ])
                .to_string(),
            ),
            actor_name: Some("Karl".to_string()),
        };

        let execution = execute(&store, &runtime_stores, inputs, actor, true)
            .await
            .expect("truth should execute");

        assert!(execution.result.converged, "engine should complete");

        let has_blocked = execution.result.criteria_outcomes.iter().any(|outcome| {
            matches!(
                outcome.result,
                converge_kernel::CriterionResult::Blocked { .. }
            )
        });
        assert!(
            has_blocked,
            "should have blocked criteria awaiting human confirmation"
        );

        let has_slate = execution
            .result
            .context
            .get(ContextKey::Strategies)
            .iter()
            .any(|f| f.id == SLATE_FACT_ID);
        assert!(has_slate, "meeting slate should exist in context");

        let has_confirmation = execution
            .result
            .context
            .get(ContextKey::Evaluations)
            .iter()
            .any(|f| f.id == CONFIRMATION_FACT_ID);
        assert!(has_confirmation, "human confirmation fact should exist");

        let alignment_count = execution
            .result
            .context
            .get(ContextKey::Signals)
            .iter()
            .filter(|f| f.id.starts_with("meeting:alignment:"))
            .count();
        assert!(
            alignment_count >= 3,
            "should have alignment evidence for top candidates, got {alignment_count}"
        );

        let slate_fact = execution
            .result
            .context
            .get(ContextKey::Strategies)
            .iter()
            .find(|f| f.id == SLATE_FACT_ID)
            .expect("slate fact should exist");
        let slate: MeetingSlatePayload =
            serde_json::from_str(&slate_fact.content).expect("slate should deserialize");
        assert_eq!(slate.proposals.len(), 3);
        assert_eq!(slate.proposals[0].rank, 1);

        assert!(
            slate.proposals[0].combined_score_bps >= slate.proposals[1].combined_score_bps,
            "proposals should be ranked by combined score"
        );

        let projection = execution.projection.expect("projection should persist");
        assert!(
            projection.facts.len() >= 4,
            "should have slate fact + alignment facts, got {}",
            projection.facts.len()
        );
    }

    #[test]
    fn ranking_prefers_strategy_aligned_prospects() {
        let strategy = StrategyContext {
            target_segments: vec!["mid-market".to_string()],
            priority_territories: vec!["nordics".to_string()],
            focus_tags: vec!["ai-interested".to_string()],
            campaign_id: None,
        };

        let aligned = ProspectSeed {
            organization_id: None,
            name: "Aligned Corp".to_string(),
            contact_name: None,
            contact_email: None,
            fit_score_bps: Some(7_000),
            pipeline_stage: Some("qualified".to_string()),
            last_contact_days_ago: Some(2),
            estimated_value: Some(50_000.0),
            territory: Some("nordics".to_string()),
            segment: Some("mid-market".to_string()),
            tags: vec!["ai-interested".to_string()],
        };

        let unaligned = ProspectSeed {
            organization_id: None,
            name: "Random Inc".to_string(),
            contact_name: None,
            contact_email: None,
            fit_score_bps: Some(3_000),
            pipeline_stage: Some("new".to_string()),
            last_contact_days_ago: Some(60),
            estimated_value: Some(10_000.0),
            territory: Some("apac".to_string()),
            segment: Some("smb".to_string()),
            tags: vec![],
        };

        let aligned_score = compute_strategy_score(&aligned, &strategy);
        let unaligned_score = compute_strategy_score(&unaligned, &strategy);
        assert!(
            aligned_score > unaligned_score,
            "aligned prospect ({aligned_score}) should outscore unaligned ({unaligned_score})"
        );

        let aligned_readiness = compute_readiness_score(&aligned);
        let unaligned_readiness = compute_readiness_score(&unaligned);
        assert!(
            aligned_readiness > unaligned_readiness,
            "aligned prospect readiness ({aligned_readiness}) should outscore unaligned ({unaligned_readiness})"
        );
    }

    #[test]
    fn empty_strategy_still_ranks_by_fit_and_readiness() {
        let strategy = StrategyContext::default();

        let high_fit = ProspectSeed {
            organization_id: None,
            name: "High Fit".to_string(),
            contact_name: None,
            contact_email: None,
            fit_score_bps: Some(9_000),
            pipeline_stage: Some("qualified".to_string()),
            last_contact_days_ago: Some(1),
            estimated_value: Some(100_000.0),
            territory: None,
            segment: None,
            tags: vec![],
        };

        let low_fit = ProspectSeed {
            organization_id: None,
            name: "Low Fit".to_string(),
            contact_name: None,
            contact_email: None,
            fit_score_bps: Some(2_000),
            pipeline_stage: Some("new".to_string()),
            last_contact_days_ago: Some(30),
            estimated_value: None,
            territory: None,
            segment: None,
            tags: vec![],
        };

        let high_strategy = compute_strategy_score(&high_fit, &strategy);
        let low_strategy = compute_strategy_score(&low_fit, &strategy);
        assert!(high_strategy > low_strategy);
    }
}
