use std::collections::HashMap;
use std::sync::Arc;

use application_kernel::{Actor as CrmActor, FactRecord};
use application_storage::{KernelStore, StoreWriteResult};
use converge_kernel::{ContextState as Context, ConvergeResult, Engine};
use converge_pack::{AgentEffect, Context as ContextView, ContextKey, ProposedFact, Suggestor};
use converge_provider::{BoxFuture, ChatRequest, ChatResponse, DynChatBackend, LlmError};
use organism_pack::{
    BreadthResearchSuggestor, ContradictionFinderSuggestor, DdError, DdSearch,
    DepthResearchSuggestor, FactExtractorSuggestor, GapDetectorSuggestor, HuddleSeedSuggestor,
    IntentPacket, Plan, PlanStep, ReasoningSystem, SearchHit, SharedBudget, SynthesisSuggestor,
};
use tonic::Status;
use truth_catalog::{EvaluateAcquisitionTargetEvaluator, converge_binding_for_truth};

use super::{
    TruthExecutionArtifacts, TruthProjection,
    common::{optional_input, required_input},
    domain_event_kind_name,
};

const DD_PACK_ID: &str = "prio-dd-pack";
const TRUST_PACK_ID: &str = "trust";

// ── Input ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EvaluateAcquisitionTargetInput {
    pub target_company: String,
    pub focus_areas: Option<String>,
    pub max_searches: Option<usize>,
    pub max_llm_calls: Option<usize>,
}

impl EvaluateAcquisitionTargetInput {
    pub fn from_map(inputs: &HashMap<String, String>) -> Result<Self, Status> {
        Ok(Self {
            target_company: required_input(inputs, "target_company")?.to_string(),
            focus_areas: optional_input(inputs, "focus_areas"),
            max_searches: optional_input(inputs, "max_searches").and_then(|s| s.parse().ok()),
            max_llm_calls: optional_input(inputs, "max_llm_calls").and_then(|s| s.parse().ok()),
        })
    }
}

// ── Executor ────────────────────────────────────────────────────────

pub(super) async fn execute<S: KernelStore>(
    store: &S,
    runtime_stores: &application_storage::AppRuntimeStores,
    inputs: EvaluateAcquisitionTargetInput,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    let binding = converge_binding_for_truth("evaluate-acquisition-target")
        .ok_or_else(|| Status::not_found("truth not found: evaluate-acquisition-target"))?;

    let company = inputs.target_company.clone();
    let max_searches = inputs.max_searches.unwrap_or(10);
    let max_llm_calls = inputs.max_llm_calls.unwrap_or(8);

    let budget = Arc::new(
        SharedBudget::new()
            .with_limit("searches", max_searches)
            .with_limit("llm", max_llm_calls),
    );

    // Build search and LLM backends.
    // Option A (dev/demo): stub backends that prove the wiring works.
    // Option B (production): MCP directory → real provider backends.
    // Option C (governed): Converge capability axioms enforce credentials + budget.
    let search: Arc<dyn DdSearch> = Arc::new(StubDdSearch);
    let llm: Arc<dyn DynChatBackend> = Arc::new(StubChatBackend);

    // Build the initial plans (Organism huddle seed pattern)
    let intent = build_dd_intent(&company);
    let plans = build_dd_plans(&company);
    let huddle_seed = HuddleSeedSuggestor::from_plans(intent, plans);

    let mut engine = Engine::new();

    // Register organism DD suggestors — the convergent research loop
    engine.register_suggestor_in_pack(DD_PACK_ID, huddle_seed);
    engine.register_suggestor_in_pack(
        DD_PACK_ID,
        BreadthResearchSuggestor::new(&company, budget.clone(), search.clone()),
    );
    engine.register_suggestor_in_pack(
        DD_PACK_ID,
        DepthResearchSuggestor::new(&company, budget.clone(), search),
    );
    engine.register_suggestor_in_pack(
        DD_PACK_ID,
        FactExtractorSuggestor::new(&company, budget.clone(), llm.clone()),
    );
    engine.register_suggestor_in_pack(
        DD_PACK_ID,
        GapDetectorSuggestor::new(&company, budget.clone(), llm.clone())
            .with_max_generations(3)
            .with_min_hypotheses(5),
    );
    engine.register_suggestor_in_pack(DD_PACK_ID, ContradictionFinderSuggestor::new());
    engine.register_suggestor_in_pack(
        DD_PACK_ID,
        SynthesisSuggestor::new(&company, budget, llm).with_required_stable_cycles(1),
    );

    // Governance gate — blocks recommendation when contradictions need human review
    engine.register_suggestor_in_pack(TRUST_PACK_ID, ContradictionGateAgent);

    let seed_ctx = seed_context(&company)?;

    let (result, experience_events) = super::run_engine_with_runtime(
        runtime_stores,
        &mut engine,
        &super::RuntimeContext {
            scope_id: format!("dd:{}", company.to_lowercase().replace(' ', "-")),
        },
        seed_ctx,
        &binding.intent,
        std::sync::Arc::new(EvaluateAcquisitionTargetEvaluator),
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

// ── Governance Gate Suggestor ────────────────────────────────────────

struct ContradictionGateAgent;

#[async_trait::async_trait]
impl Suggestor for ContradictionGateAgent {
    fn name(&self) -> &str {
        "contradiction-gate"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Evaluations]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.get(ContextKey::Evaluations)
            .iter()
            .any(|f| f.id().starts_with("contradiction-"))
            && !ctx
                .get(ContextKey::Evaluations)
                .iter()
                .any(|f| f.id() == "dd:human-review-required")
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        let contradictions: Vec<_> = ctx
            .get(ContextKey::Evaluations)
            .iter()
            .filter(|f| f.id().starts_with("contradiction-"))
            .map(|f| f.id().clone())
            .collect();

        if contradictions.is_empty() {
            return AgentEffect::empty();
        }

        let content = serde_json::json!({
            "type": "governance-gate",
            "reason": "material contradictions detected in DD research",
            "contradiction_count": contradictions.len(),
            "contradiction_ids": contradictions,
            "required_action": "investment committee must review contradictions before recommendation",
        })
        .to_string();

        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Evaluations,
                "dd:human-review-required",
                content,
                "contradiction-gate",
            )
            .with_confidence(1.0),
        )
    }
}

// ── Intent & Plans ──────────────────────────────────────────────────

fn build_dd_intent(company: &str) -> IntentPacket {
    let expires = chrono::Utc::now() + chrono::Duration::hours(1);
    let mut intent = IntentPacket::new(
        format!("Build a due diligence brief for {company}"),
        expires,
    )
    .with_context(serde_json::json!({
        "company": company,
        "goal": "research breadth, depth, and investment risks",
    }));
    intent.authority = vec!["research".to_string()];
    intent
}

fn build_dd_plans(company: &str) -> Vec<Plan> {
    let expires = chrono::Utc::now() + chrono::Duration::hours(1);
    let intent = IntentPacket::new(format!("DD for {company}"), expires);

    let mut breadth1 = Plan::new(
        &intent,
        "Search wide for product, customer, and market context",
    );
    breadth1.contributor = ReasoningSystem::DomainModel;
    breadth1.steps = vec![PlanStep {
        action: format!("[breadth] {company} products customers market position overview"),
        expected_effect: "discover product scope, customer segments, and market presence".into(),
    }];

    let mut breadth2 = Plan::new(
        &intent,
        "Search wide for competitors, growth, and positioning",
    );
    breadth2.contributor = ReasoningSystem::CausalAnalysis;
    breadth2.steps = vec![PlanStep {
        action: format!("[breadth] {company} competitors trends growth tech stack USP"),
        expected_effect: "map competitive landscape and growth trajectory".into(),
    }];

    let mut depth1 = Plan::new(
        &intent,
        "Search deep for architecture and integration evidence",
    );
    depth1.contributor = ReasoningSystem::ConstraintSolver;
    depth1.steps = vec![PlanStep {
        action: format!("[depth] {company} technology architecture platform integrations API"),
        expected_effect: "understand technical moat and integration surface".into(),
    }];

    let mut depth2 = Plan::new(&intent, "Search deep for financial and ownership evidence");
    depth2.contributor = ReasoningSystem::CostEstimation;
    depth2.steps = vec![PlanStep {
        action: format!("[depth] {company} revenue ARR funding investors ownership financials"),
        expected_effect: "find financial metrics and ownership structure".into(),
    }];

    vec![breadth1, breadth2, depth1, depth2]
}

fn seed_context(company: &str) -> Result<Context, Status> {
    let mut ctx = Context::new();
    ctx.add_input(
        ContextKey::Seeds,
        "dd:target",
        serde_json::json!({ "company": company }).to_string(),
    )
    .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(ctx)
}

// ── Projection ──────────────────────────────────────────────────────

fn project<S: KernelStore>(
    store: &S,
    inputs: &EvaluateAcquisitionTargetInput,
    result: &ConvergeResult,
    actor: CrmActor,
) -> Result<TruthProjection, Status> {
    let synthesis_content = result
        .context
        .get(ContextKey::Proposals)
        .iter()
        .find(|f| f.id().starts_with("synthesis-"))
        .map(|f| f.content().to_string());

    let hypothesis_count = result.context.get(ContextKey::Hypotheses).len();
    let contradiction_count = result
        .context
        .get(ContextKey::Evaluations)
        .iter()
        .filter(|f| f.id().starts_with("contradiction-"))
        .count();

    let StoreWriteResult {
        value: facts,
        events,
    } = store
        .write_with_events(|kernel| {
            let dd_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "Due diligence for {} completed: {} hypotheses, {} contradictions{}",
                        inputs.target_company,
                        hypothesis_count,
                        contradiction_count,
                        synthesis_content
                            .as_ref()
                            .map(|_| ", synthesis produced")
                            .unwrap_or(", no synthesis (budget or convergence limit)")
                    ),
                    confidence_bps: if synthesis_content.is_some() {
                        8_000
                    } else {
                        5_000
                    },
                    related_to: vec![],
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            Ok(vec![dd_fact])
        })
        .map_err(super::status_from_storage)?;

    Ok(TruthProjection {
        organization: None,
        person: None,
        opportunity: None,
        subscription: None,
        entitlements: vec![],
        ledger_entries: vec![],
        documents: vec![],
        workflow_cases: vec![],
        facts,
        domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
    })
}

// ── Stub Backends (Option A: static/dev) ────────────────────────────
//
// These stubs prove the Truth→Formation→Convergence→Governance chain
// compiles and runs without external API keys. Replace with real
// backends for production (see kb/Architecture/Capability Binding.md).

struct StubDdSearch;

#[async_trait::async_trait]
impl DdSearch for StubDdSearch {
    async fn search(&self, query: &str) -> Result<Vec<SearchHit>, DdError> {
        Ok(vec![SearchHit {
            title: format!("Stub result for: {query}"),
            url: format!("https://stub.example.com/{}", query.replace(' ', "-")),
            content: format!(
                "This is a stub search result for the query '{query}'. \
                 In production, this would be a real Brave or Tavily search result."
            ),
            provider: "stub".into(),
        }])
    }
}

struct StubChatBackend;

impl DynChatBackend for StubChatBackend {
    fn chat(&self, _req: ChatRequest) -> BoxFuture<'_, Result<ChatResponse, LlmError>> {
        Box::pin(async {
            let content = serde_json::json!({
                "facts": [
                    {
                        "claim": "Stub company operates in the B2B SaaS market",
                        "category": "market",
                        "source_indices": [0],
                        "confidence": 0.8
                    },
                    {
                        "claim": "Stub company has approximately 200 employees",
                        "category": "team",
                        "source_indices": [0],
                        "confidence": 0.7
                    },
                    {
                        "claim": "Stub company uses a cloud-native architecture",
                        "category": "technology",
                        "source_indices": [0],
                        "confidence": 0.75
                    },
                    {
                        "claim": "Stub company competes with established players in the space",
                        "category": "competition",
                        "source_indices": [0],
                        "confidence": 0.7
                    },
                    {
                        "claim": "Stub company serves enterprise customers across Europe",
                        "category": "customers",
                        "source_indices": [0],
                        "confidence": 0.8
                    }
                ]
            })
            .to_string();
            Ok(ChatResponse {
                content,
                tool_calls: Vec::new(),
                usage: None,
                model: None,
                finish_reason: None,
                metadata: std::collections::HashMap::new(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn input_parsing_requires_target_company() {
        let inputs = HashMap::new();
        assert!(EvaluateAcquisitionTargetInput::from_map(&inputs).is_err());
    }

    #[test]
    fn input_parsing_accepts_minimal_inputs() {
        let mut inputs = HashMap::new();
        inputs.insert("target_company".to_string(), "Acme Corp".to_string());
        let parsed = EvaluateAcquisitionTargetInput::from_map(&inputs).unwrap();
        assert_eq!(parsed.target_company, "Acme Corp");
        assert!(parsed.focus_areas.is_none());
        assert!(parsed.max_searches.is_none());
    }

    #[test]
    fn dd_plans_cover_four_research_vectors() {
        let plans = build_dd_plans("TestCo");
        assert_eq!(plans.len(), 4);
        assert_eq!(plans[0].contributor, ReasoningSystem::DomainModel);
        assert_eq!(plans[1].contributor, ReasoningSystem::CausalAnalysis);
        assert_eq!(plans[2].contributor, ReasoningSystem::ConstraintSolver);
        assert_eq!(plans[3].contributor, ReasoningSystem::CostEstimation);
    }
}
