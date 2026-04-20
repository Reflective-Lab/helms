use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct MemoryModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "memory",
    display_name: "Memory",
    suite: ModuleSuite::IntelligenceCore,
    crate_name: "prio-memory",
    purpose: "Semantic memory, entity graph context, embeddings, and retrieval over business state.",
    dependencies: &["documents", "facts", "parties"],
    owned_objects: &[
        "entity",
        "relation",
        "embedding",
        "memory_fragment",
        "source_reference",
    ],
    api: ApiSurface {
        grpc_package: "prio.memory.v1",
        grpc_service: "MemoryService",
        openapi_tag: "Memory",
        openapi_base_path: "/v1/memory",
        graphql_query_root: "MemoryQuery",
        graphql_mutation_root: "MemoryMutation",
    },
};

impl ModuleManifest for MemoryModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
