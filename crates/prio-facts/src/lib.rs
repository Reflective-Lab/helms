use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct FactsModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "facts",
    display_name: "Facts",
    suite: ModuleSuite::TrustCore,
    crate_name: "prio-facts",
    purpose: "Proposed facts, durable facts, evidence, provenance, supersession, and promotion decisions.",
    dependencies: &["policies", "approvals", "documents", "parties"],
    owned_objects: &[
        "proposed_fact",
        "fact",
        "confidence",
        "evidence",
        "promotion_decision",
        "supersession",
        "provenance_link",
    ],
    api: ApiSurface {
        grpc_package: "prio.facts.v1",
        grpc_service: "FactsService",
        openapi_tag: "Facts",
        openapi_base_path: "/v1/facts",
        graphql_query_root: "FactsQuery",
        graphql_mutation_root: "FactsMutation",
    },
};

impl ModuleManifest for FactsModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
