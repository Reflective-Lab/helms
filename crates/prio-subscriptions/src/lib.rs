use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct SubscriptionsModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "subscriptions",
    display_name: "Subscriptions",
    suite: ModuleSuite::CommercialCore,
    crate_name: "prio-subscriptions",
    purpose: "Orders, subscriptions, recurring commitments, billing periods, and usage plans.",
    dependencies: &["parties"],
    owned_objects: &[
        "order",
        "subscription",
        "subscription_item",
        "billing_period",
        "usage_plan",
        "credit_balance",
    ],
    api: ApiSurface {
        grpc_package: "prio.subscriptions.v1",
        grpc_service: "SubscriptionsService",
        openapi_tag: "Subscriptions",
        openapi_base_path: "/v1/subscriptions",
        graphql_query_root: "SubscriptionsQuery",
        graphql_mutation_root: "SubscriptionsMutation",
    },
};

impl ModuleManifest for SubscriptionsModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
