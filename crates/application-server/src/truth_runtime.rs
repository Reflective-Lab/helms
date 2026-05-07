mod activate_subscription;
mod common;
mod evaluate_acquisition_target;
mod generate_data_transformer;
mod match_renewal_context;
mod plan_outbound_campaign;
mod qualify_inbound_lead;
mod reconcile_model_usage_against_customer_ledger;
mod refill_prepaid_ai_credits;
mod schedule_strategic_meetings;
mod score_inbound_fit;
mod suspend_service_on_payment_failure;
mod upgrade_subscription_plan;

use std::collections::HashMap;
use std::sync::Mutex;

use application_kernel::{
    Actor as CrmActor, Document, Entitlement, Fact as CrmFact, LedgerEntry, Opportunity,
    OrderSubscription, Organization, Person, WorkflowCase,
};
use application_storage::{AppRuntimeStores, KernelStore, StorageError};
use converge_kernel::{
    ContextState as Context, ConvergeError, ConvergeResult, CriterionEvaluator, Engine,
    ExperienceEvent, ExperienceEventEnvelope, ExperienceEventObserver, TypesRootIntent,
    TypesRunHooks,
};
use tonic::Status;
use uuid::Uuid;

#[derive(Debug)]
pub struct TruthExecutionArtifacts {
    pub result: ConvergeResult,
    pub experience_events: Vec<ExperienceEvent>,
    pub projection: Option<TruthProjection>,
}

#[derive(Debug)]
pub struct TruthProjection {
    pub organization: Option<Organization>,
    pub person: Option<Person>,
    pub opportunity: Option<Opportunity>,
    pub subscription: Option<OrderSubscription>,
    pub entitlements: Vec<Entitlement>,
    pub ledger_entries: Vec<LedgerEntry>,
    pub documents: Vec<Document>,
    pub workflow_cases: Vec<WorkflowCase>,
    pub facts: Vec<CrmFact>,
    pub domain_event_kinds: Vec<&'static str>,
}

#[derive(Default)]
pub(super) struct RecordingObserver {
    events: Mutex<Vec<ExperienceEvent>>,
}

impl RecordingObserver {
    pub(super) fn snapshot(&self) -> Vec<ExperienceEvent> {
        self.events
            .lock()
            .expect("recording observer lock poisoned")
            .clone()
    }
}

impl ExperienceEventObserver for RecordingObserver {
    fn on_event(&self, event: &ExperienceEvent) {
        self.events
            .lock()
            .expect("recording observer lock poisoned")
            .push(event.clone());
    }
}

pub async fn execute_truth<S: KernelStore>(
    store: &S,
    runtime_stores: &AppRuntimeStores,
    truth_key: &str,
    inputs: HashMap<String, String>,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    match truth_key {
        "activate-subscription" => {
            let parsed = activate_subscription::ActivateSubscriptionInput::from_map(&inputs)?;
            activate_subscription::execute(store, runtime_stores, parsed, actor, persist_projection).await
        }
        "upgrade-subscription-plan" => {
            let parsed =
                upgrade_subscription_plan::UpgradeSubscriptionPlanInput::from_map(&inputs)?;
            upgrade_subscription_plan::execute(
                store,
                runtime_stores,
                parsed,
                actor,
                persist_projection,
            ).await
        }
        "suspend-service-on-payment-failure" => {
            let parsed =
                suspend_service_on_payment_failure::SuspendServiceOnPaymentFailureInput::from_map(
                    &inputs,
                )?;
            suspend_service_on_payment_failure::execute(
                store,
                runtime_stores,
                parsed,
                actor,
                persist_projection,
            ).await
        }
        "reconcile-model-usage-against-customer-ledger" => {
            let parsed = reconcile_model_usage_against_customer_ledger::ReconcileModelUsageAgainstCustomerLedgerInput::from_map(&inputs)?;
            reconcile_model_usage_against_customer_ledger::execute(
                store,
                runtime_stores,
                parsed,
                actor,
                persist_projection,
            ).await
        }
        "refill-prepaid-ai-credits" => {
            let parsed = refill_prepaid_ai_credits::RefillPrepaidAiCreditsInput::from_map(&inputs)?;
            refill_prepaid_ai_credits::execute(
                store,
                runtime_stores,
                parsed,
                actor,
                persist_projection,
            ).await
        }
        "qualify-inbound-lead" => {
            let parsed = qualify_inbound_lead::QualifyInboundLeadInput::from_map(&inputs)?;
            qualify_inbound_lead::execute(store, runtime_stores, parsed, actor, persist_projection).await
        }
        "score-inbound-fit" => {
            let parsed = score_inbound_fit::ScoreInboundFitInput::from_map(&inputs)?;
            score_inbound_fit::execute(store, runtime_stores, parsed, actor, persist_projection).await
        }
        "plan-outbound-campaign" => {
            let parsed = plan_outbound_campaign::PlanOutboundCampaignInput::from_map(&inputs)?;
            plan_outbound_campaign::execute(
                store,
                runtime_stores,
                parsed,
                actor,
                persist_projection,
            ).await
        }
        "match-renewal-context" => {
            let parsed = match_renewal_context::MatchRenewalContextInput::from_map(&inputs)?;
            match_renewal_context::execute(store, runtime_stores, parsed, actor, persist_projection).await
        }
        "schedule-strategic-meetings" => {
            let parsed =
                schedule_strategic_meetings::ScheduleStrategicMeetingsInput::from_map(&inputs)?;
            schedule_strategic_meetings::execute(
                store,
                runtime_stores,
                parsed,
                actor,
                persist_projection,
            ).await
        }
        "evaluate-acquisition-target" => {
            let parsed =
                evaluate_acquisition_target::EvaluateAcquisitionTargetInput::from_map(&inputs)?;
            evaluate_acquisition_target::execute(
                store,
                runtime_stores,
                parsed,
                actor,
                persist_projection,
            ).await
        }
        _ => Err(Status::unimplemented(format!(
            "truth execution is not implemented yet for {truth_key}"
        ))),
    }
}

pub(super) fn status_from_converge(error: ConvergeError) -> Status {
    match error {
        ConvergeError::BudgetExhausted { kind } => {
            Status::resource_exhausted(format!("converge budget exhausted: {kind}"))
        }
        ConvergeError::InvariantViolation { name, reason, .. } => {
            Status::failed_precondition(format!("converge invariant violated: {name}: {reason}"))
        }
        ConvergeError::AgentFailed { agent_id } => {
            Status::internal(format!("converge agent failed: {agent_id}"))
        }
        ConvergeError::Conflict { id, .. } => {
            Status::aborted(format!("converge fact conflict: {id}"))
        }
        ConvergeError::InvalidResume { reason } => {
            Status::failed_precondition(format!("converge invalid resume: {reason}"))
        }
        ConvergeError::InvalidAdmission { reason } => {
            Status::invalid_argument(format!("converge invalid admission: {reason}"))
        }
        ConvergeError::InvalidSnapshot { reason } => {
            Status::data_loss(format!("converge invalid context snapshot: {reason}"))
        }
    }
}

pub(super) fn status_from_storage(error: StorageError) -> Status {
    match error {
        StorageError::LockPoisoned => Status::internal("storage lock poisoned"),
        StorageError::Kernel(application_kernel::KernelError::Validation(message)) => {
            Status::invalid_argument(message)
        }
        StorageError::Kernel(application_kernel::KernelError::NotFound { kind, id }) => {
            Status::not_found(format!("{kind} not found: {id}"))
        }
        StorageError::Kernel(application_kernel::KernelError::Invariant(message)) => {
            Status::failed_precondition(message)
        }
        StorageError::Kernel(application_kernel::KernelError::Conflict(message)) => {
            Status::already_exists(message)
        }
        StorageError::ConnectionFailed { backend, message } => {
            Status::unavailable(format!("{backend} connection failed: {message}"))
        }
        StorageError::SerializationFailed { message } => Status::internal(message),
        StorageError::Timeout { operation } => Status::deadline_exceeded(operation),
        StorageError::RuntimeStore { message } => Status::internal(message),
    }
}

pub(super) fn domain_event_kind_name(event: &application_kernel::DomainEvent) -> &'static str {
    match event {
        application_kernel::DomainEvent::OrganizationUpserted { .. } => "organization-upserted",
        application_kernel::DomainEvent::PersonUpserted { .. } => "person-upserted",
        application_kernel::DomainEvent::RelationshipLinked { .. } => "relationship-linked",
        application_kernel::DomainEvent::OpportunityCreated { .. } => "opportunity-created",
        application_kernel::DomainEvent::OpportunityStageChanged { .. } => {
            "opportunity-stage-changed"
        }
        application_kernel::DomainEvent::ActivityAppended { .. } => "activity-appended",
        application_kernel::DomainEvent::NoteAppended { .. } => "note-appended",
        application_kernel::DomainEvent::DocumentAttached { .. } => "document-attached",
        application_kernel::DomainEvent::CommunicationRecorded { .. } => "communication-recorded",
        application_kernel::DomainEvent::WorkflowCaseCreated { .. } => "workflow-case-created",
        application_kernel::DomainEvent::WorkflowCaseStateChanged { .. } => {
            "workflow-case-state-changed"
        }
        application_kernel::DomainEvent::PermissionGranted { .. } => "permission-granted",
        application_kernel::DomainEvent::CatalogItemUpserted { .. } => "catalog-item-upserted",
        application_kernel::DomainEvent::OrderSubscriptionCreated { .. } => "subscription-created",
        application_kernel::DomainEvent::OrderSubscriptionStateChanged { .. } => {
            "subscription-state-changed"
        }
        application_kernel::DomainEvent::OrderSubscriptionPlanChanged { .. } => {
            "subscription-plan-changed"
        }
        application_kernel::DomainEvent::EntitlementsGranted { .. } => "entitlements-granted",
        application_kernel::DomainEvent::EntitlementsReplaced { .. } => "entitlements-replaced",
        application_kernel::DomainEvent::EntitlementAdjusted { .. } => "entitlement-adjusted",
        application_kernel::DomainEvent::LedgerEntryAppended { .. } => "ledger-entry-appended",
        application_kernel::DomainEvent::FactRecorded { .. } => "fact-recorded",
        application_kernel::DomainEvent::ObjectDefinitionUpserted { .. } => {
            "object-definition-upserted"
        }
        application_kernel::DomainEvent::ViewDefinitionUpserted { .. } => {
            "view-definition-upserted"
        }
        application_kernel::DomainEvent::AuditRecorded { .. } => "audit-recorded",
        application_kernel::DomainEvent::TimelineEntryRecorded { .. } => "timeline-entry-recorded",
    }
}

pub(super) struct RuntimeContext {
    pub scope_id: String,
}

pub(super) async fn run_engine_with_runtime(
    runtime_stores: &AppRuntimeStores,
    engine: &mut Engine,
    runtime_ctx: &RuntimeContext,
    seed_context: Context,
    intent: &TypesRootIntent,
    criterion_evaluator: std::sync::Arc<dyn CriterionEvaluator>,
) -> Result<(ConvergeResult, Vec<ExperienceEvent>), Status> {
    let observer = std::sync::Arc::new(RecordingObserver::default());
    let result = engine
        .run_with_types_intent_and_hooks(
            seed_context,
            intent,
            TypesRunHooks {
                criterion_evaluator: Some(criterion_evaluator),
                event_observer: Some(observer.clone()),
            },
        )
        .await
        .map_err(status_from_converge)?;

    runtime_stores
        .save_context(&runtime_ctx.scope_id, &result.context)
        .map_err(status_from_storage)?;

    let experience_events = observer.snapshot();
    let envelopes = experience_events
        .iter()
        .enumerate()
        .map(|(index, event)| {
            ExperienceEventEnvelope::new(
                format!("evt-{}-{:04}", Uuid::new_v4().simple(), index + 1),
                event.clone(),
            )
            .with_correlation(runtime_ctx.scope_id.clone())
        })
        .collect::<Vec<_>>();
    runtime_stores
        .append_experience_events(&envelopes)
        .map_err(status_from_storage)?;

    Ok((result, experience_events))
}
