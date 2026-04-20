use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct IntentsModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "intents",
    display_name: "Intents",
    suite: ModuleSuite::IntelligenceCore,
    crate_name: "prio-intents",
    purpose: "JTBD-oriented jobs, intent context, success criteria, outcomes, and current obstacles.",
    dependencies: &["parties", "workflow", "facts", "conversations"],
    owned_objects: &[
        "intent",
        "job",
        "job_context",
        "success_criterion",
        "outcome",
        "current_obstacle",
        "recommendation",
        "agent_run",
    ],
    api: ApiSurface {
        grpc_package: "prio.intents.v1",
        grpc_service: "IntentsService",
        openapi_tag: "Intents",
        openapi_base_path: "/v1/intents",
        graphql_query_root: "IntentsQuery",
        graphql_mutation_root: "IntentsMutation",
    },
};

impl ModuleManifest for IntentsModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
