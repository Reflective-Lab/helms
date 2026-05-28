use std::collections::HashMap;

use application_kernel::{
    Actor as CrmActor, EntitlementValue, FactRecord, LedgerEntryKind, OrderSubscription,
    RecordKind, RecordRef, WorkflowCaseAdvance, WorkflowCaseCreate, WorkflowPriority,
    WorkflowState,
};
use application_storage::{KernelStore, StoreWriteResult};
use converge_domain::packs::ReconciliationMatcherAgent;
use converge_kernel::{ContextState as Context, ConvergeResult, Engine};
use converge_pack::{AgentEffect, Context as ContextView, ContextKey, Suggestor};
use serde::{Deserialize, Serialize};
use tonic::Status;
use truth_catalog::{
    ReconcileModelUsageAgainstCustomerLedgerEvaluator,
    admission::{admit_truth_intent, default_helms_capabilities, select_formation_for_intent},
    converge_binding_for_truth,
};
use uuid::Uuid;

use super::{
    TruthExecutionArtifacts, TruthProjection,
    common::{has_fact_id, optional_i64, optional_input, payload_from_result, required_uuid},
    domain_event_kind_name, status_from_storage,
};

const REVENUE_PACK_ID: &str = "prio-revenue-pack";
const WORK_PACK_ID: &str = "prio-work-pack";
const DEFAULT_THRESHOLD_MINOR: i64 = 10_000;

const USAGE_SUMMARY_FACT_ID: &str = "reconciliation:usage-summary";
const PROVIDER_SUMMARY_FACT_ID: &str = "reconciliation:provider-summary";
const LEDGER_SUMMARY_FACT_ID: &str = "reconciliation:ledger-summary";
const ENTITLEMENT_SUMMARY_FACT_ID: &str = "reconciliation:entitlement-summary";
const CLEAN_FACT_ID: &str = "reconciliation:clean";
const EXCEPTION_FACT_ID: &str = "reconciliation:exception";
const ROUTE_FACT_ID: &str = "reconciliation:route";
const MANUAL_REVIEW_FACT_ID: &str = "reconciliation:manual-review-required";

const USAGE_PROVENANCE: &str = "prio.reconcile-ledger.usage";
const PROVIDER_PROVENANCE: &str = "prio.reconcile-ledger.provider";
const LEDGER_PROVENANCE: &str = "prio.reconcile-ledger.ledger";
const ENTITLEMENT_PROVENANCE: &str = "prio.reconcile-ledger.entitlements";
const ASSESSMENT_PROVENANCE: &str = "prio.reconcile-ledger.assessment";
const ROUTING_PROVENANCE: &str = "prio.reconcile-ledger.routing";

#[derive(Debug, Clone)]
struct ReconciliationSeed {
    subscription: OrderSubscription,
    usage: UsageSnapshot,
    provider: ProviderBillingSnapshot,
    ledger: LedgerSnapshot,
    entitlements: EntitlementSnapshot,
    threshold_minor: i64,
}

#[derive(Debug, Clone)]
struct UsageSnapshot {
    burn_minor: i64,
    meter_name: String,
    period_label: String,
}

#[derive(Debug, Clone)]
struct ProviderBillingSnapshot {
    settled_minor: i64,
    provider_reference: String,
    provider_name: String,
    status: String,
}

#[derive(Debug, Clone)]
struct LedgerSnapshot {
    opening_balance_minor: i64,
    credit_grants_minor: i64,
    debits_minor: i64,
    adjustments_minor: i64,
}

#[derive(Debug, Clone)]
struct EntitlementSnapshot {
    credit_balance_minor: i64,
    service_access_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageSummaryPayload {
    subscription_id: Uuid,
    burn_minor: i64,
    meter_name: String,
    period_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderSummaryPayload {
    subscription_id: Uuid,
    settled_minor: i64,
    provider_reference: String,
    provider_name: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LedgerSummaryPayload {
    subscription_id: Uuid,
    opening_balance_minor: i64,
    credit_grants_minor: i64,
    debits_minor: i64,
    adjustments_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntitlementSummaryPayload {
    subscription_id: Uuid,
    current_credit_balance_minor: i64,
    service_access_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReconciliationAssessmentPayload {
    subscription_id: Uuid,
    expected_credit_balance_minor: i64,
    current_credit_balance_minor: i64,
    usage_ledger_delta_minor: i64,
    provider_ledger_delta_minor: i64,
    entitlement_delta_minor: i64,
    matched_provider_to_ledger: bool,
    summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReconciliationRoutePayload {
    severity: String,
    workflow_state: String,
    summary: String,
}

trait ReconciliationSourceAdapter<S: KernelStore> {
    fn load_usage(
        &self,
        inputs: &ReconcileModelUsageAgainstCustomerLedgerInput,
    ) -> Result<UsageSnapshot, Status>;

    fn load_provider_billing(
        &self,
        inputs: &ReconcileModelUsageAgainstCustomerLedgerInput,
    ) -> Result<ProviderBillingSnapshot, Status>;

    fn load_ledger(
        &self,
        store: &S,
        subscription: &OrderSubscription,
    ) -> Result<LedgerSnapshot, Status>;

    fn load_entitlements(
        &self,
        store: &S,
        subscription: &OrderSubscription,
    ) -> Result<EntitlementSnapshot, Status>;
}

struct KernelInputReconciliationAdapter;

impl<S: KernelStore> ReconciliationSourceAdapter<S> for KernelInputReconciliationAdapter {
    fn load_usage(
        &self,
        inputs: &ReconcileModelUsageAgainstCustomerLedgerInput,
    ) -> Result<UsageSnapshot, Status> {
        let burn_minor = inputs
            .usage_burn_minor
            .filter(|value| *value >= 0)
            .ok_or_else(|| {
                Status::invalid_argument("usage_burn_minor must be a non-negative integer")
            })?;
        Ok(UsageSnapshot {
            burn_minor,
            meter_name: inputs
                .meter_name
                .clone()
                .unwrap_or_else(|| "model-token-usage".to_string()),
            period_label: inputs
                .period_label
                .clone()
                .unwrap_or_else(|| "current-period".to_string()),
        })
    }

    fn load_provider_billing(
        &self,
        inputs: &ReconcileModelUsageAgainstCustomerLedgerInput,
    ) -> Result<ProviderBillingSnapshot, Status> {
        let settled_minor = inputs
            .provider_settled_minor
            .filter(|value| *value >= 0)
            .ok_or_else(|| {
                Status::invalid_argument("provider_settled_minor must be a non-negative integer")
            })?;
        Ok(ProviderBillingSnapshot {
            settled_minor,
            provider_reference: inputs
                .provider_reference
                .clone()
                .unwrap_or_else(|| "provider-reconciliation".to_string()),
            provider_name: inputs
                .provider_name
                .clone()
                .unwrap_or_else(|| "stripe".to_string()),
            status: inputs
                .provider_status
                .clone()
                .unwrap_or_else(|| "settled".to_string()),
        })
    }

    fn load_ledger(
        &self,
        store: &S,
        subscription: &OrderSubscription,
    ) -> Result<LedgerSnapshot, Status> {
        store
            .read(|kernel| {
                let entries = kernel
                    .ledger_entries
                    .values()
                    .filter(|entry| entry.subscription_id == subscription.id)
                    .cloned()
                    .collect::<Vec<_>>();
                let opening_balance_minor = entries
                    .iter()
                    .filter(|entry| entry.kind == LedgerEntryKind::OpeningBalance)
                    .map(|entry| entry.amount.amount_minor)
                    .sum();
                let credit_grants_minor = entries
                    .iter()
                    .filter(|entry| entry.kind == LedgerEntryKind::CreditGrant)
                    .map(|entry| entry.amount.amount_minor)
                    .sum();
                let debits_minor = entries
                    .iter()
                    .filter(|entry| entry.kind == LedgerEntryKind::Debit)
                    .map(|entry| entry.amount.amount_minor)
                    .sum();
                let adjustments_minor = entries
                    .iter()
                    .filter(|entry| entry.kind == LedgerEntryKind::Adjustment)
                    .map(|entry| entry.amount.amount_minor)
                    .sum();
                Ok(LedgerSnapshot {
                    opening_balance_minor,
                    credit_grants_minor,
                    debits_minor,
                    adjustments_minor,
                })
            })
            .map_err(status_from_storage)?
    }

    fn load_entitlements(
        &self,
        store: &S,
        subscription: &OrderSubscription,
    ) -> Result<EntitlementSnapshot, Status> {
        store
            .read(|kernel| {
                let balance = kernel
                    .entitlements
                    .values()
                    .find(|entitlement| {
                        entitlement.subscription_id == subscription.id
                            && entitlement.key == "credit_balance_minor"
                    })
                    .ok_or_else(|| {
                        Status::failed_precondition(
                            "reconciliation requires a credit balance entitlement".to_string(),
                        )
                    })?;
                let credit_balance_minor = match balance.value {
                    EntitlementValue::Credits(value) => value,
                    _ => {
                        return Err(Status::failed_precondition(
                            "credit balance entitlement must use credits semantics".to_string(),
                        ));
                    }
                };
                let service_access_state = kernel
                    .entitlements
                    .values()
                    .find(|entitlement| {
                        entitlement.subscription_id == subscription.id
                            && entitlement.key == "service_access_state"
                    })
                    .and_then(|entitlement| match &entitlement.value {
                        EntitlementValue::Text(value) => Some(value.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "active".to_string());
                Ok(EntitlementSnapshot {
                    credit_balance_minor,
                    service_access_state,
                })
            })
            .map_err(status_from_storage)?
    }
}

#[derive(Clone)]
struct UsageSummaryAgent {
    seed: ReconciliationSeed,
}

#[derive(Clone)]
struct ProviderBillingSummaryAgent {
    seed: ReconciliationSeed,
}

#[derive(Clone)]
struct LedgerSummaryAgent {
    seed: ReconciliationSeed,
}

#[derive(Clone)]
struct EntitlementSummaryAgent {
    seed: ReconciliationSeed,
}

#[derive(Clone)]
struct ReconciliationAssessmentAgent {
    seed: ReconciliationSeed,
}

#[derive(Clone)]
struct ExceptionRoutingAgent;

#[derive(Debug, Clone)]
pub struct ReconcileModelUsageAgainstCustomerLedgerInput {
    pub subscription_id: Uuid,
    pub threshold_minor: Option<i64>,
    pub usage_burn_minor: Option<i64>,
    pub meter_name: Option<String>,
    pub period_label: Option<String>,
    pub provider_settled_minor: Option<i64>,
    pub provider_reference: Option<String>,
    pub provider_name: Option<String>,
    pub provider_status: Option<String>,
}

impl ReconcileModelUsageAgainstCustomerLedgerInput {
    pub fn from_map(inputs: &HashMap<String, String>) -> Result<Self, Status> {
        Ok(Self {
            subscription_id: required_uuid(inputs, "subscription_id")?,
            threshold_minor: optional_i64(inputs, "threshold_minor"),
            usage_burn_minor: optional_i64(inputs, "usage_burn_minor"),
            meter_name: optional_input(inputs, "meter_name"),
            period_label: optional_input(inputs, "period_label"),
            provider_settled_minor: optional_i64(inputs, "provider_settled_minor"),
            provider_reference: optional_input(inputs, "provider_reference"),
            provider_name: optional_input(inputs, "provider_name"),
            provider_status: optional_input(inputs, "provider_status"),
        })
    }
}

pub(super) async fn execute<S: KernelStore>(
    store: &S,
    runtime_stores: &application_storage::AppRuntimeStores,
    inputs: ReconcileModelUsageAgainstCustomerLedgerInput,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    let binding = converge_binding_for_truth("reconcile-model-usage-against-customer-ledger")
        .ok_or_else(|| {
            Status::not_found("truth not found: reconcile-model-usage-against-customer-ledger")
        })?;

    let seed = load_seed(store, &inputs, &KernelInputReconciliationAdapter)?;

    let mut engine = Engine::new();
    engine.register_suggestor_in_pack(REVENUE_PACK_ID, UsageSummaryAgent { seed: seed.clone() });
    engine.register_suggestor_in_pack(
        REVENUE_PACK_ID,
        ProviderBillingSummaryAgent { seed: seed.clone() },
    );
    engine.register_suggestor_in_pack(REVENUE_PACK_ID, LedgerSummaryAgent { seed: seed.clone() });
    engine.register_suggestor_in_pack(
        REVENUE_PACK_ID,
        EntitlementSummaryAgent { seed: seed.clone() },
    );
    engine.register_suggestor_in_pack(REVENUE_PACK_ID, ReconciliationMatcherAgent);
    engine.register_suggestor_in_pack(
        REVENUE_PACK_ID,
        ReconciliationAssessmentAgent { seed: seed.clone() },
    );
    engine.register_suggestor_in_pack(WORK_PACK_ID, ExceptionRoutingAgent);

    let mut seed_ctx = seed_context(seed.subscription.id)?;
    let intent = admit_truth_intent(
        "reconcile-model-usage-against-customer-ledger",
        &actor.actor_id,
        "truth:reconcile-model-usage-against-customer-ledger",
        &mut seed_ctx,
    )
    .map_err(|e| Status::internal(format!("admit intent failed: {e}")))?;
    let selection = select_formation_for_intent(&intent, &default_helms_capabilities())
        .map_err(|e| Status::internal(format!("formation selection failed: {e}")))?;
    tracing::info!(
        truth = "reconcile-model-usage-against-customer-ledger",
        primary = %selection.primary_template_id,
        alternates = ?selection.alternate_template_ids,
        "formation selected"
    );

    let runtime_ctx = super::RuntimeContext {
        scope_id: inputs.subscription_id.to_string(),
    };
    let (result, experience_events) = super::run_engine_with_runtime(
        runtime_stores,
        &mut engine,
        &runtime_ctx,
        seed_ctx,
        &binding.intent,
        std::sync::Arc::new(ReconcileModelUsageAgainstCustomerLedgerEvaluator),
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
        runtime_scope_id: runtime_ctx.scope_id,
    })
}

fn load_seed<S: KernelStore, A: ReconciliationSourceAdapter<S>>(
    store: &S,
    inputs: &ReconcileModelUsageAgainstCustomerLedgerInput,
    adapter: &A,
) -> Result<ReconciliationSeed, Status> {
    let subscription_id = inputs.subscription_id;
    let threshold_minor = inputs
        .threshold_minor
        .unwrap_or(DEFAULT_THRESHOLD_MINOR)
        .max(0);

    let subscription = store
        .read(|kernel| -> Result<OrderSubscription, Status> {
            let subscription = kernel
                .orders
                .get(&subscription_id)
                .cloned()
                .ok_or_else(|| {
                    Status::not_found(format!("subscription not found: {subscription_id}"))
                })?;
            Ok(subscription)
        })
        .map_err(status_from_storage)??;

    Ok(ReconciliationSeed {
        usage: adapter.load_usage(inputs)?,
        provider: adapter.load_provider_billing(inputs)?,
        ledger: adapter.load_ledger(store, &subscription)?,
        entitlements: adapter.load_entitlements(store, &subscription)?,
        subscription,
        threshold_minor,
    })
}

fn project<S: KernelStore>(
    store: &S,
    inputs: &ReconcileModelUsageAgainstCustomerLedgerInput,
    result: &ConvergeResult,
    actor: CrmActor,
) -> Result<TruthProjection, Status> {
    let manual_review = manual_review_from_result(result)?;
    let clean = clean_from_result(result).ok();
    let exception = exception_from_result(result).ok();
    let route = route_from_result(result).ok();

    if let Some(review) = manual_review {
        let subscription_id = inputs.subscription_id;
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
                        title: format!("Manual review: reconcile {}", subscription.id),
                        priority: WorkflowPriority::High,
                        owner_user_id: None,
                        related_to: reconcile_related_to(
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
                        statement: review.summary.clone(),
                        confidence_bps: 10_000,
                        related_to: reconcile_related_to(
                            subscription.organization_id,
                            subscription.id,
                        ),
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

    if let Some(clean) = clean {
        let subscription_id = inputs.subscription_id;
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
                let fact = kernel.record_fact(
                    FactRecord {
                        statement: clean.summary.clone(),
                        confidence_bps: 10_000,
                        related_to: reconcile_related_to(
                            subscription.organization_id,
                            subscription.id,
                        ),
                        source_note_id: None,
                    },
                    actor.clone(),
                )?;
                Ok(fact)
            })
            .map_err(status_from_storage)?;
        return Ok(TruthProjection {
            organization: None,
            person: None,
            opportunity: None,
            subscription: None,
            entitlements: Vec::new(),
            ledger_entries: Vec::new(),
            documents: Vec::new(),
            workflow_cases: Vec::new(),
            facts: vec![value],
            domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
        });
    }

    let exception = exception.ok_or_else(|| {
        Status::failed_precondition(
            "reconciliation finished without a clean or exception assessment".to_string(),
        )
    })?;
    let route = route.ok_or_else(|| {
        Status::failed_precondition(
            "reconciliation exception did not produce a routing fact".to_string(),
        )
    })?;
    let subscription_id = inputs.subscription_id;
    let StoreWriteResult { value, events } = store
        .write_with_events(|kernel| {
            let subscription = kernel
                .orders
                .get(&subscription_id)
                .cloned()
                .ok_or_else(|| application_kernel::KernelError::NotFound {
                    kind: "subscription",
                    id: subscription_id.to_string(),
                })?;
            let workflow_case = kernel.create_workflow_case(
                WorkflowCaseCreate {
                    title: format!("Investigate reconciliation drift for {}", subscription.id),
                    priority: WorkflowPriority::Medium,
                    owner_user_id: None,
                    related_to: reconcile_related_to(subscription.organization_id, subscription.id),
                },
                actor.clone(),
            )?;
            let workflow_case = kernel.advance_workflow_case(
                WorkflowCaseAdvance {
                    workflow_case_id: workflow_case.id,
                    state: WorkflowState::Blocked,
                },
                actor.clone(),
            )?;
            let exception_fact = kernel.record_fact(
                FactRecord {
                    statement: exception.summary.clone(),
                    confidence_bps: 10_000,
                    related_to: reconcile_related_to(subscription.organization_id, subscription.id),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            let route_fact = kernel.record_fact(
                FactRecord {
                    statement: route.summary.clone(),
                    confidence_bps: 10_000,
                    related_to: reconcile_related_to(subscription.organization_id, subscription.id),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            Ok((workflow_case, vec![exception_fact, route_fact]))
        })
        .map_err(status_from_storage)?;
    let (workflow_case, facts) = value;
    Ok(TruthProjection {
        organization: None,
        person: None,
        opportunity: None,
        subscription: None,
        entitlements: Vec::new(),
        ledger_entries: Vec::new(),
        documents: Vec::new(),
        workflow_cases: vec![workflow_case],
        facts,
        domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
    })
}

#[async_trait::async_trait]
impl Suggestor for UsageSummaryAgent {
    fn name(&self) -> &str {
        "UsageSummaryAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.has(ContextKey::Seeds) && !has_fact_id(ctx, ContextKey::Signals, USAGE_SUMMARY_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        AgentEffect::with_proposals(vec![
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Signals,
                USAGE_SUMMARY_FACT_ID.to_string(),
                serde_json::to_string(&UsageSummaryPayload {
                    subscription_id: self.seed.subscription.id,
                    burn_minor: self.seed.usage.burn_minor,
                    meter_name: self.seed.usage.meter_name.clone(),
                    period_label: self.seed.usage.period_label.clone(),
                })
                .expect("usage summary should serialize"),
                USAGE_PROVENANCE.to_string(),
            )
            .with_confidence(0.99),
        ])
    }
}

#[async_trait::async_trait]
impl Suggestor for ProviderBillingSummaryAgent {
    fn name(&self) -> &str {
        "ProviderBillingSummaryAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.has(ContextKey::Seeds)
            && !has_fact_id(ctx, ContextKey::Signals, PROVIDER_SUMMARY_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        AgentEffect::with_proposals(vec![
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Signals,
                PROVIDER_SUMMARY_FACT_ID.to_string(),
                serde_json::to_string(&ProviderSummaryPayload {
                    subscription_id: self.seed.subscription.id,
                    settled_minor: self.seed.provider.settled_minor,
                    provider_reference: self.seed.provider.provider_reference.clone(),
                    provider_name: self.seed.provider.provider_name.clone(),
                    status: self.seed.provider.status.clone(),
                })
                .expect("provider summary should serialize"),
                PROVIDER_PROVENANCE.to_string(),
            )
            .with_confidence(0.99),
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Signals,
                format!(
                    "reconciliation:bank_txn:{}",
                    self.seed.provider.provider_reference
                ),
                serde_json::json!({
                    "type": "bank_txn",
                    "subscription_id": self.seed.subscription.id,
                    "amount_minor": self.seed.provider.settled_minor,
                    "provider_reference": self.seed.provider.provider_reference,
                })
                .to_string(),
                PROVIDER_PROVENANCE.to_string(),
            )
            .with_confidence(0.95),
        ])
    }
}

#[async_trait::async_trait]
impl Suggestor for LedgerSummaryAgent {
    fn name(&self) -> &str {
        "LedgerSummaryAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.has(ContextKey::Seeds)
            && !has_fact_id(ctx, ContextKey::Proposals, LEDGER_SUMMARY_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        AgentEffect::with_proposals(vec![
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Proposals,
                LEDGER_SUMMARY_FACT_ID.to_string(),
                serde_json::to_string(&LedgerSummaryPayload {
                    subscription_id: self.seed.subscription.id,
                    opening_balance_minor: self.seed.ledger.opening_balance_minor,
                    credit_grants_minor: self.seed.ledger.credit_grants_minor,
                    debits_minor: self.seed.ledger.debits_minor,
                    adjustments_minor: self.seed.ledger.adjustments_minor,
                })
                .expect("ledger summary should serialize"),
                LEDGER_PROVENANCE.to_string(),
            )
            .with_confidence(0.99),
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Proposals,
                format!("invoice:reconciliation:{}", self.seed.subscription.id),
                serde_json::json!({
                    "type": "invoice",
                    "state": "open",
                    "subscription_id": self.seed.subscription.id,
                    "amount_minor": self.seed.ledger.credit_grants_minor,
                    "currency": self.seed.subscription.value.currency_code,
                })
                .to_string(),
                LEDGER_PROVENANCE.to_string(),
            )
            .with_confidence(0.95),
        ])
    }
}

#[async_trait::async_trait]
impl Suggestor for EntitlementSummaryAgent {
    fn name(&self) -> &str {
        "EntitlementSummaryAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Seeds]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.has(ContextKey::Seeds)
            && !has_fact_id(ctx, ContextKey::Signals, ENTITLEMENT_SUMMARY_FACT_ID)
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        AgentEffect::with_proposals(vec![
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Signals,
                ENTITLEMENT_SUMMARY_FACT_ID.to_string(),
                serde_json::to_string(&EntitlementSummaryPayload {
                    subscription_id: self.seed.subscription.id,
                    current_credit_balance_minor: self.seed.entitlements.credit_balance_minor,
                    service_access_state: self.seed.entitlements.service_access_state.clone(),
                })
                .expect("entitlement summary should serialize"),
                ENTITLEMENT_PROVENANCE.to_string(),
            )
            .with_confidence(0.99),
        ])
    }
}

#[async_trait::async_trait]
impl Suggestor for ReconciliationAssessmentAgent {
    fn name(&self) -> &str {
        "ReconciliationAssessmentAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Signals, ContextKey::Proposals]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        let matched_provider_to_ledger = ctx
            .get(ContextKey::Proposals)
            .iter()
            .any(|fact| fact.id().starts_with("ledger:") && fact.id().contains("bank_txn"));
        has_fact_id(ctx, ContextKey::Signals, USAGE_SUMMARY_FACT_ID)
            && has_fact_id(ctx, ContextKey::Signals, PROVIDER_SUMMARY_FACT_ID)
            && has_fact_id(ctx, ContextKey::Signals, ENTITLEMENT_SUMMARY_FACT_ID)
            && has_fact_id(ctx, ContextKey::Proposals, LEDGER_SUMMARY_FACT_ID)
            && (matched_provider_to_ledger || self.seed.provider.settled_minor == 0)
            && !has_fact_id(ctx, ContextKey::Evaluations, CLEAN_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Evaluations, EXCEPTION_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Evaluations, MANUAL_REVIEW_FACT_ID)
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        let matched_provider_to_ledger = ctx
            .get(ContextKey::Proposals)
            .iter()
            .any(|fact| fact.id().starts_with("ledger:") && fact.id().contains("bank_txn"));

        let expected_credit_balance_minor = self.seed.ledger.opening_balance_minor
            + self.seed.ledger.credit_grants_minor
            + self.seed.ledger.adjustments_minor
            - self.seed.ledger.debits_minor;
        let usage_ledger_delta_minor = self.seed.usage.burn_minor - self.seed.ledger.debits_minor;
        let provider_ledger_delta_minor =
            self.seed.provider.settled_minor - self.seed.ledger.credit_grants_minor;
        let entitlement_delta_minor =
            self.seed.entitlements.credit_balance_minor - expected_credit_balance_minor;

        let assessment = ReconciliationAssessmentPayload {
            subscription_id: self.seed.subscription.id,
            expected_credit_balance_minor,
            current_credit_balance_minor: self.seed.entitlements.credit_balance_minor,
            usage_ledger_delta_minor,
            provider_ledger_delta_minor,
            entitlement_delta_minor,
            matched_provider_to_ledger,
            summary: format!(
                "usage/ledger delta {}, provider/ledger delta {}, entitlement delta {}",
                usage_ledger_delta_minor, provider_ledger_delta_minor, entitlement_delta_minor
            ),
        };

        let max_delta_minor = usage_ledger_delta_minor
            .abs()
            .max(provider_ledger_delta_minor.abs())
            .max(entitlement_delta_minor.abs());

        if matched_provider_to_ledger && max_delta_minor == 0 {
            return AgentEffect::with_proposals(vec![
                crate::truth_runtime::common::proposed_text_fact(
                    ContextKey::Evaluations,
                    CLEAN_FACT_ID.to_string(),
                    serde_json::to_string(&assessment)
                        .expect("clean reconciliation payload should serialize"),
                    ASSESSMENT_PROVENANCE.to_string(),
                )
                .with_confidence(0.99),
            ]);
        }

        let target_id = if max_delta_minor > self.seed.threshold_minor {
            MANUAL_REVIEW_FACT_ID
        } else {
            EXCEPTION_FACT_ID
        };
        AgentEffect::with_proposals(vec![
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Evaluations,
                target_id.to_string(),
                serde_json::to_string(&assessment)
                    .expect("reconciliation exception payload should serialize"),
                ASSESSMENT_PROVENANCE.to_string(),
            )
            .with_confidence(0.99),
        ])
    }
}

#[async_trait::async_trait]
impl Suggestor for ExceptionRoutingAgent {
    fn name(&self) -> &str {
        "ReconciliationExceptionRoutingAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Evaluations]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        has_fact_id(ctx, ContextKey::Evaluations, EXCEPTION_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Strategies, ROUTE_FACT_ID)
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        let exception = ctx
            .get(ContextKey::Evaluations)
            .iter()
            .find(|fact| fact.id() == EXCEPTION_FACT_ID)
            .expect("exception fact should exist before routing");
        let assessment: ReconciliationAssessmentPayload =
            serde_json::from_str(exception.text().unwrap_or_default())
                .expect("exception payload should deserialize");
        AgentEffect::with_proposals(vec![
            crate::truth_runtime::common::proposed_text_fact(
                ContextKey::Strategies,
                ROUTE_FACT_ID.to_string(),
                serde_json::to_string(&ReconciliationRoutePayload {
                    severity: "warning".to_string(),
                    workflow_state: "blocked".to_string(),
                    summary: format!(
                        "reconciliation drift routed for investigation: {}",
                        assessment.summary
                    ),
                })
                .expect("route payload should serialize"),
                ROUTING_PROVENANCE.to_string(),
            )
            .with_confidence(0.98),
        ])
    }
}

fn clean_from_result(result: &ConvergeResult) -> Result<ReconciliationAssessmentPayload, Status> {
    payload_from_result(result, ContextKey::Evaluations, CLEAN_FACT_ID)
}

fn exception_from_result(
    result: &ConvergeResult,
) -> Result<ReconciliationAssessmentPayload, Status> {
    payload_from_result(result, ContextKey::Evaluations, EXCEPTION_FACT_ID)
}

fn route_from_result(result: &ConvergeResult) -> Result<ReconciliationRoutePayload, Status> {
    payload_from_result(result, ContextKey::Strategies, ROUTE_FACT_ID)
}

fn manual_review_from_result(
    result: &ConvergeResult,
) -> Result<Option<ReconciliationAssessmentPayload>, Status> {
    result
        .context
        .get(ContextKey::Evaluations)
        .iter()
        .find(|fact| fact.id() == MANUAL_REVIEW_FACT_ID)
        .map(|fact| {
            serde_json::from_str(fact.text().unwrap_or_default()).map_err(|error| {
                Status::internal(format!(
                    "invalid reconciliation manual review payload: {error}"
                ))
            })
        })
        .transpose()
}

fn reconcile_related_to(organization_id: Uuid, subscription_id: Uuid) -> Vec<RecordRef> {
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
            "reconcile-model-usage-against-customer-ledger:seed",
            subscription_id.to_string(),
        )
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(context)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use application_kernel::{
        Actor, CatalogItemUpsert, CreditGrantApply, EntitlementTemplate, LedgerEntry, Money,
        OrganizationLifecycle, OrganizationUpsert, SubscriptionActivate, SubscriptionCreate,
        SubscriptionStatus,
    };
    use application_storage::InMemoryKernelStore;
    use chrono::Utc;
    use converge_core::StopReason;
    use converge_kernel::CriterionResult;

    use super::*;

    fn seed_reconciliation_subscription(store: &InMemoryKernelStore, actor: &Actor) -> Uuid {
        store
            .write(|kernel| {
                let organization = kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Reconciliation Truth Test".to_string(),
                        external_key: None,
                        website: None,
                        industry: None,
                        lifecycle: OrganizationLifecycle::Active,
                        owner_user_id: None,
                        tags: vec![],
                    },
                    actor.clone(),
                )?;
                let catalog_item = kernel.upsert_catalog_item(
                    CatalogItemUpsert {
                        catalog_item_id: None,
                        sku: "prio-metered".to_string(),
                        name: "Prio Metered".to_string(),
                        description: Some("Metered credits".to_string()),
                        plan_kind: application_kernel::CatalogPlanKind::Subscription,
                        pricing: Some(application_kernel::PricingMetadata {
                            billing_period: application_kernel::BillingPeriod::Monthly,
                            list_price: Money {
                                currency_code: "USD".to_string(),
                                amount_minor: 2_000_00,
                            },
                            meter_name: Some("token-meter".to_string()),
                        }),
                        entitlement_template: EntitlementTemplate {
                            feature_flags: vec!["workspace_access".to_string()],
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
                    CreditGrantApply {
                        subscription_id: subscription.id,
                        amount: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 100_000,
                        },
                        payment_reference: "pay_reconcile_seed".to_string(),
                        reason: Some("seed grant".to_string()),
                    },
                    actor.clone(),
                )?;
                let debit = LedgerEntry {
                    id: Uuid::new_v4(),
                    organization_id: organization.id,
                    subscription_id: subscription.id,
                    kind: LedgerEntryKind::Debit,
                    amount: Money {
                        currency_code: "USD".to_string(),
                        amount_minor: 40_000,
                    },
                    description: "Usage burn".to_string(),
                    external_reference: None,
                    created_at: Utc::now(),
                };
                kernel.ledger_entries.insert(debit.id, debit);
                if let Some(entitlement) = kernel.entitlements.values_mut().find(|entitlement| {
                    entitlement.subscription_id == subscription.id
                        && entitlement.key == "credit_balance_minor"
                }) {
                    entitlement.value = EntitlementValue::Credits(60_000);
                }
                Ok(subscription.id)
            })
            .expect("seed reconciliation subscription")
    }

    #[tokio::test]
    async fn reconcile_model_usage_against_customer_ledger_executes_cleanly() {
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
        let subscription_id = seed_reconciliation_subscription(&store, &actor);

        let execution = execute(
            &store,
            &runtime_stores,
            ReconcileModelUsageAgainstCustomerLedgerInput {
                subscription_id,
                threshold_minor: None,
                usage_burn_minor: Some(40000),
                meter_name: None,
                period_label: None,
                provider_settled_minor: Some(100000),
                provider_reference: None,
                provider_name: None,
                provider_status: None,
            },
            actor,
            true,
        )
        .await
        .expect("reconciliation should execute");

        assert!(matches!(
            execution.result.stop_reason,
            StopReason::CriteriaMet { .. }
        ));
        let projection = execution.projection.expect("projection should persist");
        assert!(projection.workflow_cases.is_empty());
        assert_eq!(projection.facts.len(), 1);
        assert!(
            projection.facts[0]
                .statement
                .contains("usage/ledger delta 0, provider/ledger delta 0, entitlement delta 0")
        );
    }

    #[tokio::test]
    async fn reconcile_model_usage_against_customer_ledger_routes_exceptions() {
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
        let subscription_id = seed_reconciliation_subscription(&store, &actor);

        let execution = execute(
            &store,
            &runtime_stores,
            ReconcileModelUsageAgainstCustomerLedgerInput {
                subscription_id,
                threshold_minor: Some(10000),
                usage_burn_minor: Some(40500),
                meter_name: None,
                period_label: None,
                provider_settled_minor: Some(100000),
                provider_reference: None,
                provider_name: None,
                provider_status: None,
            },
            actor,
            true,
        )
        .await
        .expect("reconciliation should execute");

        assert!(matches!(
            execution.result.stop_reason,
            StopReason::Converged
        ));
        let projection = execution.projection.expect("projection should persist");
        assert_eq!(projection.workflow_cases.len(), 1);
        assert_eq!(projection.workflow_cases[0].state, WorkflowState::Blocked);
        assert_eq!(projection.facts.len(), 2);
    }

    #[tokio::test]
    async fn reconcile_model_usage_against_customer_ledger_blocks_for_large_drift() {
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
        let subscription_id = seed_reconciliation_subscription(&store, &actor);

        let execution = execute(
            &store,
            &runtime_stores,
            ReconcileModelUsageAgainstCustomerLedgerInput {
                subscription_id,
                threshold_minor: Some(1000),
                usage_burn_minor: Some(70000),
                meter_name: None,
                period_label: None,
                provider_settled_minor: Some(100000),
                provider_reference: None,
                provider_name: None,
                provider_status: None,
            },
            actor,
            true,
        )
        .await
        .expect("reconciliation should execute");

        assert!(matches!(
            execution.result.stop_reason,
            StopReason::HumanInterventionRequired { .. }
        ));
        assert!(
            execution
                .result
                .criteria_outcomes
                .iter()
                .any(|outcome| matches!(outcome.result, CriterionResult::Blocked { .. }))
        );
        let projection = execution.projection.expect("projection should persist");
        assert_eq!(projection.workflow_cases.len(), 1);
        assert_eq!(
            projection.workflow_cases[0].state,
            WorkflowState::AwaitingApproval
        );
        assert_eq!(projection.facts.len(), 1);
    }
}
