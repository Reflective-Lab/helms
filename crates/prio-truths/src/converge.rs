use converge_core::{
    Criterion, RiskPosture, TypesBudgets, TypesIntentConstraint, TypesIntentId, TypesIntentKind,
    TypesObjective, TypesRootIntent,
};
use prio_module_core::ModuleSuite;
use prio_modules::find_module;
use serde::Serialize;

use crate::{find_truth, TruthDefinition};

const FOUNDATION_PACK_ID: &str = "prio-foundation-pack";
const RELATIONSHIP_PACK_ID: &str = "prio-relationship-pack";
const COMMERCIAL_PACK_ID: &str = "prio-commercial-pack";
const REVENUE_PACK_ID: &str = "prio-revenue-pack";
const WORK_PACK_ID: &str = "prio-work-pack";
const TRUST_PACK_ID: &str = "trust";
const KNOWLEDGE_PACK_ID: &str = "knowledge";

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
