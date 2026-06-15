//! Transitional commercial module manifest.
//!
//! Boundary debt under H-2026-06-15-02: do not use this crate as new
//! marquee-app billing-metering authority. See
//! kb/Architecture/Commercial Authority Inventory.md.

use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct MeteringModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "metering",
    display_name: "Metering",
    suite: ModuleSuite::UsageRevenueCore,
    crate_name: "prio-metering",
    purpose: "Usage event ingestion, consumption translation, credit burn, and anomaly detection.",
    dependencies: &["parties", "subscriptions"],
    owned_objects: &[
        "usage_event",
        "meter",
        "consumption_record",
        "token_class",
        "pricing_unit",
        "anomaly",
    ],
    api: ApiSurface {
        grpc_package: "prio.metering.v1",
        grpc_service: "MeteringService",
        openapi_tag: "Metering",
        openapi_base_path: "/v1/metering",
        graphql_query_root: "MeteringQuery",
        graphql_mutation_root: "MeteringMutation",
    },
};

impl ModuleManifest for MeteringModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
