use std::collections::HashMap;

use application_kernel::{
    Actor as CrmActor, CreditGrantApply, EntitlementValue, FactRecord, OrderSubscription,
    Organization, RecordKind, RecordRef, SubscriptionStatus, SubscriptionSuspend,
    WorkflowCaseAdvance, WorkflowCaseCreate, WorkflowPriority, WorkflowState,
};
use application_storage::{KernelStore, StoreWriteResult};
use converge_domain::packs::OverdueDetectorAgent;
use converge_kernel::{ContextState as Context, ConvergeResult, Engine};
use converge_pack::{AgentEffect, Context as ContextView, ContextKey, ProposedFact, Suggestor};
use serde::{Deserialize, Serialize};
use tonic::Status;
use truth_catalog::{
    SuspendServiceOnPaymentFailureEvaluator,
    admission::{admit_truth_intent, default_helms_capabilities, select_formation_for_intent},
    converge_binding_for_truth,
};
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
const WORK_PACK_ID: &str = "prio-work-pack";
const SUSPENSION_READY_FACT_ID: &str = "subscription:suspension-ready";
const SUSPENSION_DEFERRED_FACT_ID: &str = "subscription:suspension-deferred";
const ENTITLEMENT_IMPACT_FACT_ID: &str = "subscription:entitlement-impact";
const RECOVERY_PATH_FACT_ID: &str = "subscription:recovery-path";
const MANUAL_REVIEW_FACT_ID: &str = "subscription:suspension-manual-review-required";
const SUSPENSION_PROVENANCE: &str = "prio.suspend-service-on-payment-failure.policy";
const ENTITLEMENT_PROVENANCE: &str = "prio.suspend-service-on-payment-failure.entitlements";
const RECOVERY_PROVENANCE: &str = "prio.suspend-service-on-payment-failure.recovery";
const REVIEW_PROVENANCE: &str = "prio.suspend-service-on-payment-failure.approvals";
const DEFAULT_GRACE_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaymentFailureStatus {
    Failed,
    Overdue,
}

#[derive(Debug, Clone)]
struct SuspensionSeed {
    subscription: OrderSubscription,
    organization: Organization,
    payment_status: PaymentFailureStatus,
    days_overdue: i64,
    grace_days: i64,
    manual_review_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SuspensionReadyPayload {
    subscription_id: Uuid,
    organization_id: Uuid,
    payment_status: String,
    days_overdue: i64,
    grace_days: i64,
    reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SuspensionDeferredPayload {
    subscription_id: Uuid,
    organization_id: Uuid,
    payment_status: String,
    days_overdue: i64,
    grace_days: i64,
    reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntitlementImpactPayload {
    service_access_state: String,
    workspace_access_enabled: bool,
    recovery_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecoveryPathPayload {
    summary: String,
    next_action: String,
    workflow_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManualReviewPayload {
    reason: String,
}

#[derive(Clone)]
struct SuspensionDecisionAgent {
    seed: SuspensionSeed,
}

#[derive(Clone)]
struct EntitlementImpactAgent;

#[derive(Clone)]
struct RecoveryPathAgent {
    seed: SuspensionSeed,
}

#[derive(Debug, Clone)]
pub struct SuspendServiceOnPaymentFailureInput {
    pub subscription_id: Uuid,
    pub payment_status: String,
    pub days_overdue: Option<i64>,
    pub grace_days: Option<i64>,
    pub force_manual_review: Option<bool>,
    pub manual_review_reason: Option<String>,
    pub strategic_account: Option<bool>,
}

impl SuspendServiceOnPaymentFailureInput {
    pub fn from_map(inputs: &HashMap<String, String>) -> Result<Self, Status> {
        Ok(Self {
            subscription_id: required_uuid(inputs, "subscription_id")?,
            payment_status: required_input(inputs, "payment_status")?.to_string(),
            days_overdue: optional_i64(inputs, "days_overdue"),
            grace_days: optional_i64(inputs, "grace_days"),
            force_manual_review: optional_bool(inputs, "force_manual_review"),
            manual_review_reason: optional_input(inputs, "manual_review_reason"),
            strategic_account: optional_bool(inputs, "strategic_account"),
        })
    }
}

pub(super) async fn execute<S: KernelStore>(
    store: &S,
    runtime_stores: &application_storage::AppRuntimeStores,
    inputs: SuspendServiceOnPaymentFailureInput,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    let binding = converge_binding_for_truth("suspend-service-on-payment-failure")
        .ok_or_else(|| Status::not_found("truth not found: suspend-service-on-payment-failure"))?;

    let seed = load_suspension_seed(store, &inputs)?;

    let mut engine = Engine::new();
    engine.register_suggestor_in_pack(REVENUE_PACK_ID, OverdueDetectorAgent);
    engine.register_suggestor_in_pack(
        REVENUE_PACK_ID,
        SuspensionDecisionAgent { seed: seed.clone() },
    );
    engine.register_suggestor_in_pack(REVENUE_PACK_ID, EntitlementImpactAgent);
    engine.register_suggestor_in_pack(WORK_PACK_ID, RecoveryPathAgent { seed: seed.clone() });

    let mut seed_ctx = seed_context(&seed)?;
    let intent = admit_truth_intent(
        "suspend-service-on-payment-failure",
        &actor.actor_id,
        "truth:suspend-service-on-payment-failure",
        &mut seed_ctx,
    )
    .map_err(|e| Status::internal(format!("admit intent failed: {e}")))?;
    let selection = select_formation_for_intent(&intent, &default_helms_capabilities())
        .map_err(|e| Status::internal(format!("formation selection failed: {e}")))?;
    tracing::info!(
        truth = "suspend-service-on-payment-failure",
        primary = %selection.primary_template_id,
        alternates = ?selection.alternate_template_ids,
        "formation selected"
    );

    let (result, experience_events) = super::run_engine_with_runtime(
        runtime_stores,
        &mut engine,
        &super::RuntimeContext {
            scope_id: inputs.subscription_id.to_string(),
        },
        seed_ctx,
        &binding.intent,
        std::sync::Arc::new(SuspendServiceOnPaymentFailureEvaluator),
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

fn load_suspension_seed<S: KernelStore>(
    store: &S,
    inputs: &SuspendServiceOnPaymentFailureInput,
) -> Result<SuspensionSeed, Status> {
    let payment_status = payment_failure_status(&inputs.payment_status)?;
    let days_overdue = inputs.days_overdue.unwrap_or(0).max(0);
    let grace_days = inputs.grace_days.unwrap_or(DEFAULT_GRACE_DAYS).max(0);

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
                    "service suspension requires an active subscription".to_string(),
                ));
            }
            let organization = kernel
                .organizations
                .get(&subscription.organization_id)
                .cloned()
                .ok_or_else(|| {
                    Status::not_found(format!(
                        "organization not found: {}",
                        subscription.organization_id
                    ))
                })?;

            let is_strategic = inputs.strategic_account.unwrap_or_else(|| {
                organization.tags.iter().any(|tag| {
                    let tag = tag.trim().to_ascii_lowercase();
                    tag == "strategic" || tag == "vip"
                })
            });
            let should_suspend =
                payment_status == PaymentFailureStatus::Failed || days_overdue > grace_days;
            let policy_review_reason = if inputs.force_manual_review.unwrap_or(false) {
                Some(
                    inputs
                        .manual_review_reason
                        .clone()
                        .unwrap_or_else(|| "manual review requested by operator".to_string()),
                )
            } else if should_suspend && is_strategic {
                Some(inputs.manual_review_reason.clone().unwrap_or_else(|| {
                    "strategic account suspension requires approval".to_string()
                }))
            } else {
                inputs.manual_review_reason.clone()
            };

            Ok(SuspensionSeed {
                subscription,
                organization,
                payment_status,
                days_overdue,
                grace_days,
                manual_review_reason: policy_review_reason,
            })
        })
        .map_err(status_from_storage)?
}

fn project<S: KernelStore>(
    store: &S,
    inputs: &SuspendServiceOnPaymentFailureInput,
    result: &ConvergeResult,
    actor: CrmActor,
) -> Result<TruthProjection, Status> {
    let manual_review = manual_review_from_result(result)?;

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
                        title: format!("Manual review: suspend service for {}", subscription.id),
                        priority: WorkflowPriority::Critical,
                        owner_user_id: None,
                        related_to: suspension_related_to(
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
                            "service suspension awaiting manual review: {}",
                            review.reason
                        ),
                        confidence_bps: 10_000,
                        related_to: suspension_related_to(
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

    let recovery_path = recovery_path_from_result(result)?;

    if let Ok(ready) = suspension_ready_from_result(result) {
        let StoreWriteResult { value, events } = store
            .write_with_events(|kernel| {
                let suspension = kernel.suspend_subscription(
                    SubscriptionSuspend {
                        subscription_id: ready.subscription_id,
                        occurred_at: chrono::Utc::now(),
                        reason: Some(ready.reason.clone()),
                    },
                    actor.clone(),
                )?;
                let workflow_case = kernel.create_workflow_case(
                    WorkflowCaseCreate {
                        title: format!(
                            "Payment recovery required for {}",
                            suspension.subscription.id
                        ),
                        priority: WorkflowPriority::High,
                        owner_user_id: None,
                        related_to: suspension_related_to(
                            suspension.subscription.organization_id,
                            suspension.subscription.id,
                        ),
                    },
                    actor.clone(),
                )?;
                let workflow_case = kernel.advance_workflow_case(
                    WorkflowCaseAdvance {
                        workflow_case_id: workflow_case.id,
                        state: WorkflowState::WaitingExternal,
                    },
                    actor.clone(),
                )?;
                let suspension_fact = kernel.record_fact(
                    FactRecord {
                        statement: format!(
                            "service suspended after {} payment state",
                            ready.payment_status
                        ),
                        confidence_bps: 10_000,
                        related_to: suspension_related_to(
                            suspension.subscription.organization_id,
                            suspension.subscription.id,
                        ),
                        source_note_id: None,
                    },
                    actor.clone(),
                )?;
                let recovery_fact = kernel.record_fact(
                    FactRecord {
                        statement: recovery_path.summary.clone(),
                        confidence_bps: 10_000,
                        related_to: suspension_related_to(
                            suspension.subscription.organization_id,
                            suspension.subscription.id,
                        ),
                        source_note_id: None,
                    },
                    actor.clone(),
                )?;
                Ok((
                    suspension,
                    workflow_case,
                    vec![suspension_fact, recovery_fact],
                ))
            })
            .map_err(status_from_storage)?;
        let (suspension, workflow_case, facts) = value;
        return Ok(TruthProjection {
            organization: None,
            person: None,
            opportunity: None,
            subscription: Some(suspension.subscription),
            entitlements: suspension.entitlements,
            ledger_entries: Vec::new(),
            documents: Vec::new(),
            workflow_cases: vec![workflow_case],
            facts,
            domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
        });
    }

    let deferred = suspension_deferred_from_result(result)?;
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
                    title: format!("Grace period: payment recovery for {}", subscription.id),
                    priority: WorkflowPriority::High,
                    owner_user_id: None,
                    related_to: suspension_related_to(
                        subscription.organization_id,
                        subscription.id,
                    ),
                },
                actor.clone(),
            )?;
            let workflow_case = kernel.advance_workflow_case(
                WorkflowCaseAdvance {
                    workflow_case_id: workflow_case.id,
                    state: WorkflowState::WaitingExternal,
                },
                actor.clone(),
            )?;
            let policy_fact = kernel.record_fact(
                FactRecord {
                    statement: deferred.reason.clone(),
                    confidence_bps: 10_000,
                    related_to: suspension_related_to(
                        subscription.organization_id,
                        subscription.id,
                    ),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            let recovery_fact = kernel.record_fact(
                FactRecord {
                    statement: recovery_path.summary.clone(),
                    confidence_bps: 10_000,
                    related_to: suspension_related_to(
                        subscription.organization_id,
                        subscription.id,
                    ),
                    source_note_id: None,
                },
                actor.clone(),
            )?;
            Ok((
                subscription,
                workflow_case,
                vec![policy_fact, recovery_fact],
            ))
        })
        .map_err(status_from_storage)?;
    let (subscription, workflow_case, facts) = value;
    Ok(TruthProjection {
        organization: None,
        person: None,
        opportunity: None,
        subscription: Some(subscription),
        entitlements: Vec::new(),
        ledger_entries: Vec::new(),
        documents: Vec::new(),
        workflow_cases: vec![workflow_case],
        facts,
        domain_event_kinds: events.iter().map(domain_event_kind_name).collect(),
    })
}

#[async_trait::async_trait]
impl Suggestor for SuspensionDecisionAgent {
    fn name(&self) -> &str {
        "SubscriptionSuspensionDecisionAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Proposals]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        ctx.get(ContextKey::Proposals)
            .iter()
            .any(|fact| fact.id().starts_with("invoice:overdue_action:"))
            && !has_fact_id(ctx, ContextKey::Strategies, SUSPENSION_READY_FACT_ID)
            && !has_fact_id(ctx, ContextKey::Strategies, SUSPENSION_DEFERRED_FACT_ID)
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

        if should_suspend(&self.seed) {
            return AgentEffect::with_proposal(
                ProposedFact::new(
                    ContextKey::Strategies,
                    SUSPENSION_READY_FACT_ID.to_string(),
                    serde_json::to_string(&SuspensionReadyPayload {
                        subscription_id: self.seed.subscription.id,
                        organization_id: self.seed.organization.id,
                        payment_status: payment_failure_status_name(self.seed.payment_status)
                            .to_string(),
                        days_overdue: self.seed.days_overdue,
                        grace_days: self.seed.grace_days,
                        reason: suspension_reason(&self.seed),
                    })
                    .expect("suspension ready payload should serialize"),
                    SUSPENSION_PROVENANCE.to_string(),
                )
                .with_confidence(0.99),
            );
        }

        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Strategies,
                SUSPENSION_DEFERRED_FACT_ID.to_string(),
                serde_json::to_string(&SuspensionDeferredPayload {
                    subscription_id: self.seed.subscription.id,
                    organization_id: self.seed.organization.id,
                    payment_status: payment_failure_status_name(self.seed.payment_status)
                        .to_string(),
                    days_overdue: self.seed.days_overdue,
                    grace_days: self.seed.grace_days,
                    reason: format!(
                        "service remains active during grace period ({} of {} days overdue)",
                        self.seed.days_overdue, self.seed.grace_days
                    ),
                })
                .expect("suspension deferred payload should serialize"),
                SUSPENSION_PROVENANCE.to_string(),
            )
            .with_confidence(0.98),
        )
    }
}

#[async_trait::async_trait]
impl Suggestor for EntitlementImpactAgent {
    fn name(&self) -> &str {
        "SubscriptionEntitlementImpactAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Strategies]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        (has_fact_id(ctx, ContextKey::Strategies, SUSPENSION_READY_FACT_ID)
            || has_fact_id(ctx, ContextKey::Strategies, SUSPENSION_DEFERRED_FACT_ID))
            && !has_fact_id(ctx, ContextKey::Signals, ENTITLEMENT_IMPACT_FACT_ID)
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        let (service_access_state, workspace_access_enabled) =
            if has_fact_id(ctx, ContextKey::Strategies, SUSPENSION_READY_FACT_ID) {
                ("suspended", false)
            } else {
                ("grace", true)
            };
        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Signals,
                ENTITLEMENT_IMPACT_FACT_ID.to_string(),
                serde_json::to_string(&EntitlementImpactPayload {
                    service_access_state: service_access_state.to_string(),
                    workspace_access_enabled,
                    recovery_allowed: true,
                })
                .expect("entitlement impact payload should serialize"),
                ENTITLEMENT_PROVENANCE.to_string(),
            )
            .with_confidence(0.99),
        )
    }
}

#[async_trait::async_trait]
impl Suggestor for RecoveryPathAgent {
    fn name(&self) -> &str {
        "SubscriptionRecoveryPathAgent"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Strategies]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        (has_fact_id(ctx, ContextKey::Strategies, SUSPENSION_READY_FACT_ID)
            || has_fact_id(ctx, ContextKey::Strategies, SUSPENSION_DEFERRED_FACT_ID))
            && !has_fact_id(ctx, ContextKey::Strategies, RECOVERY_PATH_FACT_ID)
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        let (summary, next_action) =
            if has_fact_id(ctx, ContextKey::Strategies, SUSPENSION_READY_FACT_ID) {
                (
                    format!(
                        "customer must clear the {} payment failure before access is reinstated",
                        payment_failure_status_name(self.seed.payment_status)
                    ),
                    "collect payment and route to reinstatement review".to_string(),
                )
            } else {
                (
                    format!(
                        "customer remains in grace while payment recovery is pursued for {} days",
                        self.seed.grace_days
                    ),
                    "notify customer and reassess when grace expires".to_string(),
                )
            };

        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Strategies,
                RECOVERY_PATH_FACT_ID.to_string(),
                serde_json::to_string(&RecoveryPathPayload {
                    summary,
                    next_action,
                    workflow_state: WorkflowState::WaitingExternal.as_ref().to_string(),
                })
                .expect("recovery path payload should serialize"),
                RECOVERY_PROVENANCE.to_string(),
            )
            .with_confidence(0.99),
        )
    }
}

fn payment_failure_status(input: &str) -> Result<PaymentFailureStatus, Status> {
    match input.trim().to_ascii_lowercase().as_str() {
        "failed" | "declined" | "payment_failed" => Ok(PaymentFailureStatus::Failed),
        "overdue" | "past_due" => Ok(PaymentFailureStatus::Overdue),
        value => Err(Status::invalid_argument(format!(
            "unsupported payment_status for suspension: {value}"
        ))),
    }
}

fn payment_failure_status_name(status: PaymentFailureStatus) -> &'static str {
    match status {
        PaymentFailureStatus::Failed => "failed",
        PaymentFailureStatus::Overdue => "overdue",
    }
}

fn should_suspend(seed: &SuspensionSeed) -> bool {
    matches!(seed.payment_status, PaymentFailureStatus::Failed)
        || seed.days_overdue > seed.grace_days
}

fn suspension_reason(seed: &SuspensionSeed) -> String {
    match seed.payment_status {
        PaymentFailureStatus::Failed => {
            "service suspended immediately after payment failure".to_string()
        }
        PaymentFailureStatus::Overdue => format!(
            "service suspended after {} overdue days exceeded grace of {}",
            seed.days_overdue, seed.grace_days
        ),
    }
}

fn suspension_ready_from_result(result: &ConvergeResult) -> Result<SuspensionReadyPayload, Status> {
    payload_from_result(result, ContextKey::Strategies, SUSPENSION_READY_FACT_ID)
}

fn suspension_deferred_from_result(
    result: &ConvergeResult,
) -> Result<SuspensionDeferredPayload, Status> {
    payload_from_result(result, ContextKey::Strategies, SUSPENSION_DEFERRED_FACT_ID)
}

fn recovery_path_from_result(result: &ConvergeResult) -> Result<RecoveryPathPayload, Status> {
    payload_from_result(result, ContextKey::Strategies, RECOVERY_PATH_FACT_ID)
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
                Status::internal(format!("invalid suspension manual review payload: {error}"))
            })
        })
        .transpose()
}

fn suspension_related_to(organization_id: Uuid, subscription_id: Uuid) -> Vec<RecordRef> {
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

fn seed_context(seed: &SuspensionSeed) -> Result<Context, Status> {
    let mut context = Context::new();
    context
        .add_input(
            ContextKey::Seeds,
            "suspend-service-on-payment-failure:seed",
            seed.subscription.id.to_string(),
        )
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    context
        .add_input(
            ContextKey::Proposals,
            format!("invoice:subscription-payment:{}", seed.subscription.id),
            serde_json::json!({
                "type": "invoice",
                "state": "open",
                "overdue": true,
                "subscription_id": seed.subscription.id,
                "organization_id": seed.organization.id,
                "payment_status": payment_failure_status_name(seed.payment_status),
                "days_overdue": seed.days_overdue,
                "amount": seed.subscription.value.amount_minor,
                "currency": seed.subscription.value.currency_code,
            })
            .to_string(),
        )
        .map_err(|error| Status::failed_precondition(error.to_string()))?;
    Ok(context)
}

trait WorkflowStateExt {
    fn as_ref(self) -> &'static str;
}

impl WorkflowStateExt for WorkflowState {
    fn as_ref(self) -> &'static str {
        match self {
            WorkflowState::Open => "open",
            WorkflowState::AwaitingApproval => "awaiting-approval",
            WorkflowState::WaitingExternal => "waiting-external",
            WorkflowState::Blocked => "blocked",
            WorkflowState::Done => "done",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use application_kernel::{
        Actor, CatalogItemUpsert, EntitlementTemplate, OrganizationLifecycle, OrganizationUpsert,
        SubscriptionActivate, SubscriptionCreate,
    };
    use application_storage::InMemoryKernelStore;
    use converge_core::StopReason;
    use converge_kernel::CriterionResult;

    fn seeded_active_subscription_for_suspension(
        store: &InMemoryKernelStore,
        actor: &Actor,
        tags: Vec<String>,
    ) -> Uuid {
        store
            .write(|kernel| {
                let organization = kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Suspension Truth Test".to_string(),
                        external_key: None,
                        website: None,
                        industry: None,
                        lifecycle: OrganizationLifecycle::Active,
                        owner_user_id: None,
                        tags,
                    },
                    actor.clone(),
                )?;
                let catalog_item = kernel.upsert_catalog_item(
                    CatalogItemUpsert {
                        catalog_item_id: None,
                        sku: "prio-revenue".to_string(),
                        name: "Prio Revenue".to_string(),
                        description: Some("Revenue plan".to_string()),
                        plan_kind: application_kernel::CatalogPlanKind::Subscription,
                        pricing: Some(application_kernel::PricingMetadata {
                            billing_period: application_kernel::BillingPeriod::Monthly,
                            list_price: application_kernel::Money {
                                currency_code: "USD".to_string(),
                                amount_minor: 2_000_00,
                            },
                            meter_name: Some("workspace-seat".to_string()),
                        }),
                        entitlement_template: EntitlementTemplate {
                            feature_flags: vec![
                                "workspace_access".to_string(),
                                "priority_support".to_string(),
                            ],
                            quotas: BTreeMap::from([("seats".to_string(), 5)]),
                            credit_balance_minor: Some(100_000),
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
                        value: application_kernel::Money {
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
                Ok(subscription.id)
            })
            .expect("seed active subscription")
    }

    #[tokio::test]
    async fn suspend_service_on_payment_failure_executes_end_to_end() {
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
        let subscription_id = seeded_active_subscription_for_suspension(&store, &actor, Vec::new());

        let execution = execute(
            &store,
            &runtime_stores,
            SuspendServiceOnPaymentFailureInput {
                subscription_id,
                payment_status: "failed".to_string(),
                days_overdue: None,
                grace_days: None,
                force_manual_review: None,
                manual_review_reason: None,
                strategic_account: None,
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
        assert_eq!(
            projection
                .subscription
                .as_ref()
                .map(|subscription| subscription.status),
            Some(SubscriptionStatus::Suspended)
        );
        assert!(matches!(
            projection
                .entitlements
                .iter()
                .find(|entitlement| entitlement.key == "workspace_access")
                .expect("workspace access entitlement")
                .value,
            EntitlementValue::FeatureFlag(false)
        ));
        assert_eq!(projection.workflow_cases.len(), 1);
        assert_eq!(
            projection.workflow_cases[0].state,
            WorkflowState::WaitingExternal
        );

        let grant_error = store
            .write(|kernel| {
                kernel.apply_credit_grant(
                    CreditGrantApply {
                        subscription_id,
                        amount: application_kernel::Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 10_000,
                        },
                        payment_reference: "pay_after_suspend".to_string(),
                        reason: Some("should fail".to_string()),
                    },
                    actor,
                )
            })
            .expect_err("suspended subscriptions should block credit grants");
        assert!(
            grant_error
                .to_string()
                .contains("credit grants require an active subscription")
        );
    }

    #[tokio::test]
    async fn suspend_service_on_payment_failure_respects_grace_period() {
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
        let subscription_id = seeded_active_subscription_for_suspension(&store, &actor, Vec::new());

        let execution = execute(
            &store,
            &runtime_stores,
            SuspendServiceOnPaymentFailureInput {
                subscription_id,
                payment_status: "overdue".to_string(),
                days_overdue: Some(3),
                grace_days: Some(7),
                force_manual_review: None,
                manual_review_reason: None,
                strategic_account: None,
            },
            actor,
            true,
        )
        .await
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
            Some(SubscriptionStatus::Active)
        );
        assert!(projection.entitlements.is_empty());
        assert_eq!(projection.workflow_cases.len(), 1);
        assert_eq!(
            projection.workflow_cases[0].state,
            WorkflowState::WaitingExternal
        );
    }

    #[tokio::test]
    async fn suspend_service_on_payment_failure_blocks_for_strategic_accounts() {
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
        let subscription_id = seeded_active_subscription_for_suspension(
            &store,
            &actor,
            vec!["strategic".to_string()],
        );

        let execution = execute(
            &store,
            &runtime_stores,
            SuspendServiceOnPaymentFailureInput {
                subscription_id,
                payment_status: "failed".to_string(),
                days_overdue: None,
                grace_days: None,
                force_manual_review: None,
                manual_review_reason: None,
                strategic_account: None,
            },
            actor,
            true,
        )
        .await
        .expect("truth should execute");

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
        assert_eq!(
            projection
                .subscription
                .as_ref()
                .map(|subscription| subscription.status),
            Some(SubscriptionStatus::Active)
        );
        assert!(projection.entitlements.is_empty());
        assert_eq!(projection.workflow_cases.len(), 1);
        assert_eq!(
            projection.workflow_cases[0].state,
            WorkflowState::AwaitingApproval
        );
    }
}
