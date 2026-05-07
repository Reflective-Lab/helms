use std::collections::HashMap;

use application_kernel::{
    Actor as CrmActor, CreditGrantApply, EntitlementValue, FactRecord, Money, OrderSubscription,
    RecordKind, RecordRef, WorkflowCaseAdvance, WorkflowCaseCreate, WorkflowPriority,
    WorkflowState,
};
use application_storage::{KernelStore, StoreWriteResult};
use converge_kernel::{ContextState as Context, ConvergeResult, Engine};
use converge_pack::{AgentEffect, Context as ContextView, ContextKey, ProposedFact, Suggestor};
use serde::{Deserialize, Serialize};
use tonic::Status;
use truth_catalog::{RefillPrepaidAiCreditsEvaluator, converge_binding_for_truth};
use uuid::Uuid;

use super::{
    TruthExecutionArtifacts, TruthProjection,
    common::{
        has_fact_id, optional_bool, optional_i64, optional_input, payload_from_result,
        required_input, required_uuid,
    },
    domain_event_kind_name, status_from_storage,
};

const REVENUE_PACK_ID: &str = "prio-revenue-pack";
const PAYMENT_CONFIRMED_FACT_ID: &str = "payment:confirmed";
const CREDIT_GRANT_READY_FACT_ID: &str = "credit-top-up:grant-ready";
const ENTITLEMENT_ADJUSTMENT_FACT_ID: &str = "credit-top-up:entitlement-adjustment";
const MANUAL_REVIEW_FACT_ID: &str = "credit-top-up:manual-review-required";
const PAYMENT_PROVENANCE: &str = "prio.refill-prepaid-ai-credits.payment";
const GRANT_PROVENANCE: &str = "prio.refill-prepaid-ai-credits.ledger";
const ENTITLEMENT_PROVENANCE: &str = "prio.refill-prepaid-ai-credits.entitlements";
const REVIEW_PROVENANCE: &str = "prio.refill-prepaid-ai-credits.policy";
const HIGH_RISK_THRESHOLD_MINOR: i64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaymentStatus {
    Confirmed,
    Pending,
    Failed,
    Unknown,
}

#[derive(Debug, Clone)]
struct RefillSeed {
    subscription: OrderSubscription,
    current_credit_balance_minor: i64,
    top_up_amount_minor: i64,
    payment_reference: String,
    payment_status: PaymentStatus,
    manual_review_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaymentConfirmationPayload {
    payment_reference: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreditGrantReadyPayload {
    subscription_id: Uuid,
    organization_id: Uuid,
    payment_reference: String,
    amount_minor: i64,
    currency_code: String,
    previous_balance_minor: i64,
    resulting_balance_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntitlementAdjustmentPayload {
    key: String,
    previous_balance_minor: i64,
    resulting_balance_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManualReviewPayload {
    reason: String,
}

#[derive(Clone)]
struct PaymentVerificationAgent {
    seed: RefillSeed,
}

#[derive(Clone)]
struct CreditGrantPlanAgent {
    seed: RefillSeed,
}

#[derive(Clone)]
struct EntitlementAdjustmentAgent {
    seed: RefillSeed,
}

#[derive(Debug, Clone)]
pub struct RefillPrepaidAiCreditsInput {
    pub subscription_id: Uuid,
    pub top_up_amount_minor: Option<i64>,
    pub payment_reference: String,
    pub payment_confirmed: Option<bool>,
    pub payment_status: Option<String>,
    pub risk_signal: Option<bool>,
    pub force_manual_review: Option<bool>,
    pub manual_review_reason: Option<String>,
}

impl RefillPrepaidAiCreditsInput {
    pub fn from_map(inputs: &HashMap<String, String>) -> Result<Self, Status> {
        Ok(Self {
            subscription_id: required_uuid(inputs, "subscription_id")?,
            top_up_amount_minor: optional_i64(inputs, "top_up_amount_minor"),
            payment_reference: required_input(inputs, "payment_reference")?.to_string(),
            payment_confirmed: optional_bool(inputs, "payment_confirmed"),
            payment_status: optional_input(inputs, "payment_status"),
            risk_signal: optional_bool(inputs, "risk_signal"),
            force_manual_review: optional_bool(inputs, "force_manual_review"),
            manual_review_reason: optional_input(inputs, "manual_review_reason"),
        })
    }
}

pub(super) async fn execute<S: KernelStore>(
    store: &S,
    runtime_stores: &application_storage::AppRuntimeStores,
    inputs: RefillPrepaidAiCreditsInput,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    let binding = converge_binding_for_truth("refill-prepaid-ai-credits")
        .ok_or_else(|| Status::not_found("truth not found: refill-prepaid-ai-credits"))?;

    let seed = load_refill_seed(store, &inputs)?;

    let mut engine = Engine::new();
    engine.register_suggestor_in_pack(
        REVENUE_PACK_ID,
        PaymentVerificationAgent { seed: seed.clone() },
    );
    engine.register_suggestor_in_pack(REVENUE_PACK_ID, CreditGrantPlanAgent { seed: seed.clone() });
    engine.register_suggestor_in_pack(
        REVENUE_PACK_ID,
        EntitlementAdjustmentAgent { seed: seed.clone() },
    );

    let (result, experience_events) = super::run_engine_with_runtime(
        runtime_stores,
        &mut engine,
        &super::RuntimeContext {
            scope_id: inputs.subscription_id.to_string(),
        },
        seed_context(seed.subscription.id)?,
        &binding.intent,
        std::sync::Arc::new(RefillPrepaidAiCreditsEvaluator),
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

fn load_refill_seed<S: KernelStore>(
    store: &S,
    inputs: &RefillPrepaidAiCreditsInput,
) -> Result<RefillSeed, Status> {
    let top_up_amount_minor = inputs
        .top_up_amount_minor
        .filter(|amount| *amount > 0)
        .ok_or_else(|| Status::invalid_argument("top_up_amount_minor must be positive"))?;
    let payment_status = payment_status_from_inputs(inputs);

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
                    "refill requires an active subscription".to_string(),
                ));
            }
            let entitlement = kernel
                .entitlements
                .values()
                .find(|entitlement| {
                    entitlement.subscription_id == subscription.id
                        && entitlement.key == "credit_balance_minor"
                })
                .cloned()
                .ok_or_else(|| {
                    Status::failed_precondition(
                        "refill requires an existing credit balance entitlement".to_string(),
                    )
                })?;
            let current_credit_balance_minor = match entitlement.value {
                EntitlementValue::Credits(value) => value,
                _ => {
                    return Err(Status::failed_precondition(
                        "credit balance entitlement must use credits semantics".to_string(),
                    ));
                }
            };

            let policy_review_reason = if payment_status != PaymentStatus::Confirmed {
                Some(format!(
                    "payment {} is not confirmed; credit grant is blocked",
                    payment_status_name(payment_status)
                ))
            } else if inputs.risk_signal.unwrap_or(false) {
                Some("top-up flagged for risk review".to_string())
            } else if top_up_amount_minor >= HIGH_RISK_THRESHOLD_MINOR {
                Some("top-up size exceeds the automatic approval threshold".to_string())
            } else {
                None
            };

            let manual_review_reason = if inputs.force_manual_review.unwrap_or(false) {
                inputs
                    .manual_review_reason
                    .clone()
                    .or_else(|| Some("manual review requested by operator".to_string()))
            } else {
                inputs.manual_review_reason.clone()
            };

            Ok(RefillSeed {
                subscription,
                current_credit_balance_minor,
                top_up_amount_minor,
                payment_reference: inputs.payment_reference.clone(),
                payment_status,
                manual_review_reason: manual_review_reason.or(policy_review_reason),
            })
        })
        .map_err(status_from_storage)?
}

fn project<S: KernelStore>(
    store: &S,
    inputs: &RefillPrepaidAiCreditsInput,
    result: &ConvergeResult,
    actor: CrmActor,
) -> Result<TruthProjection, Status> {
    let manual_review = manual_review_from_result(result)?;

    if let Some(review) = manual_review {
        let subscription_id = inputs.subscription_id;
        let payment_reference = inputs.payment_reference.clone();
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
                let workflow_case = kernel.create_workflow_case(
                    WorkflowCaseCreate {
                        title: format!("Manual review: prepaid refill {}", payment_reference),
                        priority: WorkflowPriority::High,
                        owner_user_id: None,
                        related_to: refill_related_to(
                            subscription.organization_id,
                            subscription.id,
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
                            "prepaid top-up blocked pending confirmation: {}",
                            review.reason
                        ),
                        confidence_bps: 10_000,
                        related_to: refill_related_to(
                            subscription.organization_id,
                            subscription.id,
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

    let grant_ready = credit_grant_ready_from_result(result)?;
    let adjustment = entitlement_adjustment_from_result(result)?;
    let StoreWriteResult { value, events } = store
        .write_with_events(|kernel| {
            let grant = kernel.apply_credit_grant(
                CreditGrantApply {
                    subscription_id: grant_ready.subscription_id,
                    amount: Money {
                        currency_code: grant_ready.currency_code.clone(),
                        amount_minor: grant_ready.amount_minor,
                    },
                    payment_reference: grant_ready.payment_reference.clone(),
                    reason: Some("Prepaid AI credit top-up".to_string()),
                },
                actor.clone(),
            )?;
            let confirmed_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "confirmed prepaid top-up {} applied for {} {}",
                        grant_ready.payment_reference,
                        grant_ready.amount_minor,
                        grant_ready.currency_code
                    ),
                    confidence_bps: 10_000,
                    related_to: refill_related_to(
                        grant_ready.organization_id,
                        grant_ready.subscription_id,
                    ),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            let balance_fact = kernel.record_fact(
                FactRecord {
                    statement: format!(
                        "credit balance increased from {} to {}",
                        adjustment.previous_balance_minor, adjustment.resulting_balance_minor
                    ),
                    confidence_bps: 10_000,
                    related_to: refill_related_to(
                        grant_ready.organization_id,
                        grant_ready.subscription_id,
                    ),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            Ok((grant, vec![confirmed_fact, balance_fact]))
        })
        .map_err(status_from_storage)?;

    let (grant, facts) = value;
    Ok(TruthProjection {
        organization: None,
        person: None,
        opportunity: None,
        subscription: Some(grant.subscription),
        entitlements: vec![grant.entitlement],
        ledger_entries: vec![grant.ledger_entry],
        documents: Vec::new(),
        workflow_cases: Vec::new(),
        facts,
        domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
    })
}

#[async_trait::async_trait]
impl Suggestor for PaymentVerificationAgent {
    fn name(&self) -> &str {
        "PaymentVerificationAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.has(ContextKey::Seeds)
            && !has_fact_id(ctx, ContextKey::Evaluations, PAYMENT_CONFIRMED_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Evaluations, MANUAL_REVIEW_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        if let Some(reason) = &self.seed.manual_review_reason {
            return AgentEffect::with_proposal(
                ProposedFact::new(
                    ContextKey::Evaluations,
                    MANUAL_REVIEW_FACT_ID.to_string(),
                    serde_json::to_string(&ManualReviewPayload {
                        reason: reason.clone(),
                    })
                    .expect("manual review payload should serialize"),
                    REVIEW_PROVENANCE.to_string(),
                )
                .with_confidence(1.0),
            );
        }

        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Evaluations,
                PAYMENT_CONFIRMED_FACT_ID.to_string(),
                serde_json::to_string(&PaymentConfirmationPayload {
                    payment_reference: self.seed.payment_reference.clone(),
                    status: payment_status_name(self.seed.payment_status).to_string(),
                })
                .expect("payment confirmation payload should serialize"),
                PAYMENT_PROVENANCE.to_string(),
            )
            .with_confidence(1.0),
        )
    }
}

#[async_trait::async_trait]
impl Suggestor for CreditGrantPlanAgent {
    fn name(&self) -> &str {
        "CreditGrantPlanAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Evaluations]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Evaluations, PAYMENT_CONFIRMED_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Strategies, CREDIT_GRANT_READY_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Strategies,
                CREDIT_GRANT_READY_FACT_ID.to_string(),
                serde_json::to_string(&CreditGrantReadyPayload {
                    subscription_id: self.seed.subscription.id,
                    organization_id: self.seed.subscription.organization_id,
                    payment_reference: self.seed.payment_reference.clone(),
                    amount_minor: self.seed.top_up_amount_minor,
                    currency_code: self.seed.subscription.value.currency_code.clone(),
                    previous_balance_minor: self.seed.current_credit_balance_minor,
                    resulting_balance_minor: self.seed.current_credit_balance_minor
                        + self.seed.top_up_amount_minor,
                })
                .expect("credit grant plan payload should serialize"),
                GRANT_PROVENANCE.to_string(),
            )
            .with_confidence(0.99),
        )
    }
}

#[async_trait::async_trait]
impl Suggestor for EntitlementAdjustmentAgent {
    fn name(&self) -> &str {
        "EntitlementAdjustmentAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Strategies]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Strategies, CREDIT_GRANT_READY_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Signals, ENTITLEMENT_ADJUSTMENT_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Signals,
                ENTITLEMENT_ADJUSTMENT_FACT_ID.to_string(),
                serde_json::to_string(&EntitlementAdjustmentPayload {
                    key: "credit_balance_minor".to_string(),
                    previous_balance_minor: self.seed.current_credit_balance_minor,
                    resulting_balance_minor: self.seed.current_credit_balance_minor
                        + self.seed.top_up_amount_minor,
                })
                .expect("entitlement adjustment payload should serialize"),
                ENTITLEMENT_PROVENANCE.to_string(),
            )
            .with_confidence(0.99),
        )
    }
}

fn payment_status_from_inputs(inputs: &RefillPrepaidAiCreditsInput) -> PaymentStatus {
    if let Some(confirmed) = inputs.payment_confirmed {
        return if confirmed {
            PaymentStatus::Confirmed
        } else {
            PaymentStatus::Pending
        };
    }

    match inputs
        .payment_status
        .as_deref()
        .unwrap_or("unknown")
        .to_ascii_lowercase()
        .as_str()
    {
        "confirmed" | "paid" | "settled" => PaymentStatus::Confirmed,
        "pending" | "authorized" | "processing" => PaymentStatus::Pending,
        "failed" | "declined" | "overdue" => PaymentStatus::Failed,
        _ => PaymentStatus::Unknown,
    }
}

fn payment_status_name(status: PaymentStatus) -> &'static str {
    match status {
        PaymentStatus::Confirmed => "confirmed",
        PaymentStatus::Pending => "pending",
        PaymentStatus::Failed => "failed",
        PaymentStatus::Unknown => "unknown",
    }
}

fn credit_grant_ready_from_result(
    result: &ConvergeResult,
) -> Result<CreditGrantReadyPayload, Status> {
    payload_from_result(result, ContextKey::Strategies, CREDIT_GRANT_READY_FACT_ID)
}

fn entitlement_adjustment_from_result(
    result: &ConvergeResult,
) -> Result<EntitlementAdjustmentPayload, Status> {
    payload_from_result(result, ContextKey::Signals, ENTITLEMENT_ADJUSTMENT_FACT_ID)
}

fn manual_review_from_result(
    result: &ConvergeResult,
) -> Result<Option<ManualReviewPayload>, Status> {
    result
        .context
        .get(ContextKey::Evaluations)
        .iter()
        .find(|fact| fact.id() == MANUAL_REVIEW_FACT_ID)
        .map(|fact| {
            serde_json::from_str(&fact.content()).map_err(|error| {
                Status::internal(format!("invalid manual review payload: {error}"))
            })
        })
        .transpose()
}

fn refill_related_to(organization_id: Uuid, subscription_id: Uuid) -> Vec<RecordRef> {
    vec![
        RecordRef {
            kind: RecordKind::Organization,
            id: organization_id,
        },
        RecordRef {
            kind: RecordKind::OrderSubscription,
            id: subscription_id,
        },
    ]
}

fn seed_context(subscription_id: Uuid) -> Result<Context, Status> {
    let mut context = Context::new();
    context
        .add_input(
            ContextKey::Seeds,
            "refill-prepaid-ai-credits:seed",
            subscription_id.to_string(),
        )
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(context)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use application_kernel::{
        Actor, BillingPeriod, CatalogItemUpsert, CatalogPlanKind, EntitlementTemplate,
        OrganizationLifecycle, OrganizationUpsert, SubscriptionActivate, SubscriptionCreate,
        SubscriptionStatus,
    };
    use application_storage::InMemoryKernelStore;
    use converge_core::StopReason;

    fn seeded_active_credit_subscription(
        store: &InMemoryKernelStore,
        actor: &Actor,
    ) -> (Uuid, Uuid) {
        store
            .write(|kernel| {
                let organization = kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Top-up Test".to_string(),
                        external_key: None,
                        website: None,
                        industry: None,
                        lifecycle: OrganizationLifecycle::Active,
                        owner_user_id: None,
                        tags: vec!["top-up".to_string()],
                    },
                    actor.clone(),
                )?;
                let catalog_item = kernel.upsert_catalog_item(
                    CatalogItemUpsert {
                        catalog_item_id: None,
                        sku: "prio-prepaid".to_string(),
                        name: "Prio Prepaid".to_string(),
                        description: Some("Prepaid credits".to_string()),
                        plan_kind: CatalogPlanKind::PrepaidCredits,
                        pricing: Some(application_kernel::PricingMetadata {
                            billing_period: BillingPeriod::OneTime,
                            list_price: Money {
                                currency_code: "USD".to_string(),
                                amount_minor: 1_000_00,
                            },
                            meter_name: Some("prepaid-credits".to_string()),
                        }),
                        entitlement_template: EntitlementTemplate {
                            feature_flags: vec![],
                            quotas: BTreeMap::new(),
                            credit_balance_minor: Some(0),
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
                            amount_minor: 1_000_00,
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
                Ok((organization.id, subscription.id))
            })
            .expect("seeded subscription")
    }

    #[tokio::test]
    async fn refill_prepaid_ai_credits_executes_end_to_end() {
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
        let (_organization_id, subscription_id) = seeded_active_credit_subscription(&store, &actor);

        let execution = execute(
            &store,
            &runtime_stores,
            RefillPrepaidAiCreditsInput {
                subscription_id,
                top_up_amount_minor: Some(250000),
                payment_reference: "pay_topup_123".to_string(),
                payment_confirmed: None,
                payment_status: Some("confirmed".to_string()),
                risk_signal: None,
                force_manual_review: None,
                manual_review_reason: None,
            },
            actor.clone(),
            true,
        )
        .await
        .expect("truth should execute");

        assert!(matches!(
            execution.result.stop_reason,
            StopReason::CriteriaMet { .. }
        ));
        let projection = execution.projection.expect("projection should persist");
        assert_eq!(projection.ledger_entries.len(), 1);
        assert_eq!(projection.entitlements.len(), 1);
        assert!(projection.workflow_cases.is_empty());
        assert!(matches!(
            projection.entitlements[0].value,
            EntitlementValue::Credits(250_000)
        ));
    }

    #[tokio::test]
    async fn refill_prepaid_ai_credits_blocks_when_payment_is_unconfirmed() {
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
        let (_organization_id, subscription_id) = seeded_active_credit_subscription(&store, &actor);

        let execution = execute(
            &store,
            &runtime_stores,
            RefillPrepaidAiCreditsInput {
                subscription_id,
                top_up_amount_minor: Some(250000),
                payment_reference: "pay_topup_blocked".to_string(),
                payment_confirmed: None,
                payment_status: Some("pending".to_string()),
                risk_signal: None,
                force_manual_review: None,
                manual_review_reason: None,
            },
            actor.clone(),
            true,
        )
        .await
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
                    converge_kernel::CriterionResult::Blocked { .. }
                ))
        );

        let projection = execution.projection.expect("projection should persist");
        assert_eq!(projection.ledger_entries.len(), 0);
        assert_eq!(projection.entitlements.len(), 0);
        assert_eq!(projection.workflow_cases.len(), 1);
        assert!(matches!(
            projection.workflow_cases[0].state,
            WorkflowState::AwaitingApproval
        ));
    }

    #[tokio::test]
    async fn refill_without_active_subscription_returns_error() {
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
        let subscription_id = Uuid::new_v4();

        let result = execute(
            &store,
            &runtime_stores,
            RefillPrepaidAiCreditsInput {
                subscription_id,
                top_up_amount_minor: Some(100000),
                payment_reference: "pay_no_sub".to_string(),
                payment_confirmed: None,
                payment_status: Some("confirmed".to_string()),
                risk_signal: None,
                force_manual_review: None,
                manual_review_reason: None,
            },
            actor,
            true,
        )
        .await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn refill_with_zero_amount_returns_error() {
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
        let (_organization_id, subscription_id) = seeded_active_credit_subscription(&store, &actor);

        let result = execute(
            &store,
            &runtime_stores,
            RefillPrepaidAiCreditsInput {
                subscription_id,
                top_up_amount_minor: Some(0),
                payment_reference: "pay_zero".to_string(),
                payment_confirmed: None,
                payment_status: Some("confirmed".to_string()),
                risk_signal: None,
                force_manual_review: None,
                manual_review_reason: None,
            },
            actor,
            true,
        )
        .await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn refill_without_persist_produces_no_side_effects() {
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
        let (_organization_id, subscription_id) = seeded_active_credit_subscription(&store, &actor);

        let ledger_count_before = store
            .read(|kernel| kernel.ledger_entries.len())
            .expect("read ledger count");
        let entitlement_balance_before: i64 = store
            .read(|kernel| {
                let entitlement = kernel
                    .entitlements
                    .values()
                    .find(|e| {
                        e.subscription_id == subscription_id && e.key == "credit_balance_minor"
                    })
                    .expect("entitlement should exist");
                match entitlement.value {
                    EntitlementValue::Credits(v) => v,
                    _ => panic!("expected credits entitlement"),
                }
            })
            .expect("read entitlement balance");

        let execution = execute(
            &store,
            &runtime_stores,
            RefillPrepaidAiCreditsInput {
                subscription_id,
                top_up_amount_minor: Some(250000),
                payment_reference: "pay_no_persist".to_string(),
                payment_confirmed: None,
                payment_status: Some("confirmed".to_string()),
                risk_signal: None,
                force_manual_review: None,
                manual_review_reason: None,
            },
            actor,
            false,
        )
        .await
        .expect("truth should execute");

        assert!(matches!(
            execution.result.stop_reason,
            StopReason::CriteriaMet { .. }
        ));
        assert!(execution.projection.is_none());

        let ledger_count_after = store
            .read(|kernel| kernel.ledger_entries.len())
            .expect("read ledger count");
        let entitlement_balance_after: i64 = store
            .read(|kernel| {
                let entitlement = kernel
                    .entitlements
                    .values()
                    .find(|e| {
                        e.subscription_id == subscription_id && e.key == "credit_balance_minor"
                    })
                    .expect("entitlement should exist");
                match entitlement.value {
                    EntitlementValue::Credits(v) => v,
                    _ => panic!("expected credits entitlement"),
                }
            })
            .expect("read entitlement balance");

        assert_eq!(ledger_count_before, ledger_count_after);
        assert_eq!(entitlement_balance_before, entitlement_balance_after);
    }

    #[tokio::test]
    async fn sequential_refills_accumulate_balance() {
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
        let (_organization_id, subscription_id) = seeded_active_credit_subscription(&store, &actor);

        let initial_balance: i64 = store
            .read(|kernel| {
                let entitlement = kernel
                    .entitlements
                    .values()
                    .find(|e| {
                        e.subscription_id == subscription_id && e.key == "credit_balance_minor"
                    })
                    .expect("entitlement should exist");
                match entitlement.value {
                    EntitlementValue::Credits(v) => v,
                    _ => panic!("expected credits entitlement"),
                }
            })
            .expect("read initial balance");

        let first_refill = execute(
            &store,
            &runtime_stores,
            RefillPrepaidAiCreditsInput {
                subscription_id,
                top_up_amount_minor: Some(100000),
                payment_reference: "pay_seq_first".to_string(),
                payment_confirmed: None,
                payment_status: Some("confirmed".to_string()),
                risk_signal: None,
                force_manual_review: None,
                manual_review_reason: None,
            },
            actor.clone(),
            true,
        )
        .await
        .expect("first refill should execute");

        assert!(matches!(
            first_refill.result.stop_reason,
            StopReason::CriteriaMet { .. }
        ));

        let second_refill = execute(
            &store,
            &runtime_stores,
            RefillPrepaidAiCreditsInput {
                subscription_id,
                top_up_amount_minor: Some(50000),
                payment_reference: "pay_seq_second".to_string(),
                payment_confirmed: None,
                payment_status: Some("confirmed".to_string()),
                risk_signal: None,
                force_manual_review: None,
                manual_review_reason: None,
            },
            actor,
            true,
        )
        .await
        .expect("second refill should execute");

        assert!(matches!(
            second_refill.result.stop_reason,
            StopReason::CriteriaMet { .. }
        ));

        let final_balance: i64 = store
            .read(|kernel| {
                let entitlement = kernel
                    .entitlements
                    .values()
                    .find(|e| {
                        e.subscription_id == subscription_id && e.key == "credit_balance_minor"
                    })
                    .expect("entitlement should exist");
                match entitlement.value {
                    EntitlementValue::Credits(v) => v,
                    _ => panic!("expected credits entitlement"),
                }
            })
            .expect("read final balance");

        assert_eq!(final_balance, initial_balance + 100_000 + 50_000);

        let ledger_count = store
            .read(|kernel| {
                kernel
                    .ledger_entries
                    .values()
                    .filter(|entry| entry.subscription_id == subscription_id)
                    .count()
            })
            .expect("read ledger count");

        assert_eq!(ledger_count, 3);
    }
}
