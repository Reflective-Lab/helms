//! Helm-owned adjacent module manifest.
//!
//! Opportunity and pipeline state may remain Helm-owned, but pricing, plan
//! grants, payments, entitlements, and commercial ledger authority must resolve
//! through Commerce-Rails. See
//! kb/Architecture/Commercial Authority Inventory.md.

use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct OpportunitiesModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "opportunities",
    display_name: "Opportunities",
    suite: ModuleSuite::CommercialCore,
    crate_name: "prio-opportunities",
    purpose: "Lead intake, qualification, opportunity state, forecasting inputs, and pipeline semantics.",
    dependencies: &["identity", "parties", "conversations"],
    owned_objects: &[
        "lead",
        "source",
        "campaign_touch",
        "inbound_request",
        "qualification_state",
        "opportunity",
        "stage",
        "competitor",
    ],
    api: ApiSurface {
        grpc_package: "prio.opportunities.v1",
        grpc_service: "OpportunitiesService",
        openapi_tag: "Opportunities",
        openapi_base_path: "/v1/opportunities",
        graphql_query_root: "OpportunitiesQuery",
        graphql_mutation_root: "OpportunitiesMutation",
    },
};

impl ModuleManifest for OpportunitiesModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
