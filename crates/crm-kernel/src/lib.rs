mod capabilities;
mod error;
mod events;
mod kernel;
mod model;

pub use capabilities::{
    ConversationsCommands, DocumentsCommands, FactsCommands, IdentityCommands, MetadataCommands,
    OpportunitiesCommands, PartiesCommands, RevenueCommands, WorkflowCommands,
};
pub use error::{KernelError, KernelResult};
pub use events::DomainEvent;
pub use kernel::{
    ActivityAppend, CatalogItemUpsert, CommunicationRecord, CreditGrantApplication,
    CreditGrantApply, CrmKernel, DocumentAttach, FactRecord, NoteAppend, ObjectDefinitionUpsert,
    OpportunityAdvance, OpportunityCreate, OrganizationUpsert, PermissionGrantInput, PersonUpsert,
    RelationshipLink, SubscriptionActivate, SubscriptionActivation, SubscriptionCreate,
    SubscriptionPlanChange, SubscriptionPlanChangeResult, SubscriptionSuspend,
    SubscriptionSuspension, ViewDefinitionUpsert, WorkflowCaseAdvance, WorkflowCaseCreate,
};
pub use model::*;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use uuid::Uuid;

    use crate::{
        ActivityAppend, ActivityOutcome, Actor, ActorKind, BillingPeriod, CatalogItemUpsert,
        CatalogPlanKind, CreditGrantApply, CrmKernel, DomainEvent, EntitlementTemplate, FactRecord,
        Money, ObjectDefinitionKind, ObjectDefinitionUpsert, OpportunityCreate,
        OrganizationLifecycle, OrganizationUpsert, PersonUpsert, RecordKind, RecordRef,
        SubscriptionActivate, SubscriptionCreate, SubscriptionPlanChange, SubscriptionStatus,
        SubscriptionSuspend, ViewDefinitionUpsert, ViewLayout,
    };

    fn human() -> Actor {
        Actor {
            actor_id: "user-1".to_string(),
            display_name: "Kenneth".to_string(),
            kind: ActorKind::Human,
        }
    }

    #[test]
    fn fact_promotion_marks_source_note() {
        let actor = human();
        let mut kernel = CrmKernel::default();
        let organization = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: None,
                    name: "Aprio".to_string(),
                    external_key: None,
                    website: None,
                    industry: None,
                    lifecycle: OrganizationLifecycle::Prospect,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("organization should be created");

        let note = kernel
            .append_note(
                crate::NoteAppend {
                    subject: "Buying signal".to_string(),
                    body: "Champion asked for implementation timing.".to_string(),
                    related_to: vec![RecordRef {
                        kind: RecordKind::Organization,
                        id: organization.id,
                    }],
                },
                actor.clone(),
            )
            .expect("note should be created");

        kernel
            .record_fact(
                FactRecord {
                    statement: "The buyer is actively evaluating implementation timing."
                        .to_string(),
                    confidence_bps: 8_500,
                    related_to: vec![RecordRef {
                        kind: RecordKind::Organization,
                        id: organization.id,
                    }],
                    source_note_id: Some(note.id),
                },
                actor,
            )
            .expect("fact should be recorded");

        assert!(
            kernel
                .notes
                .get(&note.id)
                .expect("note exists")
                .promoted_to_fact
        );
    }

    #[test]
    fn account_summary_collects_related_records() {
        let actor = human();
        let mut kernel = CrmKernel::default();
        let organization = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: None,
                    name: "Northwind".to_string(),
                    external_key: None,
                    website: None,
                    industry: Some("software".to_string()),
                    lifecycle: OrganizationLifecycle::Active,
                    owner_user_id: Some("owner-1".to_string()),
                    tags: vec!["priority".to_string()],
                },
                actor.clone(),
            )
            .expect("organization should be created");

        let person = kernel
            .upsert_person(
                PersonUpsert {
                    person_id: None,
                    organization_id: Some(organization.id),
                    full_name: "Alice Doe".to_string(),
                    title: Some("CTO".to_string()),
                    email: None,
                    phone: None,
                    linkedin_url: None,
                },
                actor.clone(),
            )
            .expect("person should be created");

        let opportunity = kernel
            .create_opportunity(
                OpportunityCreate {
                    organization_id: organization.id,
                    primary_contact_id: Some(person.id),
                    name: "Expansion deal".to_string(),
                    value: Money {
                        currency_code: "USD".to_string(),
                        amount_minor: 120_000_00,
                    },
                    confidence_bps: 6_000,
                    next_step: Some("Send architecture note".to_string()),
                    expected_close_at: None,
                },
                actor.clone(),
            )
            .expect("opportunity should be created");

        kernel
            .append_activity(
                ActivityAppend {
                    subject: "Discovery call".to_string(),
                    details: "Validated JTBD and approval path.".to_string(),
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
                actor,
            )
            .expect("activity should be created");

        let summary = kernel
            .get_account_summary(organization.id, 10)
            .expect("summary should resolve");

        assert_eq!(summary.organization.name, "Northwind");
        assert_eq!(summary.contacts.len(), 1);
        assert_eq!(summary.opportunities.len(), 1);
        assert!(!summary.recent_timeline.is_empty());
    }

    #[test]
    fn list_people_filters_by_account() {
        let actor = human();
        let mut kernel = CrmKernel::default();
        let first = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: Some(Uuid::new_v4()),
                    name: "First".to_string(),
                    external_key: None,
                    website: None,
                    industry: None,
                    lifecycle: OrganizationLifecycle::Prospect,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("first org");
        let second = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: Some(Uuid::new_v4()),
                    name: "Second".to_string(),
                    external_key: None,
                    website: None,
                    industry: None,
                    lifecycle: OrganizationLifecycle::Prospect,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("second org");

        kernel
            .upsert_person(
                PersonUpsert {
                    person_id: None,
                    organization_id: Some(first.id),
                    full_name: "Alice".to_string(),
                    title: None,
                    email: None,
                    phone: None,
                    linkedin_url: None,
                },
                actor.clone(),
            )
            .expect("alice");

        kernel
            .upsert_person(
                PersonUpsert {
                    person_id: None,
                    organization_id: Some(second.id),
                    full_name: "Bob".to_string(),
                    title: None,
                    email: None,
                    phone: None,
                    linkedin_url: None,
                },
                actor,
            )
            .expect("bob");

        assert_eq!(kernel.list_people(Some(first.id)).len(), 1);
    }

    #[test]
    fn metadata_requires_object_before_view() {
        let actor = human();
        let mut kernel = CrmKernel::default();

        kernel
            .upsert_object_definition(
                ObjectDefinitionUpsert {
                    object_definition_id: None,
                    key: "usage_event".to_string(),
                    display_name: "Usage Event".to_string(),
                    kind: ObjectDefinitionKind::Custom,
                    fields: vec![],
                    relationships: vec![],
                    active: true,
                },
                actor.clone(),
            )
            .expect("object definition");

        kernel
            .upsert_view_definition(
                ViewDefinitionUpsert {
                    view_definition_id: None,
                    object_key: "usage_event".to_string(),
                    name: "High intent visits".to_string(),
                    layout: ViewLayout::Table,
                    filter_expression: Some("path startsWith /pricing".to_string()),
                    sort_expression: None,
                    visible_fields: vec!["session_id".to_string(), "path".to_string()],
                    group_by: None,
                    favorite: true,
                    owner_user_id: None,
                },
                actor,
            )
            .expect("view definition");

        assert_eq!(kernel.list_object_definitions().len(), 1);
        assert_eq!(kernel.list_view_definitions(Some("usage_event")).len(), 1);
    }

    #[test]
    fn organization_upsert_emits_domain_audit_and_timeline_events() {
        let actor = human();
        let mut kernel = CrmKernel::default();

        let organization = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: None,
                    name: "Converge".to_string(),
                    external_key: None,
                    website: Some("https://converge.zone".to_string()),
                    industry: None,
                    lifecycle: OrganizationLifecycle::Prospect,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("organization should be created");

        let events = kernel.drain_events();
        assert_eq!(events.len(), 3);
        assert!(matches!(
            &events[0],
            DomainEvent::OrganizationUpserted {
                organization: event_org,
                actor: event_actor
            } if event_org.id == organization.id && event_actor.actor_id == actor.actor_id
        ));
        assert!(matches!(&events[1], DomainEvent::AuditRecorded { .. }));
        assert!(matches!(
            &events[2],
            DomainEvent::TimelineEntryRecorded { .. }
        ));
    }

    #[test]
    fn activate_subscription_creates_entitlements_and_opening_balance() {
        let actor = human();
        let mut kernel = CrmKernel::default();
        let organization = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: None,
                    name: "Revenue Test".to_string(),
                    external_key: None,
                    website: None,
                    industry: None,
                    lifecycle: OrganizationLifecycle::Active,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("organization should be created");

        let catalog_item = kernel
            .upsert_catalog_item(
                CatalogItemUpsert {
                    catalog_item_id: None,
                    sku: "prio-pro".to_string(),
                    name: "Prio Pro".to_string(),
                    description: Some("Annual plan".to_string()),
                    plan_kind: CatalogPlanKind::Subscription,
                    pricing: Some(crate::PricingMetadata {
                        billing_period: BillingPeriod::Annual,
                        list_price: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 12_000_00,
                        },
                        meter_name: Some("annual-seat".to_string()),
                    }),
                    entitlement_template: EntitlementTemplate {
                        feature_flags: vec!["workspace_access".to_string()],
                        quotas: BTreeMap::from([("seats".to_string(), 25)]),
                        credit_balance_minor: Some(500_000),
                    },
                    active: true,
                },
                actor.clone(),
            )
            .expect("catalog item should be created");

        let subscription = kernel
            .create_order_subscription(
                SubscriptionCreate {
                    subscription_id: None,
                    organization_id: organization.id,
                    quote_id: None,
                    catalog_item_id: Some(catalog_item.id),
                    status: SubscriptionStatus::PendingActivation,
                    value: Money {
                        currency_code: "USD".to_string(),
                        amount_minor: 12_000_00,
                    },
                    started_at: None,
                },
                actor.clone(),
            )
            .expect("subscription should be created");

        let activation = kernel
            .activate_subscription(
                SubscriptionActivate {
                    subscription_id: subscription.id,
                    catalog_item_id: None,
                    opening_balance: None,
                },
                actor.clone(),
            )
            .expect("subscription should activate");

        assert_eq!(activation.subscription.status, SubscriptionStatus::Active);
        assert_eq!(
            activation.subscription.catalog_item_id,
            Some(catalog_item.id)
        );
        assert_eq!(activation.entitlements.len(), 3);
        assert_eq!(
            activation.opening_balance.kind,
            crate::LedgerEntryKind::OpeningBalance
        );
        assert_eq!(kernel.list_entitlements(Some(organization.id)).len(), 3);
        assert_eq!(kernel.list_ledger_entries(Some(organization.id)).len(), 1);
        assert!(
            kernel
                .pending_events
                .iter()
                .any(|event| matches!(event, DomainEvent::OrderSubscriptionStateChanged { .. }))
        );
    }

    #[test]
    fn activate_subscription_requires_valid_catalog_plan() {
        let actor = human();
        let mut kernel = CrmKernel::default();
        let organization = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: None,
                    name: "Blocked Revenue Test".to_string(),
                    external_key: None,
                    website: None,
                    industry: None,
                    lifecycle: OrganizationLifecycle::Active,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("organization should be created");

        let catalog_item = kernel
            .upsert_catalog_item(
                CatalogItemUpsert {
                    catalog_item_id: None,
                    sku: "prio-custom".to_string(),
                    name: "Prio Custom".to_string(),
                    description: None,
                    plan_kind: CatalogPlanKind::EnterpriseCustom,
                    pricing: None,
                    entitlement_template: EntitlementTemplate {
                        feature_flags: vec!["workspace_access".to_string()],
                        quotas: BTreeMap::new(),
                        credit_balance_minor: None,
                    },
                    active: true,
                },
                actor.clone(),
            )
            .expect("catalog item should be created");

        let subscription = kernel
            .create_order_subscription(
                SubscriptionCreate {
                    subscription_id: None,
                    organization_id: organization.id,
                    quote_id: None,
                    catalog_item_id: Some(catalog_item.id),
                    status: SubscriptionStatus::PendingActivation,
                    value: Money {
                        currency_code: "USD".to_string(),
                        amount_minor: 25_000_00,
                    },
                    started_at: None,
                },
                actor,
            )
            .expect("subscription should be created");

        let error = kernel
            .activate_subscription(
                SubscriptionActivate {
                    subscription_id: subscription.id,
                    catalog_item_id: None,
                    opening_balance: None,
                },
                human(),
            )
            .expect_err("activation should reject incomplete plan metadata");

        assert!(matches!(error, crate::KernelError::Invariant(_)));
        assert!(kernel.list_entitlements(Some(organization.id)).is_empty());
        assert!(kernel.list_ledger_entries(Some(organization.id)).is_empty());
    }

    #[test]
    fn change_subscription_plan_replaces_entitlements_and_records_delta() {
        let actor = human();
        let mut kernel = CrmKernel::default();
        let organization = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: None,
                    name: "Upgrade Test".to_string(),
                    external_key: None,
                    website: None,
                    industry: None,
                    lifecycle: OrganizationLifecycle::Active,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("organization");
        let starter = kernel
            .upsert_catalog_item(
                CatalogItemUpsert {
                    catalog_item_id: None,
                    sku: "prio-starter".to_string(),
                    name: "Prio Starter".to_string(),
                    description: None,
                    plan_kind: CatalogPlanKind::Subscription,
                    pricing: Some(crate::PricingMetadata {
                        billing_period: BillingPeriod::Monthly,
                        list_price: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 2_000_00,
                        },
                        meter_name: Some("starter-seat".to_string()),
                    }),
                    entitlement_template: EntitlementTemplate {
                        feature_flags: vec!["workspace_access".to_string()],
                        quotas: BTreeMap::from([("seats".to_string(), 5)]),
                        credit_balance_minor: Some(100_000),
                    },
                    active: true,
                },
                actor.clone(),
            )
            .expect("starter");
        let growth = kernel
            .upsert_catalog_item(
                CatalogItemUpsert {
                    catalog_item_id: None,
                    sku: "prio-growth".to_string(),
                    name: "Prio Growth".to_string(),
                    description: None,
                    plan_kind: CatalogPlanKind::Subscription,
                    pricing: Some(crate::PricingMetadata {
                        billing_period: BillingPeriod::Monthly,
                        list_price: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 5_000_00,
                        },
                        meter_name: Some("growth-seat".to_string()),
                    }),
                    entitlement_template: EntitlementTemplate {
                        feature_flags: vec![
                            "workspace_access".to_string(),
                            "priority_support".to_string(),
                        ],
                        quotas: BTreeMap::from([("seats".to_string(), 25)]),
                        credit_balance_minor: Some(300_000),
                    },
                    active: true,
                },
                actor.clone(),
            )
            .expect("growth");
        let subscription = kernel
            .create_order_subscription(
                SubscriptionCreate {
                    subscription_id: None,
                    organization_id: organization.id,
                    quote_id: None,
                    catalog_item_id: Some(starter.id),
                    status: SubscriptionStatus::PendingActivation,
                    value: Money {
                        currency_code: "USD".to_string(),
                        amount_minor: 2_000_00,
                    },
                    started_at: None,
                },
                actor.clone(),
            )
            .expect("subscription");
        kernel
            .activate_subscription(
                SubscriptionActivate {
                    subscription_id: subscription.id,
                    catalog_item_id: None,
                    opening_balance: None,
                },
                actor.clone(),
            )
            .expect("activation");
        kernel
            .apply_credit_grant(
                CreditGrantApply {
                    subscription_id: subscription.id,
                    amount: Money {
                        currency_code: "USD".to_string(),
                        amount_minor: 50_000,
                    },
                    payment_reference: "pay_upgrade_seed".to_string(),
                    reason: Some("Top-up".to_string()),
                },
                actor.clone(),
            )
            .expect("seed credit balance");

        let changed = kernel
            .change_subscription_plan(
                SubscriptionPlanChange {
                    subscription_id: subscription.id,
                    target_catalog_item_id: growth.id,
                    effective_at: chrono::Utc::now(),
                    target_value: None,
                    reason: Some("Customer accepted growth upgrade".to_string()),
                },
                actor.clone(),
            )
            .expect("plan change should apply");

        assert_eq!(changed.subscription.catalog_item_id, Some(growth.id));
        assert_eq!(changed.subscription.value.amount_minor, 5_000_00);
        assert_eq!(
            changed.ledger_entry.kind,
            crate::LedgerEntryKind::Adjustment
        );
        assert_eq!(changed.ledger_entry.amount.amount_minor, 3_000_00);
        assert_eq!(changed.entitlements.len(), 4);
        assert!(matches!(
            changed
                .entitlements
                .iter()
                .find(|entitlement| entitlement.key == "credit_balance_minor")
                .expect("credit entitlement")
                .value,
            crate::EntitlementValue::Credits(350_000)
        ));
        assert!(
            kernel
                .pending_events
                .iter()
                .any(|event| matches!(event, DomainEvent::OrderSubscriptionPlanChanged { .. }))
        );
        assert!(
            kernel
                .pending_events
                .iter()
                .any(|event| matches!(event, DomainEvent::EntitlementsReplaced { .. }))
        );
    }

    #[test]
    fn credit_grant_updates_balance_and_appends_ledger_entry() {
        let actor = human();
        let mut kernel = CrmKernel::default();
        let organization = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: None,
                    name: "Credits Test".to_string(),
                    external_key: None,
                    website: None,
                    industry: None,
                    lifecycle: OrganizationLifecycle::Active,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("organization");
        let catalog_item = kernel
            .upsert_catalog_item(
                CatalogItemUpsert {
                    catalog_item_id: None,
                    sku: "prio-credits".to_string(),
                    name: "Prio Credits".to_string(),
                    description: None,
                    plan_kind: CatalogPlanKind::PrepaidCredits,
                    pricing: Some(crate::PricingMetadata {
                        billing_period: BillingPeriod::OneTime,
                        list_price: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 5_000_00,
                        },
                        meter_name: Some("credit-pack".to_string()),
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
            .expect("catalog item");
        let subscription = kernel
            .create_order_subscription(
                SubscriptionCreate {
                    subscription_id: None,
                    organization_id: organization.id,
                    quote_id: None,
                    catalog_item_id: Some(catalog_item.id),
                    status: SubscriptionStatus::PendingActivation,
                    value: Money {
                        currency_code: "USD".to_string(),
                        amount_minor: 5_000_00,
                    },
                    started_at: None,
                },
                actor.clone(),
            )
            .expect("subscription");
        kernel
            .activate_subscription(
                SubscriptionActivate {
                    subscription_id: subscription.id,
                    catalog_item_id: None,
                    opening_balance: None,
                },
                actor.clone(),
            )
            .expect("activation");

        let grant = kernel
            .apply_credit_grant(
                CreditGrantApply {
                    subscription_id: subscription.id,
                    amount: Money {
                        currency_code: "USD".to_string(),
                        amount_minor: 150_000,
                    },
                    payment_reference: "pay_123".to_string(),
                    reason: Some("Top-up package".to_string()),
                },
                actor.clone(),
            )
            .expect("credit grant should apply");

        assert_eq!(grant.ledger_entry.kind, crate::LedgerEntryKind::CreditGrant);
        assert!(matches!(
            grant.entitlement.value,
            crate::EntitlementValue::Credits(150_000)
        ));
        assert_eq!(kernel.list_ledger_entries(Some(organization.id)).len(), 2);
        assert!(
            kernel
                .pending_events
                .iter()
                .any(|event| matches!(event, DomainEvent::EntitlementAdjusted { .. }))
        );
    }

    #[test]
    fn suspend_subscription_sets_state_and_service_access_marker() {
        let actor = human();
        let mut kernel = CrmKernel::default();
        let organization = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: None,
                    name: "Suspension Test".to_string(),
                    external_key: None,
                    website: None,
                    industry: None,
                    lifecycle: OrganizationLifecycle::Active,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("organization");
        let catalog_item = kernel
            .upsert_catalog_item(
                CatalogItemUpsert {
                    catalog_item_id: None,
                    sku: "prio-workspace".to_string(),
                    name: "Prio Workspace".to_string(),
                    description: None,
                    plan_kind: CatalogPlanKind::Subscription,
                    pricing: Some(crate::PricingMetadata {
                        billing_period: BillingPeriod::Monthly,
                        list_price: Money {
                            currency_code: "USD".to_string(),
                            amount_minor: 2_000_00,
                        },
                        meter_name: Some("workspace-seat".to_string()),
                    }),
                    entitlement_template: EntitlementTemplate {
                        feature_flags: vec![
                            "workspace_access".to_string(),
                            "priority_support".to_string(),
                        ],
                        quotas: BTreeMap::from([("seats".to_string(), 5)]),
                        credit_balance_minor: Some(100_000),
                    },
                    active: true,
                },
                actor.clone(),
            )
            .expect("catalog item");
        let subscription = kernel
            .create_order_subscription(
                SubscriptionCreate {
                    subscription_id: None,
                    organization_id: organization.id,
                    quote_id: None,
                    catalog_item_id: Some(catalog_item.id),
                    status: SubscriptionStatus::PendingActivation,
                    value: Money {
                        currency_code: "USD".to_string(),
                        amount_minor: 2_000_00,
                    },
                    started_at: None,
                },
                actor.clone(),
            )
            .expect("subscription");
        kernel
            .activate_subscription(
                SubscriptionActivate {
                    subscription_id: subscription.id,
                    catalog_item_id: None,
                    opening_balance: None,
                },
                actor.clone(),
            )
            .expect("activation");

        let suspended = kernel
            .suspend_subscription(
                SubscriptionSuspend {
                    subscription_id: subscription.id,
                    occurred_at: chrono::Utc::now(),
                    reason: Some("invoice overdue beyond grace".to_string()),
                },
                actor,
            )
            .expect("suspension should apply");

        assert_eq!(suspended.subscription.status, SubscriptionStatus::Suspended);
        assert!(matches!(
            suspended
                .entitlements
                .iter()
                .find(|entitlement| entitlement.key == "workspace_access")
                .expect("workspace access entitlement")
                .value,
            crate::EntitlementValue::FeatureFlag(false)
        ));
        assert!(matches!(
            suspended
                .entitlements
                .iter()
                .find(|entitlement| entitlement.key == "service_access_state")
                .expect("service access state entitlement")
                .value,
            crate::EntitlementValue::Text(ref value) if value == "suspended"
        ));
        assert!(
            kernel
                .pending_events
                .iter()
                .any(|event| matches!(event, DomainEvent::OrderSubscriptionStateChanged { .. }))
        );
        assert!(
            kernel
                .pending_events
                .iter()
                .filter(|event| matches!(event, DomainEvent::EntitlementAdjusted { .. }))
                .count()
                >= 2
        );
    }
}
