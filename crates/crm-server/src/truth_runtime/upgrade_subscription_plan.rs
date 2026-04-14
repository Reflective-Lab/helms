use std::collections::HashMap;

use application_kernel::{
    Actor as CrmActor, BillingPeriod, CatalogItem, CatalogPlanKind, EntitlementValue, FactRecord,
    Money, OrderSubscription, RecordKind, RecordRef, SubscriptionPlanChange, WorkflowCaseAdvance,
    WorkflowCaseCreate, WorkflowPriority, WorkflowState,
};
use application_storage::{KernelStore, StoreWriteResult};
use chrono::{DateTime, Utc};
use converge_core::{
    Agent, AgentEffect, Context, ContextKey, ConvergeResult, Engine, Fact as ConvergeFact,
    ProposedFact,
};
use prio_truths::{UpgradeSubscriptionPlanEvaluator, converge_binding_for_truth};
use serde::{Deserialize, Serialize};
use tonic::Status;
use uuid::Uuid;

use super::{
    TruthExecutionArtifacts, TruthProjection,
    common::{
        has_fact_id, optional_bool, optional_i64, optional_input, payload_from_result,
        required_datetime, required_uuid,
    },
    domain_event_kind_name, status_from_storage,
};

const COMMERCIAL_PACK_ID: &str = "prio-commercial-pack";
const REVENUE_PACK_ID: &str = "prio-revenue-pack";
const PLAN_CHANGE_READY_FACT_ID: &str = "subscription:plan-change-ready";
const ENTITLEMENT_PREVIEW_FACT_ID: &str = "subscription:plan-change-entitlements";
const COMMERCIAL_DELTA_FACT_ID: &str = "subscription:plan-change-delta";
const MANUAL_REVIEW_FACT_ID: &str = "subscription:plan-change-manual-review-required";
const PLAN_CHANGE_PROVENANCE: &str = "prio.upgrade-subscription-plan.validation";
const ENTITLEMENT_PROVENANCE: &str = "prio.upgrade-subscription-plan.entitlements";
const DELTA_PROVENANCE: &str = "prio.upgrade-subscription-plan.ledger";
const REVIEW_PROVENANCE: &str = "prio.upgrade-subscription-plan.approvals";

#[derive(Debug, Clone)]
struct PlanChangeSeed {
    subscription: OrderSubscription,
    current_catalog_item: CatalogItem,
    target_catalog_item: CatalogItem,
    effective_at: DateTime<Utc>,
    target_value: Money,
    resulting_credit_balance_minor: Option<i64>,
    manual_review_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlanChangeReadyPayload {
    subscription_id: Uuid,
    organization_id: Uuid,
    previous_catalog_item_id: Uuid,
    target_catalog_item_id: Uuid,
    previous_sku: String,
    target_sku: String,
    effective_at: String,
    previous_value_minor: i64,
    target_value_minor: i64,
    delta_amount_minor: i64,
    currency_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntitlementPreviewItem {
    key: String,
    kind: String,
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntitlementPreviewPayload {
    grants: Vec<EntitlementPreviewItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommercialDeltaPayload {
    previous_value_minor: i64,
    target_value_minor: i64,
    delta_amount_minor: i64,
    currency_code: String,
    effective_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManualReviewPayload {
    reason: String,
}

#[derive(Clone)]
struct PlanChangeValidationAgent {
    seed: PlanChangeSeed,
}

#[derive(Clone)]
struct EntitlementPreviewAgent {
    seed: PlanChangeSeed,
}

#[derive(Clone)]
struct CommercialDeltaAgent {
    seed: PlanChangeSeed,
}

#[derive(Debug, Clone)]
pub struct UpgradeSubscriptionPlanInput {
    pub subscription_id: Uuid,
    pub target_catalog_item_id: Uuid,
    pub effective_at: DateTime<Utc>,
    pub force_manual_review: Option<bool>,
    pub manual_review_reason: Option<String>,
    pub target_value_minor: Option<i64>,
    pub target_value_currency_code: Option<String>,
}

impl UpgradeSubscriptionPlanInput {
    pub fn from_map(inputs: &HashMap<String, String>) -> Result<Self, Status> {
        Ok(Self {
            subscription_id: required_uuid(inputs, "subscription_id")?,
            target_catalog_item_id: required_uuid(inputs, "target_catalog_item_id")?,
            effective_at: required_datetime(inputs, "effective_at")?,
            force_manual_review: optional_bool(inputs, "force_manual_review"),
            manual_review_reason: optional_input(inputs, "manual_review_reason"),
            target_value_minor: optional_i64(inputs, "target_value_minor"),
            target_value_currency_code: optional_input(inputs, "target_value_currency_code"),
        })
    }
}

pub(super) fn execute<S: KernelStore>(
    store: &S,
    runtime_stores: &application_storage::AppRuntimeStores,
    inputs: UpgradeSubscriptionPlanInput,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    let binding = converge_binding_for_truth("upgrade-subscription-plan")
        .ok_or_else(|| Status::not_found("truth not found: upgrade-subscription-plan"))?;

    let seed = load_plan_change_seed(store, &inputs)?;

    let mut engine = Engine::new();
    engine.register_in_pack(
        COMMERCIAL_PACK_ID,
        PlanChangeValidationAgent { seed: seed.clone() },
    );
    engine.register_in_pack(
        REVENUE_PACK_ID,
        EntitlementPreviewAgent { seed: seed.clone() },
    );
    engine.register_in_pack(REVENUE_PACK_ID, CommercialDeltaAgent { seed: seed.clone() });

    let (result, experience_events) = super::run_engine_with_runtime(
        runtime_stores,
        &mut engine,
        &super::RuntimeContext {
            scope_id: inputs.subscription_id.to_string(),
        },
        seed_context(seed.subscription.id)?,
        &binding.intent,
        std::sync::Arc::new(UpgradeSubscriptionPlanEvaluator),
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

fn load_plan_change_seed<S: KernelStore>(
    store: &S,
    inputs: &UpgradeSubscriptionPlanInput,
) -> Result<PlanChangeSeed, Status> {
    store
        .read(|kernel| {
            let subscription = kernel
                .orders
                .get(&inputs.subscription_id)
                .cloned()
                .ok_or_else(|| {
                    Status::not_found(format!(
                        "subscription not found: {}",
                        inputs.subscription_id
                    ))
                })?;
            if subscription.status != application_kernel::SubscriptionStatus::Active {
                return Err(Status::failed_precondition(
                    "plan upgrades require an active subscription".to_string(),
                ));
            }

            let current_catalog_item_id = subscription.catalog_item_id.ok_or_else(|| {
                Status::failed_precondition(
                    "active subscription does not resolve to a current catalog plan".to_string(),
                )
            })?;
            if current_catalog_item_id == inputs.target_catalog_item_id {
                return Err(Status::invalid_argument(
                    "target_catalog_item_id must differ from the current subscription plan"
                        .to_string(),
                ));
            }

            let current_catalog_item = kernel
                .catalog_items
                .get(&current_catalog_item_id)
                .cloned()
                .ok_or_else(|| {
                    Status::not_found(format!("catalog item not found: {current_catalog_item_id}"))
                })?;
            let target_catalog_item = kernel
                .catalog_items
                .get(&inputs.target_catalog_item_id)
                .cloned()
                .ok_or_else(|| {
                    Status::not_found(format!(
                        "catalog item not found: {}",
                        inputs.target_catalog_item_id
                    ))
                })?;

            let explicit_target_value = inputs.target_value_minor.map(|amount_minor| Money {
                currency_code: inputs
                    .target_value_currency_code
                    .clone()
                    .unwrap_or_else(|| subscription.value.currency_code.clone()),
                amount_minor,
            });
            let target_value = explicit_target_value
                .clone()
                .or_else(|| {
                    target_catalog_item
                        .pricing
                        .as_ref()
                        .map(|pricing| pricing.list_price.clone())
                })
                .unwrap_or_else(|| subscription.value.clone());
            if target_value.currency_code != subscription.value.currency_code {
                return Err(Status::failed_precondition(
                    "target commercial terms must use the subscription currency".to_string(),
                ));
            }

            let current_credit_balance_minor = kernel
                .entitlements
                .values()
                .find(|entitlement| {
                    entitlement.subscription_id == subscription.id
                        && entitlement.key == "credit_balance_minor"
                })
                .and_then(|entitlement| match entitlement.value {
                    EntitlementValue::Credits(value) => Some(value),
                    _ => None,
                });
            let resulting_credit_balance_minor = resulting_credit_balance_minor(
                &current_catalog_item,
                &target_catalog_item,
                current_credit_balance_minor,
            );

            let inferred_review_reason = infer_manual_review_reason(
                &subscription,
                &target_catalog_item,
                explicit_target_value.as_ref(),
                &target_value,
            );
            let review_reason = if inputs.force_manual_review.unwrap_or(false) {
                Some(
                    inputs
                        .manual_review_reason
                        .clone()
                        .unwrap_or_else(|| "manual review requested by operator".to_string()),
                )
            } else {
                inputs
                    .manual_review_reason
                    .clone()
                    .or(inferred_review_reason)
            };

            Ok(PlanChangeSeed {
                subscription,
                current_catalog_item,
                target_catalog_item,
                effective_at: inputs.effective_at,
                target_value,
                resulting_credit_balance_minor,
                manual_review_reason: review_reason,
            })
        })
        .map_err(status_from_storage)?
}

fn project<S: KernelStore>(
    store: &S,
    inputs: &UpgradeSubscriptionPlanInput,
    result: &ConvergeResult,
    actor: CrmActor,
) -> Result<TruthProjection, Status> {
    let manual_review = manual_review_from_result(result)?;

    if let Some(review) = manual_review {
        let subscription_id = inputs.subscription_id;
        let target_catalog_item_id = inputs.target_catalog_item_id;
        let effective_at = inputs.effective_at;
        let StoreWriteResult { value, events } = store
            .write_with_events(|kernel| {
                let subscription =
                    kernel
                        .orders
                        .get(&subscription_id)
                        .cloned()
                        .ok_or_else(|| application_kernel::KernelError::NotFound {
                            kind: "subscription",
                            id: subscription_id.to_string(),
                        })?;
                let target_catalog_item = kernel
                    .catalog_items
                    .get(&target_catalog_item_id)
                    .cloned()
                    .ok_or_else(|| application_kernel::KernelError::NotFound {
                        kind: "catalog_item",
                        id: target_catalog_item_id.to_string(),
                    })?;
                let workflow_case = kernel.create_workflow_case(
                    WorkflowCaseCreate {
                        title: format!("Manual review: upgrade to {}", target_catalog_item.sku),
                        priority: WorkflowPriority::High,
                        owner_user_id: None,
                        related_to: plan_change_related_to(
                            subscription.organization_id,
                            subscription.id,
                            target_catalog_item.id,
                        ),
                    },
                    actor.clone(),
                )?;
                let workflow_case = kernel.advance_workflow_case(
                    WorkflowCaseAdvance {
                        workflow_case_id: workflow_case.id,
                        state: WorkflowState::AwaitingApproval,
                    },
                    actor.clone(),
                )?;
                let fact = kernel.record_fact(
                    FactRecord {
                        statement: format!(
                            "subscription upgrade awaiting manual review for {} effective {}: {}",
                            target_catalog_item.sku,
                            effective_at.to_rfc3339(),
                            review.reason
                        ),
                        confidence_bps: 10_000,
                        related_to: plan_change_related_to(
                            subscription.organization_id,
                            subscription.id,
                            target_catalog_item.id,
                        ),
                        source_note_id: None,
                    },
                    actor.clone(),
                )?;
                Ok((subscription, workflow_case, fact))
            })
            .map_err(status_from_storage)?;

        let (subscription, workflow_case, fact) = value;
        return Ok(TruthProjection {
            organization: None,
            person: None,
            opportunity: None,
            subscription: Some(subscription),
            entitlements: Vec::new(),
            ledger_entries: Vec::new(),
            documents: Vec::new(),
            workflow_cases: vec![workflow_case],
            facts: vec![fact],
            domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
        });
    }

    let ready = plan_change_ready_from_result(result)?;
    let delta = commercial_delta_from_result(result)?;
    let StoreWriteResult { value, events } = store
        .write_with_events(|kernel| {
            let changed = kernel.change_subscription_plan(
                SubscriptionPlanChange {
                    subscription_id: ready.subscription_id,
                    target_catalog_item_id: ready.target_catalog_item_id,
                    effective_at: chrono::DateTime::parse_from_rfc3339(&ready.effective_at)
                        .expect("plan change effective_at should parse")
                        .with_timezone(&Utc),
                    target_value: Some(Money {
                        currency_code: ready.currency_code.clone(),
                        amount_minor: ready.target_value_minor,
                    }),
                    reason: Some(format!(
                        "Converged plan upgrade {} -> {}",
                        ready.previous_sku, ready.target_sku
                    )),
                },
                actor.clone(),
            )?;
            let plan_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "subscription upgraded from {} to {} effective {}",
                        ready.previous_sku, ready.target_sku, ready.effective_at
                    ),
                    confidence_bps: 10_000,
                    related_to: plan_change_related_to(
                        ready.organization_id,
                        ready.subscription_id,
                        ready.target_catalog_item_id,
                    ),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            let delta_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "commercial delta recorded at {} {}",
                        delta.delta_amount_minor, delta.currency_code
                    ),
                    confidence_bps: 10_000,
                    related_to: plan_change_related_to(
                        ready.organization_id,
                        ready.subscription_id,
                        ready.target_catalog_item_id,
                    ),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            Ok((changed, vec![plan_fact, delta_fact]))
        })
        .map_err(status_from_storage)?;

    let (changed, facts) = value;
    Ok(TruthProjection {
        organization: None,
        person: None,
        opportunity: None,
        subscription: Some(changed.subscription),
        entitlements: changed.entitlements,
        ledger_entries: vec![changed.ledger_entry],
        documents: Vec::new(),
        workflow_cases: Vec::new(),
        facts,
        domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
    })
}

impl Agent for PlanChangeValidationAgent {
    fn name(&self) -> &str {
        "SubscriptionPlanChangeValidationAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
        ctx.has(ContextKey::Seeds)
            && !has_fact_id(ctx, ContextKey::Strategies, PLAN_CHANGE_READY_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Evaluations, MANUAL_REVIEW_FACT_ID)
    }

    fn execute(&self, _ctx: &dyn converge_core::ContextView) -> AgentEffect {
        if let Some(reason) = &self.seed.manual_review_reason {
            return AgentEffect::with_proposal(ProposedFact {
                key: ContextKey::Evaluations,
                id: MANUAL_REVIEW_FACT_ID.to_string(),
                content: serde_json::to_string(&ManualReviewPayload {
                    reason: reason.clone(),
                })
                .expect("manual review payload should serialize"),
                confidence: 1.0,
                provenance: REVIEW_PROVENANCE.to_string(),
            });
        }

        AgentEffect::with_proposal(ProposedFact {
            key: ContextKey::Strategies,
            id: PLAN_CHANGE_READY_FACT_ID.to_string(),
            content: serde_json::to_string(&PlanChangeReadyPayload {
                subscription_id: self.seed.subscription.id,
                organization_id: self.seed.subscription.organization_id,
                previous_catalog_item_id: self.seed.current_catalog_item.id,
                target_catalog_item_id: self.seed.target_catalog_item.id,
                previous_sku: self.seed.current_catalog_item.sku.clone(),
                target_sku: self.seed.target_catalog_item.sku.clone(),
                effective_at: self.seed.effective_at.to_rfc3339(),
                previous_value_minor: self.seed.subscription.value.amount_minor,
                target_value_minor: self.seed.target_value.amount_minor,
                delta_amount_minor: self.seed.target_value.amount_minor
                    - self.seed.subscription.value.amount_minor,
                currency_code: self.seed.target_value.currency_code.clone(),
            })
            .expect("plan change payload should serialize"),
            confidence: 0.99,
            provenance: PLAN_CHANGE_PROVENANCE.to_string(),
        })
    }
}

impl Agent for EntitlementPreviewAgent {
    fn name(&self) -> &str {
        "SubscriptionPlanEntitlementPreviewAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Strategies]
    }

    fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Strategies, PLAN_CHANGE_READY_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Signals, ENTITLEMENT_PREVIEW_FACT_ID)
    }

    fn execute(&self, _ctx: &dyn converge_core::ContextView) -> AgentEffect {
        AgentEffect::with_proposal(ProposedFact {
            key: ContextKey::Signals,
            id: ENTITLEMENT_PREVIEW_FACT_ID.to_string(),
            content: serde_json::to_string(&EntitlementPreviewPayload {
                grants: plan_change_preview_items(&self.seed),
            })
            .expect("entitlement preview should serialize"),
            confidence: 0.98,
            provenance: ENTITLEMENT_PROVENANCE.to_string(),
        })
    }
}

impl Agent for CommercialDeltaAgent {
    fn name(&self) -> &str {
        "SubscriptionPlanCommercialDeltaAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Strategies]
    }

    fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Strategies, PLAN_CHANGE_READY_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Evaluations, COMMERCIAL_DELTA_FACT_ID)
    }

    fn execute(&self, _ctx: &dyn converge_core::ContextView) -> AgentEffect {
        AgentEffect::with_proposal(ProposedFact {
            key: ContextKey::Evaluations,
            id: COMMERCIAL_DELTA_FACT_ID.to_string(),
            content: serde_json::to_string(&CommercialDeltaPayload {
                previous_value_minor: self.seed.subscription.value.amount_minor,
                target_value_minor: self.seed.target_value.amount_minor,
                delta_amount_minor: self.seed.target_value.amount_minor
                    - self.seed.subscription.value.amount_minor,
                currency_code: self.seed.target_value.currency_code.clone(),
                effective_at: self.seed.effective_at.to_rfc3339(),
            })
            .expect("commercial delta should serialize"),
            confidence: 0.99,
            provenance: DELTA_PROVENANCE.to_string(),
        })
    }
}

fn infer_manual_review_reason(
    subscription: &OrderSubscription,
    target_catalog_item: &CatalogItem,
    explicit_target_value: Option<&Money>,
    target_value: &Money,
) -> Option<String> {
    if matches!(
        target_catalog_item.plan_kind,
        CatalogPlanKind::EnterpriseCustom
    ) {
        Some("enterprise custom plan changes require manual review".to_string())
    } else if target_catalog_item
        .pricing
        .as_ref()
        .is_some_and(|pricing| matches!(pricing.billing_period, BillingPeriod::Custom))
    {
        Some("custom billing terms require manual review".to_string())
    } else if target_catalog_item.pricing.is_none() && explicit_target_value.is_none() {
        Some("target plan requires explicit commercial terms".to_string())
    } else if explicit_target_value.is_some_and(|value| {
        target_catalog_item
            .pricing
            .as_ref()
            .is_some_and(|pricing| pricing.list_price != *value)
    }) {
        Some("price override requires approval".to_string())
    } else if target_value.amount_minor <= subscription.value.amount_minor {
        Some("non-positive commercial delta requires approval".to_string())
    } else {
        None
    }
}

fn resulting_credit_balance_minor(
    current_catalog_item: &CatalogItem,
    target_catalog_item: &CatalogItem,
    current_credit_balance_minor: Option<i64>,
) -> Option<i64> {
    let target_included = target_catalog_item
        .entitlement_template
        .credit_balance_minor?;
    let previous_included = current_catalog_item
        .entitlement_template
        .credit_balance_minor
        .unwrap_or(0);
    let uplift = (target_included - previous_included).max(0);
    Some(current_credit_balance_minor.unwrap_or(0) + uplift)
}

fn plan_change_preview_items(seed: &PlanChangeSeed) -> Vec<EntitlementPreviewItem> {
    let mut grants = seed
        .target_catalog_item
        .entitlement_template
        .feature_flags
        .iter()
        .map(|feature| EntitlementPreviewItem {
            key: feature.clone(),
            kind: "feature-flag".to_string(),
            value: "true".to_string(),
        })
        .collect::<Vec<_>>();
    grants.extend(
        seed.target_catalog_item
            .entitlement_template
            .quotas
            .iter()
            .map(|(key, value)| EntitlementPreviewItem {
                key: key.clone(),
                kind: "quota".to_string(),
                value: value.to_string(),
            }),
    );
    if let Some(credits) = seed.resulting_credit_balance_minor {
        grants.push(EntitlementPreviewItem {
            key: "credit_balance_minor".to_string(),
            kind: "credits".to_string(),
            value: credits.to_string(),
        });
    }
    grants
}

fn plan_change_ready_from_result(
    result: &ConvergeResult,
) -> Result<PlanChangeReadyPayload, Status> {
    payload_from_result(result, ContextKey::Strategies, PLAN_CHANGE_READY_FACT_ID)
}

fn commercial_delta_from_result(result: &ConvergeResult) -> Result<CommercialDeltaPayload, Status> {
    payload_from_result(result, ContextKey::Evaluations, COMMERCIAL_DELTA_FACT_ID)
}

fn manual_review_from_result(
    result: &ConvergeResult,
) -> Result<Option<ManualReviewPayload>, Status> {
    result
        .context
        .get(ContextKey::Evaluations)
        .iter()
        .find(|fact| fact.id == MANUAL_REVIEW_FACT_ID)
        .map(|fact| {
            serde_json::from_str(&fact.content).map_err(|error| {
                Status::internal(format!(
                    "invalid plan-change manual review payload: {error}"
                ))
            })
        })
        .transpose()
}

fn plan_change_related_to(
    organization_id: Uuid,
    subscription_id: Uuid,
    target_catalog_item_id: Uuid,
) -> Vec<RecordRef> {
    vec![
        RecordRef {
            kind: RecordKind::Organization,
            id: organization_id,
        },
        RecordRef {
            kind: RecordKind::OrderSubscription,
            id: subscription_id,
        },
        RecordRef {
            kind: RecordKind::CatalogItem,
            id: target_catalog_item_id,
        },
    ]
}

fn seed_context(subscription_id: Uuid) -> Result<Context, Status> {
    let mut context = Context::new();
    context
        .add_fact(ConvergeFact::new(
            ContextKey::Seeds,
            "upgrade-subscription-plan:seed",
            subscription_id.to_string(),
        ))
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(context)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use application_kernel::{
        Actor, CatalogItemUpsert, EntitlementTemplate, OrganizationLifecycle, OrganizationUpsert,
        SubscriptionActivate, SubscriptionCreate, SubscriptionStatus,
    };
    use application_storage::InMemoryKernelStore;
    use converge_core::StopReason;

    fn seeded_active_subscription_for_upgrade(
        store: &InMemoryKernelStore,
        actor: &Actor,
    ) -> (Uuid, Uuid, Uuid) {
        store
            .write(|kernel| {
                let organization = kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Upgrade Truth Test".to_string(),
                        external_key: None,
                        website: None,
                        industry: None,
                        lifecycle: OrganizationLifecycle::Active,
                        owner_user_id: None,
                        tags: vec!["upgrade".to_string()],
                    },
                    actor.clone(),
                )?;
                let starter = kernel.upsert_catalog_item(
                    CatalogItemUpsert {
                        catalog_item_id: None,
                        sku: "prio-starter".to_string(),
                        name: "Prio Starter".to_string(),
                        description: Some("Starter plan".to_string()),
                        plan_kind: CatalogPlanKind::Subscription,
                        pricing: Some(application_kernel::PricingMetadata {
                            billing_period: BillingPeriod::Monthly,
                            list_price: Money {
                                currency_code: "USD".to_string(),
                                amount_minor: 2_000_00,
                            },
                            meter_name: Some("starter-seat".to_string()),
                        }),
                        entitlement_template: EntitlementTemplate {
                            feature_flags: vec!["workspace_access".to_string()],
                            quotas: BTreeMap::from([("seats".to_string(), 5)]),
                            credit_balance_minor: Some(100_000),
                        },
                        active: true,
                    },
                    actor.clone(),
                )?;
                let growth = kernel.upsert_catalog_item(
                    CatalogItemUpsert {
                        catalog_item_id: None,
                        sku: "prio-growth".to_string(),
                        name: "Prio Growth".to_string(),
                        description: Some("Growth plan".to_string()),
                        plan_kind: CatalogPlanKind::Subscription,
                        pricing: Some(application_kernel::PricingMetadata {
                            billing_period: BillingPeriod::Monthly,
                            list_price: Money {
                                currency_code: "USD".to_string(),
                                amount_minor: 5_000_00,
                            },
                            meter_name: Some("growth-seat".to_string()),
                        }),
                        entitlement_template: EntitlementTemplate {
                            feature_flags: vec![
                                "workspace_access".to_string(),
                                "priority_support".to_string(),
                            ],
                            quotas: BTreeMap::from([("seats".to_string(), 25)]),
                            credit_balance_minor: Some(300_000),
                        },
                        active: true,
                    },
                    actor.clone(),
                )?;
                let subscription = kernel.create_order_subscription(
                    SubscriptionCreate {
                        subscription_id: None,
                        organization_id: organization.id,
                        quote_id: None,
                        catalog_item_id: Some(starter.id),
                        status: SubscriptionStatus::PendingActivation,
                        value: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 2_000_00,
                        },
                        started_at: None,
                    },
                    actor.clone(),
                )?;
                let _ = kernel.activate_subscription(
                    SubscriptionActivate {
                        subscription_id: subscription.id,
                        catalog_item_id: None,
                        opening_balance: None,
                    },
                    actor.clone(),
                )?;
                let _ = kernel.apply_credit_grant(
                    application_kernel::CreditGrantApply {
                        subscription_id: subscription.id,
                        amount: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 50_000,
                        },
                        payment_reference: "pay_upgrade_truth_seed".to_string(),
                        reason: Some("seed credit".to_string()),
                    },
                    actor.clone(),
                )?;
                Ok((organization.id, subscription.id, growth.id))
            })
            .expect("seed active subscription for upgrade")
    }

    #[test]
    fn upgrade_subscription_plan_executes_end_to_end() {
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
        let (_organization_id, subscription_id, target_catalog_item_id) =
            seeded_active_subscription_for_upgrade(&store, &actor);

        let execution = execute(
            &store,
            &runtime_stores,
            UpgradeSubscriptionPlanInput {
                subscription_id,
                target_catalog_item_id,
                effective_at: Utc::now(),
                force_manual_review: None,
                manual_review_reason: None,
                target_value_minor: None,
                target_value_currency_code: None,
            },
            actor,
            true,
        )
        .expect("truth should execute");

        assert!(matches!(
            execution.result.stop_reason,
            StopReason::CriteriaMet { .. }
        ));
        let projection = execution.projection.expect("projection should persist");
        assert_eq!(
            projection
                .subscription
                .as_ref()
                .and_then(|subscription| subscription.catalog_item_id),
            Some(target_catalog_item_id)
        );
        assert_eq!(projection.ledger_entries.len(), 1);
        assert_eq!(
            projection.ledger_entries[0].kind,
            application_kernel::LedgerEntryKind::Adjustment
        );
        assert!(projection.workflow_cases.is_empty());
        assert!(matches!(
            projection
                .entitlements
                .iter()
                .find(|entitlement| entitlement.key == "credit_balance_minor")
                .expect("credit entitlement")
                .value,
            EntitlementValue::Credits(350_000)
        ));
    }

    #[test]
    fn upgrade_subscription_plan_blocks_for_price_override() {
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
        let (_organization_id, subscription_id, target_catalog_item_id) =
            seeded_active_subscription_for_upgrade(&store, &actor);

        let execution = execute(
            &store,
            &runtime_stores,
            UpgradeSubscriptionPlanInput {
                subscription_id,
                target_catalog_item_id,
                effective_at: Utc::now(),
                force_manual_review: None,
                manual_review_reason: None,
                target_value_minor: Some(450000),
                target_value_currency_code: None,
            },
            actor,
            true,
        )
        .expect("truth should execute in blocked mode");

        assert!(matches!(
            execution.result.stop_reason,
            StopReason::HumanInterventionRequired { .. }
        ));
        assert!(
            execution
                .result
                .criteria_outcomes
                .iter()
                .any(|outcome| matches!(
                    outcome.result,
                    converge_core::CriterionResult::Blocked { .. }
                ))
        );

        let projection = execution.projection.expect("projection should persist");
        assert!(projection.ledger_entries.is_empty());
        assert!(projection.entitlements.is_empty());
        assert_eq!(projection.workflow_cases.len(), 1);
        assert_eq!(
            projection
                .subscription
                .as_ref()
                .map(|subscription| subscription.value.amount_minor),
            Some(2_000_00)
        );
    }
}
