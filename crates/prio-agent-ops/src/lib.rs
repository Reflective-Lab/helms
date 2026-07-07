use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct AgentOpsModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "agent-ops",
    display_name: "Agent Ops",
    suite: ModuleSuite::IntelligenceCore,
    crate_name: "prio-agent-ops",
    purpose: "Agent runs, operator control, validation contracts, and execution traceability.",
    dependencies: &["workflow", "facts", "audit", "approvals", "memory"],
    owned_objects: &[
        "agent",
        "agent_run",
        "tool_invocation",
        "job_readiness_packet",
        "operator_receipt",
        "operator_ledger_entry",
        "output_contract",
        "validation_result",
    ],
    api: ApiSurface {
        grpc_package: "prio.agentops.v1",
        grpc_service: "AgentOpsService",
        openapi_tag: "AgentOps",
        openapi_base_path: "/v1/agent-ops",
        graphql_query_root: "AgentOpsQuery",
        graphql_mutation_root: "AgentOpsMutation",
    },
};

impl ModuleManifest for AgentOpsModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
