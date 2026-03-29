use prio_module_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct DocumentsModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "documents",
    display_name: "Documents",
    suite: ModuleSuite::WorkCore,
    crate_name: "prio-documents",
    purpose: "Documents, notes, files, extracted facts, and versioned knowledge artifacts.",
    dependencies: &["identity", "parties"],
    owned_objects: &[
        "document",
        "note",
        "file",
        "attachment",
        "extracted_fact",
        "version",
    ],
    api: ApiSurface {
        grpc_package: "prio.documents.v1",
        grpc_service: "DocumentsService",
        openapi_tag: "Documents",
        openapi_base_path: "/v1/documents",
        graphql_query_root: "DocumentsQuery",
        graphql_mutation_root: "DocumentsMutation",
    },
};

impl ModuleManifest for DocumentsModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
