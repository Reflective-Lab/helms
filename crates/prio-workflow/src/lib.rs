use prio_module_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct WorkflowModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "workflow",
    display_name: "Workflow",
    suite: ModuleSuite::WorkCore,
    crate_name: "prio-workflow",
    purpose:
        "Stateful business execution with cases, steps, transitions, deadlines, and wait states.",
    dependencies: &["identity", "parties"],
    owned_objects: &[
        "workflow_definition",
        "case",
        "step",
        "transition",
        "waiting_state",
        "deadline",
    ],
    api: ApiSurface {
        grpc_package: "prio.workflow.v1",
        grpc_service: "WorkflowService",
        openapi_tag: "Workflow",
        openapi_base_path: "/v1/workflow",
        graphql_query_root: "WorkflowQuery",
        graphql_mutation_root: "WorkflowMutation",
    },
};

impl ModuleManifest for WorkflowModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
