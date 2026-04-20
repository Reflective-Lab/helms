use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct TasksModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "tasks",
    display_name: "Tasks",
    suite: ModuleSuite::WorkCore,
    crate_name: "prio-tasks",
    purpose: "Human and agent work queues, task dependencies, handoffs, and completion state.",
    dependencies: &["identity", "parties", "workflow"],
    owned_objects: &[
        "task",
        "checklist",
        "assignee",
        "due_date",
        "dependency",
        "completion_state",
    ],
    api: ApiSurface {
        grpc_package: "prio.tasks.v1",
        grpc_service: "TasksService",
        openapi_tag: "Tasks",
        openapi_base_path: "/v1/tasks",
        graphql_query_root: "TasksQuery",
        graphql_mutation_root: "TasksMutation",
    },
};

impl ModuleManifest for TasksModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
