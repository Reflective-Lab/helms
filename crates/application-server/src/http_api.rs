use std::collections::{HashMap, hash_map::Entry};
use std::sync::{Arc, Mutex};

use application_kernel::{
    AccountSummary as KernelAccountSummary, Actor, ActorKind, CatalogItem, Document, Fact,
    Opportunity, OrderSubscription, Organization, PermissionGrant, Person, RecordKind, RecordRef,
    TimelineEntry, WorkflowCase, WorkflowState,
};
use application_storage::{AppConfig, AppRuntimeStores, InMemoryKernelStore, KernelStore};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use capability_core::CapabilityModule;
use capability_registry::all_modules;
use truth_catalog::{TruthDefinition, TruthKind, TruthModuleTouch, all_truths, find_truth};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use workbench_backend::{
    AccountWorkspaceSummary, ApprovalFilter, ApprovalListItem, CatalogItemListItem, OperatorApp,
    OperatorAppError, OperatorDashboard, OpportunityListItem, OrganizationListItem,
    SubscriptionListItem, SystemProfile as WorkbenchSystemProfile, TruthDetailItem,
    TruthExecutionSession, TruthListItem, WorkbenchAppManifest, WorkflowCaseFilter,
    WorkflowCaseListItem,
};

use crate::truth_runtime::{TruthExecutionArtifacts, TruthProjection, execute_truth};

#[derive(Clone)]
pub struct HttpState<S = InMemoryKernelStore> {
    pub config: AppConfig,
    pub store: S,
    pub runtime_stores: AppRuntimeStores,
    pub operator: OperatorApp<S>,
    billing_ingress_token: Option<String>,
    processed_billing_events: Arc<Mutex<HashMap<String, ProcessedBillingEvent>>>,
}

impl<S> HttpState<S> {
    #[must_use]
    pub fn new(
        config: AppConfig,
        store: S,
        runtime_stores: AppRuntimeStores,
        billing_ingress_token: Option<String>,
    ) -> Self
    where
        S: KernelStore + Clone,
    {
        Self {
            operator: OperatorApp::new(config.clone(), store.clone()),
            config,
            store,
            runtime_stores,
            billing_ingress_token,
            processed_billing_events: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthPayload {
    status: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemProfilePayload {
    config: AppConfig,
    modules: Vec<CapabilityModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationSummaryPayload {
    organization: Organization,
    contacts: Vec<Person>,
    opportunities: Vec<Opportunity>,
    subscriptions: Vec<OrderSubscription>,
    workflow_cases: Vec<WorkflowCase>,
    facts: Vec<Fact>,
    documents: Vec<Document>,
    permissions: Vec<PermissionGrant>,
    recent_timeline: Vec<TimelineEntry>,
}

impl OrganizationSummaryPayload {
    fn from_summary(summary: KernelAccountSummary, subscriptions: Vec<OrderSubscription>) -> Self {
        Self {
            organization: summary.organization,
            contacts: summary.contacts,
            opportunities: summary.opportunities,
            subscriptions,
            workflow_cases: summary.workflow_cases,
            facts: summary.facts,
            documents: summary.documents,
            permissions: summary.permissions,
            recent_timeline: summary.recent_timeline,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TruthCatalogItem {
    key: &'static str,
    display_name: &'static str,
    kind: TruthKind,
    summary: &'static str,
    feature_path: &'static str,
    actor_roles: &'static [&'static str],
    approval_points: &'static [&'static str],
    desired_outcomes: &'static [&'static str],
    guardrails: &'static [&'static str],
    modules: &'static [TruthModuleTouch],
    #[serde(skip_serializing_if = "Option::is_none")]
    gherkin: Option<&'static str>,
}

impl TruthCatalogItem {
    fn from_truth(truth: TruthDefinition, include_gherkin: bool) -> Self {
        Self {
            key: truth.key,
            display_name: truth.display_name,
            kind: truth.kind,
            summary: truth.summary,
            feature_path: truth.feature_path,
            actor_roles: truth.actor_roles,
            approval_points: truth.approval_points,
            desired_outcomes: truth.desired_outcomes,
            guardrails: truth.guardrails,
            modules: truth.modules,
            gherkin: include_gherkin.then_some(truth.gherkin),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct TruthExecutionResponse {
    truth: TruthCatalogItem,
    execution: BillingExecutionSummary,
    projection: Option<BillingProjectionSummary>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct OrganizationSummaryQuery {
    timeline_limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct SubscriptionsQuery {
    organization_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct CatalogQuery {
    active_only: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct WorkflowCasesQuery {
    state: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TimelineQuery {
    organization_id: Option<String>,
    anchor_kind: Option<String>,
    anchor_id: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TruthsQuery {
    kind: Option<String>,
    module_key: Option<String>,
    include_gherkin: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ExecuteTruthRequest {
    #[serde(default)]
    inputs: HashMap<String, String>,
    #[serde(default)]
    actor: Option<Actor>,
    #[serde(default)]
    persist_projection: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ExecuteWorkbenchTruthRequest {
    #[serde(default)]
    inputs: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BillingEventKind {
    PrepaidTopUpSettled,
    SubscriptionActivationRequested,
    SubscriptionPaymentFailed,
    LedgerReconciliationRequested,
}

impl BillingEventKind {
    fn truth_key(&self) -> &'static str {
        match self {
            Self::PrepaidTopUpSettled => "refill-prepaid-ai-credits",
            Self::SubscriptionActivationRequested => "activate-subscription",
            Self::SubscriptionPaymentFailed => "suspend-service-on-payment-failure",
            Self::LedgerReconciliationRequested => "reconcile-model-usage-against-customer-ledger",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingIngressRequest {
    pub source: String,
    pub event_id: String,
    pub event_kind: BillingEventKind,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub subscription_id: Option<String>,
    #[serde(default)]
    pub catalog_item_id: Option<String>,
    #[serde(default)]
    pub payment_reference: Option<String>,
    #[serde(default)]
    pub amount_minor: Option<i64>,
    #[serde(default)]
    pub opening_balance_minor: Option<i64>,
    #[serde(default)]
    pub currency_code: Option<String>,
    #[serde(default)]
    pub payment_status: Option<String>,
    #[serde(default)]
    pub risk_signal: Option<bool>,
    #[serde(default)]
    pub days_overdue: Option<i64>,
    #[serde(default)]
    pub grace_days: Option<i64>,
    #[serde(default)]
    pub strategic_account: Option<bool>,
    #[serde(default)]
    pub usage_burn_minor: Option<i64>,
    #[serde(default)]
    pub provider_settled_minor: Option<i64>,
    #[serde(default)]
    pub provider_reference: Option<String>,
    #[serde(default)]
    pub provider_name: Option<String>,
    #[serde(default)]
    pub provider_status: Option<String>,
    #[serde(default)]
    pub threshold_minor: Option<i64>,
    #[serde(default)]
    pub force_manual_review: Option<bool>,
    #[serde(default)]
    pub manual_review_reason: Option<String>,
    #[serde(default)]
    pub persist_projection: Option<bool>,
    #[serde(default)]
    pub attributes: HashMap<String, String>,
}

/// A shared normalized event type used for communicating between external systems and the CRM billing module.
/// This type can be shared with Wolfgang or upstream converge-runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedBillingEvent {
    pub source: String,
    pub event_id: String,
    pub event_kind: BillingEventKind,
    pub truth_key: String,
    pub idempotency_key: String,
    pub inputs: HashMap<String, String>,
    pub persist_projection: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingIngressResponse {
    source: String,
    event_id: String,
    event_kind: BillingEventKind,
    truth_key: String,
    idempotency_key: String,
    duplicate: bool,
    in_flight: bool,
    execution: Option<BillingExecutionSummary>,
    projection: Option<BillingProjectionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BillingExecutionSummary {
    converged: bool,
    cycles: u32,
    stop_reason: String,
    criteria: Vec<BillingCriterionSummary>,
    experience_event_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BillingCriterionSummary {
    criterion_id: String,
    description: String,
    required: bool,
    status: BillingCriterionStatus,
    evidence_fact_ids: Vec<String>,
    detail: Option<String>,
    approval_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BillingCriterionStatus {
    Met,
    Unmet,
    Indeterminate,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BillingProjectionSummary {
    persisted: bool,
    organization_id: Option<String>,
    person_id: Option<String>,
    opportunity_id: Option<String>,
    subscription_id: Option<String>,
    workflow_case_ids: Vec<String>,
    document_ids: Vec<String>,
    fact_ids: Vec<String>,
    entitlement_ids: Vec<String>,
    ledger_entry_ids: Vec<String>,
    projected_event_kinds: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ErrorPayload {
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorPayload {
                error: self.message,
            }),
        )
            .into_response()
    }
}

#[derive(Debug, Clone)]
enum ProcessedBillingEvent {
    InFlight {
        source: String,
        event_id: String,
        event_kind: BillingEventKind,
        truth_key: String,
        idempotency_key: String,
    },
    Completed(BillingIngressResponse),
}

pub fn app_router<S>(state: HttpState<S>) -> Router
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/health", get(health::<S>))
        .route("/v1/system/profile", get(system_profile::<S>))
        .route(
            "/v1/workbench/system/profile",
            get(workbench_system_profile::<S>),
        )
        .route("/v1/workbench/dashboard", get(workbench_dashboard::<S>))
        .route("/v1/workbench/apps", get(list_workbench_apps::<S>))
        .route("/v1/workbench/truths", get(list_workbench_truths::<S>))
        .route(
            "/v1/workbench/truths/{key}",
            get(get_workbench_truth_detail::<S>),
        )
        .route(
            "/v1/workbench/truths/{key}/execute",
            post(execute_workbench_truth::<S>),
        )
        .route(
            "/v1/workbench/organizations",
            get(list_workbench_organizations::<S>),
        )
        .route(
            "/v1/workbench/organizations/{id}/summary",
            get(get_workbench_account_summary::<S>),
        )
        .route(
            "/v1/workbench/opportunities",
            get(list_workbench_opportunities::<S>),
        )
        .route(
            "/v1/workbench/subscriptions",
            get(list_workbench_subscriptions::<S>),
        )
        .route("/v1/workbench/catalog", get(list_workbench_catalog::<S>))
        .route(
            "/v1/workbench/workflow/cases",
            get(list_workbench_workflow_cases::<S>),
        )
        .route(
            "/v1/workbench/approvals",
            get(list_workbench_approvals::<S>),
        )
        .route("/v1/organizations", get(list_organizations::<S>))
        .route(
            "/v1/organizations/{id}/summary",
            get(get_organization_summary::<S>),
        )
        .route("/v1/subscriptions", get(list_subscriptions::<S>))
        .route("/v1/catalog", get(list_catalog::<S>))
        .route("/v1/workflow/cases", get(list_workflow_cases::<S>))
        .route("/v1/timeline", get(list_timeline::<S>))
        .route("/v1/truths", get(list_truths::<S>))
        .route("/v1/truths/{key}/execute", post(execute_truth_http::<S>))
        .route(
            "/v1/integrations/billing/events",
            post(handle_billing_event::<S>),
        )
        .with_state(state)
}

async fn health<S>(_state: State<HttpState<S>>) -> Json<HealthPayload> {
    Json(HealthPayload { status: "ok" })
}

async fn system_profile<S>(State(state): State<HttpState<S>>) -> Json<SystemProfilePayload> {
    Json(SystemProfilePayload {
        config: state.config,
        modules: all_modules(),
    })
}

async fn workbench_system_profile<S>(
    State(state): State<HttpState<S>>,
) -> Json<WorkbenchSystemProfile>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    Json(state.operator.system_profile())
}

async fn workbench_dashboard<S>(
    State(state): State<HttpState<S>>,
) -> Result<Json<OperatorDashboard>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    state
        .operator
        .operator_dashboard()
        .map(Json)
        .map_err(api_error_from_operator)
}

async fn list_workbench_apps<S>(
    State(state): State<HttpState<S>>,
) -> Json<Vec<WorkbenchAppManifest>>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    Json(state.operator.workbench_apps())
}

async fn list_workbench_truths<S>(State(state): State<HttpState<S>>) -> Json<Vec<TruthListItem>>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    Json(state.operator.list_truths())
}

async fn get_workbench_truth_detail<S>(
    State(state): State<HttpState<S>>,
    Path(key): Path<String>,
) -> Result<Json<TruthDetailItem>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    state
        .operator
        .truth_detail(&key)
        .map(Json)
        .ok_or_else(|| api_error_from_operator(OperatorAppError::TruthNotFound(key)))
}

async fn execute_workbench_truth<S>(
    State(state): State<HttpState<S>>,
    Path(key): Path<String>,
    Json(request): Json<ExecuteWorkbenchTruthRequest>,
) -> Result<Json<TruthExecutionSession>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    state
        .operator
        .execute_truth(&key, request.inputs)
        .map(Json)
        .map_err(api_error_from_operator)
}

async fn list_workbench_organizations<S>(
    State(state): State<HttpState<S>>,
) -> Result<Json<Vec<OrganizationListItem>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    state
        .operator
        .list_organizations()
        .map(Json)
        .map_err(api_error_from_operator)
}

async fn get_workbench_account_summary<S>(
    State(state): State<HttpState<S>>,
    Path(id): Path<String>,
) -> Result<Json<AccountWorkspaceSummary>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    state
        .operator
        .account_summary(&id)
        .map(Json)
        .map_err(api_error_from_operator)
}

async fn list_workbench_opportunities<S>(
    State(state): State<HttpState<S>>,
) -> Result<Json<Vec<OpportunityListItem>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    state
        .operator
        .list_opportunities()
        .map(Json)
        .map_err(api_error_from_operator)
}

async fn list_workbench_subscriptions<S>(
    State(state): State<HttpState<S>>,
    Query(query): Query<SubscriptionsQuery>,
) -> Result<Json<Vec<SubscriptionListItem>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    state
        .operator
        .list_subscriptions(query.organization_id.as_deref())
        .map(Json)
        .map_err(api_error_from_operator)
}

async fn list_workbench_catalog<S>(
    State(state): State<HttpState<S>>,
    Query(query): Query<CatalogQuery>,
) -> Result<Json<Vec<CatalogItemListItem>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    state
        .operator
        .list_catalog_items(query.active_only.unwrap_or(false))
        .map(Json)
        .map_err(api_error_from_operator)
}

async fn list_workbench_workflow_cases<S>(
    State(state): State<HttpState<S>>,
    Query(filter): Query<WorkflowCaseFilter>,
) -> Result<Json<Vec<WorkflowCaseListItem>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    state
        .operator
        .list_workflow_cases(filter)
        .map(Json)
        .map_err(api_error_from_operator)
}

async fn list_workbench_approvals<S>(
    State(state): State<HttpState<S>>,
    Query(filter): Query<ApprovalFilter>,
) -> Result<Json<Vec<ApprovalListItem>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    state
        .operator
        .list_approvals(filter)
        .map(Json)
        .map_err(api_error_from_operator)
}

async fn list_organizations<S>(
    State(state): State<HttpState<S>>,
) -> Result<Json<Vec<Organization>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let organizations = state
        .store
        .read(|kernel| kernel.list_organizations())
        .map_err(api_error_from_storage)?;
    Ok(Json(organizations))
}

async fn get_organization_summary<S>(
    State(state): State<HttpState<S>>,
    Path(id): Path<String>,
    Query(query): Query<OrganizationSummaryQuery>,
) -> Result<Json<OrganizationSummaryPayload>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let organization_id = parse_uuid_field(&id, "id")?;
    let timeline_limit = query.timeline_limit.unwrap_or(25);
    let (summary, subscriptions) = state
        .store
        .read(|kernel| {
            kernel
                .get_account_summary(organization_id, timeline_limit)
                .map(|summary| (summary, kernel.list_subscriptions(Some(organization_id))))
        })
        .map_err(api_error_from_storage)?
        .map_err(api_error_from_kernel)?;
    Ok(Json(OrganizationSummaryPayload::from_summary(
        summary,
        subscriptions,
    )))
}

async fn list_subscriptions<S>(
    State(state): State<HttpState<S>>,
    Query(query): Query<SubscriptionsQuery>,
) -> Result<Json<Vec<OrderSubscription>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let organization_id = query
        .organization_id
        .as_deref()
        .map(|value| parse_uuid_field(value, "organization_id"))
        .transpose()?;
    let subscriptions = state
        .store
        .read(|kernel| kernel.list_subscriptions(organization_id))
        .map_err(api_error_from_storage)?;
    Ok(Json(subscriptions))
}

async fn list_catalog<S>(
    State(state): State<HttpState<S>>,
    Query(query): Query<CatalogQuery>,
) -> Result<Json<Vec<CatalogItem>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let items = state
        .store
        .read(|kernel| kernel.list_catalog_items(query.active_only.unwrap_or(false)))
        .map_err(api_error_from_storage)?;
    Ok(Json(items))
}

async fn list_workflow_cases<S>(
    State(state): State<HttpState<S>>,
    Query(query): Query<WorkflowCasesQuery>,
) -> Result<Json<Vec<WorkflowCase>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let state_filter = query
        .state
        .as_deref()
        .map(parse_workflow_state)
        .transpose()?;
    let cases = state
        .store
        .read(|kernel| kernel.list_workflow_cases(state_filter))
        .map_err(api_error_from_storage)?;
    Ok(Json(cases))
}

async fn list_timeline<S>(
    State(state): State<HttpState<S>>,
    Query(query): Query<TimelineQuery>,
) -> Result<Json<Vec<TimelineEntry>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let limit = query.limit.unwrap_or(50);
    let anchor = if let Some(organization_id) = query.organization_id.as_deref() {
        RecordRef {
            kind: RecordKind::Organization,
            id: parse_uuid_field(organization_id, "organization_id")?,
        }
    } else {
        let anchor_kind = query
            .anchor_kind
            .as_deref()
            .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "anchor_kind is required"))?;
        let anchor_id = query
            .anchor_id
            .as_deref()
            .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "anchor_id is required"))?;
        RecordRef {
            kind: parse_record_kind(anchor_kind)?,
            id: parse_uuid_field(anchor_id, "anchor_id")?,
        }
    };

    let timeline = state
        .store
        .read(|kernel| kernel.list_timeline(&[anchor], limit))
        .map_err(api_error_from_storage)?;
    Ok(Json(timeline))
}

async fn list_truths<S>(
    State(_state): State<HttpState<S>>,
    Query(query): Query<TruthsQuery>,
) -> Result<Json<Vec<TruthCatalogItem>>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let kind_filter = query.kind.as_deref().map(parse_truth_kind).transpose()?;
    let module_filter = query.module_key.as_deref().map(str::trim).unwrap_or("");
    let include_gherkin = query.include_gherkin.unwrap_or(false);

    let mut truths = all_truths()
        .into_iter()
        .filter(|truth| kind_filter.is_none_or(|kind| truth.kind == kind))
        .filter(|truth| {
            module_filter.is_empty()
                || truth
                    .modules
                    .iter()
                    .any(|touch| touch.module_key == module_filter)
        })
        .map(|truth| TruthCatalogItem::from_truth(truth, include_gherkin))
        .collect::<Vec<_>>();
    truths.sort_by(|left, right| left.key.cmp(right.key));
    Ok(Json(truths))
}

async fn execute_truth_http<S>(
    State(state): State<HttpState<S>>,
    Path(key): Path<String>,
    Json(request): Json<ExecuteTruthRequest>,
) -> Result<Json<TruthExecutionResponse>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    let truth_key = key.trim();
    if truth_key.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "truth key is required",
        ));
    }

    let truth = find_truth(truth_key).ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            format!("truth not found: {truth_key}"),
        )
    })?;
    let execution = execute_truth(
        &state.store,
        &state.runtime_stores,
        truth_key,
        request.inputs,
        request.actor.unwrap_or_else(Actor::system),
        request.persist_projection.unwrap_or(true),
    ).await.map_err(api_error_from_tonic)?;

    Ok(Json(TruthExecutionResponse {
        truth: TruthCatalogItem::from_truth(truth, false),
        execution: execution_summary(&execution),
        projection: execution.projection.map(projection_summary),
    }))
}

async fn handle_billing_event<S>(
    State(state): State<HttpState<S>>,
    headers: HeaderMap,
    Json(request): Json<BillingIngressRequest>,
) -> Result<Json<BillingIngressResponse>, ApiError>
where
    S: KernelStore + Clone + Send + Sync + 'static,
{
    authorize_billing_ingress(&state, &headers)?;
    let normalized = normalize_billing_event(request)?;

    {
        let mut processed = state.processed_billing_events.lock().map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "billing ingress lock poisoned",
            )
        })?;
        match processed.entry(normalized.idempotency_key.clone()) {
            Entry::Occupied(entry) => match entry.get() {
                ProcessedBillingEvent::Completed(response) => {
                    let mut duplicate = response.clone();
                    duplicate.duplicate = true;
                    return Ok(Json(duplicate));
                }
                ProcessedBillingEvent::InFlight {
                    source,
                    event_id,
                    event_kind,
                    truth_key,
                    idempotency_key,
                } => {
                    return Ok(Json(BillingIngressResponse {
                        source: source.clone(),
                        event_id: event_id.clone(),
                        event_kind: event_kind.clone(),
                        truth_key: truth_key.clone(),
                        idempotency_key: idempotency_key.clone(),
                        duplicate: true,
                        in_flight: true,
                        execution: None,
                        projection: None,
                    }));
                }
            },
            Entry::Vacant(vacant) => {
                vacant.insert(ProcessedBillingEvent::InFlight {
                    source: normalized.source.clone(),
                    event_id: normalized.event_id.clone(),
                    event_kind: normalized.event_kind.clone(),
                    truth_key: normalized.truth_key.clone(),
                    idempotency_key: normalized.idempotency_key.clone(),
                });
            }
        }
    }

    let actor = Actor {
        actor_id: format!("billing-ingress:{}", normalized.source),
        display_name: format!("{} billing ingress", normalized.source),
        kind: ActorKind::System,
    };

    let response = match execute_truth(
        &state.store,
        &state.runtime_stores,
        &normalized.truth_key,
        normalized.inputs,
        actor,
        normalized.persist_projection,
    )
    .await
    {
        Ok(execution) => billing_response_from_execution(
            normalized.source.clone(),
            normalized.event_id.clone(),
            normalized.event_kind.clone(),
            &normalized.truth_key,
            normalized.idempotency_key.clone(),
            execution,
        ),
        Err(status) => {
            let mut processed = state.processed_billing_events.lock().map_err(|_| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "billing ingress lock poisoned",
                )
            })?;
            processed.remove(&normalized.idempotency_key);
            return Err(api_error_from_tonic(status));
        }
    };

    let mut processed = state.processed_billing_events.lock().map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "billing ingress lock poisoned",
        )
    })?;
    processed.insert(
        normalized.idempotency_key.clone(),
        ProcessedBillingEvent::Completed(response.clone()),
    );

    Ok(Json(response))
}

fn authorize_billing_ingress<S>(state: &HttpState<S>, headers: &HeaderMap) -> Result<(), ApiError> {
    let expected = state.billing_ingress_token.as_deref().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "billing ingress is disabled; CRM_BILLING_INGRESS_TOKEN is not configured",
        )
    })?;

    let provided = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "missing bearer token for billing ingress",
            )
        })?;

    if !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid bearer token for billing ingress",
        ));
    }

    Ok(())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn normalize_billing_event(
    request: BillingIngressRequest,
) -> Result<NormalizedBillingEvent, ApiError> {
    let source = required_trimmed(&request.source, "source")?.to_string();
    let event_id = required_trimmed(&request.event_id, "event_id")?.to_string();
    let event_kind = request.event_kind.clone();
    let truth_key = event_kind.truth_key();
    let idempotency_key = request
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{source}:{truth_key}:{event_id}"));

    let mut inputs = HashMap::new();
    let persist_projection = request.persist_projection.unwrap_or(true);

    match event_kind {
        BillingEventKind::PrepaidTopUpSettled => {
            insert_required(
                &mut inputs,
                "subscription_id",
                request.subscription_id.as_deref(),
                "subscription_id",
            )?;
            insert_i64(
                &mut inputs,
                "top_up_amount_minor",
                request.amount_minor,
                "amount_minor",
            )?;
            insert_string(
                &mut inputs,
                "payment_reference",
                request
                    .payment_reference
                    .as_deref()
                    .unwrap_or(event_id.as_str()),
            );
            insert_string(
                &mut inputs,
                "payment_status",
                request.payment_status.as_deref().unwrap_or("confirmed"),
            );
            insert_optional_bool(&mut inputs, "risk_signal", request.risk_signal);
            insert_optional_bool(
                &mut inputs,
                "force_manual_review",
                request.force_manual_review,
            );
            insert_optional_string(
                &mut inputs,
                "manual_review_reason",
                request.manual_review_reason.as_deref(),
            );
        }
        BillingEventKind::SubscriptionActivationRequested => {
            insert_required(
                &mut inputs,
                "subscription_id",
                request.subscription_id.as_deref(),
                "subscription_id",
            )?;
            insert_optional_string(
                &mut inputs,
                "catalog_item_id",
                request.catalog_item_id.as_deref(),
            );
            insert_optional_i64(
                &mut inputs,
                "opening_balance_minor",
                request.opening_balance_minor,
            );
            insert_optional_string(
                &mut inputs,
                "opening_balance_currency_code",
                request.currency_code.as_deref(),
            );
            insert_optional_bool(
                &mut inputs,
                "force_manual_review",
                request.force_manual_review,
            );
            insert_optional_string(
                &mut inputs,
                "manual_review_reason",
                request.manual_review_reason.as_deref(),
            );
        }
        BillingEventKind::SubscriptionPaymentFailed => {
            insert_required(
                &mut inputs,
                "subscription_id",
                request.subscription_id.as_deref(),
                "subscription_id",
            )?;
            insert_string(
                &mut inputs,
                "payment_status",
                request.payment_status.as_deref().unwrap_or("failed"),
            );
            insert_optional_i64(&mut inputs, "days_overdue", request.days_overdue);
            insert_optional_i64(&mut inputs, "grace_days", request.grace_days);
            insert_optional_bool(&mut inputs, "strategic_account", request.strategic_account);
            insert_optional_bool(
                &mut inputs,
                "force_manual_review",
                request.force_manual_review,
            );
            insert_optional_string(
                &mut inputs,
                "manual_review_reason",
                request.manual_review_reason.as_deref(),
            );
        }
        BillingEventKind::LedgerReconciliationRequested => {
            insert_required(
                &mut inputs,
                "subscription_id",
                request.subscription_id.as_deref(),
                "subscription_id",
            )?;
            insert_i64(
                &mut inputs,
                "usage_burn_minor",
                request.usage_burn_minor,
                "usage_burn_minor",
            )?;
            insert_i64(
                &mut inputs,
                "provider_settled_minor",
                request.provider_settled_minor,
                "provider_settled_minor",
            )?;
            insert_optional_string(
                &mut inputs,
                "provider_reference",
                request.provider_reference.as_deref(),
            );
            insert_optional_string(
                &mut inputs,
                "provider_name",
                request.provider_name.as_deref(),
            );
            insert_optional_string(
                &mut inputs,
                "provider_status",
                request.provider_status.as_deref(),
            );
            insert_optional_i64(&mut inputs, "threshold_minor", request.threshold_minor);
        }
    }

    if request.attributes.len() > MAX_ATTRIBUTES_COUNT {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            format!(
                "attributes count {} exceeds maximum of {MAX_ATTRIBUTES_COUNT}",
                request.attributes.len()
            ),
        ));
    }
    for (key, value) in request.attributes {
        if key.len() > MAX_STRING_INPUT_LEN || value.len() > MAX_STRING_INPUT_LEN {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                format!("attribute key or value exceeds maximum length of {MAX_STRING_INPUT_LEN}"),
            ));
        }
        inputs.entry(key).or_insert(value);
    }

    Ok(NormalizedBillingEvent {
        source,
        event_id,
        event_kind,
        truth_key: truth_key.to_string(),
        idempotency_key,
        inputs,
        persist_projection,
    })
}

const MAX_STRING_INPUT_LEN: usize = 4_096;
const MAX_ATTRIBUTES_COUNT: usize = 64;

fn required_trimmed<'a>(value: &'a str, field: &str) -> Result<&'a str, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("{field} is required"),
        ))
    } else if trimmed.len() > MAX_STRING_INPUT_LEN {
        Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("{field} exceeds maximum length of {MAX_STRING_INPUT_LEN}"),
        ))
    } else {
        Ok(trimmed)
    }
}

fn insert_required(
    inputs: &mut HashMap<String, String>,
    key: &str,
    value: Option<&str>,
    field: &str,
) -> Result<(), ApiError> {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, format!("{field} is required")))?;
    inputs.insert(key.to_string(), value.to_string());
    Ok(())
}

fn insert_string(inputs: &mut HashMap<String, String>, key: &str, value: &str) {
    inputs.insert(key.to_string(), value.to_string());
}

fn insert_optional_string(inputs: &mut HashMap<String, String>, key: &str, value: Option<&str>) {
    if let Some(value) = value
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= MAX_STRING_INPUT_LEN)
    {
        inputs.insert(key.to_string(), value.to_string());
    }
}

fn insert_i64(
    inputs: &mut HashMap<String, String>,
    key: &str,
    value: Option<i64>,
    field: &str,
) -> Result<(), ApiError> {
    let value = value
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, format!("{field} is required")))?;
    inputs.insert(key.to_string(), value.to_string());
    Ok(())
}

fn insert_optional_i64(inputs: &mut HashMap<String, String>, key: &str, value: Option<i64>) {
    if let Some(value) = value {
        inputs.insert(key.to_string(), value.to_string());
    }
}

fn insert_optional_bool(inputs: &mut HashMap<String, String>, key: &str, value: Option<bool>) {
    if let Some(value) = value {
        inputs.insert(key.to_string(), value.to_string());
    }
}

fn parse_uuid_field(value: &str, field: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value.trim()).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("{field} must be a valid UUID"),
        )
    })
}

fn parse_workflow_state(value: &str) -> Result<WorkflowState, ApiError> {
    match normalize_name(value).as_str() {
        "open" => Ok(WorkflowState::Open),
        "awaiting-approval" => Ok(WorkflowState::AwaitingApproval),
        "waiting-external" => Ok(WorkflowState::WaitingExternal),
        "blocked" => Ok(WorkflowState::Blocked),
        "done" => Ok(WorkflowState::Done),
        _ => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("unsupported workflow state: {value}"),
        )),
    }
}

fn parse_record_kind(value: &str) -> Result<RecordKind, ApiError> {
    match normalize_name(value).as_str() {
        "organization" => Ok(RecordKind::Organization),
        "person" => Ok(RecordKind::Person),
        "relationship" => Ok(RecordKind::Relationship),
        "lead" => Ok(RecordKind::Lead),
        "opportunity" => Ok(RecordKind::Opportunity),
        "conversation" => Ok(RecordKind::Conversation),
        "activity" => Ok(RecordKind::Activity),
        "task" => Ok(RecordKind::Task),
        "offer-quote" => Ok(RecordKind::OfferQuote),
        "order-subscription" | "subscription" => Ok(RecordKind::OrderSubscription),
        "document" => Ok(RecordKind::Document),
        "fact" => Ok(RecordKind::Fact),
        "intent" => Ok(RecordKind::Intent),
        "workflow-case" => Ok(RecordKind::WorkflowCase),
        "communication-event" => Ok(RecordKind::CommunicationEvent),
        "permission-grant" => Ok(RecordKind::PermissionGrant),
        "audit-entry" => Ok(RecordKind::AuditEntry),
        "note" => Ok(RecordKind::Note),
        "catalog-item" => Ok(RecordKind::CatalogItem),
        _ => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("unsupported record kind: {value}"),
        )),
    }
}

fn parse_truth_kind(value: &str) -> Result<TruthKind, ApiError> {
    match normalize_name(value).as_str() {
        "job" => Ok(TruthKind::Job),
        "policy" => Ok(TruthKind::Policy),
        "module-local" => Ok(TruthKind::ModuleLocal),
        _ => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("unsupported truth kind: {value}"),
        )),
    }
}

fn normalize_name(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-")
        .replace(' ', "-")
}

fn billing_response_from_execution(
    source: String,
    event_id: String,
    event_kind: BillingEventKind,
    truth_key: &str,
    idempotency_key: String,
    execution: TruthExecutionArtifacts,
) -> BillingIngressResponse {
    let execution_summary = execution_summary(&execution);
    let projection_summary = execution.projection.map(projection_summary);

    BillingIngressResponse {
        source,
        event_id,
        event_kind,
        truth_key: truth_key.to_string(),
        idempotency_key,
        duplicate: false,
        in_flight: false,
        execution: Some(execution_summary),
        projection: projection_summary,
    }
}

fn execution_summary(execution: &TruthExecutionArtifacts) -> BillingExecutionSummary {
    BillingExecutionSummary {
        converged: execution.result.converged,
        cycles: execution.result.cycles,
        stop_reason: stop_reason_name(&execution.result.stop_reason).to_string(),
        criteria: execution
            .result
            .criteria_outcomes
            .iter()
            .map(|outcome| BillingCriterionSummary {
                criterion_id: outcome.criterion.id.clone(),
                description: outcome.criterion.description.clone(),
                required: outcome.criterion.required,
                status: criterion_status_name(&outcome.result),
                evidence_fact_ids: match &outcome.result {
                    converge_core::CriterionResult::Met { evidence } => {
                        evidence.iter().map(ToString::to_string).collect()
                    }
                    _ => Vec::new(),
                },
                detail: match &outcome.result {
                    converge_core::CriterionResult::Blocked { reason, .. }
                    | converge_core::CriterionResult::Unmet { reason } => Some(reason.clone()),
                    converge_core::CriterionResult::Met { .. }
                    | converge_core::CriterionResult::Indeterminate => None,
                },
                approval_ref: match &outcome.result {
                    converge_core::CriterionResult::Blocked { approval_ref, .. } => {
                        approval_ref.clone()
                    }
                    _ => None,
                },
            })
            .collect(),
        experience_event_kinds: execution
            .experience_events
            .iter()
            .map(experience_event_kind_name)
            .map(str::to_string)
            .collect(),
    }
}

fn projection_summary(projection: TruthProjection) -> BillingProjectionSummary {
    BillingProjectionSummary {
        persisted: true,
        organization_id: projection.organization.map(|value| value.id.to_string()),
        person_id: projection.person.map(|value| value.id.to_string()),
        opportunity_id: projection.opportunity.map(|value| value.id.to_string()),
        subscription_id: projection.subscription.map(|value| value.id.to_string()),
        workflow_case_ids: projection
            .workflow_cases
            .into_iter()
            .map(|value| value.id.to_string())
            .collect(),
        document_ids: projection
            .documents
            .into_iter()
            .map(|value| value.id.to_string())
            .collect(),
        fact_ids: projection
            .facts
            .into_iter()
            .map(|value| value.id.to_string())
            .collect(),
        entitlement_ids: projection
            .entitlements
            .into_iter()
            .map(|value| value.id.to_string())
            .collect(),
        ledger_entry_ids: projection
            .ledger_entries
            .into_iter()
            .map(|value| value.id.to_string())
            .collect(),
        projected_event_kinds: projection
            .domain_event_kinds
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
    }
}

fn experience_event_kind_name(event: &converge_core::ExperienceEvent) -> &'static str {
    match event.kind() {
        converge_core::ExperienceEventKind::ProposalCreated => "proposal-created",
        converge_core::ExperienceEventKind::ProposalValidated => "proposal-validated",
        converge_core::ExperienceEventKind::FactPromoted => "fact-promoted",
        converge_core::ExperienceEventKind::RecallExecuted => "recall-executed",
        converge_core::ExperienceEventKind::ReplayabilityDowngraded => "replayability-downgraded",
        converge_core::ExperienceEventKind::ArtifactStateTransitioned => {
            "artifact-state-transitioned"
        }
        converge_core::ExperienceEventKind::ArtifactRollbackRecorded => {
            "artifact-rollback-recorded"
        }
        converge_core::ExperienceEventKind::BackendInvoked => "backend-invoked",
        converge_core::ExperienceEventKind::OutcomeRecorded => "outcome-recorded",
        converge_core::ExperienceEventKind::BudgetExceeded => "budget-exceeded",
        converge_core::ExperienceEventKind::PolicySnapshotCaptured => "policy-snapshot-captured",
        converge_core::ExperienceEventKind::ReplayTraceRecorded => "replay-trace-recorded",
        converge_core::ExperienceEventKind::HypothesisResolved => "hypothesis-resolved",
    }
}

fn stop_reason_name(stop_reason: &converge_core::StopReason) -> &'static str {
    match stop_reason {
        converge_core::StopReason::Converged => "converged",
        converge_core::StopReason::CriteriaMet { .. } => "criteria-met",
        converge_core::StopReason::UserCancelled => "user-cancelled",
        converge_core::StopReason::HumanInterventionRequired { .. } => {
            "human-intervention-required"
        }
        converge_core::StopReason::CycleBudgetExhausted { .. } => "cycle-budget-exhausted",
        converge_core::StopReason::FactBudgetExhausted { .. } => "fact-budget-exhausted",
        converge_core::StopReason::TokenBudgetExhausted { .. } => "token-budget-exhausted",
        converge_core::StopReason::TimeBudgetExhausted { .. } => "time-budget-exhausted",
        converge_core::StopReason::InvariantViolated { .. } => "invariant-violated",
        converge_core::StopReason::PromotionRejected { .. } => "promotion-rejected",
        converge_core::StopReason::Error { .. } => "error",
        converge_core::StopReason::AgentRefused { .. } => "agent-refused",
        converge_core::StopReason::HitlGatePending { .. } => "hitl-gate-pending",
        _ => "unknown",
    }
}

fn criterion_status_name(result: &converge_core::CriterionResult) -> BillingCriterionStatus {
    match result {
        converge_core::CriterionResult::Met { .. } => BillingCriterionStatus::Met,
        converge_core::CriterionResult::Unmet { .. } => BillingCriterionStatus::Unmet,
        converge_core::CriterionResult::Indeterminate => BillingCriterionStatus::Indeterminate,
        converge_core::CriterionResult::Blocked { .. } => BillingCriterionStatus::Blocked,
    }
}

fn api_error_from_storage(error: application_storage::StorageError) -> ApiError {
    match error {
        application_storage::StorageError::LockPoisoned => {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "storage lock poisoned")
        }
        application_storage::StorageError::Kernel(error) => api_error_from_kernel(error),
        application_storage::StorageError::ConnectionFailed { backend, message } => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("{backend} connection failed: {message}"),
        ),
        application_storage::StorageError::SerializationFailed { message } => {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, message)
        }
        application_storage::StorageError::Timeout { operation } => {
            ApiError::new(StatusCode::GATEWAY_TIMEOUT, operation)
        }
        application_storage::StorageError::RuntimeStore { message } => {
            ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, message)
        }
    }
}

fn api_error_from_kernel(error: application_kernel::KernelError) -> ApiError {
    match error {
        application_kernel::KernelError::Validation(message) => {
            ApiError::new(StatusCode::BAD_REQUEST, message)
        }
        application_kernel::KernelError::NotFound { kind, id } => {
            ApiError::new(StatusCode::NOT_FOUND, format!("{kind} not found: {id}"))
        }
        application_kernel::KernelError::Invariant(message) => {
            ApiError::new(StatusCode::PRECONDITION_FAILED, message)
        }
        application_kernel::KernelError::Conflict(message) => {
            ApiError::new(StatusCode::CONFLICT, message)
        }
    }
}

fn api_error_from_operator(error: OperatorAppError) -> ApiError {
    match error {
        OperatorAppError::Storage(error) => api_error_from_storage(error),
        OperatorAppError::TruthNotFound(key) => {
            ApiError::new(StatusCode::NOT_FOUND, format!("truth not found: {key}"))
        }
        OperatorAppError::MissingInput(field) => ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("missing required input: {field}"),
        ),
        OperatorAppError::InvalidUuid { field, value } => ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("invalid uuid for {field}: {value}"),
        ),
        OperatorAppError::InvalidInteger { field, value } => ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("invalid integer for {field}: {value}"),
        ),
        OperatorAppError::Validation(message) | OperatorAppError::UnsupportedTruth(message) => {
            ApiError::new(StatusCode::BAD_REQUEST, message)
        }
    }
}

fn api_error_from_tonic(status: tonic::Status) -> ApiError {
    let http_status = match status.code() {
        tonic::Code::InvalidArgument => StatusCode::BAD_REQUEST,
        tonic::Code::NotFound => StatusCode::NOT_FOUND,
        tonic::Code::AlreadyExists => StatusCode::CONFLICT,
        tonic::Code::PermissionDenied => StatusCode::FORBIDDEN,
        tonic::Code::FailedPrecondition => StatusCode::PRECONDITION_FAILED,
        tonic::Code::Aborted => StatusCode::CONFLICT,
        tonic::Code::OutOfRange => StatusCode::BAD_REQUEST,
        tonic::Code::Unimplemented => StatusCode::NOT_IMPLEMENTED,
        tonic::Code::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        tonic::Code::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        tonic::Code::Unauthenticated => StatusCode::UNAUTHORIZED,
        tonic::Code::ResourceExhausted => StatusCode::TOO_MANY_REQUESTS,
        _ => StatusCode::BAD_GATEWAY,
    };
    ApiError::new(http_status, status.message().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use application_kernel::{
        ActivityAppend, ActivityOutcome, BillingPeriod, CatalogItemUpsert, CatalogPlanKind,
        EntitlementTemplate, Money, OpportunityCreate, OrganizationLifecycle, OrganizationUpsert,
        PricingMetadata, RecordKind, RecordRef, SubscriptionActivate, SubscriptionCreate,
        SubscriptionStatus, WorkflowCaseCreate, WorkflowPriority,
    };
    use application_storage::InMemoryKernelStore;
    use axum::body::{Body, to_bytes};
    use serde::de::DeserializeOwned;
    use tower::ServiceExt;

    fn actor() -> Actor {
        Actor {
            actor_id: "test-operator".to_string(),
            display_name: "Test Operator".to_string(),
            kind: ActorKind::Human,
        }
    }

    fn test_state(store: InMemoryKernelStore) -> HttpState<InMemoryKernelStore> {
        HttpState::new(
            store.config.clone(),
            store,
            AppRuntimeStores::default(),
            Some("integration-secret".to_string()),
        )
    }

    fn auth_header() -> (&'static str, &'static str) {
        ("authorization", "Bearer integration-secret")
    }

    fn seeded_active_credit_subscription(store: &InMemoryKernelStore) -> String {
        store
            .write(|kernel| {
                let actor = actor();
                let organization = kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Billing Customer".to_string(),
                        external_key: Some("cust_live".to_string()),
                        website: None,
                        industry: None,
                        lifecycle: OrganizationLifecycle::Active,
                        owner_user_id: None,
                        tags: vec!["billing".to_string()],
                    },
                    actor.clone(),
                )?;
                let catalog_item = kernel.upsert_catalog_item(
                    CatalogItemUpsert {
                        catalog_item_id: None,
                        sku: "prio-prepaid-live".to_string(),
                        name: "Prio Prepaid Live".to_string(),
                        description: Some("Prepaid live credits".to_string()),
                        plan_kind: CatalogPlanKind::PrepaidCredits,
                        pricing: Some(PricingMetadata {
                            billing_period: BillingPeriod::OneTime,
                            list_price: Money {
                                currency_code: "USD".to_string(),
                                amount_minor: 100_000,
                            },
                            meter_name: Some("prio-credits".to_string()),
                        }),
                        entitlement_template: EntitlementTemplate {
                            feature_flags: vec![],
                            quotas: std::collections::BTreeMap::new(),
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
                            amount_minor: 100_000,
                        },
                        started_at: None,
                    },
                    actor.clone(),
                )?;
                let activation = kernel.activate_subscription(
                    SubscriptionActivate {
                        subscription_id: subscription.id,
                        catalog_item_id: Some(catalog_item.id),
                        opening_balance: Some(Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 0,
                        }),
                    },
                    actor,
                )?;
                Ok(activation.subscription.id.to_string())
            })
            .expect("seeded subscription")
    }

    fn seeded_operator_context(store: &InMemoryKernelStore) -> (String, String) {
        store
            .write(|kernel| {
                let actor = actor();
                let organization = kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Operator Org".to_string(),
                        external_key: Some("acct_operator".to_string()),
                        website: Some("https://prio.ai".to_string()),
                        industry: Some("software".to_string()),
                        lifecycle: OrganizationLifecycle::Active,
                        owner_user_id: Some("owner-1".to_string()),
                        tags: vec!["priority".to_string()],
                    },
                    actor.clone(),
                )?;
                let catalog_item = kernel.upsert_catalog_item(
                    CatalogItemUpsert {
                        catalog_item_id: None,
                        sku: "prio-operator".to_string(),
                        name: "Prio Operator".to_string(),
                        description: Some("Operator plan".to_string()),
                        plan_kind: CatalogPlanKind::Subscription,
                        pricing: Some(PricingMetadata {
                            billing_period: BillingPeriod::Monthly,
                            list_price: Money {
                                currency_code: "USD".to_string(),
                                amount_minor: 20_000,
                            },
                            meter_name: Some("workspace-seat".to_string()),
                        }),
                        entitlement_template: EntitlementTemplate {
                            feature_flags: vec!["workspace".to_string()],
                            quotas: std::collections::BTreeMap::from([("seats".to_string(), 5)]),
                            credit_balance_minor: Some(0),
                        },
                        active: true,
                    },
                    actor.clone(),
                )?;
                let opportunity = kernel.create_opportunity(
                    OpportunityCreate {
                        organization_id: organization.id,
                        primary_contact_id: None,
                        name: "Operator Expansion".to_string(),
                        value: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 40_000,
                        },
                        confidence_bps: 6_500,
                        next_step: Some("Review account".to_string()),
                        expected_close_at: None,
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
                            amount_minor: 20_000,
                        },
                        started_at: None,
                    },
                    actor.clone(),
                )?;
                kernel.activate_subscription(
                    SubscriptionActivate {
                        subscription_id: subscription.id,
                        catalog_item_id: Some(catalog_item.id),
                        opening_balance: Some(Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 0,
                        }),
                    },
                    actor.clone(),
                )?;
                kernel.append_activity(
                    ActivityAppend {
                        subject: "Operator timeline event".to_string(),
                        details: "Initial operator sync".to_string(),
                        related_to: vec![
                            RecordRef {
                                kind: RecordKind::Organization,
                                id: organization.id,
                            },
                            RecordRef {
                                kind: RecordKind::Opportunity,
                                id: opportunity.id,
                            },
                        ],
                        outcome: ActivityOutcome::Completed,
                        occurred_at: None,
                        next_action_due_at: None,
                    },
                    actor.clone(),
                )?;
                let _ = kernel.create_workflow_case(
                    WorkflowCaseCreate {
                        title: "Manual operator review".to_string(),
                        priority: WorkflowPriority::High,
                        owner_user_id: Some("owner-1".to_string()),
                        related_to: vec![RecordRef {
                            kind: RecordKind::Organization,
                            id: organization.id,
                        }],
                    },
                    actor,
                )?;
                Ok((organization.id.to_string(), subscription.id.to_string()))
            })
            .expect("seeded operator context")
    }

    async fn response_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        serde_json::from_slice(&body).expect("response should deserialize")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn billing_top_up_event_executes_refill_truth() {
        let store = InMemoryKernelStore::default_local();
        let subscription_id = seeded_active_credit_subscription(&store);
        let app = app_router(test_state(store.clone()));

        let request = BillingIngressRequest {
            source: "converge-runtime".to_string(),
            event_id: "evt_topup_1".to_string(),
            event_kind: BillingEventKind::PrepaidTopUpSettled,
            idempotency_key: None,
            subscription_id: Some(subscription_id.clone()),
            catalog_item_id: None,
            payment_reference: Some("pi_topup_1".to_string()),
            amount_minor: Some(25_000),
            opening_balance_minor: None,
            currency_code: Some("USD".to_string()),
            payment_status: Some("confirmed".to_string()),
            risk_signal: None,
            days_overdue: None,
            grace_days: None,
            strategic_account: None,
            usage_burn_minor: None,
            provider_settled_minor: None,
            provider_reference: None,
            provider_name: None,
            provider_status: None,
            threshold_minor: None,
            force_manual_review: None,
            manual_review_reason: None,
            persist_projection: Some(true),
            attributes: HashMap::new(),
        };

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/integrations/billing/events")
                    .header(auth_header().0, auth_header().1)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&request).expect("request should serialize"),
                    ))
                    .expect("http request"),
            )
            .await
            .expect("http response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        let payload: BillingIngressResponse =
            serde_json::from_slice(&body).expect("response should deserialize");
        assert_eq!(payload.truth_key, "refill-prepaid-ai-credits");
        assert!(!payload.duplicate);
        assert!(!payload.in_flight);
        assert_eq!(
            payload
                .execution
                .as_ref()
                .map(|value| value.stop_reason.as_str()),
            Some("criteria-met")
        );
        assert_eq!(
            payload
                .projection
                .as_ref()
                .map(|value| value.ledger_entry_ids.len()),
            Some(1)
        );

        let ledger_entries = store
            .read(|kernel| {
                kernel
                    .ledger_entries
                    .values()
                    .filter(|entry| entry.subscription_id.to_string() == subscription_id)
                    .count()
            })
            .expect("read ledger entries");
        assert_eq!(ledger_entries, 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn billing_top_up_event_is_idempotent() {
        let store = InMemoryKernelStore::default_local();
        let subscription_id = seeded_active_credit_subscription(&store);
        let app = app_router(test_state(store.clone()));

        let request = BillingIngressRequest {
            source: "converge-runtime".to_string(),
            event_id: "evt_topup_dup".to_string(),
            event_kind: BillingEventKind::PrepaidTopUpSettled,
            idempotency_key: Some("dedupe-topup-1".to_string()),
            subscription_id: Some(subscription_id.clone()),
            catalog_item_id: None,
            payment_reference: Some("pi_topup_dup".to_string()),
            amount_minor: Some(15_000),
            opening_balance_minor: None,
            currency_code: Some("USD".to_string()),
            payment_status: Some("confirmed".to_string()),
            risk_signal: None,
            days_overdue: None,
            grace_days: None,
            strategic_account: None,
            usage_burn_minor: None,
            provider_settled_minor: None,
            provider_reference: None,
            provider_name: None,
            provider_status: None,
            threshold_minor: None,
            force_manual_review: None,
            manual_review_reason: None,
            persist_projection: Some(true),
            attributes: HashMap::new(),
        };

        for expected_duplicate in [false, true] {
            let response = app
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/v1/integrations/billing/events")
                        .header(auth_header().0, auth_header().1)
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::to_vec(&request).expect("request should serialize"),
                        ))
                        .expect("http request"),
                )
                .await
                .expect("http response");

            assert_eq!(response.status(), StatusCode::OK);
            let body = to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body bytes");
            let payload: BillingIngressResponse =
                serde_json::from_slice(&body).expect("response should deserialize");
            assert_eq!(payload.duplicate, expected_duplicate);
        }

        let (credit_grants, credit_balance_minor) = store
            .read(|kernel| {
                let credit_grants = kernel
                    .ledger_entries
                    .values()
                    .filter(|entry| entry.subscription_id.to_string() == subscription_id)
                    .filter(|entry| entry.kind == application_kernel::LedgerEntryKind::CreditGrant)
                    .count();
                let credit_balance_minor = kernel
                    .entitlements
                    .values()
                    .find(|entitlement| {
                        entitlement.subscription_id.to_string() == subscription_id
                            && entitlement.key == "credit_balance_minor"
                    })
                    .and_then(|entitlement| match entitlement.value {
                        application_kernel::EntitlementValue::Credits(value) => Some(value),
                        _ => None,
                    })
                    .unwrap_or_default();
                (credit_grants, credit_balance_minor)
            })
            .expect("read store state");
        assert_eq!(credit_grants, 1);
        assert_eq!(credit_balance_minor, 15_000);
    }

    #[tokio::test]
    async fn billing_event_requires_auth() {
        let store = InMemoryKernelStore::default_local();
        let app = app_router(test_state(store));

        let request = BillingIngressRequest {
            source: "converge-runtime".to_string(),
            event_id: "evt_auth".to_string(),
            event_kind: BillingEventKind::PrepaidTopUpSettled,
            idempotency_key: None,
            subscription_id: Some("missing".to_string()),
            catalog_item_id: None,
            payment_reference: Some("pi_auth".to_string()),
            amount_minor: Some(10_000),
            opening_balance_minor: None,
            currency_code: Some("USD".to_string()),
            payment_status: Some("confirmed".to_string()),
            risk_signal: None,
            days_overdue: None,
            grace_days: None,
            strategic_account: None,
            usage_burn_minor: None,
            provider_settled_minor: None,
            provider_reference: None,
            provider_name: None,
            provider_status: None,
            threshold_minor: None,
            force_manual_review: None,
            manual_review_reason: None,
            persist_projection: Some(true),
            attributes: HashMap::new(),
        };

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/integrations/billing/events")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&request).expect("request should serialize"),
                    ))
                    .expect("http request"),
            )
            .await
            .expect("http response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn billing_event_rejects_missing_required_fields() {
        let store = InMemoryKernelStore::default_local();
        let app = app_router(test_state(store));

        let request = BillingIngressRequest {
            source: "converge-runtime".to_string(),
            event_id: "evt_missing".to_string(),
            event_kind: BillingEventKind::LedgerReconciliationRequested,
            idempotency_key: None,
            subscription_id: None,
            catalog_item_id: None,
            payment_reference: None,
            amount_minor: None,
            opening_balance_minor: None,
            currency_code: None,
            payment_status: None,
            risk_signal: None,
            days_overdue: None,
            grace_days: None,
            strategic_account: None,
            usage_burn_minor: Some(10_000),
            provider_settled_minor: Some(10_000),
            provider_reference: None,
            provider_name: None,
            provider_status: None,
            threshold_minor: None,
            force_manual_review: None,
            manual_review_reason: None,
            persist_projection: Some(true),
            attributes: HashMap::new(),
        };

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/integrations/billing/events")
                    .header(auth_header().0, auth_header().1)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&request).expect("request should serialize"),
                    ))
                    .expect("http request"),
            )
            .await
            .expect("http response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn organizations_endpoint_lists_seeded_accounts() {
        let store = InMemoryKernelStore::default_local();
        let (organization_id, _) = seeded_operator_context(&store);
        let app = app_router(test_state(store));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri("/v1/organizations")
                    .body(Body::empty())
                    .expect("http request"),
            )
            .await
            .expect("http response");

        assert_eq!(response.status(), StatusCode::OK);
        let payload: Vec<Organization> = response_json(response).await;
        assert!(
            payload
                .iter()
                .any(|organization| organization.id.to_string() == organization_id)
        );
    }

    #[tokio::test]
    async fn organization_summary_endpoint_includes_subscriptions() {
        let store = InMemoryKernelStore::default_local();
        let (organization_id, subscription_id) = seeded_operator_context(&store);
        let app = app_router(test_state(store));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/v1/organizations/{organization_id}/summary?timeline_limit=10"
                    ))
                    .body(Body::empty())
                    .expect("http request"),
            )
            .await
            .expect("http response");

        assert_eq!(response.status(), StatusCode::OK);
        let payload: OrganizationSummaryPayload = response_json(response).await;
        assert_eq!(payload.organization.id.to_string(), organization_id);
        assert!(
            payload
                .subscriptions
                .iter()
                .any(|subscription| subscription.id.to_string() == subscription_id)
        );
        assert!(!payload.recent_timeline.is_empty());
    }

    #[tokio::test]
    async fn truths_endpoint_lists_truth_catalog() {
        let store = InMemoryKernelStore::default_local();
        let app = app_router(test_state(store));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri("/v1/truths?kind=job")
                    .body(Body::empty())
                    .expect("http request"),
            )
            .await
            .expect("http response");

        assert_eq!(response.status(), StatusCode::OK);
        let payload: serde_json::Value = response_json(response).await;
        let truths = payload.as_array().expect("truth list should be an array");
        assert!(truths.iter().any(|truth| {
            truth.get("key").and_then(serde_json::Value::as_str) == Some("qualify-inbound-lead")
        }));
        assert!(
            truths.iter().all(|truth| truth.get("gherkin").is_none()),
            "list endpoint should omit gherkin by default"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn execute_truth_endpoint_runs_truth_without_persisting() {
        let store = InMemoryKernelStore::default_local();
        let app = app_router(test_state(store.clone()));
        let request = ExecuteTruthRequest {
            inputs: HashMap::from([
                ("organization_name".to_string(), "Northwind".to_string()),
                (
                    "inbound_summary".to_string(),
                    "We need pricing and rollout timing this week".to_string(),
                ),
            ]),
            actor: Some(actor()),
            persist_projection: Some(false),
        };

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/truths/qualify-inbound-lead/execute")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&request).expect("request should serialize"),
                    ))
                    .expect("http request"),
            )
            .await
            .expect("http response");

        assert_eq!(response.status(), StatusCode::OK);
        let payload: serde_json::Value = response_json(response).await;
        assert_eq!(
            payload
                .get("truth")
                .and_then(|value| value.get("key"))
                .and_then(serde_json::Value::as_str),
            Some("qualify-inbound-lead")
        );
        assert_eq!(
            payload
                .get("execution")
                .and_then(|value| value.get("converged"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert!(
            payload
                .get("projection")
                .is_some_and(serde_json::Value::is_null),
            "projection should stay null when persist_projection=false"
        );

        let organizations = store
            .read(|kernel| kernel.list_organizations())
            .expect("read organizations");
        assert!(organizations.is_empty());
    }
}
