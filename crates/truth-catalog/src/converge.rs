use converge_model::{
    RiskPosture, TruthDefinition as ConvergeTruth, TruthKind as ConvergeTruthKind, TypesBudgets,
    TypesConstraintSeverity, TypesIntentConstraint, TypesIntentId, TypesIntentKind, TypesObjective,
    TypesRootIntent,
};
use serde::Serialize;

use crate::resolve::{PackResolver, UnknownModule};
use crate::{TruthDefinition, TruthKind};

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
            .filter(|constraint| constraint.severity == TypesConstraintSeverity::Hard)
            .map(|constraint| constraint.value.to_string())
            .collect()
    }
}

impl TruthConvergeBinding {
    /// Build a [`TruthConvergeBinding`] from a definition and an injected
    /// [`PackResolver`].
    ///
    /// # Errors
    ///
    /// Returns [`UnknownModule`] when any module touched by `def` is not
    /// resolvable by `packs`.  The `truth_key` field of the error is populated
    /// here from `def.key`.
    pub fn build(def: TruthDefinition, packs: &dyn PackResolver) -> Result<Self, UnknownModule> {
        let pack_ids = packs.pack_ids_for(def.modules).map_err(|mut e| {
            e.truth_key = def.key.to_owned();
            e
        })?;
        Ok(Self {
            truth_key: def.key,
            runtime: "converge",
            pack_ids: pack_ids.clone(),
            approval_points: def.approval_points.to_vec(),
            intent: TypesRootIntent::builder()
                .id(TypesIntentId::new(format!("truth:{}", def.key)))
                .kind(TypesIntentKind::Custom)
                .request(truth_request(def))
                .objective(Some(TypesObjective::Custom(def.display_name.to_string())))
                .risk_posture(truth_risk_posture(def))
                .constraints(truth_constraints(def))
                .active_packs(pack_ids.iter().map(|p| (*p).into()).collect())
                .success_criteria(truth_success_criteria(def))
                .budgets(truth_budgets(def))
                .build(),
        })
    }
}

/// Build a [`ConvergeTruth`] from a definition and an injected [`PackResolver`].
///
/// # Errors
///
/// Propagates [`UnknownModule`] from [`TruthConvergeBinding::build`].
pub fn to_converge_truth(
    def: TruthDefinition,
    packs: &dyn PackResolver,
) -> Result<ConvergeTruth, UnknownModule> {
    // TruthDefinition is Copy so def remains available after build().
    let binding = TruthConvergeBinding::build(def, packs)?;
    Ok(ConvergeTruth {
        key: def.key.into(),
        kind: def.kind.into(),
        summary: def.summary.to_string(),
        success_criteria: binding.intent.success_criteria,
        constraints: binding.intent.constraints,
        approval_points: def.approval_points.iter().map(|p| (*p).into()).collect(),
        participating_packs: binding.pack_ids.into_iter().map(Into::into).collect(),
    })
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

fn truth_success_criteria(truth: TruthDefinition) -> Vec<converge_model::Criterion> {
    truth
        .desired_outcomes
        .iter()
        .map(|outcome| converge_model::Criterion::required(format!("outcome.{}", slug(outcome)), *outcome))
        .collect()
}

fn truth_budgets(_truth: TruthDefinition) -> TypesBudgets {
    TypesBudgets::default()
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

#[cfg(test)]
mod tests {
    use crate::resolve::{PackResolver, UnknownModule};
    use crate::{TruthDefinition, TruthKind, TruthModuleTouch};

    use super::TruthConvergeBinding;

    // --- Fixture types for mechanism tests (no real capability-* access) ---

    /// A resolver that maps module keys to pack IDs via a static table.
    struct StaticPackResolver(&'static [(&'static str, &'static str)]);

    impl PackResolver for StaticPackResolver {
        fn pack_ids_for(
            &self,
            modules: &[TruthModuleTouch],
        ) -> Result<Vec<&'static str>, UnknownModule> {
            let mut pack_ids = Vec::new();
            for touch in modules {
                let pack_id = self
                    .0
                    .iter()
                    .find(|(k, _)| *k == touch.module_key)
                    .map(|(_, p)| *p)
                    .ok_or_else(|| UnknownModule {
                        truth_key: String::new(),
                        module_key: touch.module_key.to_owned(),
                    })?;
                if !pack_ids.contains(&pack_id) {
                    pack_ids.push(pack_id);
                }
            }
            Ok(pack_ids)
        }
    }

    /// A resolver that always fails on the first module key it encounters.
    struct AlwaysUnknownResolver;

    impl PackResolver for AlwaysUnknownResolver {
        fn pack_ids_for(
            &self,
            modules: &[TruthModuleTouch],
        ) -> Result<Vec<&'static str>, UnknownModule> {
            Err(UnknownModule {
                truth_key: String::new(),
                module_key: modules
                    .first()
                    .map(|m| m.module_key.to_owned())
                    .unwrap_or_else(|| "unknown".to_owned()),
            })
        }
    }

    const FIXTURE_TRUTH: TruthDefinition = TruthDefinition {
        key: "approve-access-request",
        display_name: "Approve access request",
        kind: TruthKind::Job,
        summary: "Review and approve or deny an access request.",
        feature_path: "truths/jobs/approve_access_request.feature",
        actor_roles: &["security-operator"],
        approval_points: &["manual approval when risk is elevated"],
        desired_outcomes: &["access decision is recorded"],
        guardrails: &["decision must cite a policy"],
        modules: &[
            TruthModuleTouch {
                module_key: "identity",
                responsibility: "verify requestor identity",
            },
            TruthModuleTouch {
                module_key: "policies",
                responsibility: "apply access policy",
            },
        ],
        gherkin: "",
    };

    const FIXTURE_RESOLVER: StaticPackResolver = StaticPackResolver(&[
        ("identity", "trust"),
        ("policies", "prio-foundation-pack"),
    ]);

    // --- Negative test: unknown module → Err, not panic ---

    #[test]
    fn build_unknown_module_returns_err_not_panic() {
        let result = TruthConvergeBinding::build(FIXTURE_TRUTH, &AlwaysUnknownResolver);
        assert!(
            result.is_err(),
            "expected Err for unknown module but got Ok"
        );
    }

    #[test]
    fn unknown_module_error_carries_truth_key() {
        let err = TruthConvergeBinding::build(FIXTURE_TRUTH, &AlwaysUnknownResolver)
            .unwrap_err();
        assert_eq!(
            err.truth_key, "approve-access-request",
            "build() must fill truth_key into the error"
        );
    }

    #[test]
    fn unknown_module_error_carries_module_key() {
        let err = TruthConvergeBinding::build(FIXTURE_TRUTH, &AlwaysUnknownResolver)
            .unwrap_err();
        assert_eq!(
            err.module_key, "identity",
            "error must carry the unresolvable module key"
        );
    }

    #[test]
    fn unknown_module_error_message_matches_former_panic_text() {
        let err = TruthConvergeBinding::build(FIXTURE_TRUTH, &AlwaysUnknownResolver)
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("approve-access-request"),
            "message must contain truth key; got: {msg}"
        );
        assert!(
            msg.contains("identity"),
            "message must contain module key; got: {msg}"
        );
        assert!(
            msg.contains("references unknown module"),
            "message must match former panic text; got: {msg}"
        );
    }

    // --- Positive test: build over fixture resolver ---

    #[test]
    fn build_with_fixture_resolver_succeeds() {
        let binding =
            TruthConvergeBinding::build(FIXTURE_TRUTH, &FIXTURE_RESOLVER).expect("build failed");
        assert_eq!(binding.truth_key, "approve-access-request");
        assert_eq!(binding.runtime, "converge");
        assert_eq!(binding.pack_ids, vec!["trust", "prio-foundation-pack"]);
        assert_eq!(binding.approval_points, vec!["manual approval when risk is elevated"]);
    }

    #[test]
    fn build_pack_ids_are_deduped() {
        // Both modules resolve to the same pack; result must be len 1.
        let both_trust: StaticPackResolver = StaticPackResolver(&[
            ("identity", "trust"),
            ("policies", "trust"),
        ]);
        let binding =
            TruthConvergeBinding::build(FIXTURE_TRUTH, &both_trust).expect("build failed");
        assert_eq!(binding.pack_ids, vec!["trust"], "pack_ids must be deduped");
    }

    #[test]
    fn build_populates_intent_id() {
        let binding =
            TruthConvergeBinding::build(FIXTURE_TRUTH, &FIXTURE_RESOLVER).expect("build failed");
        assert_eq!(
            binding.intent.id.as_str(),
            "truth:approve-access-request"
        );
    }
}
