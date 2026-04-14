use std::collections::HashMap;
use std::path::PathBuf;

use application_storage::{AppConfig, RecordStoreConfig, SurrealDbKernelStore, SurrealStoreConfig};
use uuid::Uuid;
use workbench_backend::{OperatorApp, OperatorDashboard, TruthExecutionSession, TruthListItem};

fn default_desktop_store_endpoint() -> String {
    let path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".desktop-surreal-test-flows");
    format!("rocksdb://{}", path.display())
}

fn desktop_storage_config() -> AppConfig {
    let endpoint = default_desktop_store_endpoint();
    if let Some(path) = endpoint.strip_prefix("rocksdb://") {
        let _ = std::fs::create_dir_all(path);
    }
    let mut config = AppConfig::from_env();
    config.record_store = RecordStoreConfig::Surreal(SurrealStoreConfig {
        endpoint,
        namespace: "outcome_workbench".to_string(),
        database: "desktop_test".to_string(),
        username: None,
        password: None,
    });
    config
}

use application_kernel::{
    Actor, BillingPeriod, CatalogItemUpsert, CatalogPlanKind, EntitlementTemplate, Money,
    OrganizationLifecycle, OrganizationUpsert, SubscriptionActivate, SubscriptionCreate,
    SubscriptionStatus,
};
use application_storage::KernelStore;
use std::collections::BTreeMap;

fn seed_uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("valid seed uuid")
}

fn seed_revenue_data(store: &SurrealDbKernelStore) {
    let actor = Actor::system();
    let organization_id = seed_uuid("11111111-1111-4111-8111-111111111111");
    let subscription_catalog_id = seed_uuid("22222222-2222-4222-8222-222222222222");
    let activation_subscription_id = seed_uuid("33333333-3333-4333-8333-333333333333");
    let credits_catalog_id = seed_uuid("44444444-4444-4444-8444-444444444444");
    let refill_subscription_id = seed_uuid("55555555-5555-4555-8555-555555555555");

    let _ = store.write(|kernel| {
        kernel
            .upsert_organization(
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
            )
            .unwrap();

        kernel
            .upsert_catalog_item(
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
                        feature_flags: vec![
                            "workspace_access".to_string(),
                            "audit_trail".to_string(),
                        ],
                        quotas: BTreeMap::from([("seats".to_string(), 25)]),
                        credit_balance_minor: None,
                    },
                    active: true,
                },
                actor.clone(),
            )
            .unwrap();

        kernel
            .upsert_catalog_item(
                CatalogItemUpsert {
                    catalog_item_id: Some(credits_catalog_id),
                    sku: "prio-ai-credits-500".to_string(),
                    name: "Prepaid AI Credits ($500)".to_string(),
                    description: Some("Add $500 to prepaid credit balance".to_string()),
                    plan_kind: CatalogPlanKind::PrepaidCredits,
                    pricing: Some(application_kernel::PricingMetadata {
                        billing_period: BillingPeriod::OneTime,
                        list_price: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 500_00,
                        },
                        meter_name: None,
                    }),
                    entitlement_template: EntitlementTemplate {
                        feature_flags: vec![],
                        quotas: BTreeMap::new(),
                        credit_balance_minor: Some(0),
                    },
                    active: true,
                },
                actor.clone(),
            )
            .unwrap();

        kernel
            .create_order_subscription(
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
            )
            .unwrap();

        let refill_subscription = kernel
            .create_order_subscription(
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
            )
            .unwrap();

        kernel
            .activate_subscription(
                SubscriptionActivate {
                    subscription_id: refill_subscription.id,
                    catalog_item_id: None,
                    opening_balance: None,
                },
                actor.clone(),
            )
            .unwrap();

        Ok(())
    });
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "persistence" {
        println!("\nFlow 4: Persistence");
        let config = desktop_storage_config();
        let store =
            SurrealDbKernelStore::connect_blocking(config).expect("failed to connect persistence");
        let operator = OperatorApp::new(store.config.clone(), store);
        let orgs = operator.list_organizations().unwrap();
        println!("Reopened Orgs count: {}", orgs.len());
        let found = orgs.iter().any(|o| o.name == "TestOrg");
        println!(
            "Previously projected Org 'TestOrg' still present: {}",
            found
        );
        return;
    }

    println!("--- Starting Flow Tests ---");

    // Clear previous DB
    let endpoint = default_desktop_store_endpoint();
    if let Some(path) = endpoint.strip_prefix("rocksdb://") {
        let _ = std::fs::remove_dir_all(path);
    }

    let config = desktop_storage_config();
    let store = SurrealDbKernelStore::connect_blocking(config.clone()).expect("failed to connect");

    // Seed DB
    seed_revenue_data(&store);

    let operator = OperatorApp::new(store.config.clone(), store.clone());

    // FLOW 1: Lead qualification
    println!("\nFlow 1: Lead qualification");
    let f1_res = operator.execute_truth(
        "qualify-inbound-lead",
        HashMap::from([
            ("organization_name".to_string(), "TestOrg".to_string()),
            ("inbound_summary".to_string(), "Test summary".to_string()),
            ("contact_name".to_string(), "Test Contact".to_string()),
            ("contact_email".to_string(), "test@test.com".to_string()),
        ]),
    );
    let mut f1_org_id = None;
    match f1_res {
        Ok(session) => {
            println!("Execution state: {:?}", session.state);
            let org_id = session
                .projection
                .as_ref()
                .and_then(|p| p.organization_id.clone());
            println!("Projected Org ID: {:?}", org_id);
            f1_org_id = org_id.clone();

            let orgs = operator.list_organizations().unwrap();
            println!("Orgs count: {}", orgs.len());
            let found = orgs.iter().any(|o| Some(o.id.clone()) == org_id);
            println!("Org in accounts view: {}", found);

            if let Some(id) = org_id {
                let summary = operator.account_summary(&id).unwrap();
                println!("Account summary people: {:?}", summary.people.len());
            }
        }
        Err(e) => println!("F1 Error: {:?}", e),
    }

    // FLOW 2: Subscription activation
    println!("\nFlow 2: Subscription activation");
    let rev_org_id = "11111111-1111-4111-8111-111111111111";
    let rev_sub_id = "33333333-3333-4333-8333-333333333333";
    let rev_cat_id = "22222222-2222-4222-8222-222222222222";
    let f2_res = operator.execute_truth(
        "activate-subscription",
        HashMap::from([
            ("organization_id".to_string(), rev_org_id.to_string()),
            ("subscription_id".to_string(), rev_sub_id.to_string()),
            ("catalog_item_id".to_string(), rev_cat_id.to_string()),
            ("payment_confirmed".to_string(), "true".to_string()),
        ]),
    );
    match f2_res {
        Ok(session) => {
            println!("Execution state: {:?}", session.state);
            let subs = operator
                .list_subscriptions(Some(rev_org_id))
                .unwrap_or_default();
            if let Some(sub) = subs.iter().find(|s| s.id == rev_sub_id) {
                println!("Subscription status in revenue view: {:?}", sub.status);
            } else {
                println!("Subscription not found in revenue view");
            }
        }
        Err(e) => println!("F2 Error: {:?}", e),
    }

    // FLOW 3: Credit refill
    println!("\nFlow 3: Credit refill");
    let refill_sub_id = "55555555-5555-4555-8555-555555555555";
    let f3_res = operator.execute_truth(
        "refill-prepaid-ai-credits",
        HashMap::from([
            ("organization_id".to_string(), rev_org_id.to_string()),
            ("subscription_id".to_string(), refill_sub_id.to_string()),
            ("amount_minor".to_string(), "10000".to_string()),
            ("currency_code".to_string(), "USD".to_string()),
            ("payment_reference".to_string(), "ref123".to_string()),
            ("payment_status".to_string(), "confirmed".to_string()),
        ]),
    );
    match f3_res {
        Ok(session) => {
            println!("Execution state: {:?}", session.state);
            let summary = operator.account_summary(rev_org_id).unwrap();
            if let Some(entitlement) = summary
                .entitlements
                .iter()
                .find(|e| e.key == "credit_balance_minor")
            {
                println!(
                    "Ledger entry found in account summary: {} = {:?}",
                    entitlement.key, entitlement.value_summary
                );
            } else {
                println!("Ledger entry NOT found in account summary");
            }
        }
        Err(e) => println!("F3 Error: {:?}", e),
    }

    // FLOW 5 (Doing before drop for Persistence)
    println!("\nFlow 5: Truth catalog");
    let truths = operator.list_truths();
    let exec_count = truths.iter().filter(|t| t.executable).count();
    let non_exec_count = truths.iter().filter(|t| !t.executable).count();
    println!(
        "Truths: {} executable, {} catalog-only",
        exec_count, non_exec_count
    );

    // Test execute from detail route for criteria outcomes
    if let Ok(session) = operator.execute_truth(
        "qualify-inbound-lead",
        HashMap::from([
            ("organization_name".to_string(), "AnotherOrg".to_string()),
            ("inbound_summary".to_string(), "Summary 2".to_string()),
            ("contact_name".to_string(), "Another Contact".to_string()),
            ("contact_email".to_string(), "another@test.com".to_string()),
        ]),
    ) {
        println!("Criteria outcomes: {}", session.criteria_outcomes.len());
        println!(
            "Projection summary present: {}",
            session.projection.is_some()
        );
    }

    println!("--- Flow Tests Complete ---");
}
