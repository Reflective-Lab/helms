#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::collections::BTreeMap;

#[cfg(feature = "embedded-backend")]
use std::collections::HashMap;
#[cfg(feature = "embedded-backend")]
use std::path::PathBuf;

#[cfg(feature = "embedded-backend")]
use application_kernel::{
    Actor, BillingPeriod, CatalogItemUpsert, CatalogPlanKind, EntitlementTemplate, Money,
    OrganizationLifecycle, OrganizationUpsert, SubscriptionActivate, SubscriptionCreate,
    SubscriptionStatus,
};
#[cfg(feature = "embedded-backend")]
use application_storage::{
    AppConfig, KernelStore, RecordStoreConfig, SurrealDbKernelStore, SurrealStoreConfig,
};
use organism_intelligence::ocr::{OllamaReceiptConfig, TesseractCliConfig};
use prio_expenses::receipt_extractor::{
    ExtractorEngine, FieldComparison, ReceiptSample, benchmark_output,
    discover_receipt_fixture_root, find_sample, load_receipt_samples,
};
use serde::Serialize;
#[cfg(feature = "embedded-backend")]
use tauri::State;
#[cfg(feature = "embedded-backend")]
use uuid::Uuid;
#[cfg(feature = "embedded-backend")]
use workbench_backend::{
    AccountWorkspaceSummary, ApprovalFilter, ApprovalListItem, CatalogItemListItem, OperatorApp,
    OperatorDashboard, OpportunityListItem, OrganizationListItem, RecordReferenceItem,
    SubscriptionListItem, SystemProfile, TruthDetailItem, TruthExecutionSession, TruthListItem,
    WorkbenchAppManifest, WorkflowCaseFilter, WorkflowCaseListItem,
};

#[cfg(feature = "embedded-backend")]
type DesktopStore = SurrealDbKernelStore;

#[cfg(feature = "embedded-backend")]
#[derive(Clone)]
struct AppState {
    operator: OperatorApp<DesktopStore>,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopExpenseMoney {
    currency_code: String,
    amount_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopExpenseReport {
    id: String,
    title: String,
    employee_name: String,
    employee_email: String,
    status: String,
    currency_code: String,
    total_minor: i64,
    description: Option<String>,
    submitted_at: Option<String>,
    booking_export_reference: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopExpenseItem {
    id: String,
    report_id: String,
    merchant: String,
    amount: DesktopExpenseMoney,
    category: String,
    occurred_at: String,
    description: Option<String>,
    capture_source: String,
    receipt_document_id: Option<String>,
    ocr_status: String,
    ocr_engine: Option<String>,
    extracted_summary: Option<String>,
    ocr_fields: BTreeMap<String, String>,
    policy_flags: Vec<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopReceiptSampleView {
    sample_id: String,
    report_id: Option<String>,
    document_file: String,
    original_file_name: String,
    document_path: String,
    reference_path: String,
    document_type: String,
    capture_type: String,
    expense_candidate: bool,
    reference_status: String,
    expected_fields: BTreeMap<String, String>,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopReceiptFieldComparisonView {
    field: String,
    expected: String,
    actual: String,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopReceiptBenchmarkView {
    matched_fields: usize,
    compared_fields: usize,
    missing_fields: Vec<DesktopReceiptFieldComparisonView>,
    mismatched_fields: Vec<DesktopReceiptFieldComparisonView>,
}

#[derive(Debug, Clone, Serialize)]
struct DesktopReceiptExtractionRunView {
    sample_id: String,
    engine: String,
    status: String,
    implementation: Option<String>,
    fields: BTreeMap<String, String>,
    raw_text: Option<String>,
    warnings: Vec<String>,
    metadata: BTreeMap<String, String>,
    benchmark: Option<DesktopReceiptBenchmarkView>,
    error: Option<String>,
}

fn load_receipt_fixtures() -> Result<Vec<ReceiptSample>, String> {
    let root = discover_receipt_fixture_root().map_err(|error| error.to_string())?;
    load_receipt_samples(&root).map_err(|error| error.to_string())
}

fn report_id_for_sample(sample: &ReceiptSample) -> String {
    format!("expense-report:{}", sample.sample_id())
}

fn item_id_for_sample(sample: &ReceiptSample) -> String {
    format!("expense-item:{}", sample.sample_id())
}

fn sample_date(sample: &ReceiptSample) -> String {
    for candidate in [
        sample.fixture.expected.issue_date.as_str(),
        sample.fixture.expected.service_date.as_str(),
        sample.fixture.expected.due_date.as_str(),
    ] {
        if !candidate.trim().is_empty() {
            return candidate.trim().to_string();
        }
    }
    "2026-04-12".to_string()
}

fn sample_timestamp(date: &str, hour: &str) -> String {
    format!("{date}T{hour}:00Z")
}

fn sample_total_minor(sample: &ReceiptSample) -> i64 {
    parse_amount_minor(&sample.fixture.expected.total).unwrap_or_default()
}

fn parse_amount_minor(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.replace(',', ".");
    let parsed = normalized.parse::<f64>().ok()?;
    Some((parsed * 100.0).round() as i64)
}

fn merchant_fallback(sample: &ReceiptSample) -> String {
    let stem = sample
        .document_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("receipt");
    stem.split('-')
        .skip(1)
        .take_while(|token| token.chars().all(|character| !character.is_ascii_digit()))
        .map(|token| {
            let mut chars = token.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut value = String::new();
            value.extend(first.to_uppercase());
            value.push_str(chars.as_str());
            value
        })
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn sample_merchant(sample: &ReceiptSample) -> String {
    if !sample.fixture.expected.merchant.trim().is_empty() {
        sample.fixture.expected.merchant.trim().to_string()
    } else {
        merchant_fallback(sample)
    }
}

fn sample_currency(sample: &ReceiptSample) -> String {
    let currency = sample.fixture.expected.currency.trim();
    if currency.is_empty() {
        "EUR".to_string()
    } else {
        currency.to_string()
    }
}

fn sample_category(sample: &ReceiptSample) -> String {
    let merchant = sample_merchant(sample).to_ascii_lowercase();
    if merchant.contains("anthropic") || merchant.contains("apple") || merchant.contains("temporal")
    {
        "software".to_string()
    } else if merchant.contains("hotel")
        || merchant.contains("rail")
        || merchant.contains("train")
        || merchant.contains("flight")
    {
        "travel".to_string()
    } else if merchant.contains("primagaz") || merchant.contains("credit agricole") {
        "utilities".to_string()
    } else {
        "other".to_string()
    }
}

fn sample_report_status(sample: &ReceiptSample) -> String {
    let capture = sample.fixture.capture_type.to_ascii_lowercase();
    if capture.contains("photo") || capture.contains("scan") {
        "in-review".to_string()
    } else {
        "export-pending".to_string()
    }
}

fn sample_ocr_status(sample: &ReceiptSample) -> String {
    let capture = sample.fixture.capture_type.to_ascii_lowercase();
    if capture.contains("photo") || capture.contains("scan") {
        "needs-review".to_string()
    } else {
        "scanned".to_string()
    }
}

fn sample_policy_flags(sample: &ReceiptSample) -> Vec<String> {
    let mut flags = Vec::new();
    let capture = sample.fixture.capture_type.to_ascii_lowercase();
    if capture.contains("photo") || capture.contains("scan") {
        flags.push("verify-fields".to_string());
        flags.push("manual-review-required".to_string());
    }
    if !sample.fixture.expense_candidate {
        flags.push("non-expense-document".to_string());
    }
    if sample.fixture.expected.tax.trim().is_empty() {
        flags.push("verify-tax".to_string());
    }
    flags
}

fn sample_extracted_summary(sample: &ReceiptSample) -> String {
    let merchant = sample_merchant(sample);
    let total_minor = sample_total_minor(sample);
    let currency = sample_currency(sample);
    let date = sample_date(sample);
    format!(
        "Reference fixture for {merchant} on {date}, total {} {}.",
        total_minor as f64 / 100.0,
        currency
    )
}

fn expense_report_from_sample(sample: &ReceiptSample) -> DesktopExpenseReport {
    let date = sample_date(sample);
    let status = sample_report_status(sample);
    let merchant = sample_merchant(sample);
    DesktopExpenseReport {
        id: report_id_for_sample(sample),
        title: format!("Expense Report · {merchant}"),
        employee_name: "Kenneth Pernyer".to_string(),
        employee_email: "kenneth@prio.ai".to_string(),
        status: status.clone(),
        currency_code: sample_currency(sample),
        total_minor: sample_total_minor(sample),
        description: Some(format!(
            "{} sample from {}",
            sample.fixture.document_type, sample.fixture.original_file_name
        )),
        submitted_at: if status == "in-review" {
            None
        } else {
            Some(sample_timestamp(&date, "10:15"))
        },
        booking_export_reference: if status == "export-pending" {
            Some(format!("manual-export/{}", sample.sample_id()))
        } else {
            None
        },
        created_at: sample_timestamp(&date, "09:30"),
        updated_at: sample_timestamp(&date, "10:20"),
    }
}

fn expense_item_from_sample(sample: &ReceiptSample) -> DesktopExpenseItem {
    let date = sample_date(sample);
    let merchant = sample_merchant(sample);
    let mut ocr_fields = sample.fixture.expected.to_field_map();
    ocr_fields.insert(
        "source_file".to_string(),
        format!("data/receipts/{}", sample.fixture.document_file),
    );
    ocr_fields.insert(
        "capture_type".to_string(),
        sample.fixture.capture_type.clone(),
    );
    ocr_fields.insert(
        "reference_status".to_string(),
        sample.fixture.reference_status.clone(),
    );

    DesktopExpenseItem {
        id: item_id_for_sample(sample),
        report_id: report_id_for_sample(sample),
        merchant,
        amount: DesktopExpenseMoney {
            currency_code: sample_currency(sample),
            amount_minor: sample_total_minor(sample),
        },
        category: sample_category(sample),
        occurred_at: sample_timestamp(&date, "09:00"),
        description: Some(sample.fixture.original_file_name.clone()),
        capture_source: sample.fixture.capture_type.clone(),
        receipt_document_id: Some(format!("document:{}", sample.sample_id())),
        ocr_status: sample_ocr_status(sample),
        ocr_engine: Some("reference-sidecar".to_string()),
        extracted_summary: Some(sample_extracted_summary(sample)),
        ocr_fields,
        policy_flags: sample_policy_flags(sample),
        created_at: sample_timestamp(&date, "09:00"),
        updated_at: sample_timestamp(&date, "10:20"),
    }
}

fn receipt_sample_view(sample: &ReceiptSample) -> DesktopReceiptSampleView {
    DesktopReceiptSampleView {
        sample_id: sample.sample_id().to_string(),
        report_id: sample
            .fixture
            .expense_candidate
            .then(|| report_id_for_sample(sample)),
        document_file: sample.fixture.document_file.clone(),
        original_file_name: sample.fixture.original_file_name.clone(),
        document_path: sample.document_path.display().to_string(),
        reference_path: sample.reference_path.display().to_string(),
        document_type: sample.fixture.document_type.clone(),
        capture_type: sample.fixture.capture_type.clone(),
        expense_candidate: sample.fixture.expense_candidate,
        reference_status: sample.fixture.reference_status.clone(),
        expected_fields: sample.fixture.expected.to_field_map(),
        notes: sample.fixture.notes.clone(),
    }
}

fn field_comparison_view(value: &FieldComparison) -> DesktopReceiptFieldComparisonView {
    DesktopReceiptFieldComparisonView {
        field: value.field.clone(),
        expected: value.expected.clone(),
        actual: value.actual.clone(),
    }
}

fn extraction_run_view(
    sample: &ReceiptSample,
    engine: ExtractorEngine,
) -> DesktopReceiptExtractionRunView {
    match engine.extract(sample) {
        Ok(output) => {
            let benchmark = benchmark_output(&sample.fixture.expected, &output);
            DesktopReceiptExtractionRunView {
                sample_id: sample.sample_id().to_string(),
                engine: output.engine,
                status: "completed".to_string(),
                implementation: Some(output.implementation),
                fields: output.fields,
                raw_text: output.raw_text,
                warnings: output.warnings,
                metadata: output.metadata,
                benchmark: Some(DesktopReceiptBenchmarkView {
                    matched_fields: benchmark.matched_fields,
                    compared_fields: benchmark.compared_fields,
                    missing_fields: benchmark
                        .missing_fields
                        .iter()
                        .map(field_comparison_view)
                        .collect(),
                    mismatched_fields: benchmark
                        .mismatched_fields
                        .iter()
                        .map(field_comparison_view)
                        .collect(),
                }),
                error: None,
            }
        }
        Err(error) => DesktopReceiptExtractionRunView {
            sample_id: sample.sample_id().to_string(),
            engine: engine.engine_name().to_string(),
            status: "failed".to_string(),
            implementation: None,
            fields: BTreeMap::new(),
            raw_text: None,
            warnings: Vec::new(),
            metadata: BTreeMap::new(),
            benchmark: None,
            error: Some(error.to_string()),
        },
    }
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn operator_dashboard(state: State<'_, AppState>) -> Result<OperatorDashboard, String> {
    state
        .operator
        .operator_dashboard()
        .map_err(|error| error.to_string())
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn system_profile(state: State<'_, AppState>) -> SystemProfile {
    state.operator.system_profile()
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn list_truths(state: State<'_, AppState>) -> Vec<TruthListItem> {
    state.operator.list_truths()
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn get_truth_detail(state: State<'_, AppState>, key: String) -> Result<TruthDetailItem, String> {
    state
        .operator
        .truth_detail(&key)
        .ok_or_else(|| format!("truth not found: {key}"))
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn list_workbench_apps(state: State<'_, AppState>) -> Vec<WorkbenchAppManifest> {
    state.operator.workbench_apps()
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn execute_truth(
    state: State<'_, AppState>,
    key: String,
    inputs: HashMap<String, String>,
) -> Result<TruthExecutionSession, String> {
    state
        .operator
        .execute_truth(&key, inputs)
        .map_err(|error| error.to_string())
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn list_organizations(state: State<'_, AppState>) -> Result<Vec<OrganizationListItem>, String> {
    state
        .operator
        .list_organizations()
        .map_err(|error| error.to_string())
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn list_opportunities(state: State<'_, AppState>) -> Result<Vec<OpportunityListItem>, String> {
    state
        .operator
        .list_opportunities()
        .map_err(|error| error.to_string())
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn list_subscriptions(
    state: State<'_, AppState>,
    organization_id: Option<String>,
) -> Result<Vec<SubscriptionListItem>, String> {
    state
        .operator
        .list_subscriptions(organization_id.as_deref())
        .map_err(|error| error.to_string())
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn list_catalog_items(
    state: State<'_, AppState>,
    active_only: Option<bool>,
) -> Result<Vec<CatalogItemListItem>, String> {
    state
        .operator
        .list_catalog_items(active_only.unwrap_or(false))
        .map_err(|error| error.to_string())
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn account_summary(
    state: State<'_, AppState>,
    org_id: String,
) -> Result<AccountWorkspaceSummary, String> {
    state
        .operator
        .account_summary(&org_id)
        .map_err(|error| error.to_string())
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn list_timeline(
    state: State<'_, AppState>,
    anchor: Option<RecordReferenceItem>,
    limit: Option<usize>,
) -> Result<Vec<workbench_backend::TimelineEventItem>, String> {
    state
        .operator
        .list_timeline(anchor, limit.unwrap_or(12))
        .map_err(|error| error.to_string())
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn list_workflow_cases(
    state: State<'_, AppState>,
    filter: Option<WorkflowCaseFilter>,
) -> Result<Vec<WorkflowCaseListItem>, String> {
    state
        .operator
        .list_workflow_cases(filter.unwrap_or_default())
        .map_err(|error| error.to_string())
}

#[cfg(feature = "embedded-backend")]
#[tauri::command]
fn list_approvals(
    state: State<'_, AppState>,
    filter: Option<ApprovalFilter>,
) -> Result<Vec<ApprovalListItem>, String> {
    state
        .operator
        .list_approvals(filter.unwrap_or_default())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_expense_reports() -> Result<Vec<DesktopExpenseReport>, String> {
    let mut reports = load_receipt_fixtures()?
        .into_iter()
        .filter(|sample| sample.fixture.expense_candidate)
        .map(|sample| expense_report_from_sample(&sample))
        .collect::<Vec<_>>();
    reports.sort_by(|left, right| left.updated_at.cmp(&right.updated_at).reverse());
    Ok(reports)
}

#[tauri::command]
fn list_expense_items(report_id: Option<String>) -> Result<Vec<DesktopExpenseItem>, String> {
    let mut items = load_receipt_fixtures()?
        .into_iter()
        .filter(|sample| sample.fixture.expense_candidate)
        .map(|sample| expense_item_from_sample(&sample))
        .collect::<Vec<_>>();
    if let Some(report_id) = report_id.as_deref() {
        items.retain(|item| item.report_id == report_id);
    }
    items.sort_by(|left, right| left.occurred_at.cmp(&right.occurred_at).reverse());
    Ok(items)
}

#[tauri::command]
fn list_receipt_samples() -> Result<Vec<DesktopReceiptSampleView>, String> {
    let mut samples = load_receipt_fixtures()?
        .into_iter()
        .map(|sample| receipt_sample_view(&sample))
        .collect::<Vec<_>>();
    samples.sort_by(|left, right| left.sample_id.cmp(&right.sample_id));
    Ok(samples)
}

#[tauri::command]
fn compare_receipt_ocr(sample_id: String) -> Result<Vec<DesktopReceiptExtractionRunView>, String> {
    let fixtures = load_receipt_fixtures()?;
    let sample = find_sample(&fixtures, &sample_id).map_err(|error| error.to_string())?;
    Ok(vec![
        extraction_run_view(sample, ExtractorEngine::Reference),
        extraction_run_view(sample, ExtractorEngine::CanonicalName),
        extraction_run_view(
            sample,
            ExtractorEngine::TesseractCli(TesseractCliConfig::default()),
        ),
        extraction_run_view(
            sample,
            ExtractorEngine::Ollama(OllamaReceiptConfig::default()),
        ),
    ])
}


#[cfg(feature = "embedded-backend")]
fn seed_demo_data<S>(operator: &OperatorApp<S>)
where
    S: KernelStore,
{
    let _ = operator.execute_truth(
        "qualify-inbound-lead",
        HashMap::from([
            ("organization_name".to_string(), "Northwind".to_string()),
            (
                "inbound_summary".to_string(),
                "Champion asked for a governed CRM substrate and audit trail.".to_string(),
            ),
            ("contact_name".to_string(), "Alice Doe".to_string()),
            ("contact_title".to_string(), "CTO".to_string()),
            (
                "contact_email".to_string(),
                "alice@northwind.example".to_string(),
            ),
            (
                "website".to_string(),
                "https://northwind.example".to_string(),
            ),
            ("industry".to_string(), "Software".to_string()),
            ("owner_user_id".to_string(), "kenneth".to_string()),
            (
                "next_step".to_string(),
                "Send architecture brief and qualification follow-up.".to_string(),
            ),
            (
                "opportunity_value_minor".to_string(),
                "24000000".to_string(),
            ),
        ]),
    );

    let _ = operator.execute_truth(
        "qualify-inbound-lead",
        HashMap::from([
            ("organization_name".to_string(), "Apex Labs".to_string()),
            (
                "inbound_summary".to_string(),
                "Procurement path is non-standard and needs explicit review.".to_string(),
            ),
            ("contact_name".to_string(), "Morgan Lee".to_string()),
            ("contact_title".to_string(), "VP Operations".to_string()),
            ("website".to_string(), "https://apex.example".to_string()),
            ("owner_user_id".to_string(), "revops-queue".to_string()),
            ("require_manual_review".to_string(), "true".to_string()),
            (
                "manual_review_reason".to_string(),
                "Commercial terms exceed the standard qualification path.".to_string(),
            ),
        ]),
    );
}

#[cfg(feature = "embedded-backend")]
fn seed_revenue_data(store: &DesktopStore) {
    let actor = Actor::system();
    let organization_id = seed_uuid("11111111-1111-4111-8111-111111111111");
    let subscription_catalog_id = seed_uuid("22222222-2222-4222-8222-222222222222");
    let activation_subscription_id = seed_uuid("33333333-3333-4333-8333-333333333333");
    let credits_catalog_id = seed_uuid("44444444-4444-4444-8444-444444444444");
    let refill_subscription_id = seed_uuid("55555555-5555-4555-8555-555555555555");

    let _ = store.write(|kernel| {
        kernel.upsert_organization(
            OrganizationUpsert {
                organization_id: Some(organization_id),
                name: "Northwind Revenue".to_string(),
                external_key: Some("northwind-revenue".to_string()),
                website: Some("https://northwind.example".to_string()),
                industry: Some("Software".to_string()),
                lifecycle: OrganizationLifecycle::Active,
                owner_user_id: Some("revops".to_string()),
                tags: vec!["demo".to_string(), "revenue".to_string()],
            },
            actor.clone(),
        )?;

        kernel.upsert_catalog_item(
            CatalogItemUpsert {
                catalog_item_id: Some(subscription_catalog_id),
                sku: "prio-platform-annual".to_string(),
                name: "Operator Workspace Annual".to_string(),
                description: Some("Annual governed operator workspace".to_string()),
                plan_kind: CatalogPlanKind::Subscription,
                pricing: Some(application_kernel::PricingMetadata {
                    billing_period: BillingPeriod::Annual,
                    list_price: Money {
                        currency_code: "USD".to_string(),
                        amount_minor: 12_000_00,
                    },
                    meter_name: Some("workspace-annual".to_string()),
                }),
                entitlement_template: EntitlementTemplate {
                    feature_flags: vec!["workspace_access".to_string(), "audit_trail".to_string()],
                    quotas: BTreeMap::from([("seats".to_string(), 25)]),
                    credit_balance_minor: None,
                },
                active: true,
            },
            actor.clone(),
        )?;

        kernel.upsert_catalog_item(
            CatalogItemUpsert {
                catalog_item_id: Some(credits_catalog_id),
                sku: "prio-ai-credits-100k".to_string(),
                name: "AI Credits 100k".to_string(),
                description: Some("Prepaid AI credits pack".to_string()),
                plan_kind: CatalogPlanKind::PrepaidCredits,
                pricing: Some(application_kernel::PricingMetadata {
                    billing_period: BillingPeriod::OneTime,
                    list_price: Money {
                        currency_code: "USD".to_string(),
                        amount_minor: 5_000_00,
                    },
                    meter_name: Some("ai-credits".to_string()),
                }),
                entitlement_template: EntitlementTemplate {
                    feature_flags: vec![],
                    quotas: BTreeMap::new(),
                    credit_balance_minor: Some(0),
                },
                active: true,
            },
            actor.clone(),
        )?;

        kernel.create_order_subscription(
            SubscriptionCreate {
                subscription_id: Some(activation_subscription_id),
                organization_id,
                quote_id: None,
                catalog_item_id: Some(subscription_catalog_id),
                status: SubscriptionStatus::PendingActivation,
                value: Money {
                    currency_code: "USD".to_string(),
                    amount_minor: 12_000_00,
                },
                started_at: None,
            },
            actor.clone(),
        )?;

        let refill_subscription = kernel.create_order_subscription(
            SubscriptionCreate {
                subscription_id: Some(refill_subscription_id),
                organization_id,
                quote_id: None,
                catalog_item_id: Some(credits_catalog_id),
                status: SubscriptionStatus::PendingActivation,
                value: Money {
                    currency_code: "USD".to_string(),
                    amount_minor: 5_000_00,
                },
                started_at: None,
            },
            actor.clone(),
        )?;

        kernel.activate_subscription(
            SubscriptionActivate {
                subscription_id: refill_subscription.id,
                catalog_item_id: None,
                opening_balance: None,
            },
            actor.clone(),
        )?;

        Ok(())
    });
}

#[cfg(feature = "embedded-backend")]
fn seed_uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("valid seed uuid")
}

#[cfg(feature = "embedded-backend")]
fn default_desktop_store_endpoint() -> String {
    let path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".desktop-surreal");
    format!("rocksdb://{}", path.display())
}

#[cfg(feature = "embedded-backend")]
fn ensure_store_path(endpoint: &str) {
    if let Some(path) = endpoint.strip_prefix("rocksdb://") {
        let _ = std::fs::create_dir_all(path);
    }
}

#[cfg(feature = "embedded-backend")]
fn desktop_storage_config() -> AppConfig {
    let endpoint =
        std::env::var("CRM_SURREAL_ENDPOINT").unwrap_or_else(|_| default_desktop_store_endpoint());
    ensure_store_path(&endpoint);
    desktop_storage_config_for_endpoint(endpoint)
}

#[cfg(feature = "embedded-backend")]
fn desktop_storage_config_for_endpoint(endpoint: String) -> AppConfig {
    let mut config = AppConfig::from_env();
    config.record_store = RecordStoreConfig::Surreal(SurrealStoreConfig {
        endpoint,
        namespace: std::env::var("CRM_SURREAL_NAMESPACE")
            .unwrap_or_else(|_| "outcome_workbench".to_string()),
        database: std::env::var("CRM_SURREAL_DATABASE").unwrap_or_else(|_| "desktop".to_string()),
        username: std::env::var("CRM_SURREAL_USERNAME").ok(),
        password: std::env::var("CRM_SURREAL_PASSWORD").ok(),
    });
    config
}

#[cfg(feature = "embedded-backend")]
fn maybe_run_headless_mode(operator: &OperatorApp<DesktopStore>) -> Result<bool, String> {
    let mode = std::env::var("OUTCOME_WORKBENCH_DESKTOP_MODE")
        .or_else(|_| std::env::var("WORKBENCH_DESKTOP_MODE"))
        .or_else(|_| std::env::var("PRIO_CRM_DESKTOP_MODE"));
    let Ok(mode) = mode else {
        return Ok(false);
    };

    match mode.as_str() {
        "execute-qualify" => {
            let organization_name = std::env::var("OUTCOME_WORKBENCH_DESKTOP_ORG_NAME")
                .or_else(|_| std::env::var("WORKBENCH_DESKTOP_ORG_NAME"))
                .or_else(|_| std::env::var("PRIO_CRM_DESKTOP_ORG_NAME"))
                .unwrap_or_else(|_| "Persistence Test".to_string());
            let session = operator
                .execute_truth(
                    "qualify-inbound-lead",
                    HashMap::from([
                        ("organization_name".to_string(), organization_name),
                        (
                            "inbound_summary".to_string(),
                            "Verify projection survives process restart.".to_string(),
                        ),
                        ("contact_name".to_string(), "Persistence Lead".to_string()),
                        (
                            "contact_email".to_string(),
                            "lead@persistence.example".to_string(),
                        ),
                    ]),
                )
                .map_err(|error| error.to_string())?;
            let organization_id = session
                .projection
                .and_then(|projection| projection.organization_id)
                .ok_or_else(|| "missing organization projection".to_string())?;
            println!("{organization_id}");
            Ok(true)
        }
        "assert-org" => {
            let organization_name = std::env::var("OUTCOME_WORKBENCH_DESKTOP_ORG_NAME")
                .or_else(|_| std::env::var("WORKBENCH_DESKTOP_ORG_NAME"))
                .or_else(|_| std::env::var("PRIO_CRM_DESKTOP_ORG_NAME"))
                .unwrap_or_else(|_| "Persistence Test".to_string());
            let organizations = operator
                .list_organizations()
                .map_err(|error| error.to_string())?;
            let organization = organizations
                .into_iter()
                .find(|entry| entry.name == organization_name)
                .ok_or_else(|| format!("organization '{organization_name}' not found"))?;
            let summary = operator
                .account_summary(&organization.id)
                .map_err(|error| error.to_string())?;
            if summary.people.is_empty() {
                return Err(format!(
                    "organization '{organization_name}' has no projected people"
                ));
            }
            println!("{}", organization.id);
            Ok(true)
        }
        other => Err(format!(
            "unsupported OUTCOME_WORKBENCH_DESKTOP_MODE: {other}"
        )),
    }
}

#[cfg(feature = "embedded-backend")]
fn main() {
    let config = desktop_storage_config();
    let store = DesktopStore::connect_blocking(config).expect("failed to connect desktop store");
    let should_seed = store
        .read(|kernel| kernel.organizations.is_empty())
        .expect("failed to inspect desktop store");

    if should_seed {
        seed_revenue_data(&store);
    }

    let operator = OperatorApp::new(store.config.clone(), store.clone());

    if should_seed {
        seed_demo_data(&operator);
    }

    if let Err(error) = maybe_run_headless_mode(&operator) {
        eprintln!("{error}");
        std::process::exit(1);
    }
    if matches!(
        std::env::var("OUTCOME_WORKBENCH_DESKTOP_MODE").as_deref(),
        Ok(_)
    ) || matches!(std::env::var("WORKBENCH_DESKTOP_MODE").as_deref(), Ok(_))
        || matches!(std::env::var("PRIO_CRM_DESKTOP_MODE").as_deref(), Ok(_))
    {
        return;
    }

    tauri::Builder::default()
        .manage(AppState { operator })
        .invoke_handler(tauri::generate_handler![
            operator_dashboard,
            system_profile,
            list_truths,
            get_truth_detail,
            list_workbench_apps,
            execute_truth,
            list_organizations,
            list_opportunities,
            list_subscriptions,
            list_catalog_items,
            account_summary,
            list_timeline,
            list_workflow_cases,
            list_approvals,
            list_expense_reports,
            list_expense_items,
            list_receipt_samples,
            compare_receipt_ocr
        ])
        .run(tauri::generate_context!())
        .expect("failed to run outcome workbench");
}

#[cfg(not(feature = "embedded-backend"))]
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_expense_reports,
            list_expense_items,
            list_receipt_samples,
            compare_receipt_ocr,
            get_note_vault_root,
            list_notes,
            read_note,
            save_note,
            create_note,
            move_note,
            import_markdown_tree,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run outcome workbench");
}

#[cfg(all(test, feature = "embedded-backend"))]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{DesktopStore, desktop_storage_config_for_endpoint};
    use workbench_backend::OperatorApp;

    #[test]
    #[ignore = "requires a separate process restart to release rocksdb locks"]
    fn desktop_surreal_store_persists_truth_projection_across_reopen() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("outcome-workbench-{nonce}.db"));
        let endpoint = format!("rocksdb://{}", path.display());
        let config = desktop_storage_config_for_endpoint(endpoint);

        let store = DesktopStore::connect_blocking(config.clone()).expect("connect first store");
        let operator = OperatorApp::new(store.config.clone(), store);
        let session = operator
            .execute_truth(
                "qualify-inbound-lead",
                std::collections::HashMap::from([
                    (
                        "organization_name".to_string(),
                        "Persistence Test".to_string(),
                    ),
                    (
                        "inbound_summary".to_string(),
                        "Verify projection survives reopen.".to_string(),
                    ),
                ]),
            )
            .expect("truth should execute");
        let organization_id = session
            .projection
            .and_then(|projection| projection.organization_id)
            .expect("organization projection");
        drop(operator);

        let reopened_store =
            DesktopStore::connect_blocking(config).expect("connect reopened store");
        let reopened_operator = OperatorApp::new(reopened_store.config.clone(), reopened_store);
        let organizations = reopened_operator
            .list_organizations()
            .expect("list organizations after reopen");
        assert!(
            organizations
                .iter()
                .any(|organization| organization.id == organization_id),
            "organization should persist across reopen"
        );
        let summary = reopened_operator
            .account_summary(&organization_id)
            .expect("account summary after reopen");
        assert_eq!(summary.organization.name, "Persistence Test");
    }
}
