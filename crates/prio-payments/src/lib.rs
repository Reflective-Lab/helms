//! Transitional commercial module manifest.
//!
//! Boundary debt under H-2026-06-15-02: do not use this crate as new
//! marquee-app payment authority. See
//! kb/Architecture/Commercial Authority Inventory.md.

use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct PaymentsModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "payments",
    display_name: "Payments",
    suite: ModuleSuite::UsageRevenueCore,
    crate_name: "prio-payments",
    purpose: "Payment state, provider reconciliation, refunds, and settlement visibility.",
    dependencies: &["parties", "ledger"],
    owned_objects: &[
        "payment",
        "payment_method",
        "transaction",
        "refund",
        "settlement",
    ],
    api: ApiSurface {
        grpc_package: "prio.payments.v1",
        grpc_service: "PaymentsService",
        openapi_tag: "Payments",
        openapi_base_path: "/v1/payments",
        graphql_query_root: "PaymentsQuery",
        graphql_mutation_root: "PaymentsMutation",
    },
};

impl ModuleManifest for PaymentsModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
