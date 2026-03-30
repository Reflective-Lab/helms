#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use crm_app::{
    AccountWorkspaceSummary, ApprovalFilter, ApprovalListItem, CatalogItemListItem, OperatorApp,
    OperatorDashboard, OpportunityListItem, OrganizationListItem, RecordReferenceItem,
    SubscriptionListItem, SystemProfile, TruthExecutionSession, TruthListItem, WorkflowCaseFilter,
    WorkflowCaseListItem,
};
use crm_kernel::{
    Actor, BillingPeriod, CatalogItemUpsert, CatalogPlanKind, EntitlementTemplate, Money,
    OrganizationLifecycle, OrganizationUpsert, SubscriptionActivate, SubscriptionCreate,
    SubscriptionStatus,
};
use crm_storage::{
    AppConfig, KernelStore, RecordStoreConfig, SurrealDbKernelStore, SurrealStoreConfig,
};
use tauri::State;
use uuid::Uuid;

type DesktopStore = SurrealDbKernelStore;

#[derive(Clone)]
struct AppState {
    operator: OperatorApp<DesktopStore>,
}

#[tauri::command]
fn operator_dashboard(state: State<'_, AppState>) -> Result<OperatorDashboard, String> {
    state
        .operator
        .operator_dashboard()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn system_profile(state: State<'_, AppState>) -> SystemProfile {
    state.operator.system_profile()
}

#[tauri::command]
fn list_truths(state: State<'_, AppState>) -> Vec<TruthListItem> {
    state.operator.list_truths()
}

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

#[tauri::command]
fn list_organizations(state: State<'_, AppState>) -> Result<Vec<OrganizationListItem>, String> {
    state
        .operator
        .list_organizations()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_opportunities(state: State<'_, AppState>) -> Result<Vec<OpportunityListItem>, String> {
    state
        .operator
        .list_opportunities()
        .map_err(|error| error.to_string())
}

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

#[tauri::command]
fn list_timeline(
    state: State<'_, AppState>,
    anchor: Option<RecordReferenceItem>,
    limit: Option<usize>,
) -> Result<Vec<crm_app::TimelineEventItem>, String> {
    state
        .operator
        .list_timeline(anchor, limit.unwrap_or(12))
        .map_err(|error| error.to_string())
}

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
                name: "Prio Platform Annual".to_string(),
                description: Some("Annual governed CRM workspace".to_string()),
                plan_kind: CatalogPlanKind::Subscription,
                pricing: Some(crm_kernel::PricingMetadata {
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
                name: "Prio AI Credits 100k".to_string(),
                description: Some("Prepaid AI credits pack".to_string()),
                plan_kind: CatalogPlanKind::PrepaidCredits,
                pricing: Some(crm_kernel::PricingMetadata {
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

fn seed_uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("valid seed uuid")
}

fn default_desktop_store_endpoint() -> String {
    let path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".desktop-surreal");
    format!("rocksdb://{}", path.display())
}

fn ensure_store_path(endpoint: &str) {
    if let Some(path) = endpoint.strip_prefix("rocksdb://") {
        let _ = std::fs::create_dir_all(path);
    }
}

fn desktop_storage_config() -> AppConfig {
    let endpoint =
        std::env::var("CRM_SURREAL_ENDPOINT").unwrap_or_else(|_| default_desktop_store_endpoint());
    ensure_store_path(&endpoint);
    desktop_storage_config_for_endpoint(endpoint)
}

fn desktop_storage_config_for_endpoint(endpoint: String) -> AppConfig {
    let mut config = AppConfig::from_env();
    config.record_store = RecordStoreConfig::Surreal(SurrealStoreConfig {
        endpoint,
        namespace: std::env::var("CRM_SURREAL_NAMESPACE")
            .unwrap_or_else(|_| "crm_prio_ai".to_string()),
        database: std::env::var("CRM_SURREAL_DATABASE")
            .unwrap_or_else(|_| "desktop".to_string()),
        username: std::env::var("CRM_SURREAL_USERNAME").ok(),
        password: std::env::var("CRM_SURREAL_PASSWORD").ok(),
    });
    config
}

fn maybe_run_headless_mode(operator: &OperatorApp<DesktopStore>) -> Result<bool, String> {
    let Ok(mode) = std::env::var("PRIO_CRM_DESKTOP_MODE") else {
        return Ok(false);
    };

    match mode.as_str() {
        "execute-qualify" => {
            let organization_name = std::env::var("PRIO_CRM_DESKTOP_ORG_NAME")
                .unwrap_or_else(|_| "Persistence Test".to_string());
            let session = operator.execute_truth(
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
            let organization_name = std::env::var("PRIO_CRM_DESKTOP_ORG_NAME")
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
        other => Err(format!("unsupported PRIO_CRM_DESKTOP_MODE: {other}")),
    }
}

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
    if matches!(std::env::var("PRIO_CRM_DESKTOP_MODE").as_deref(), Ok(_)) {
        return;
    }

    tauri::Builder::default()
        .manage(AppState { operator })
        .invoke_handler(tauri::generate_handler![
            operator_dashboard,
            system_profile,
            list_truths,
            execute_truth,
            list_organizations,
            list_opportunities,
            list_subscriptions,
            list_catalog_items,
            account_summary,
            list_timeline,
            list_workflow_cases,
            list_approvals
        ])
        .run(tauri::generate_context!())
        .expect("failed to run prio crm desktop");
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{DesktopStore, desktop_storage_config_for_endpoint};
    use crm_app::OperatorApp;

    #[test]
    #[ignore = "requires a separate process restart to release rocksdb locks"]
    fn desktop_surreal_store_persists_truth_projection_across_reopen() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("prio-crm-desktop-{nonce}.db"));
        let endpoint = format!("rocksdb://{}", path.display());
        let config = desktop_storage_config_for_endpoint(endpoint);

        let store = DesktopStore::connect_blocking(config.clone()).expect("connect first store");
        let operator = OperatorApp::new(store.config.clone(), store);
        let session = operator
            .execute_truth(
                "qualify-inbound-lead",
                std::collections::HashMap::from([
                    ("organization_name".to_string(), "Persistence Test".to_string()),
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
