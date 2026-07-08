use std::collections::HashMap;
use std::sync::Mutex;

use application_kernel::{
    Actor as CrmActor, Document, Entitlement, Fact as CrmFact, LedgerEntry, Opportunity,
    OrderSubscription, Organization, Person, WorkflowCase,
};
use application_storage::{AppKernelStore, AppRuntimeStores, StorageError};
use converge_core::FactId;
use converge_kernel::{
    ContextState as Context, ConvergeError, ConvergeResult, Criterion, CriterionEvaluator,
    CriterionResult, Engine, EventQuery, ExperienceEvent, ExperienceEventEnvelope,
    ExperienceEventObserver, ExperienceRecord, OverrideTarget, TypesRootIntent, TypesRunHooks,
    UserExperienceEvent,
};
use uuid::Uuid;

use crate::{TruthExecutionError, TruthExecutionModule};

// ── Public output types ────────────────────────────────────────────────────────

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

// ── Internal runtime types used by truth bodies ────────────────────────────────

/// Scope identifier for a single truth execution run.
pub struct RuntimeContext {
    pub scope_id: String,
}

/// Accumulates `ExperienceEvent`s emitted during an engine run.
#[derive(Default)]
pub struct RecordingObserver {
    events: Mutex<Vec<ExperienceEvent>>,
}

impl RecordingObserver {
    pub fn snapshot(&self) -> Vec<ExperienceEvent> {
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

// ── Approval-aware criterion evaluator ────────────────────────────────────────

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
    ) -> Result<Self, TruthExecutionError> {
        let records = runtime_stores
            .query_experience_records(&EventQuery {
                correlation_id: Some(runtime_ctx.scope_id.clone().into()),
                ..EventQuery::default()
            })
            .map_err(error_from_storage)?;
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

// ── Public helpers consumed by truth bodies ────────────────────────────────────

pub fn runtime_gate_request_id(runtime_scope_id: &str, approval_ref: &str) -> String {
    format!("{runtime_scope_id}:{approval_ref}")
}

pub async fn run_engine_with_runtime(
    runtime_stores: &AppRuntimeStores,
    engine: &mut Engine,
    runtime_ctx: &RuntimeContext,
    seed_context: Context,
    intent: &TypesRootIntent,
    criterion_evaluator: std::sync::Arc<dyn CriterionEvaluator>,
) -> Result<(ConvergeResult, Vec<ExperienceEvent>), TruthExecutionError> {
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
        .map_err(error_from_converge)?;

    runtime_stores
        .save_context(&runtime_ctx.scope_id, &result.context)
        .map_err(error_from_storage)?;

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
        .map_err(error_from_storage)?;

    Ok((result, experience_events))
}

// ── Error mappers ──────────────────────────────────────────────────────────────

/// Maps a [`ConvergeError`] to the corresponding [`TruthExecutionError`] variant.
pub fn error_from_converge(error: ConvergeError) -> TruthExecutionError {
    match error {
        ConvergeError::BudgetExhausted { kind } => TruthExecutionError::ResourceExhausted {
            message: format!("converge budget exhausted: {kind}"),
        },
        ConvergeError::InvariantViolation { name, reason, .. } => {
            TruthExecutionError::FailedPrecondition {
                message: format!("converge invariant violated: {name}: {reason}"),
            }
        }
        ConvergeError::AgentFailed { agent_id } => TruthExecutionError::Internal {
            message: format!("converge agent failed: {agent_id}"),
        },
        ConvergeError::EmptyProvenance { suggestor } => TruthExecutionError::FailedPrecondition {
            message: format!("converge suggestor emitted empty provenance: {suggestor}"),
        },
        ConvergeError::Conflict { id, .. } => TruthExecutionError::Aborted {
            message: format!("converge fact conflict: {id}"),
        },
        ConvergeError::InvalidResume { reason } => TruthExecutionError::FailedPrecondition {
            message: format!("converge invalid resume: {reason}"),
        },
        ConvergeError::InvalidAdmission { reason } => TruthExecutionError::InvalidArgument {
            message: format!("converge invalid admission: {reason}"),
        },
        ConvergeError::InvalidSnapshot { reason } => TruthExecutionError::DataLoss {
            message: format!("converge invalid context snapshot: {reason}"),
        },
    }
}

/// Maps a [`StorageError`] to the corresponding [`TruthExecutionError`] variant.
pub fn error_from_storage(error: StorageError) -> TruthExecutionError {
    match error {
        StorageError::LockPoisoned => TruthExecutionError::Internal {
            message: "storage lock poisoned".into(),
        },
        StorageError::Kernel(application_kernel::KernelError::Validation(message)) => {
            TruthExecutionError::InvalidArgument { message }
        }
        StorageError::Kernel(application_kernel::KernelError::NotFound { kind, id }) => {
            TruthExecutionError::NotFound {
                message: format!("{kind} not found: {id}"),
            }
        }
        StorageError::Kernel(application_kernel::KernelError::Invariant(message)) => {
            TruthExecutionError::FailedPrecondition { message }
        }
        StorageError::Kernel(application_kernel::KernelError::Conflict(message)) => {
            TruthExecutionError::AlreadyExists { message }
        }
        StorageError::ConnectionFailed { backend, message } => TruthExecutionError::Unavailable {
            message: format!("{backend} connection failed: {message}"),
        },
        StorageError::SerializationFailed { message } => TruthExecutionError::Internal { message },
        StorageError::Timeout { operation } => TruthExecutionError::DeadlineExceeded {
            message: operation,
        },
        StorageError::RuntimeStore { message } => TruthExecutionError::Internal { message },
    }
}

pub fn domain_event_kind_name(event: &application_kernel::DomainEvent) -> &'static str {
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

// ── Registry-based dispatcher ──────────────────────────────────────────────────

/// Context passed to every `TruthBody::execute` call.
///
/// Carries everything the original `execute_truth` function parameters did:
/// the kernel store, the app runtime stores, the flat key→value input map,
/// actor identity, and whether to persist a domain projection.
///
/// # Simplification note
///
/// The original dispatcher was generic over `S: KernelStore`.  To allow
/// trait-object registration without propagating the generic, we use the
/// concrete `AppKernelStore` enum here — it covers both the in-memory and
/// SurrealDB variants.  Phases 3b/4b can refine if a different concrete
/// type is needed.
pub struct TruthExecutionContext {
    pub store: AppKernelStore,
    pub runtime_stores: AppRuntimeStores,
    pub inputs: HashMap<String, String>,
    pub actor: CrmActor,
    pub persist_projection: bool,
}

/// Dispatch a truth body by key, using the registry held by `module`.
///
/// Returns `Err(TruthExecutionError::Unimplemented(...))` if no body is
/// registered for `truth_key`, matching the behaviour of the original
/// hard-coded match.
pub async fn execute_truth(
    module: &TruthExecutionModule,
    truth_key: &str,
    ctx: TruthExecutionContext,
) -> Result<TruthExecutionArtifacts, TruthExecutionError> {
    let body = module.lookup(truth_key).ok_or_else(|| {
        TruthExecutionError::Unimplemented {
            message: format!("truth execution is not implemented yet for {truth_key}"),
        }
    })?;
    body.execute(ctx).await
}

/// Returns `true` if a body is registered for `truth_key`.
///
/// Replaces the original hard-coded `supports_truth_execution` match list.
pub fn supports_truth_execution(module: &TruthExecutionModule, truth_key: &str) -> bool {
    module.lookup(truth_key).is_some()
}
