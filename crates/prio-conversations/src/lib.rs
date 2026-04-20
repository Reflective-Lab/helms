use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct ConversationsModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "conversations",
    display_name: "Conversations",
    suite: ModuleSuite::WorkCore,
    crate_name: "prio-conversations",
    purpose: "Unified threads, messages, participants, and channel-level operational memory.",
    dependencies: &["identity", "parties", "documents"],
    owned_objects: &[
        "thread",
        "message",
        "participant",
        "channel",
        "summary",
        "attachment",
    ],
    api: ApiSurface {
        grpc_package: "prio.conversations.v1",
        grpc_service: "ConversationsService",
        openapi_tag: "Conversations",
        openapi_base_path: "/v1/conversations",
        graphql_query_root: "ConversationsQuery",
        graphql_mutation_root: "ConversationsMutation",
    },
};

impl ModuleManifest for ConversationsModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
