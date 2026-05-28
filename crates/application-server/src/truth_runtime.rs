mod common;
// generate_data_transformer is a generic convergence-codegen experiment (EXP-002).
// TODO(Phase 9): evaluate whether to promote to a proper crate or delete.
#[cfg(test)]
mod generate_data_transformer;

use std::collections::HashMap;
use std::sync::Mutex;

use application_kernel::{
    Actor as CrmActor, Document, Entitlement, Fact as CrmFact, LedgerEntry, Opportunity,
    OrderSubscription, Organization, Person, WorkflowCase,
};
use application_storage::{AppRuntimeStores, KernelStore, StorageError};
use converge_core::FactId;
use converge_kernel::{
    ContextState as Context, ConvergeError, ConvergeResult, Criterion, CriterionEvaluator,
    CriterionResult, Engine, EventQuery, ExperienceEvent, ExperienceEventEnvelope,
    ExperienceEventObserver, ExperienceRecord, OverrideTarget, TypesRootIntent, TypesRunHooks,
    UserExperienceEvent,
};
use tonic::Status;
use uuid::Uuid;

#[derive(Debug)]
pub struct TruthExecutionArtifacts {
    pub result: ConvergeResult,
    pub experience_events: Vec<ExperienceEvent>,
    pub projection: Option<TruthProjection>,
    pub runtime_scope_id: String,
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
    // NOTE: truth_runtime dispatcher is in maintenance mode (Phase 6a cleanup).
    // Movement-territory truths (activate-subscription, upgrade-subscription-plan,
    // suspend-service-on-payment-failure, reconcile-model-usage-against-customer-ledger,
    // refill-prepaid-ai-credits) were deleted — they belong in commerce-rails.
    // CRM/Catalyst truths (qualify-inbound-lead, score-inbound-fit, plan-outbound-campaign,
    // match-renewal-context, schedule-strategic-meetings, evaluate-acquisition-target) were
    // relocated to catalyst-biz/truths or atelier-showcase/crm-helm.
    // Full dispatch now lives in helm-truth-execution. Phase 9 removes this dispatcher.
    match truth_key {
        _ => Err(Status::unimplemented(format!(
            "truth execution is not implemented yet for {truth_key}"
        ))),
    }
}

// NOTE: supports_truth_execution is in maintenance mode (Phase 6a cleanup).
// All truths were removed from the application-server dispatcher. See execute_truth for context.
// Phase 9 removes this function entirely.
pub fn supports_truth_execution(_truth_key: &str) -> bool {
    false
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
        ConvergeError::EmptyProvenance { suggestor } => Status::failed_precondition(format!(
            "converge suggestor emitted empty provenance: {suggestor}"
        )),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeGateDecision {
    Approved,
    Rejected,
}

#[derive(Debug, Default)]
struct RuntimeGateDecisions {
    decisions: HashMap<String, RuntimeGateDecision>,
}

impl RuntimeGateDecisions {
    fn load(
        runtime_stores: &AppRuntimeStores,
        runtime_ctx: &RuntimeContext,
    ) -> Result<Self, Status> {
        let records = runtime_stores
            .query_experience_records(&EventQuery {
                correlation_id: Some(runtime_ctx.scope_id.clone().into()),
                ..EventQuery::default()
            })
            .map_err(status_from_storage)?;
        let mut decisions = HashMap::new();

        for record in records {
            let ExperienceRecord::User(envelope) = record else {
                continue;
            };

            match envelope.event {
                UserExperienceEvent::UserApprovalGranted {
                    gate_request_id, ..
                } => {
                    decisions.insert(gate_request_id.to_string(), RuntimeGateDecision::Approved);
                }
                UserExperienceEvent::UserApprovalRejected {
                    gate_request_id, ..
                } => {
                    decisions.insert(gate_request_id.to_string(), RuntimeGateDecision::Rejected);
                }
                UserExperienceEvent::UserOverrideIssued {
                    target: OverrideTarget::Constraint(constraint),
                    ..
                } => {
                    decisions.insert(constraint.to_string(), RuntimeGateDecision::Rejected);
                }
                _ => {}
            }
        }

        Ok(Self { decisions })
    }

    fn decision_for(
        &self,
        runtime_scope_id: &str,
        approval_ref: &str,
    ) -> Option<RuntimeGateDecision> {
        self.decisions
            .get(&runtime_gate_request_id(runtime_scope_id, approval_ref))
            .copied()
            .or_else(|| self.decisions.get(approval_ref).copied())
    }
}

struct ApprovalAwareCriterionEvaluator {
    inner: std::sync::Arc<dyn CriterionEvaluator>,
    runtime_scope_id: String,
    decisions: RuntimeGateDecisions,
}

impl CriterionEvaluator for ApprovalAwareCriterionEvaluator {
    fn evaluate(
        &self,
        criterion: &Criterion,
        context: &dyn converge_kernel::Context,
    ) -> CriterionResult {
        match self.inner.evaluate(criterion, context) {
            CriterionResult::Blocked {
                reason,
                approval_ref: Some(approval_ref),
            } => match self
                .decisions
                .decision_for(&self.runtime_scope_id, approval_ref.as_str())
            {
                Some(RuntimeGateDecision::Approved) => CriterionResult::Met {
                    evidence: vec![FactId::new(approval_ref.to_string())],
                },
                Some(RuntimeGateDecision::Rejected) => CriterionResult::Unmet {
                    reason: format!("approval rejected for {approval_ref}: {reason}"),
                },
                None => CriterionResult::Blocked {
                    reason,
                    approval_ref: Some(approval_ref),
                },
            },
            result => result,
        }
    }
}

pub fn runtime_gate_request_id(runtime_scope_id: &str, approval_ref: &str) -> String {
    format!("{runtime_scope_id}:{approval_ref}")
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
    let decisions = RuntimeGateDecisions::load(runtime_stores, runtime_ctx)?;
    let criterion_evaluator = std::sync::Arc::new(ApprovalAwareCriterionEvaluator {
        inner: criterion_evaluator,
        runtime_scope_id: runtime_ctx.scope_id.clone(),
        decisions,
    });
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
