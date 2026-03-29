use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActorKind {
    Human,
    Agent,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub actor_id: String,
    pub display_name: String,
    pub kind: ActorKind,
}

impl Actor {
    #[must_use]
    pub fn system() -> Self {
        Self {
            actor_id: "system".to_string(),
            display_name: "System".to_string(),
            kind: ActorKind::System,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordKind {
    Organization,
    Person,
    Relationship,
    Lead,
    Opportunity,
    Conversation,
    Activity,
    Task,
    OfferQuote,
    OrderSubscription,
    Document,
    Fact,
    Intent,
    WorkflowCase,
    CommunicationEvent,
    PermissionGrant,
    AuditEntry,
    Note,
    CatalogItem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecordRef {
    pub kind: RecordKind,
    pub id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub currency_code: String,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipType {
    Employment,
    Champion,
    DecisionMaker,
    Partner,
    Competitor,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationLifecycle {
    Prospect,
    Active,
    Dormant,
    Partner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpportunityStage {
    Qualifying,
    Discovery,
    Proposal,
    Negotiation,
    ClosedWon,
    ClosedLost,
}

impl OpportunityStage {
    #[must_use]
    pub fn is_closed(self) -> bool {
        matches!(self, Self::ClosedWon | Self::ClosedLost)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityOutcome {
    Completed,
    Waiting,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentStatus {
    Draft,
    Verified,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommunicationChannel {
    Email,
    Phone,
    Meeting,
    Chat,
    Sms,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommunicationDirection {
    Inbound,
    Outbound,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowState {
    Open,
    AwaitingApproval,
    WaitingExternal,
    Blocked,
    Done,
}

impl WorkflowState {
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineEntryKind {
    Activity,
    Note,
    Document,
    Communication,
    Fact,
    Audit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectDefinitionKind {
    Standard,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    Text,
    LongText,
    Number,
    Currency,
    Boolean,
    Date,
    DateTime,
    Email,
    Phone,
    Url,
    Select,
    MultiSelect,
    Relation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipCardinality {
    OneToOne,
    OneToMany,
    ManyToMany,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewLayout {
    Table,
    Kanban,
    Calendar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub external_key: Option<String>,
    pub website: Option<String>,
    pub industry: Option<String>,
    pub lifecycle: OrganizationLifecycle,
    pub owner_user_id: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    pub id: Uuid,
    pub organization_id: Option<Uuid>,
    pub full_name: String,
    pub title: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub linkedin_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relationship {
    pub id: Uuid,
    pub from: RecordRef,
    pub to: RecordRef,
    pub relationship_type: RelationshipType,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lead {
    pub id: Uuid,
    pub organization_id: Option<Uuid>,
    pub contact_id: Option<Uuid>,
    pub source: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opportunity {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub primary_contact_id: Option<Uuid>,
    pub name: String,
    pub stage: OpportunityStage,
    pub value: Money,
    pub confidence_bps: u16,
    pub next_step: Option<String>,
    pub expected_close_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    pub subject: String,
    pub related_to: Vec<RecordRef>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activity {
    pub id: Uuid,
    pub subject: String,
    pub details: String,
    pub actor: Actor,
    pub related_to: Vec<RecordRef>,
    pub outcome: ActivityOutcome,
    pub occurred_at: DateTime<Utc>,
    pub next_action_due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub owner_user_id: Option<String>,
    pub related_to: Vec<RecordRef>,
    pub due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: Uuid,
    pub subject: String,
    pub body: String,
    pub author: Actor,
    pub related_to: Vec<RecordRef>,
    pub promoted_to_fact: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferQuote {
    pub id: Uuid,
    pub opportunity_id: Uuid,
    pub title: String,
    pub status: String,
    pub total: Money,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderSubscription {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub quote_id: Option<Uuid>,
    pub status: String,
    pub value: Money,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub title: String,
    pub media_type: String,
    pub uri: String,
    pub status: DocumentStatus,
    pub uploaded_by: Actor,
    pub related_to: Vec<RecordRef>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    pub id: Uuid,
    pub statement: String,
    pub confidence_bps: u16,
    pub promoted_by: Actor,
    pub source_note_id: Option<Uuid>,
    pub related_to: Vec<RecordRef>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intent {
    pub id: Uuid,
    pub job: String,
    pub desired_outcome: String,
    pub guardrails: Vec<String>,
    pub owner: Actor,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCase {
    pub id: Uuid,
    pub title: String,
    pub state: WorkflowState,
    pub priority: WorkflowPriority,
    pub owner_user_id: Option<String>,
    pub related_to: Vec<RecordRef>,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunicationEvent {
    pub id: Uuid,
    pub channel: CommunicationChannel,
    pub direction: CommunicationDirection,
    pub subject: Option<String>,
    pub summary: String,
    pub counterpart: String,
    pub actor: Actor,
    pub related_to: Vec<RecordRef>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionGrant {
    pub id: Uuid,
    pub subject: String,
    pub role: String,
    pub scope: String,
    pub granted_by: Actor,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub id: Uuid,
    pub key: String,
    pub label: String,
    pub field_type: FieldType,
    pub required: bool,
    pub options: Vec<String>,
    pub relation_object_key: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipDefinition {
    pub id: Uuid,
    pub target_object_key: String,
    pub cardinality: RelationshipCardinality,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectDefinition {
    pub id: Uuid,
    pub key: String,
    pub display_name: String,
    pub kind: ObjectDefinitionKind,
    pub fields: Vec<FieldDefinition>,
    pub relationships: Vec<RelationshipDefinition>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewDefinition {
    pub id: Uuid,
    pub object_key: String,
    pub name: String,
    pub layout: ViewLayout,
    pub filter_expression: Option<String>,
    pub sort_expression: Option<String>,
    pub visible_fields: Vec<String>,
    pub group_by: Option<String>,
    pub favorite: bool,
    pub owner_user_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMember {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: String,
    pub display_name: String,
    pub role_ids: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    pub id: Uuid,
    pub key: String,
    pub display_name: String,
    pub permissions: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub title: String,
    pub state: String,
    pub intent_id: Option<Uuid>,
    pub workflow_case_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedFact {
    pub id: Uuid,
    pub statement: String,
    pub confidence_bps: u16,
    pub proposed_by: Actor,
    pub related_to: Vec<RecordRef>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub id: Uuid,
    pub record: RecordRef,
    pub status: ApprovalStatus,
    pub requested_by: Actor,
    pub approver_user_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: Uuid,
    pub job_id: Uuid,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    pub id: Uuid,
    pub key: String,
    pub description: String,
    pub invariant: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub id: Uuid,
    pub key: String,
    pub trigger: String,
    pub actions: Vec<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: Uuid,
    pub workflow_definition_id: Uuid,
    pub record: RecordRef,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    pub action: String,
    pub record: Option<RecordRef>,
    pub actor: Actor,
    pub detail: BTreeMap<String, String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub id: Uuid,
    pub kind: TimelineEntryKind,
    pub anchor: Option<RecordRef>,
    pub headline: String,
    pub body: String,
    pub actor: Actor,
    pub occurred_at: DateTime<Utc>,
    pub related_to: Vec<RecordRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSummary {
    pub organization: Organization,
    pub contacts: Vec<Person>,
    pub opportunities: Vec<Opportunity>,
    pub workflow_cases: Vec<WorkflowCase>,
    pub facts: Vec<Fact>,
    pub documents: Vec<Document>,
    pub permissions: Vec<PermissionGrant>,
    pub recent_timeline: Vec<TimelineEntry>,
}
