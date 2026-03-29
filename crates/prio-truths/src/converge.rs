use converge_core::{
    Context, ContextKey, Criterion, CriterionEvaluator, CriterionResult, FactId, RiskPosture,
    TruthCatalog as ConvergeTruthCatalog, TruthDefinition as ConvergeTruth,
    TruthKind as ConvergeTruthKind, TypesBudgets, TypesIntentConstraint, TypesIntentId,
    TypesIntentKind, TypesObjective, TypesRootIntent,
};
use prio_module_core::ModuleSuite;
use prio_modules::find_module;
use serde::Serialize;

use crate::{TruthDefinition, TruthKind, all_truths, find_truth};

const FOUNDATION_PACK_ID: &str = "prio-foundation-pack";
const RELATIONSHIP_PACK_ID: &str = "prio-relationship-pack";
const COMMERCIAL_PACK_ID: &str = "prio-commercial-pack";
const REVENUE_PACK_ID: &str = "prio-revenue-pack";
const WORK_PACK_ID: &str = "prio-work-pack";
const TRUST_PACK_ID: &str = "trust";
const KNOWLEDGE_PACK_ID: &str = "knowledge";

pub struct StaticTruthCatalog;

pub struct QualifyInboundLeadEvaluator;
pub struct ActivateSubscriptionEvaluator;
pub struct RefillPrepaidAiCreditsEvaluator;
pub struct UpgradeSubscriptionPlanEvaluator;
pub struct SuspendServiceOnPaymentFailureEvaluator;
pub struct ScoreInboundFitEvaluator;
pub struct PlanOutboundCampaignEvaluator;
pub struct MatchRenewalContextEvaluator;

#[derive(Debug, Clone, Serialize)]
pub struct TruthConvergeBinding {
    pub truth_key: &'static str,
    pub runtime: &'static str,
    pub pack_ids: Vec<&'static str>,
    pub approval_points: Vec<&'static str>,
    pub intent: TypesRootIntent,
}

impl TruthConvergeBinding {
    #[must_use]
    pub fn intent_kind_name(&self) -> &'static str {
        intent_kind_name(&self.intent.kind)
    }

    #[must_use]
    pub fn required_success_criteria(&self) -> Vec<String> {
        self.intent
            .success_criteria
            .iter()
            .filter(|criterion| criterion.required)
            .map(|criterion| criterion.description.clone())
            .collect()
    }

    #[must_use]
    pub fn hard_constraints(&self) -> Vec<String> {
        self.intent
            .constraints
            .iter()
            .filter(|constraint| {
                constraint.severity == converge_core::TypesConstraintSeverity::Hard
            })
            .map(|constraint| constraint.value.clone())
            .collect()
    }
}

impl From<TruthDefinition> for TruthConvergeBinding {
    fn from(truth: TruthDefinition) -> Self {
        let pack_ids = pack_ids_for_truth(truth);
        Self {
            truth_key: truth.key,
            runtime: "converge",
            pack_ids: pack_ids.clone(),
            approval_points: truth.approval_points.to_vec(),
            intent: TypesRootIntent::builder()
                .id(TypesIntentId::new(format!("truth:{}", truth.key)))
                .kind(TypesIntentKind::Custom)
                .request(truth_request(truth))
                .objective(Some(TypesObjective::Custom(truth.display_name.to_string())))
                .risk_posture(truth_risk_posture(truth))
                .constraints(truth_constraints(truth))
                .active_packs(pack_ids.iter().map(ToString::to_string).collect())
                .success_criteria(truth_success_criteria(truth))
                .budgets(truth_budgets(truth))
                .build(),
        }
    }
}

#[must_use]
pub fn converge_binding_for_truth(truth_key: &str) -> Option<TruthConvergeBinding> {
    find_truth(truth_key).map(TruthConvergeBinding::from)
}

#[must_use]
pub fn converge_truth_definition(truth_key: &str) -> Option<ConvergeTruth> {
    find_truth(truth_key).map(ConvergeTruth::from)
}

impl ConvergeTruthCatalog for StaticTruthCatalog {
    fn list_truths(&self) -> Vec<ConvergeTruth> {
        all_truths().into_iter().map(ConvergeTruth::from).collect()
    }
}

impl From<TruthDefinition> for ConvergeTruth {
    fn from(truth: TruthDefinition) -> Self {
        let binding = TruthConvergeBinding::from(truth);
        Self {
            key: truth.key.to_string(),
            kind: truth.kind.into(),
            summary: truth.summary.to_string(),
            success_criteria: binding.intent.success_criteria,
            constraints: binding.intent.constraints,
            approval_points: truth
                .approval_points
                .iter()
                .map(ToString::to_string)
                .collect(),
            participating_packs: binding
                .pack_ids
                .into_iter()
                .map(ToString::to_string)
                .collect(),
        }
    }
}

impl From<TruthKind> for ConvergeTruthKind {
    fn from(value: TruthKind) -> Self {
        match value {
            TruthKind::Job => Self::Job,
            TruthKind::Policy => Self::Policy,
            TruthKind::ModuleLocal => Self::Invariant,
        }
    }
}

impl CriterionEvaluator for QualifyInboundLeadEvaluator {
    fn evaluate(&self, criterion: &Criterion, context: &Context) -> CriterionResult {
        match criterion.id.as_str() {
            "outcome.lead-is-explicitly-qualified-or-disqualified" => {
                if let Some(fact_id) =
                    find_fact_id(context, ContextKey::Evaluations, "lead:qualification")
                {
                    CriterionResult::Met {
                        evidence: vec![FactId::new(fact_id)],
                    }
                } else {
                    CriterionResult::Unmet {
                        reason: "lead qualification fact is missing".to_string(),
                    }
                }
            }
            "outcome.next-owner-and-next-step-are-recorded" => {
                let owner = find_fact_id(context, ContextKey::Strategies, "lead:owner");
                let next_step = find_fact_id(context, ContextKey::Strategies, "lead:next-step");
                match (owner, next_step) {
                    (Some(owner), Some(next_step)) => CriterionResult::Met {
                        evidence: vec![FactId::new(owner), FactId::new(next_step)],
                    },
                    (None, Some(_)) => CriterionResult::Unmet {
                        reason: "lead owner fact is missing".to_string(),
                    },
                    (Some(_), None) => CriterionResult::Unmet {
                        reason: "lead next-step fact is missing".to_string(),
                    },
                    (None, None) => CriterionResult::Unmet {
                        reason: "lead owner and next-step facts are missing".to_string(),
                    },
                }
            }
            _ => CriterionResult::Indeterminate,
        }
    }
}

impl CriterionEvaluator for ScoreInboundFitEvaluator {
    fn evaluate(&self, criterion: &Criterion, context: &Context) -> CriterionResult {
        match criterion.id.as_str() {
            "outcome.a-governed-fit-score-is-recorded-for-the-inbound-lead" => {
                require_fact(context, ContextKey::Evaluations, "lead:fit-score")
            }
            "outcome.the-score-cites-attributable-behavioral-evidence" => {
                require_fact(context, ContextKey::Signals, "lead:fit-evidence")
            }
            _ => CriterionResult::Indeterminate,
        }
    }
}

impl CriterionEvaluator for ActivateSubscriptionEvaluator {
    fn evaluate(&self, criterion: &Criterion, context: &Context) -> CriterionResult {
        if let Some(review_fact) = find_fact_id(
            context,
            ContextKey::Evaluations,
            "subscription:manual-review-required",
        ) {
            return CriterionResult::Blocked {
                reason: format!("manual review is required before activation ({review_fact})"),
                approval_ref: Some(review_fact),
            };
        }

        match criterion.id.as_str() {
            "outcome.subscription-becomes-active-with-an-explicit-plan" => require_fact(
                context,
                ContextKey::Strategies,
                "subscription:activation-ready",
            ),
            "outcome.entitlements-and-financial-opening-state-are-aligned" => {
                let entitlements = find_fact_id(
                    context,
                    ContextKey::Signals,
                    "subscription:entitlement-preview",
                );
                let balance = find_fact_id(
                    context,
                    ContextKey::Evaluations,
                    "subscription:opening-balance",
                );
                match (entitlements, balance) {
                    (Some(entitlements), Some(balance)) => CriterionResult::Met {
                        evidence: vec![FactId::new(entitlements), FactId::new(balance)],
                    },
                    (None, Some(_)) => CriterionResult::Unmet {
                        reason: "subscription entitlement preview fact is missing".to_string(),
                    },
                    (Some(_), None) => CriterionResult::Unmet {
                        reason: "subscription opening-balance fact is missing".to_string(),
                    },
                    (None, None) => CriterionResult::Unmet {
                        reason:
                            "subscription entitlement preview and opening-balance facts are missing"
                                .to_string(),
                    },
                }
            }
            _ => CriterionResult::Indeterminate,
        }
    }
}

impl CriterionEvaluator for RefillPrepaidAiCreditsEvaluator {
    fn evaluate(&self, criterion: &Criterion, context: &Context) -> CriterionResult {
        if let Some(review_fact) = find_fact_id(
            context,
            ContextKey::Evaluations,
            "credit-top-up:manual-review-required",
        ) {
            return CriterionResult::Blocked {
                reason: format!("manual review is required before refill ({review_fact})"),
                approval_ref: Some(review_fact),
            };
        }

        match criterion.id.as_str() {
            "outcome.confirmed-top-up-appears-in-the-ledger" => {
                let payment = find_fact_id(context, ContextKey::Evaluations, "payment:confirmed");
                let grant =
                    find_fact_id(context, ContextKey::Strategies, "credit-top-up:grant-ready");
                match (payment, grant) {
                    (Some(payment), Some(grant)) => CriterionResult::Met {
                        evidence: vec![FactId::new(payment), FactId::new(grant)],
                    },
                    (None, Some(_)) => CriterionResult::Unmet {
                        reason: "payment confirmation fact is missing".to_string(),
                    },
                    (Some(_), None) => CriterionResult::Unmet {
                        reason: "credit grant plan fact is missing".to_string(),
                    },
                    (None, None) => CriterionResult::Unmet {
                        reason: "payment confirmation and credit grant plan facts are missing"
                            .to_string(),
                    },
                }
            }
            "outcome.entitlement-balance-increases-for-the-correct-account" => require_fact(
                context,
                ContextKey::Signals,
                "credit-top-up:entitlement-adjustment",
            ),
            _ => CriterionResult::Indeterminate,
        }
    }
}

impl CriterionEvaluator for UpgradeSubscriptionPlanEvaluator {
    fn evaluate(&self, criterion: &Criterion, context: &Context) -> CriterionResult {
        if let Some(review_fact) = find_fact_id(
            context,
            ContextKey::Evaluations,
            "subscription:plan-change-manual-review-required",
        ) {
            return CriterionResult::Blocked {
                reason: format!("manual review is required before plan change ({review_fact})"),
                approval_ref: Some(review_fact),
            };
        }

        match criterion.id.as_str() {
            "outcome.subscription-moves-to-the-target-plan-on-an-explicit-date" => require_fact(
                context,
                ContextKey::Strategies,
                "subscription:plan-change-ready",
            ),
            "outcome.entitlements-and-commercial-delta-stay-aligned" => {
                let entitlements = find_fact_id(
                    context,
                    ContextKey::Signals,
                    "subscription:plan-change-entitlements",
                );
                let delta = find_fact_id(
                    context,
                    ContextKey::Evaluations,
                    "subscription:plan-change-delta",
                );
                match (entitlements, delta) {
                    (Some(entitlements), Some(delta)) => CriterionResult::Met {
                        evidence: vec![FactId::new(entitlements), FactId::new(delta)],
                    },
                    (None, Some(_)) => CriterionResult::Unmet {
                        reason: "subscription plan-change entitlement preview fact is missing"
                            .to_string(),
                    },
                    (Some(_), None) => CriterionResult::Unmet {
                        reason: "subscription commercial delta fact is missing".to_string(),
                    },
                    (None, None) => CriterionResult::Unmet {
                        reason: "subscription plan-change entitlement preview and commercial delta facts are missing"
                            .to_string(),
                    },
                }
            }
            _ => CriterionResult::Indeterminate,
        }
    }
}

impl CriterionEvaluator for SuspendServiceOnPaymentFailureEvaluator {
    fn evaluate(&self, criterion: &Criterion, context: &Context) -> CriterionResult {
        if let Some(review_fact) = find_fact_id(
            context,
            ContextKey::Evaluations,
            "subscription:suspension-manual-review-required",
        ) {
            return CriterionResult::Blocked {
                reason: format!("manual review is required before suspension ({review_fact})"),
                approval_ref: Some(review_fact),
            };
        }

        match criterion.id.as_str() {
            "outcome.service-state-matches-payment-policy" => {
                let suspended = find_fact_id(
                    context,
                    ContextKey::Strategies,
                    "subscription:suspension-ready",
                );
                let deferred = find_fact_id(
                    context,
                    ContextKey::Strategies,
                    "subscription:suspension-deferred",
                );
                let impact = find_fact_id(
                    context,
                    ContextKey::Signals,
                    "subscription:entitlement-impact",
                );
                match (suspended.or(deferred), impact) {
                    (Some(state), Some(impact)) => CriterionResult::Met {
                        evidence: vec![FactId::new(state), FactId::new(impact)],
                    },
                    (None, Some(_)) => CriterionResult::Unmet {
                        reason: "subscription suspension policy decision fact is missing"
                            .to_string(),
                    },
                    (Some(_), None) => CriterionResult::Unmet {
                        reason: "subscription entitlement impact fact is missing".to_string(),
                    },
                    (None, None) => CriterionResult::Unmet {
                        reason: "subscription suspension policy decision and entitlement impact facts are missing"
                            .to_string(),
                    },
                }
            }
            "outcome.customer-receives-a-clear-recovery-path" => require_fact(
                context,
                ContextKey::Strategies,
                "subscription:recovery-path",
            ),
            _ => CriterionResult::Indeterminate,
        }
    }
}

impl CriterionEvaluator for PlanOutboundCampaignEvaluator {
    fn evaluate(&self, criterion: &Criterion, context: &Context) -> CriterionResult {
        match criterion.id.as_str() {
            "outcome.a-governed-outbound-campaign-plan-exists" => {
                require_fact(context, ContextKey::Strategies, "campaign:plan")
            }
            "outcome.campaign-budget-status-is-explicit-and-queryable" => {
                require_fact(context, ContextKey::Evaluations, "campaign:budget-status")
            }
            _ => CriterionResult::Indeterminate,
        }
    }
}

impl CriterionEvaluator for MatchRenewalContextEvaluator {
    fn evaluate(&self, criterion: &Criterion, context: &Context) -> CriterionResult {
        match criterion.id.as_str() {
            "outcome.a-renewal-brief-is-attached-to-the-account-or-renewal-motion" => {
                require_fact(context, ContextKey::Strategies, "renewal:brief")
            }
            "outcome.retrieved-renewal-signals-stay-traceable-to-their-source-artifacts" => {
                require_any_fact(context, ContextKey::Signals, "renewal:signal:")
            }
            _ => CriterionResult::Indeterminate,
        }
    }
}

fn truth_request(truth: TruthDefinition) -> String {
    format!("{}: {}", truth.display_name, truth.summary)
}

fn truth_risk_posture(truth: TruthDefinition) -> RiskPosture {
    if truth.approval_points.is_empty() {
        RiskPosture::Balanced
    } else {
        RiskPosture::Conservative
    }
}

fn truth_constraints(truth: TruthDefinition) -> Vec<TypesIntentConstraint> {
    let mut constraints = Vec::with_capacity(truth.guardrails.len() + 1);
    constraints.push(TypesIntentConstraint::hard("truth.key", truth.key));
    constraints.extend(truth.guardrails.iter().map(|guardrail| {
        TypesIntentConstraint::hard(format!("guardrail.{}", slug(guardrail)), *guardrail)
    }));
    constraints
}

fn truth_success_criteria(truth: TruthDefinition) -> Vec<Criterion> {
    truth
        .desired_outcomes
        .iter()
        .map(|outcome| Criterion::required(format!("outcome.{}", slug(outcome)), *outcome))
        .collect()
}

fn truth_budgets(_truth: TruthDefinition) -> TypesBudgets {
    TypesBudgets::default()
}

fn pack_ids_for_truth(truth: TruthDefinition) -> Vec<&'static str> {
    let mut pack_ids = Vec::new();

    for touch in truth.modules {
        let module = find_module(touch.module_key).unwrap_or_else(|| {
            panic!(
                "truth '{}' references unknown module '{}'",
                truth.key, touch.module_key
            )
        });
        let pack_id = suite_pack_id(module.suite);
        if !pack_ids.contains(&pack_id) {
            pack_ids.push(pack_id);
        }
    }

    pack_ids
}

fn suite_pack_id(suite: ModuleSuite) -> &'static str {
    match suite {
        ModuleSuite::Foundation => FOUNDATION_PACK_ID,
        ModuleSuite::RelationshipCore => RELATIONSHIP_PACK_ID,
        ModuleSuite::CommercialCore => COMMERCIAL_PACK_ID,
        ModuleSuite::UsageRevenueCore => REVENUE_PACK_ID,
        ModuleSuite::WorkCore => WORK_PACK_ID,
        ModuleSuite::TrustCore => TRUST_PACK_ID,
        ModuleSuite::IntelligenceCore => KNOWLEDGE_PACK_ID,
    }
}

fn intent_kind_name(kind: &TypesIntentKind) -> &'static str {
    match kind {
        TypesIntentKind::GrowthStrategy => "growth-strategy",
        TypesIntentKind::Scheduling => "scheduling",
        TypesIntentKind::ResourceOptimization => "resource-optimization",
        TypesIntentKind::RiskAssessment => "risk-assessment",
        TypesIntentKind::ContentGeneration => "content-generation",
        TypesIntentKind::Custom => "custom",
        _ => "custom",
    }
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
        "value".to_string()
    } else {
        slug
    }
}

fn find_fact_id(context: &Context, key: ContextKey, fact_id: &str) -> Option<String> {
    context
        .get(key)
        .iter()
        .find(|fact| fact.id == fact_id)
        .map(|fact| fact.id.clone())
}

fn require_fact(context: &Context, key: ContextKey, fact_id: &str) -> CriterionResult {
    if let Some(fact_id) = find_fact_id(context, key, fact_id) {
        CriterionResult::Met {
            evidence: vec![FactId::new(fact_id)],
        }
    } else {
        CriterionResult::Unmet {
            reason: format!("{fact_id} fact is missing"),
        }
    }
}

fn require_any_fact(context: &Context, key: ContextKey, prefix: &str) -> CriterionResult {
    let evidence = context
        .get(key)
        .iter()
        .filter(|fact| fact.id.starts_with(prefix))
        .map(|fact| FactId::new(fact.id.clone()))
        .collect::<Vec<_>>();
    if evidence.is_empty() {
        CriterionResult::Unmet {
            reason: format!("no facts found with prefix {prefix}"),
        }
    } else {
        CriterionResult::Met { evidence }
    }
}
