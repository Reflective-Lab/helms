use prio_module_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct ApprovalsModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "approvals",
    display_name: "Approvals",
    suite: ModuleSuite::TrustCore,
    crate_name: "prio-approvals",
    purpose: "Human control points, approval requests, escalation, and explicit release decisions.",
    dependencies: &["identity", "workflow", "policies"],
    owned_objects: &[
        "approval_request",
        "approver",
        "decision",
        "rationale",
        "escalation",
    ],
    api: ApiSurface {
        grpc_package: "prio.approvals.v1",
        grpc_service: "ApprovalsService",
        openapi_tag: "Approvals",
        openapi_base_path: "/v1/approvals",
        graphql_query_root: "ApprovalsQuery",
        graphql_mutation_root: "ApprovalsMutation",
    },
};

impl ModuleManifest for ApprovalsModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
