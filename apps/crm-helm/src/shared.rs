//! Shared conversion helpers for proto ↔ kernel types.
//!
//! Extracted from helms/crates/application-server/src/service.rs and adapted
//! to reference the local proto module.

use application_kernel::{
    ActivityOutcome, Actor, ActorKind, CommunicationChannel, CommunicationDirection,
    DocumentStatus, FieldDefinition, FieldType, ObjectDefinitionKind, OpportunityStage,
    OrganizationLifecycle, RecordKind, RecordRef, RelationshipCardinality, RelationshipDefinition,
    ViewLayout, WorkflowPriority, WorkflowState,
};
use application_storage::StorageError;
use chrono::{DateTime, Utc};
use prost_types::Timestamp;
use tonic::Status;
use uuid::Uuid;

use crate::proto::common as pb;

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

pub fn status_from_storage(error: StorageError) -> Status {
    match error {
        StorageError::LockPoisoned => Status::internal("storage lock poisoned"),
        StorageError::Kernel(error) => status_from_kernel(error),
        StorageError::ConnectionFailed { backend, message } => {
            Status::unavailable(format!("{backend} connection failed: {message}"))
        }
        StorageError::SerializationFailed { message } => Status::internal(message),
        StorageError::Timeout { operation } => Status::deadline_exceeded(operation),
        StorageError::RuntimeStore { message } => Status::internal(message),
    }
}

pub fn status_from_kernel(error: application_kernel::KernelError) -> Status {
    match error {
        application_kernel::KernelError::Validation(message) => Status::invalid_argument(message),
        application_kernel::KernelError::NotFound { kind, id } => {
            Status::not_found(format!("{kind} not found: {id}"))
        }
        application_kernel::KernelError::Invariant(message) => Status::failed_precondition(message),
        application_kernel::KernelError::Conflict(message) => Status::already_exists(message),
    }
}

// ---------------------------------------------------------------------------
// UUID helpers
// ---------------------------------------------------------------------------

pub fn parse_uuid(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value).map_err(|_| Status::invalid_argument(format!("invalid uuid: {value}")))
}

pub fn parse_optional_uuid(value: Option<String>) -> Result<Option<Uuid>, Status> {
    value
        .and_then(|v| {
            let trimmed = v.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .map(|v| parse_uuid(&v))
        .transpose()
}

pub fn clamp_bps(value: u32) -> Result<u16, Status> {
    u16::try_from(value)
        .map_err(|_| Status::invalid_argument("bps value is out of range"))
        .and_then(|v| {
            if v > 10_000 {
                Err(Status::invalid_argument(
                    "bps value must be between 0 and 10000",
                ))
            } else {
                Ok(v)
            }
        })
}

pub fn default_limit(value: u32, fallback: usize) -> usize {
    if value == 0 { fallback } else { value as usize }
}

// ---------------------------------------------------------------------------
// Timestamp helpers
// ---------------------------------------------------------------------------

pub fn proto_timestamp(value: DateTime<Utc>) -> Option<Timestamp> {
    Some(Timestamp {
        seconds: value.timestamp(),
        nanos: value.timestamp_subsec_nanos() as i32,
    })
}

pub fn datetime_from_proto(value: Timestamp) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(value.seconds, value.nanos as u32)
}

// ---------------------------------------------------------------------------
// Actor
// ---------------------------------------------------------------------------

pub fn actor_from_proto(actor: Option<pb::Actor>) -> Actor {
    actor.map_or_else(Actor::system, |actor| Actor {
        actor_id: actor.actor_id,
        display_name: actor.display_name,
        kind: match pb::ActorKind::try_from(actor.kind).unwrap_or(pb::ActorKind::System) {
            pb::ActorKind::Human => ActorKind::Human,
            pb::ActorKind::Agent => ActorKind::Agent,
            pb::ActorKind::System | pb::ActorKind::Unspecified => ActorKind::System,
        },
    })
}

pub fn proto_actor(actor: Actor) -> pb::Actor {
    pb::Actor {
        actor_id: actor.actor_id,
        display_name: actor.display_name,
        kind: match actor.kind {
            ActorKind::Human => pb::ActorKind::Human as i32,
            ActorKind::Agent => pb::ActorKind::Agent as i32,
            ActorKind::System => pb::ActorKind::System as i32,
        },
    }
}

// ---------------------------------------------------------------------------
// RecordRef
// ---------------------------------------------------------------------------

pub fn record_ref_from_proto(reference: pb::RecordRef) -> Result<RecordRef, Status> {
    Ok(RecordRef {
        kind: match pb::RecordKind::try_from(reference.kind) {
            Ok(pb::RecordKind::Organization) => RecordKind::Organization,
            Ok(pb::RecordKind::Person) => RecordKind::Person,
            Ok(pb::RecordKind::Relationship) => RecordKind::Relationship,
            Ok(pb::RecordKind::Lead) => RecordKind::Lead,
            Ok(pb::RecordKind::Opportunity) => RecordKind::Opportunity,
            Ok(pb::RecordKind::Conversation) => RecordKind::Conversation,
            Ok(pb::RecordKind::Activity) => RecordKind::Activity,
            Ok(pb::RecordKind::Task) => RecordKind::Task,
            Ok(pb::RecordKind::OfferQuote) => RecordKind::OfferQuote,
            Ok(pb::RecordKind::OrderSubscription) => RecordKind::OrderSubscription,
            Ok(pb::RecordKind::Document) => RecordKind::Document,
            Ok(pb::RecordKind::Fact) => RecordKind::Fact,
            Ok(pb::RecordKind::Intent) => RecordKind::Intent,
            Ok(pb::RecordKind::WorkflowCase) => RecordKind::WorkflowCase,
            Ok(pb::RecordKind::CommunicationEvent) => RecordKind::CommunicationEvent,
            Ok(pb::RecordKind::PermissionGrant) => RecordKind::PermissionGrant,
            Ok(pb::RecordKind::AuditEntry) => RecordKind::AuditEntry,
            Ok(pb::RecordKind::Note) => RecordKind::Note,
            Ok(pb::RecordKind::CatalogItem) => RecordKind::CatalogItem,
            Ok(pb::RecordKind::Unspecified) | Err(_) => {
                return Err(Status::invalid_argument("record kind is required"));
            }
        },
        id: parse_uuid(&reference.record_id)?,
    })
}

pub fn proto_record_ref(reference: RecordRef) -> pb::RecordRef {
    pb::RecordRef {
        kind: match reference.kind {
            RecordKind::Organization => pb::RecordKind::Organization as i32,
            RecordKind::Person => pb::RecordKind::Person as i32,
            RecordKind::Relationship => pb::RecordKind::Relationship as i32,
            RecordKind::Lead => pb::RecordKind::Lead as i32,
            RecordKind::Opportunity => pb::RecordKind::Opportunity as i32,
            RecordKind::Conversation => pb::RecordKind::Conversation as i32,
            RecordKind::Activity => pb::RecordKind::Activity as i32,
            RecordKind::Task => pb::RecordKind::Task as i32,
            RecordKind::OfferQuote => pb::RecordKind::OfferQuote as i32,
            RecordKind::OrderSubscription => pb::RecordKind::OrderSubscription as i32,
            RecordKind::Document => pb::RecordKind::Document as i32,
            RecordKind::Fact => pb::RecordKind::Fact as i32,
            RecordKind::Intent => pb::RecordKind::Intent as i32,
            RecordKind::WorkflowCase => pb::RecordKind::WorkflowCase as i32,
            RecordKind::CommunicationEvent => pb::RecordKind::CommunicationEvent as i32,
            RecordKind::PermissionGrant => pb::RecordKind::PermissionGrant as i32,
            RecordKind::AuditEntry => pb::RecordKind::AuditEntry as i32,
            RecordKind::Note => pb::RecordKind::Note as i32,
            RecordKind::CatalogItem => pb::RecordKind::CatalogItem as i32,
        },
        record_id: reference.id.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Enum converters: kernel ← proto
// ---------------------------------------------------------------------------

pub fn organization_lifecycle_from_proto(value: i32) -> OrganizationLifecycle {
    match pb::OrganizationLifecycle::try_from(value).unwrap_or(pb::OrganizationLifecycle::Prospect)
    {
        pb::OrganizationLifecycle::Active => OrganizationLifecycle::Active,
        pb::OrganizationLifecycle::Dormant => OrganizationLifecycle::Dormant,
        pb::OrganizationLifecycle::Partner => OrganizationLifecycle::Partner,
        pb::OrganizationLifecycle::Prospect | pb::OrganizationLifecycle::Unspecified => {
            OrganizationLifecycle::Prospect
        }
    }
}

pub fn opportunity_stage_from_proto(value: i32) -> OpportunityStage {
    match pb::OpportunityStage::try_from(value).unwrap_or(pb::OpportunityStage::Qualifying) {
        pb::OpportunityStage::Discovery => OpportunityStage::Discovery,
        pb::OpportunityStage::Proposal => OpportunityStage::Proposal,
        pb::OpportunityStage::Negotiation => OpportunityStage::Negotiation,
        pb::OpportunityStage::ClosedWon => OpportunityStage::ClosedWon,
        pb::OpportunityStage::ClosedLost => OpportunityStage::ClosedLost,
        pb::OpportunityStage::Qualifying | pb::OpportunityStage::Unspecified => {
            OpportunityStage::Qualifying
        }
    }
}

pub fn activity_outcome_from_proto(value: i32) -> ActivityOutcome {
    match pb::ActivityOutcome::try_from(value).unwrap_or(pb::ActivityOutcome::Completed) {
        pb::ActivityOutcome::Waiting => ActivityOutcome::Waiting,
        pb::ActivityOutcome::Blocked => ActivityOutcome::Blocked,
        pb::ActivityOutcome::Completed | pb::ActivityOutcome::Unspecified => {
            ActivityOutcome::Completed
        }
    }
}

pub fn document_status_from_proto(value: i32) -> DocumentStatus {
    match pb::DocumentStatus::try_from(value).unwrap_or(pb::DocumentStatus::Draft) {
        pb::DocumentStatus::Verified => DocumentStatus::Verified,
        pb::DocumentStatus::Archived => DocumentStatus::Archived,
        pb::DocumentStatus::Draft | pb::DocumentStatus::Unspecified => DocumentStatus::Draft,
    }
}

pub fn communication_channel_from_proto(value: i32) -> CommunicationChannel {
    match pb::CommunicationChannel::try_from(value).unwrap_or(pb::CommunicationChannel::Email) {
        pb::CommunicationChannel::Phone => CommunicationChannel::Phone,
        pb::CommunicationChannel::Meeting => CommunicationChannel::Meeting,
        pb::CommunicationChannel::Chat => CommunicationChannel::Chat,
        pb::CommunicationChannel::Sms => CommunicationChannel::Sms,
        pb::CommunicationChannel::Email | pb::CommunicationChannel::Unspecified => {
            CommunicationChannel::Email
        }
    }
}

pub fn communication_direction_from_proto(value: i32) -> CommunicationDirection {
    match pb::CommunicationDirection::try_from(value).unwrap_or(pb::CommunicationDirection::Inbound)
    {
        pb::CommunicationDirection::Outbound => CommunicationDirection::Outbound,
        pb::CommunicationDirection::Internal => CommunicationDirection::Internal,
        pb::CommunicationDirection::Inbound | pb::CommunicationDirection::Unspecified => {
            CommunicationDirection::Inbound
        }
    }
}

pub fn workflow_priority_from_proto(value: i32) -> WorkflowPriority {
    match pb::WorkflowPriority::try_from(value).unwrap_or(pb::WorkflowPriority::Medium) {
        pb::WorkflowPriority::Low => WorkflowPriority::Low,
        pb::WorkflowPriority::High => WorkflowPriority::High,
        pb::WorkflowPriority::Critical => WorkflowPriority::Critical,
        pb::WorkflowPriority::Medium | pb::WorkflowPriority::Unspecified => {
            WorkflowPriority::Medium
        }
    }
}

pub fn workflow_state_from_proto(value: i32) -> WorkflowState {
    match pb::WorkflowState::try_from(value).unwrap_or(pb::WorkflowState::Open) {
        pb::WorkflowState::AwaitingApproval => WorkflowState::AwaitingApproval,
        pb::WorkflowState::WaitingExternal => WorkflowState::WaitingExternal,
        pb::WorkflowState::Blocked => WorkflowState::Blocked,
        pb::WorkflowState::Done => WorkflowState::Done,
        pb::WorkflowState::Open | pb::WorkflowState::Unspecified => WorkflowState::Open,
    }
}

pub fn relationship_type_from_proto(value: i32) -> application_kernel::RelationshipType {
    match pb::RelationshipType::try_from(value).unwrap_or(pb::RelationshipType::Other) {
        pb::RelationshipType::Employment => application_kernel::RelationshipType::Employment,
        pb::RelationshipType::Champion => application_kernel::RelationshipType::Champion,
        pb::RelationshipType::DecisionMaker => application_kernel::RelationshipType::DecisionMaker,
        pb::RelationshipType::Partner => application_kernel::RelationshipType::Partner,
        pb::RelationshipType::Competitor => application_kernel::RelationshipType::Competitor,
        pb::RelationshipType::Other | pb::RelationshipType::Unspecified => {
            application_kernel::RelationshipType::Other
        }
    }
}

pub fn object_definition_kind_from_proto(value: i32) -> ObjectDefinitionKind {
    match pb::ObjectDefinitionKind::try_from(value).unwrap_or(pb::ObjectDefinitionKind::Custom) {
        pb::ObjectDefinitionKind::Standard => ObjectDefinitionKind::Standard,
        pb::ObjectDefinitionKind::Custom | pb::ObjectDefinitionKind::Unspecified => {
            ObjectDefinitionKind::Custom
        }
    }
}

pub fn view_layout_from_proto(value: i32) -> ViewLayout {
    match pb::ViewLayout::try_from(value).unwrap_or(pb::ViewLayout::Table) {
        pb::ViewLayout::Kanban => ViewLayout::Kanban,
        pb::ViewLayout::Calendar => ViewLayout::Calendar,
        pb::ViewLayout::Table | pb::ViewLayout::Unspecified => ViewLayout::Table,
    }
}

pub fn field_type_from_proto(value: i32) -> FieldType {
    match pb::FieldType::try_from(value).unwrap_or(pb::FieldType::Text) {
        pb::FieldType::LongText => FieldType::LongText,
        pb::FieldType::Number => FieldType::Number,
        pb::FieldType::Currency => FieldType::Currency,
        pb::FieldType::Boolean => FieldType::Boolean,
        pb::FieldType::Date => FieldType::Date,
        pb::FieldType::DateTime => FieldType::DateTime,
        pb::FieldType::Email => FieldType::Email,
        pb::FieldType::Phone => FieldType::Phone,
        pb::FieldType::Url => FieldType::Url,
        pb::FieldType::Select => FieldType::Select,
        pb::FieldType::MultiSelect => FieldType::MultiSelect,
        pb::FieldType::Relation => FieldType::Relation,
        pb::FieldType::Text | pb::FieldType::Unspecified => FieldType::Text,
    }
}

pub fn relationship_cardinality_from_proto(value: i32) -> RelationshipCardinality {
    match pb::RelationshipCardinality::try_from(value)
        .unwrap_or(pb::RelationshipCardinality::OneToMany)
    {
        pb::RelationshipCardinality::OneToOne => RelationshipCardinality::OneToOne,
        pb::RelationshipCardinality::ManyToMany => RelationshipCardinality::ManyToMany,
        pb::RelationshipCardinality::OneToMany | pb::RelationshipCardinality::Unspecified => {
            RelationshipCardinality::OneToMany
        }
    }
}

// ---------------------------------------------------------------------------
// Proto constructors: proto ← kernel
// ---------------------------------------------------------------------------

pub fn proto_organization(value: application_kernel::Organization) -> pb::Organization {
    pb::Organization {
        id: value.id.to_string(),
        name: value.name,
        external_key: value.external_key,
        website: value.website,
        industry: value.industry,
        lifecycle: match value.lifecycle {
            OrganizationLifecycle::Prospect => pb::OrganizationLifecycle::Prospect as i32,
            OrganizationLifecycle::Active => pb::OrganizationLifecycle::Active as i32,
            OrganizationLifecycle::Dormant => pb::OrganizationLifecycle::Dormant as i32,
            OrganizationLifecycle::Partner => pb::OrganizationLifecycle::Partner as i32,
        },
        owner_user_id: value.owner_user_id,
        tags: value.tags,
        created_at: proto_timestamp(value.created_at),
        updated_at: proto_timestamp(value.updated_at),
    }
}

pub fn proto_person(value: application_kernel::Person) -> pb::Person {
    pb::Person {
        id: value.id.to_string(),
        organization_id: value.organization_id.map(|id| id.to_string()),
        full_name: value.full_name,
        title: value.title,
        email: value.email,
        phone: value.phone,
        linkedin_url: value.linkedin_url,
        created_at: proto_timestamp(value.created_at),
        updated_at: proto_timestamp(value.updated_at),
    }
}

pub fn proto_relationship(value: application_kernel::Relationship) -> pb::Relationship {
    pb::Relationship {
        id: value.id.to_string(),
        from: Some(proto_record_ref(value.from)),
        to: Some(proto_record_ref(value.to)),
        relationship_type: match value.relationship_type {
            application_kernel::RelationshipType::Employment => {
                pb::RelationshipType::Employment as i32
            }
            application_kernel::RelationshipType::Champion => pb::RelationshipType::Champion as i32,
            application_kernel::RelationshipType::DecisionMaker => {
                pb::RelationshipType::DecisionMaker as i32
            }
            application_kernel::RelationshipType::Partner => pb::RelationshipType::Partner as i32,
            application_kernel::RelationshipType::Competitor => {
                pb::RelationshipType::Competitor as i32
            }
            application_kernel::RelationshipType::Other => pb::RelationshipType::Other as i32,
        },
        label: value.label,
        created_at: proto_timestamp(value.created_at),
    }
}

pub fn proto_opportunity(value: application_kernel::Opportunity) -> pb::Opportunity {
    pb::Opportunity {
        id: value.id.to_string(),
        organization_id: value.organization_id.to_string(),
        primary_contact_id: value.primary_contact_id.map(|id| id.to_string()),
        name: value.name,
        stage: match value.stage {
            OpportunityStage::Qualifying => pb::OpportunityStage::Qualifying as i32,
            OpportunityStage::Discovery => pb::OpportunityStage::Discovery as i32,
            OpportunityStage::Proposal => pb::OpportunityStage::Proposal as i32,
            OpportunityStage::Negotiation => pb::OpportunityStage::Negotiation as i32,
            OpportunityStage::ClosedWon => pb::OpportunityStage::ClosedWon as i32,
            OpportunityStage::ClosedLost => pb::OpportunityStage::ClosedLost as i32,
        },
        value: Some(pb::Money {
            currency_code: value.value.currency_code,
            amount_minor: value.value.amount_minor,
        }),
        confidence_bps: u32::from(value.confidence_bps),
        next_step: value.next_step,
        expected_close_at: value.expected_close_at.and_then(proto_timestamp),
        created_at: proto_timestamp(value.created_at),
        updated_at: proto_timestamp(value.updated_at),
    }
}

pub fn proto_activity(value: application_kernel::Activity) -> pb::Activity {
    pb::Activity {
        id: value.id.to_string(),
        subject: value.subject,
        details: value.details,
        actor: Some(proto_actor(value.actor)),
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
        outcome: match value.outcome {
            ActivityOutcome::Completed => pb::ActivityOutcome::Completed as i32,
            ActivityOutcome::Waiting => pb::ActivityOutcome::Waiting as i32,
            ActivityOutcome::Blocked => pb::ActivityOutcome::Blocked as i32,
        },
        occurred_at: proto_timestamp(value.occurred_at),
        next_action_due_at: value.next_action_due_at.and_then(proto_timestamp),
    }
}

pub fn proto_note(value: application_kernel::Note) -> pb::Note {
    pb::Note {
        id: value.id.to_string(),
        subject: value.subject,
        body: value.body,
        author: Some(proto_actor(value.author)),
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
        promoted_to_fact: value.promoted_to_fact,
        created_at: proto_timestamp(value.created_at),
    }
}

pub fn proto_document(value: application_kernel::Document) -> pb::Document {
    pb::Document {
        id: value.id.to_string(),
        title: value.title,
        media_type: value.media_type,
        uri: value.uri,
        status: match value.status {
            DocumentStatus::Draft => pb::DocumentStatus::Draft as i32,
            DocumentStatus::Verified => pb::DocumentStatus::Verified as i32,
            DocumentStatus::Archived => pb::DocumentStatus::Archived as i32,
        },
        uploaded_by: Some(proto_actor(value.uploaded_by)),
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
        created_at: proto_timestamp(value.created_at),
    }
}

pub fn proto_communication_event(
    value: application_kernel::CommunicationEvent,
) -> pb::CommunicationEvent {
    pb::CommunicationEvent {
        id: value.id.to_string(),
        channel: match value.channel {
            CommunicationChannel::Email => pb::CommunicationChannel::Email as i32,
            CommunicationChannel::Phone => pb::CommunicationChannel::Phone as i32,
            CommunicationChannel::Meeting => pb::CommunicationChannel::Meeting as i32,
            CommunicationChannel::Chat => pb::CommunicationChannel::Chat as i32,
            CommunicationChannel::Sms => pb::CommunicationChannel::Sms as i32,
        },
        direction: match value.direction {
            CommunicationDirection::Inbound => pb::CommunicationDirection::Inbound as i32,
            CommunicationDirection::Outbound => pb::CommunicationDirection::Outbound as i32,
            CommunicationDirection::Internal => pb::CommunicationDirection::Internal as i32,
        },
        subject: value.subject,
        summary: value.summary,
        counterpart: value.counterpart,
        actor: Some(proto_actor(value.actor)),
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
        occurred_at: proto_timestamp(value.occurred_at),
    }
}

pub fn proto_workflow_case(value: application_kernel::WorkflowCase) -> pb::WorkflowCase {
    pb::WorkflowCase {
        id: value.id.to_string(),
        title: value.title,
        state: match value.state {
            WorkflowState::Open => pb::WorkflowState::Open as i32,
            WorkflowState::AwaitingApproval => pb::WorkflowState::AwaitingApproval as i32,
            WorkflowState::WaitingExternal => pb::WorkflowState::WaitingExternal as i32,
            WorkflowState::Blocked => pb::WorkflowState::Blocked as i32,
            WorkflowState::Done => pb::WorkflowState::Done as i32,
        },
        priority: match value.priority {
            WorkflowPriority::Low => pb::WorkflowPriority::Low as i32,
            WorkflowPriority::Medium => pb::WorkflowPriority::Medium as i32,
            WorkflowPriority::High => pb::WorkflowPriority::High as i32,
            WorkflowPriority::Critical => pb::WorkflowPriority::Critical as i32,
        },
        owner_user_id: value.owner_user_id,
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
        opened_at: proto_timestamp(value.opened_at),
        updated_at: proto_timestamp(value.updated_at),
    }
}

pub fn proto_fact(value: application_kernel::Fact) -> pb::Fact {
    pb::Fact {
        id: value.id.to_string(),
        statement: value.statement,
        confidence_bps: u32::from(value.confidence_bps),
        promoted_by: Some(proto_actor(value.promoted_by)),
        source_note_id: value.source_note_id.map(|id| id.to_string()),
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
        created_at: proto_timestamp(value.created_at),
    }
}

pub fn proto_timeline_entry(value: application_kernel::TimelineEntry) -> pb::TimelineEntry {
    pb::TimelineEntry {
        id: value.id.to_string(),
        kind: match value.kind {
            application_kernel::TimelineEntryKind::Activity => {
                pb::TimelineEntryKind::Activity as i32
            }
            application_kernel::TimelineEntryKind::Note => pb::TimelineEntryKind::Note as i32,
            application_kernel::TimelineEntryKind::Document => {
                pb::TimelineEntryKind::Document as i32
            }
            application_kernel::TimelineEntryKind::Communication => {
                pb::TimelineEntryKind::Communication as i32
            }
            application_kernel::TimelineEntryKind::Fact => pb::TimelineEntryKind::Fact as i32,
            application_kernel::TimelineEntryKind::Audit => pb::TimelineEntryKind::Audit as i32,
        },
        anchor: value.anchor.map(proto_record_ref),
        headline: value.headline,
        body: value.body,
        actor: Some(proto_actor(value.actor)),
        occurred_at: proto_timestamp(value.occurred_at),
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
    }
}

pub fn proto_account_summary(value: application_kernel::AccountSummary) -> pb::AccountSummary {
    pb::AccountSummary {
        organization: Some(proto_organization(value.organization)),
        contacts: value.contacts.into_iter().map(proto_person).collect(),
        opportunities: value
            .opportunities
            .into_iter()
            .map(proto_opportunity)
            .collect(),
        workflow_cases: value
            .workflow_cases
            .into_iter()
            .map(proto_workflow_case)
            .collect(),
        facts: value.facts.into_iter().map(proto_fact).collect(),
        documents: value.documents.into_iter().map(proto_document).collect(),
        permissions: value
            .permissions
            .into_iter()
            .map(proto_permission_grant)
            .collect(),
        recent_timeline: value
            .recent_timeline
            .into_iter()
            .map(proto_timeline_entry)
            .collect(),
    }
}

pub fn proto_permission_grant(value: application_kernel::PermissionGrant) -> pb::PermissionGrant {
    pb::PermissionGrant {
        id: value.id.to_string(),
        subject: value.subject,
        role: value.role,
        scope: value.scope,
        granted_by: Some(proto_actor(value.granted_by)),
        created_at: proto_timestamp(value.created_at),
    }
}

pub fn field_definition_from_proto(field: pb::FieldDefinition) -> Result<FieldDefinition, Status> {
    Ok(FieldDefinition {
        id: parse_optional_uuid(Some(field.id))?.unwrap_or_else(Uuid::new_v4),
        key: field.key,
        label: field.label,
        field_type: field_type_from_proto(field.field_type),
        required: field.r#required,
        options: field.options,
        relation_object_key: field.relation_object_key,
        active: field.active,
    })
}

pub fn relationship_definition_from_proto(
    definition: pb::RelationshipDefinition,
) -> Result<RelationshipDefinition, Status> {
    Ok(RelationshipDefinition {
        id: parse_optional_uuid(Some(definition.id))?.unwrap_or_else(Uuid::new_v4),
        target_object_key: definition.target_object_key,
        cardinality: relationship_cardinality_from_proto(definition.cardinality),
        label: definition.label,
    })
}

pub fn proto_field_definition(value: FieldDefinition) -> pb::FieldDefinition {
    pb::FieldDefinition {
        id: value.id.to_string(),
        key: value.key,
        label: value.label,
        field_type: match value.field_type {
            FieldType::Text => pb::FieldType::Text as i32,
            FieldType::LongText => pb::FieldType::LongText as i32,
            FieldType::Number => pb::FieldType::Number as i32,
            FieldType::Currency => pb::FieldType::Currency as i32,
            FieldType::Boolean => pb::FieldType::Boolean as i32,
            FieldType::Date => pb::FieldType::Date as i32,
            FieldType::DateTime => pb::FieldType::DateTime as i32,
            FieldType::Email => pb::FieldType::Email as i32,
            FieldType::Phone => pb::FieldType::Phone as i32,
            FieldType::Url => pb::FieldType::Url as i32,
            FieldType::Select => pb::FieldType::Select as i32,
            FieldType::MultiSelect => pb::FieldType::MultiSelect as i32,
            FieldType::Relation => pb::FieldType::Relation as i32,
        },
        r#required: value.required,
        options: value.options,
        relation_object_key: value.relation_object_key,
        active: value.active,
    }
}

pub fn proto_relationship_definition(value: RelationshipDefinition) -> pb::RelationshipDefinition {
    pb::RelationshipDefinition {
        id: value.id.to_string(),
        target_object_key: value.target_object_key,
        cardinality: match value.cardinality {
            RelationshipCardinality::OneToOne => pb::RelationshipCardinality::OneToOne as i32,
            RelationshipCardinality::OneToMany => pb::RelationshipCardinality::OneToMany as i32,
            RelationshipCardinality::ManyToMany => pb::RelationshipCardinality::ManyToMany as i32,
        },
        label: value.label,
    }
}

pub fn proto_object_definition(
    value: application_kernel::ObjectDefinition,
) -> pb::ObjectDefinition {
    pb::ObjectDefinition {
        id: value.id.to_string(),
        key: value.key,
        display_name: value.display_name,
        kind: match value.kind {
            ObjectDefinitionKind::Standard => pb::ObjectDefinitionKind::Standard as i32,
            ObjectDefinitionKind::Custom => pb::ObjectDefinitionKind::Custom as i32,
        },
        fields: value
            .fields
            .into_iter()
            .map(proto_field_definition)
            .collect(),
        relationships: value
            .relationships
            .into_iter()
            .map(proto_relationship_definition)
            .collect(),
        active: value.active,
        created_at: proto_timestamp(value.created_at),
        updated_at: proto_timestamp(value.updated_at),
    }
}

pub fn proto_view_definition(value: application_kernel::ViewDefinition) -> pb::ViewDefinition {
    pb::ViewDefinition {
        id: value.id.to_string(),
        object_key: value.object_key,
        name: value.name,
        layout: match value.layout {
            ViewLayout::Table => pb::ViewLayout::Table as i32,
            ViewLayout::Kanban => pb::ViewLayout::Kanban as i32,
            ViewLayout::Calendar => pb::ViewLayout::Calendar as i32,
        },
        filter_expression: value.filter_expression,
        sort_expression: value.sort_expression,
        visible_fields: value.visible_fields,
        group_by: value.group_by,
        favorite: value.favorite,
        owner_user_id: value.owner_user_id,
        created_at: proto_timestamp(value.created_at),
        updated_at: proto_timestamp(value.updated_at),
    }
}
