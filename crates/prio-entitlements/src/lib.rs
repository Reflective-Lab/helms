use prio_module_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct EntitlementsModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "entitlements",
    display_name: "Entitlements",
    suite: ModuleSuite::UsageRevenueCore,
    crate_name: "prio-entitlements",
    purpose: "Plan-based access, quotas, limits, feature flags, and access policy enforcement.",
    dependencies: &["parties", "subscriptions"],
    owned_objects: &[
        "entitlement",
        "feature_flag",
        "quota",
        "limit",
        "access_policy",
    ],
    api: ApiSurface {
        grpc_package: "prio.entitlements.v1",
        grpc_service: "EntitlementsService",
        openapi_tag: "Entitlements",
        openapi_base_path: "/v1/entitlements",
        graphql_query_root: "EntitlementsQuery",
        graphql_mutation_root: "EntitlementsMutation",
    },
};

impl ModuleManifest for EntitlementsModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
