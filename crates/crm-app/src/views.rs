use chrono::{DateTime, Utc};
use crm_kernel::{
    ApprovalStatus, OpportunityStage, OrganizationLifecycle, RecordKind, SubscriptionStatus,
    TimelineEntryKind, WorkflowPriority, WorkflowState,
};
use crm_storage::{AppConfig, RuntimeModuleConfig};
use prio_module_core::CapabilityModule;
use prio_truths::TruthKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct OperatorDashboard {
    pub jobs: Vec<TruthListItem>,
    pub approvals: Vec<ApprovalListItem>,
    pub exceptions: Vec<WorkflowCaseListItem>,
    pub recent_timeline: Vec<TimelineEventItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemProfile {
    pub config: AppConfig,
    pub modules: Vec<CapabilityModule>,
    pub feature_toggles: FeatureToggles,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureToggles {
    pub analytics_enabled: bool,
    pub optimization_enabled: bool,
    pub llm_enabled: bool,
    pub runtime_modules: Vec<RuntimeModuleConfig>,
    pub supported_truth_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TruthListItem {
    pub key: String,
    pub display_name: String,
    pub kind: TruthKind,
    pub summary: String,
    pub packs: Vec<String>,
    pub executable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    Idle,
    Running,
    Completed,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionStatus {
    Met,
    Unmet,
    Indeterminate,
    Blocked,
}

#[derive(Debug, Clone, Serialize)]
pub struct TruthExecutionSession {
    pub truth_key: String,
    pub state: ExecutionState,
    pub result: Option<TruthExecutionResult>,
    pub criteria_outcomes: Vec<CriteriaOutcomeItem>,
    pub projection: Option<TruthExecutionProjection>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TruthExecutionResult {
    pub converged: bool,
    pub cycles: u32,
    pub stop_reason: String,
    pub experience_event_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CriteriaOutcomeItem {
    pub criterion_id: String,
    pub description: String,
    pub required: bool,
    pub status: CriterionStatus,
    pub detail: Option<String>,
    pub approval_ref: Option<String>,
    pub evidence_fact_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TruthExecutionProjection {
    pub organization_id: Option<String>,
    pub person_id: Option<String>,
    pub opportunity_id: Option<String>,
    pub subscription_id: Option<String>,
    pub workflow_case_ids: Vec<String>,
    pub approval_ids: Vec<String>,
    pub fact_ids: Vec<String>,
    pub document_ids: Vec<String>,
    pub entitlement_ids: Vec<String>,
    pub projected_event_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApprovalListItem {
    pub id: String,
    pub truth_key: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub status: ApprovalStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowCaseListItem {
    pub id: String,
    pub definition_key: String,
    pub title: String,
    pub state: WorkflowState,
    pub related_to: Vec<RecordReferenceItem>,
    pub created_at: DateTime<Utc>,
    pub priority: WorkflowPriority,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrganizationListItem {
    pub id: String,
    pub name: String,
    pub lifecycle: OrganizationLifecycle,
    pub website: Option<String>,
    pub owner_user_id: Option<String>,
    pub tags: Vec<String>,
    pub people_count: usize,
    pub open_opportunity_count: usize,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpportunityListItem {
    pub id: String,
    pub organization_id: String,
    pub organization_name: String,
    pub name: String,
    pub stage: OpportunityStage,
    pub value_minor: i64,
    pub currency_code: String,
    pub confidence_bps: u16,
    pub next_step: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogItemListItem {
    pub id: String,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub plan_kind: String,
    pub active: bool,
    pub billing_period: Option<String>,
    pub price_minor: Option<i64>,
    pub currency_code: Option<String>,
    pub entitlements_summary: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountWorkspaceSummary {
    pub organization: OrganizationWorkspaceItem,
    pub people: Vec<PersonWorkspaceItem>,
    pub opportunities: Vec<OpportunityListItem>,
    pub subscriptions: Vec<SubscriptionListItem>,
    pub entitlements: Vec<EntitlementListItem>,
    pub recent_timeline: Vec<TimelineEventItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrganizationWorkspaceItem {
    pub id: String,
    pub name: String,
    pub lifecycle: OrganizationLifecycle,
    pub website: Option<String>,
    pub industry: Option<String>,
    pub owner_user_id: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonWorkspaceItem {
    pub id: String,
    pub full_name: String,
    pub title: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionListItem {
    pub id: String,
    pub organization_id: String,
    pub organization_name: String,
    pub status: SubscriptionStatus,
    pub catalog_item_id: Option<String>,
    pub catalog_item_name: Option<String>,
    pub value_minor: i64,
    pub currency_code: String,
    pub started_at: DateTime<Utc>,
    pub activated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntitlementListItem {
    pub id: String,
    pub subscription_id: String,
    pub key: String,
    pub value_summary: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineEventItem {
    pub id: String,
    pub kind: TimelineEntryKind,
    pub summary: String,
    pub actor: String,
    pub timestamp: DateTime<Utc>,
    pub related_to: Vec<RecordReferenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordReferenceItem {
    pub kind: RecordKind,
    pub record_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowCaseFilter {
    pub states: Vec<WorkflowState>,
    pub related_record_id: Option<String>,
    pub definition_key: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApprovalFilter {
    pub status: Option<ApprovalStatus>,
    pub truth_key: Option<String>,
}
