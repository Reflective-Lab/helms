//! Transitional commercial module manifest.
//!
//! Boundary debt under H-2026-06-15-02: do not use this crate as new
//! marquee-app commercial ledger authority. See
//! kb/Architecture/Commercial Authority Inventory.md.

use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct LedgerModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "ledger",
    display_name: "Ledger",
    suite: ModuleSuite::UsageRevenueCore,
    crate_name: "prio-ledger",
    purpose: "Auditable balance math, credit grants, debits, adjustments, and settlement-grade history.",
    dependencies: &["parties", "subscriptions", "metering"],
    owned_objects: &[
        "account_balance",
        "credit_grant",
        "debit",
        "adjustment",
        "ledger_entry",
    ],
    api: ApiSurface {
        grpc_package: "prio.ledger.v1",
        grpc_service: "LedgerService",
        openapi_tag: "Ledger",
        openapi_base_path: "/v1/ledger",
        graphql_query_root: "LedgerQuery",
        graphql_mutation_root: "LedgerMutation",
    },
};

impl ModuleManifest for LedgerModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
