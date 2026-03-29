mod activate_subscription;
mod common;
mod match_renewal_context;
mod plan_outbound_campaign;
mod qualify_inbound_lead;
mod refill_prepaid_ai_credits;
mod score_inbound_fit;
mod suspend_service_on_payment_failure;
mod upgrade_subscription_plan;

use std::collections::HashMap;
use std::sync::Mutex;

use converge_core::ExperienceEventObserver;
use converge_core::{ConvergeResult, ExperienceEvent};
use crm_kernel::{
    Actor as CrmActor, Document, Entitlement, Fact as CrmFact, LedgerEntry, Opportunity,
    OrderSubscription, Organization, Person, WorkflowCase,
};
use crm_storage::{KernelStore, StorageError};
use tonic::Status;

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

pub fn execute_truth<S: KernelStore>(
    store: &S,
    truth_key: &str,
    inputs: HashMap<String, String>,
    actor: CrmActor,
    persist_projection: bool,
) -> Result<TruthExecutionArtifacts, Status> {
    match truth_key {
        "activate-subscription" => {
            activate_subscription::execute(store, inputs, actor, persist_projection)
        }
        "upgrade-subscription-plan" => {
            upgrade_subscription_plan::execute(store, inputs, actor, persist_projection)
        }
        "suspend-service-on-payment-failure" => {
            suspend_service_on_payment_failure::execute(store, inputs, actor, persist_projection)
        }
        "refill-prepaid-ai-credits" => {
            refill_prepaid_ai_credits::execute(store, inputs, actor, persist_projection)
        }
        "qualify-inbound-lead" => {
            qualify_inbound_lead::execute(store, inputs, actor, persist_projection)
        }
        "score-inbound-fit" => score_inbound_fit::execute(store, inputs, actor, persist_projection),
        "plan-outbound-campaign" => {
            plan_outbound_campaign::execute(store, inputs, actor, persist_projection)
        }
        "match-renewal-context" => {
            match_renewal_context::execute(store, inputs, actor, persist_projection)
        }
        _ => Err(Status::unimplemented(format!(
            "truth execution is not implemented yet for {truth_key}"
        ))),
    }
}

pub(super) fn status_from_converge(error: converge_core::ConvergeError) -> Status {
    match error {
        converge_core::ConvergeError::BudgetExhausted { kind } => {
            Status::resource_exhausted(format!("converge budget exhausted: {kind}"))
        }
        converge_core::ConvergeError::InvariantViolation { name, reason, .. } => {
            Status::failed_precondition(format!("converge invariant violated: {name}: {reason}"))
        }
        converge_core::ConvergeError::AgentFailed { agent_id } => {
            Status::internal(format!("converge agent failed: {agent_id}"))
        }
        converge_core::ConvergeError::Conflict { id, .. } => {
            Status::aborted(format!("converge fact conflict: {id}"))
        }
    }
}

pub(super) fn status_from_storage(error: StorageError) -> Status {
    match error {
        StorageError::LockPoisoned => Status::internal("storage lock poisoned"),
        StorageError::Kernel(error) => Status::failed_precondition(error.to_string()),
    }
}

pub(super) fn domain_event_kind_name(event: &crm_kernel::DomainEvent) -> &'static str {
    match event {
        crm_kernel::DomainEvent::OrganizationUpserted { .. } => "organization-upserted",
        crm_kernel::DomainEvent::PersonUpserted { .. } => "person-upserted",
        crm_kernel::DomainEvent::RelationshipLinked { .. } => "relationship-linked",
        crm_kernel::DomainEvent::OpportunityCreated { .. } => "opportunity-created",
        crm_kernel::DomainEvent::OpportunityStageChanged { .. } => "opportunity-stage-changed",
        crm_kernel::DomainEvent::ActivityAppended { .. } => "activity-appended",
        crm_kernel::DomainEvent::NoteAppended { .. } => "note-appended",
        crm_kernel::DomainEvent::DocumentAttached { .. } => "document-attached",
        crm_kernel::DomainEvent::CommunicationRecorded { .. } => "communication-recorded",
        crm_kernel::DomainEvent::WorkflowCaseCreated { .. } => "workflow-case-created",
        crm_kernel::DomainEvent::WorkflowCaseStateChanged { .. } => "workflow-case-state-changed",
        crm_kernel::DomainEvent::PermissionGranted { .. } => "permission-granted",
        crm_kernel::DomainEvent::CatalogItemUpserted { .. } => "catalog-item-upserted",
        crm_kernel::DomainEvent::OrderSubscriptionCreated { .. } => "subscription-created",
        crm_kernel::DomainEvent::OrderSubscriptionStateChanged { .. } => {
            "subscription-state-changed"
        }
        crm_kernel::DomainEvent::OrderSubscriptionPlanChanged { .. } => "subscription-plan-changed",
        crm_kernel::DomainEvent::EntitlementsGranted { .. } => "entitlements-granted",
        crm_kernel::DomainEvent::EntitlementsReplaced { .. } => "entitlements-replaced",
        crm_kernel::DomainEvent::EntitlementAdjusted { .. } => "entitlement-adjusted",
        crm_kernel::DomainEvent::LedgerEntryAppended { .. } => "ledger-entry-appended",
        crm_kernel::DomainEvent::FactRecorded { .. } => "fact-recorded",
        crm_kernel::DomainEvent::ObjectDefinitionUpserted { .. } => "object-definition-upserted",
        crm_kernel::DomainEvent::ViewDefinitionUpserted { .. } => "view-definition-upserted",
        crm_kernel::DomainEvent::AuditRecorded { .. } => "audit-recorded",
        crm_kernel::DomainEvent::TimelineEntryRecorded { .. } => "timeline-entry-recorded",
    }
}
