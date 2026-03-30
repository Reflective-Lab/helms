use std::collections::HashMap;

use converge_core::{
    Agent, AgentEffect, Context, ContextKey, ConvergeResult, Engine, Fact as ConvergeFact,
    ProposedFact,
};
use crm_kernel::{
    Actor as CrmActor, BillingPeriod, CatalogItem, CatalogPlanKind, FactRecord, Money,
    OrderSubscription, RecordKind, RecordRef, SubscriptionActivate, WorkflowCaseAdvance,
    WorkflowCaseCreate, WorkflowPriority, WorkflowState,
};
use crm_storage::{KernelStore, StoreWriteResult};
use prio_truths::{ActivateSubscriptionEvaluator, converge_binding_for_truth};
use serde::{Deserialize, Serialize};
use tonic::Status;
use uuid::Uuid;

use super::{
    TruthExecutionArtifacts, TruthProjection,
    common::{
        has_fact_id, optional_bool, optional_i64, optional_input, optional_uuid,
        payload_from_result, required_uuid,
    },
    domain_event_kind_name, status_from_storage,
};

const COMMERCIAL_PACK_ID: &str = "prio-commercial-pack";
const REVENUE_PACK_ID: &str = "prio-revenue-pack";
const WORK_PACK_ID: &str = "prio-work-pack";
const ACTIVATION_READY_FACT_ID: &str = "subscription:activation-ready";
const ENTITLEMENT_PREVIEW_FACT_ID: &str = "subscription:entitlement-preview";
const OPENING_BALANCE_FACT_ID: &str = "subscription:opening-balance";
const MANUAL_REVIEW_FACT_ID: &str = "subscription:manual-review-required";
const ACTIVATION_PROVENANCE: &str = "prio.activate-subscription.validation";
const ENTITLEMENT_PROVENANCE: &str = "prio.activate-subscription.entitlements";
const OPENING_BALANCE_PROVENANCE: &str = "prio.activate-subscription.ledger";
const REVIEW_PROVENANCE: &str = "prio.activate-subscription.approvals";

#[derive(Debug, Clone)]
struct ActivationSeed {
    subscription: OrderSubscription,
    catalog_item: CatalogItem,
    opening_balance: Money,
    manual_review_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActivationReadyPayload {
    subscription_id: Uuid,
    organization_id: Uuid,
    catalog_item_id: Uuid,
    catalog_sku: String,
    opening_balance_minor: i64,
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
struct OpeningBalancePayload {
    currency_code: String,
    amount_minor: i64,
    description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManualReviewPayload {
    reason: String,
}

#[derive(Clone)]
struct ActivationPlanAgent {
    seed: ActivationSeed,
}

#[derive(Clone)]
struct EntitlementPreviewAgent {
    catalog_item: CatalogItem,
}

#[derive(Clone)]
struct OpeningBalanceAgent {
    payload: OpeningBalancePayload,
}

#[derive(Clone)]
struct ManualReviewAgent {
    reason: String,
}

#[derive(Debug, Clone)]
pub struct ActivateSubscriptionInput {
    pub subscription_id: Uuid,
    pub catalog_item_id: Option<Uuid>,
    pub opening_balance_minor: Option<i64>,
    pub opening_balance_currency_code: Option<String>,
    pub force_manual_review: Option<bool>,
    pub manual_review_reason: Option<String>,
    pub owner_user_id: Option<String>,
    pub workflow_title: Option<String>,
}

impl ActivateSubscriptionInput {
    pub fn from_map(inputs: &HashMap<String, String>) -> Result<Self, Status> {
        Ok(Self {
            subscription_id: required_uuid(inputs, "subscription_id")?,
            catalog_item_id: optional_uuid(inputs, "catalog_item_id")?,
            opening_balance_minor: optional_i64(inputs, "opening_balance_minor"),
            opening_balance_currency_code: optional_input(inputs, "opening_balance_currency_code"),
            force_manual_review: optional_bool(inputs, "force_manual_review"),
            manual_review_reason: optional_input(inputs, "manual_review_reason"),
            owner_user_id: optional_input(inputs, "owner_user_id"),
            workflow_title: optional_input(inputs, "workflow_title"),
        })
    }
}

pub(super) fn execute<S: KernelStore>(
    store: &S,
    runtime_stores: &crm_storage::AppRuntimeStores,
    inputs: ActivateSubscriptionInput,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    let binding = converge_binding_for_truth("activate-subscription")
        .ok_or_else(|| Status::not_found("truth not found: activate-subscription"))?;

    let seed = load_activation_seed(store, &inputs)?;
    let opening_balance_payload = OpeningBalancePayload {
        currency_code: seed.opening_balance.currency_code.clone(),
        amount_minor: seed.opening_balance.amount_minor,
        description: format!("Opening balance for {}", seed.catalog_item.sku),
    };

    let mut engine = Engine::new();
    engine.register_in_pack(
        COMMERCIAL_PACK_ID,
        ActivationPlanAgent { seed: seed.clone() },
    );
    engine.register_in_pack(
        REVENUE_PACK_ID,
        EntitlementPreviewAgent {
            catalog_item: seed.catalog_item.clone(),
        },
    );
    engine.register_in_pack(
        REVENUE_PACK_ID,
        OpeningBalanceAgent {
            payload: opening_balance_payload,
        },
    );
    if let Some(reason) = seed.manual_review_reason.clone() {
        engine.register_in_pack(WORK_PACK_ID, ManualReviewAgent { reason });
    }

    let (result, experience_events) = super::run_engine_with_runtime(
        runtime_stores,
        &mut engine,
        &super::RuntimeContext { scope_id: inputs.subscription_id.to_string() },
        seed_context(seed.subscription.id)?,
        &binding.intent,
        std::sync::Arc::new(ActivateSubscriptionEvaluator),
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

fn load_activation_seed<S: KernelStore>(
    store: &S,
    inputs: &ActivateSubscriptionInput,
) -> Result<ActivationSeed, Status> {
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

            let catalog_item_id = inputs
                .catalog_item_id
                .or(subscription.catalog_item_id)
                .ok_or_else(|| {
                    Status::failed_precondition(
                        "subscription does not resolve to a catalog plan".to_string(),
                    )
                })?;
            let catalog_item = kernel
                .catalog_items
                .get(&catalog_item_id)
                .cloned()
                .ok_or_else(|| {
                    Status::not_found(format!("catalog item not found: {catalog_item_id}"))
                })?;

            let inferred_review_reason = infer_manual_review_reason(&catalog_item);
            let review_reason = if inputs.force_manual_review.unwrap_or(false) {
                Some(
                    inputs
                        .manual_review_reason
                        .clone()
                        .unwrap_or_else(|| "manual review requested by operator".to_string()),
                )
            } else {
                inferred_review_reason
            };
            let opening_balance = Money {
                currency_code: inputs
                    .opening_balance_currency_code
                    .clone()
                    .unwrap_or_else(|| subscription.value.currency_code.clone()),
                amount_minor: inputs.opening_balance_minor.unwrap_or(0),
            };

            Ok(ActivationSeed {
                subscription,
                catalog_item,
                opening_balance,
                manual_review_reason: review_reason,
            })
        })
        .map_err(status_from_storage)?
}

fn project<S: KernelStore>(
    store: &S,
    inputs: &ActivateSubscriptionInput,
    result: &ConvergeResult,
    actor: CrmActor,
) -> Result<TruthProjection, Status> {
    let activation = activation_ready_from_result(result)?;
    let entitlement_preview = entitlement_preview_from_result(result)?;
    let opening_balance = opening_balance_from_result(result)?;
    let manual_review = manual_review_from_result(result)?;

    if let Some(review) = manual_review {
        let owner_user_id = inputs.owner_user_id.clone();
        let title = inputs.workflow_title.clone().unwrap_or_else(|| {
            format!(
                "Manual review: activate subscription {}",
                activation.catalog_sku
            )
        });
        let related_to = activation_related_to(&activation);
        let StoreWriteResult { value, events } = store
            .write_with_events(|kernel| {
                let workflow_case = kernel.create_workflow_case(
                    WorkflowCaseCreate {
                        title,
                        priority: WorkflowPriority::High,
                        owner_user_id,
                        related_to: related_to.clone(),
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
                            "subscription activation awaiting manual review: {}",
                            review.reason
                        ),
                        confidence_bps: 10_000,
                        related_to,
                        source_note_id: None,
                    },
                    actor.clone(),
                )?;
                Ok((workflow_case, fact))
            })
            .map_err(status_from_storage)?;

        let (workflow_case, fact) = value;
        return Ok(TruthProjection {
            organization: None,
            person: None,
            opportunity: None,
            subscription: None,
            entitlements: Vec::new(),
            ledger_entries: Vec::new(),
            documents: Vec::new(),
            workflow_cases: vec![workflow_case],
            facts: vec![fact],
            domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
        });
    }

    let StoreWriteResult { value, events } = store
        .write_with_events(|kernel| {
            let activation_result = kernel.activate_subscription(
                SubscriptionActivate {
                    subscription_id: activation.subscription_id,
                    catalog_item_id: Some(activation.catalog_item_id),
                    opening_balance: Some(Money {
                        currency_code: opening_balance.currency_code.clone(),
                        amount_minor: opening_balance.amount_minor,
                    }),
                },
                actor.clone(),
            )?;
            let related_to = activation_related_to(&activation);
            let activation_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "subscription activated on plan {} with {} entitlement grants",
                        activation.catalog_sku,
                        entitlement_preview.grants.len()
                    ),
                    confidence_bps: 10_000,
                    related_to: related_to.clone(),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            let opening_balance_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "commercial opening balance initialized at {} {}",
                        opening_balance.amount_minor, opening_balance.currency_code
                    ),
                    confidence_bps: 10_000,
                    related_to,
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            Ok((
                activation_result.subscription,
                activation_result.entitlements,
                activation_result.opening_balance,
                vec![activation_fact, opening_balance_fact],
            ))
        })
        .map_err(status_from_storage)?;

    let (subscription, entitlements, ledger_entry, facts) = value;
    Ok(TruthProjection {
        organization: None,
        person: None,
        opportunity: None,
        subscription: Some(subscription),
        entitlements,
        ledger_entries: vec![ledger_entry],
        documents: Vec::new(),
        workflow_cases: Vec::new(),
        facts,
        domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
    })
}

impl Agent for ActivationPlanAgent {
    fn name(&self) -> &str {
        "SubscriptionActivationPlanAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
        ctx.has(ContextKey::Seeds)
            && !has_fact_id(ctx, ContextKey::Strategies, ACTIVATION_READY_FACT_ID)
    }

    fn execute(&self, _ctx: &dyn converge_core::ContextView) -> AgentEffect {
        AgentEffect::with_proposal(ProposedFact {
            key: ContextKey::Strategies,
            id: ACTIVATION_READY_FACT_ID.to_string(),
            content: serde_json::to_string(&ActivationReadyPayload {
                subscription_id: self.seed.subscription.id,
                organization_id: self.seed.subscription.organization_id,
                catalog_item_id: self.seed.catalog_item.id,
                catalog_sku: self.seed.catalog_item.sku.clone(),
                opening_balance_minor: self.seed.opening_balance.amount_minor,
                currency_code: self.seed.opening_balance.currency_code.clone(),
            })
            .expect("activation payload should serialize"),
            confidence: 0.99,
            provenance: ACTIVATION_PROVENANCE.to_string(),
        })
    }
}

impl Agent for EntitlementPreviewAgent {
    fn name(&self) -> &str {
        "EntitlementPreviewAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Strategies]
    }

    fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Strategies, ACTIVATION_READY_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Signals, ENTITLEMENT_PREVIEW_FACT_ID)
    }

    fn execute(&self, _ctx: &dyn converge_core::ContextView) -> AgentEffect {
        AgentEffect::with_proposal(ProposedFact {
            key: ContextKey::Signals,
            id: ENTITLEMENT_PREVIEW_FACT_ID.to_string(),
            content: serde_json::to_string(&EntitlementPreviewPayload {
                grants: entitlement_preview_items(&self.catalog_item),
            })
            .expect("entitlement preview should serialize"),
            confidence: 0.98,
            provenance: ENTITLEMENT_PROVENANCE.to_string(),
        })
    }
}

impl Agent for OpeningBalanceAgent {
    fn name(&self) -> &str {
        "OpeningBalanceAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Strategies]
    }

    fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Strategies, ACTIVATION_READY_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Evaluations, OPENING_BALANCE_FACT_ID)
    }

    fn execute(&self, _ctx: &dyn converge_core::ContextView) -> AgentEffect {
        AgentEffect::with_proposal(ProposedFact {
            key: ContextKey::Evaluations,
            id: OPENING_BALANCE_FACT_ID.to_string(),
            content: serde_json::to_string(&self.payload)
                .expect("opening balance payload should serialize"),
            confidence: 1.0,
            provenance: OPENING_BALANCE_PROVENANCE.to_string(),
        })
    }
}

impl Agent for ManualReviewAgent {
    fn name(&self) -> &str {
        "SubscriptionManualReviewAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Strategies]
    }

    fn accepts(&self, ctx: &dyn converge_core::ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Strategies, ACTIVATION_READY_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Evaluations, MANUAL_REVIEW_FACT_ID)
    }

    fn execute(&self, _ctx: &dyn converge_core::ContextView) -> AgentEffect {
        AgentEffect::with_proposal(ProposedFact {
            key: ContextKey::Evaluations,
            id: MANUAL_REVIEW_FACT_ID.to_string(),
            content: serde_json::to_string(&ManualReviewPayload {
                reason: self.reason.clone(),
            })
            .expect("manual review payload should serialize"),
            confidence: 1.0,
            provenance: REVIEW_PROVENANCE.to_string(),
        })
    }
}

fn infer_manual_review_reason(catalog_item: &CatalogItem) -> Option<String> {
    if matches!(catalog_item.plan_kind, CatalogPlanKind::EnterpriseCustom) {
        Some("enterprise custom plans require manual review".to_string())
    } else if catalog_item
        .pricing
        .as_ref()
        .is_some_and(|pricing| matches!(pricing.billing_period, BillingPeriod::Custom))
    {
        Some("custom billing periods require manual review".to_string())
    } else {
        None
    }
}

fn entitlement_preview_items(catalog_item: &CatalogItem) -> Vec<EntitlementPreviewItem> {
    let mut grants = catalog_item
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
        catalog_item
            .entitlement_template
            .quotas
            .iter()
            .map(|(key, value)| EntitlementPreviewItem {
                key: key.clone(),
                kind: "quota".to_string(),
                value: value.to_string(),
            }),
    );
    if let Some(credits) = catalog_item.entitlement_template.credit_balance_minor {
        grants.push(EntitlementPreviewItem {
            key: "credit_balance_minor".to_string(),
            kind: "credits".to_string(),
            value: credits.to_string(),
        });
    }
    grants
}

fn activation_ready_from_result(result: &ConvergeResult) -> Result<ActivationReadyPayload, Status> {
    payload_from_result(result, ContextKey::Strategies, ACTIVATION_READY_FACT_ID)
}

fn entitlement_preview_from_result(
    result: &ConvergeResult,
) -> Result<EntitlementPreviewPayload, Status> {
    payload_from_result(result, ContextKey::Signals, ENTITLEMENT_PREVIEW_FACT_ID)
}

fn opening_balance_from_result(result: &ConvergeResult) -> Result<OpeningBalancePayload, Status> {
    payload_from_result(result, ContextKey::Evaluations, OPENING_BALANCE_FACT_ID)
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
                Status::internal(format!("invalid manual review payload: {error}"))
            })
        })
        .transpose()
}

fn activation_related_to(payload: &ActivationReadyPayload) -> Vec<RecordRef> {
    vec![
        RecordRef {
            kind: RecordKind::Organization,
            id: payload.organization_id,
        },
        RecordRef {
            kind: RecordKind::OrderSubscription,
            id: payload.subscription_id,
        },
        RecordRef {
            kind: RecordKind::CatalogItem,
            id: payload.catalog_item_id,
        },
    ]
}

fn seed_context(subscription_id: Uuid) -> Result<Context, Status> {
    let mut context = Context::new();
    context
        .add_fact(ConvergeFact::new(
            ContextKey::Seeds,
            "activate-subscription:seed",
            subscription_id.to_string(),
        ))
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(context)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use converge_core::StopReason;
    use crm_kernel::{
        Actor, BillingPeriod, CatalogItemUpsert, CatalogPlanKind, EntitlementTemplate,
        OrganizationLifecycle, OrganizationUpsert, SubscriptionCreate, SubscriptionStatus,
    };
    use crm_storage::InMemoryKernelStore;

    #[test]
    fn activate_subscription_executes_end_to_end() {
        let store = InMemoryKernelStore::default_local();
        let runtime_stores = crm_storage::AppRuntimeStores {
            context: crm_storage::AppContextStore::Memory(crm_storage::InMemoryContextStore::new()),
            experience: crm_storage::AppExperienceStore::Memory(
                crm_storage::InMemoryExperienceStoreAdapter::new(),
            ),
        };
        let actor = Actor::system();
        let (subscription_id, catalog_item_id) = store
            .write(|kernel| {
                let organization = kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Activation Test".to_string(),
                        external_key: None,
                        website: None,
                        industry: None,
                        lifecycle: OrganizationLifecycle::Active,
                        owner_user_id: None,
                        tags: vec!["revenue".to_string()],
                    },
                    actor.clone(),
                )?;
                let catalog_item = kernel.upsert_catalog_item(
                    CatalogItemUpsert {
                        catalog_item_id: None,
                        sku: "prio-growth".to_string(),
                        name: "Prio Growth".to_string(),
                        description: Some("Growth annual plan".to_string()),
                        plan_kind: CatalogPlanKind::Subscription,
                        pricing: Some(crm_kernel::PricingMetadata {
                            billing_period: BillingPeriod::Annual,
                            list_price: Money {
                                currency_code: "USD".to_string(),
                                amount_minor: 24_000_00,
                            },
                            meter_name: Some("growth-seat".to_string()),
                        }),
                        entitlement_template: EntitlementTemplate {
                            feature_flags: vec!["workspace_access".to_string()],
                            quotas: BTreeMap::from([("seats".to_string(), 50)]),
                            credit_balance_minor: Some(250_000),
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
                        catalog_item_id: Some(catalog_item.id),
                        status: SubscriptionStatus::PendingActivation,
                        value: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 24_000_00,
                        },
                        started_at: None,
                    },
                    actor.clone(),
                )?;
                Ok((subscription.id, catalog_item.id))
            })
            .expect("seed data");

        let execution = execute(
            &store,
            &runtime_stores,
            ActivateSubscriptionInput {
                subscription_id,
                catalog_item_id: Some(catalog_item_id),
                opening_balance_minor: Some(0),
                opening_balance_currency_code: None,
                force_manual_review: None,
                manual_review_reason: None,
                owner_user_id: None,
                workflow_title: None,
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
                .map(|subscription| subscription.status),
            Some(crm_kernel::SubscriptionStatus::Active)
        );
        assert_eq!(projection.entitlements.len(), 3);
        assert_eq!(projection.ledger_entries.len(), 1);
        assert!(projection.workflow_cases.is_empty());
    }

    #[test]
    fn activate_subscription_blocks_for_manual_review_plan() {
        let store = InMemoryKernelStore::default_local();
        let runtime_stores = crm_storage::AppRuntimeStores {
            context: crm_storage::AppContextStore::Memory(crm_storage::InMemoryContextStore::new()),
            experience: crm_storage::AppExperienceStore::Memory(
                crm_storage::InMemoryExperienceStoreAdapter::new(),
            ),
        };
        let actor = Actor::system();
        let (subscription_id, catalog_item_id) = store
            .write(|kernel| {
                let organization = kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Activation Review Test".to_string(),
                        external_key: None,
                        website: None,
                        industry: None,
                        lifecycle: OrganizationLifecycle::Active,
                        owner_user_id: None,
                        tags: vec!["revenue".to_string()],
                    },
                    actor.clone(),
                )?;
                let catalog_item = kernel.upsert_catalog_item(
                    CatalogItemUpsert {
                        catalog_item_id: None,
                        sku: "prio-enterprise".to_string(),
                        name: "Prio Enterprise".to_string(),
                        description: Some("Enterprise custom plan".to_string()),
                        plan_kind: CatalogPlanKind::EnterpriseCustom,
                        pricing: Some(crm_kernel::PricingMetadata {
                            billing_period: BillingPeriod::Annual,
                            list_price: Money {
                                currency_code: "USD".to_string(),
                                amount_minor: 120_000_00,
                            },
                            meter_name: Some("enterprise-seat".to_string()),
                        }),
                        entitlement_template: EntitlementTemplate {
                            feature_flags: vec!["workspace_access".to_string()],
                            quotas: BTreeMap::from([("seats".to_string(), 250)]),
                            credit_balance_minor: Some(500_000),
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
                        catalog_item_id: Some(catalog_item.id),
                        status: SubscriptionStatus::PendingActivation,
                        value: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 120_000_00,
                        },
                        started_at: None,
                    },
                    actor.clone(),
                )?;
                Ok((subscription.id, catalog_item.id))
            })
            .expect("seed data");

        let execution = execute(
            &store,
            &runtime_stores,
            ActivateSubscriptionInput {
                subscription_id,
                catalog_item_id: Some(catalog_item_id),
                opening_balance_minor: None,
                opening_balance_currency_code: None,
                force_manual_review: None,
                manual_review_reason: None,
                owner_user_id: None,
                workflow_title: None,
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
        assert!(projection.subscription.is_none());
        assert!(projection.entitlements.is_empty());
        assert!(projection.ledger_entries.is_empty());
        assert_eq!(projection.workflow_cases.len(), 1);
        assert!(matches!(
            projection.workflow_cases[0].state,
            WorkflowState::AwaitingApproval
        ));
    }

    #[test]
    fn activate_subscription_missing_subscription_id_returns_error() {
        let _store = InMemoryKernelStore::default_local();
        let _runtime_stores = crm_storage::AppRuntimeStores {
            context: crm_storage::AppContextStore::Memory(crm_storage::InMemoryContextStore::new()),
            experience: crm_storage::AppExperienceStore::Memory(
                crm_storage::InMemoryExperienceStoreAdapter::new(),
            ),
        };
        let _actor = Actor::system();

        let result = ActivateSubscriptionInput::from_map(&HashMap::new());
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn activate_subscription_with_nonexistent_subscription_returns_error() {
        let store = InMemoryKernelStore::default_local();
        let runtime_stores = crm_storage::AppRuntimeStores {
            context: crm_storage::AppContextStore::Memory(crm_storage::InMemoryContextStore::new()),
            experience: crm_storage::AppExperienceStore::Memory(
                crm_storage::InMemoryExperienceStoreAdapter::new(),
            ),
        };
        let actor = Actor::system();
        let bogus_subscription_id = Uuid::new_v4();

        let result = execute(
            &store,
            &runtime_stores,
            ActivateSubscriptionInput {
                subscription_id: bogus_subscription_id,
                catalog_item_id: None,
                opening_balance_minor: None,
                opening_balance_currency_code: None,
                force_manual_review: None,
                manual_review_reason: None,
                owner_user_id: None,
                workflow_title: None,
            },
            actor,
            true,
        );

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[test]
    fn activate_without_persist_produces_no_side_effects() {
        let store = InMemoryKernelStore::default_local();
        let runtime_stores = crm_storage::AppRuntimeStores {
            context: crm_storage::AppContextStore::Memory(crm_storage::InMemoryContextStore::new()),
            experience: crm_storage::AppExperienceStore::Memory(
                crm_storage::InMemoryExperienceStoreAdapter::new(),
            ),
        };
        let actor = Actor::system();
        let (subscription_id, catalog_item_id) = store
            .write(|kernel| {
                let organization = kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "No Persist Test".to_string(),
                        external_key: None,
                        website: None,
                        industry: None,
                        lifecycle: OrganizationLifecycle::Active,
                        owner_user_id: None,
                        tags: vec!["revenue".to_string()],
                    },
                    actor.clone(),
                )?;
                let catalog_item = kernel.upsert_catalog_item(
                    CatalogItemUpsert {
                        catalog_item_id: None,
                        sku: "prio-starter".to_string(),
                        name: "Prio Starter".to_string(),
                        description: Some("Starter plan".to_string()),
                        plan_kind: CatalogPlanKind::Subscription,
                        pricing: Some(crm_kernel::PricingMetadata {
                            billing_period: BillingPeriod::Monthly,
                            list_price: Money {
                                currency_code: "USD".to_string(),
                                amount_minor: 5_000_00,
                            },
                            meter_name: Some("starter-seat".to_string()),
                        }),
                        entitlement_template: EntitlementTemplate {
                            feature_flags: vec!["workspace_access".to_string()],
                            quotas: BTreeMap::from([("seats".to_string(), 10)]),
                            credit_balance_minor: Some(50_000),
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
                        catalog_item_id: Some(catalog_item.id),
                        status: SubscriptionStatus::PendingActivation,
                        value: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 5_000_00,
                        },
                        started_at: None,
                    },
                    actor.clone(),
                )?;
                Ok((subscription.id, catalog_item.id))
            })
            .expect("seed data");

        let execution = execute(
            &store,
            &runtime_stores,
            ActivateSubscriptionInput {
                subscription_id,
                catalog_item_id: Some(catalog_item_id),
                opening_balance_minor: Some(0),
                opening_balance_currency_code: None,
                force_manual_review: None,
                manual_review_reason: None,
                owner_user_id: None,
                workflow_title: None,
            },
            actor,
            false,
        )
        .expect("truth should execute");

        assert!(matches!(
            execution.result.stop_reason,
            StopReason::CriteriaMet { .. }
        ));
        assert!(execution.projection.is_none());

        let subscription_status = store
            .read(|kernel| {
                kernel
                    .orders
                    .get(&subscription_id)
                    .expect("subscription should exist")
                    .status
            })
            .expect("read subscription status");

        assert_eq!(subscription_status, SubscriptionStatus::PendingActivation);
    }
}
