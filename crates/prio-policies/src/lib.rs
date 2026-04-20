use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct PoliciesModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "policies",
    display_name: "Policies",
    suite: ModuleSuite::TrustCore,
    crate_name: "prio-policies",
    purpose: "Invariants, constraints, validation results, and business guardrails.",
    dependencies: &["identity"],
    owned_objects: &[
        "policy",
        "invariant",
        "constraint",
        "validation_result",
        "violation",
    ],
    api: ApiSurface {
        grpc_package: "prio.policies.v1",
        grpc_service: "PoliciesService",
        openapi_tag: "Policies",
        openapi_base_path: "/v1/policies",
        graphql_query_root: "PoliciesQuery",
        graphql_mutation_root: "PoliciesMutation",
    },
};

impl ModuleManifest for PoliciesModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
