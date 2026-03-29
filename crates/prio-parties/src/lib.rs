use prio_module_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct PartiesModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "parties",
    display_name: "Parties",
    suite: ModuleSuite::RelationshipCore,
    crate_name: "prio-parties",
    purpose:
        "The CRM relationship kernel for people, organizations, accounts, and stakeholder graphs.",
    dependencies: &["identity"],
    owned_objects: &[
        "person",
        "organization",
        "account",
        "relationship",
        "contact_point",
        "address",
        "identifier",
    ],
    api: ApiSurface {
        grpc_package: "prio.parties.v1",
        grpc_service: "PartiesService",
        openapi_tag: "Parties",
        openapi_base_path: "/v1/parties",
        graphql_query_root: "PartiesQuery",
        graphql_mutation_root: "PartiesMutation",
    },
};

impl ModuleManifest for PartiesModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
