use prio_module_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct AuditModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "audit",
    display_name: "Audit",
    suite: ModuleSuite::TrustCore,
    crate_name: "prio-audit",
    purpose: "Audit trail, provenance, evidence links, and reconstructible business history.",
    dependencies: &["workflow", "facts"],
    owned_objects: &[
        "audit_event",
        "decision_trace",
        "evidence_link",
        "provenance_record",
    ],
    api: ApiSurface {
        grpc_package: "prio.audit.v1",
        grpc_service: "AuditService",
        openapi_tag: "Audit",
        openapi_base_path: "/v1/audit",
        graphql_query_root: "AuditQuery",
        graphql_mutation_root: "AuditMutation",
    },
};

impl ModuleManifest for AuditModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
