use prio_module_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct IdentityModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "identity",
    display_name: "Identity",
    suite: ModuleSuite::Foundation,
    crate_name: "prio-identity",
    purpose: "Authentication, authorization, tenancy, and workspace membership.",
    dependencies: &[],
    owned_objects: &[
        "user",
        "team",
        "role",
        "permission",
        "tenant",
        "workspace_membership",
    ],
    api: ApiSurface {
        grpc_package: "prio.identity.v1",
        grpc_service: "IdentityService",
        openapi_tag: "Identity",
        openapi_base_path: "/v1/identity",
        graphql_query_root: "IdentityQuery",
        graphql_mutation_root: "IdentityMutation",
    },
};

impl ModuleManifest for IdentityModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
