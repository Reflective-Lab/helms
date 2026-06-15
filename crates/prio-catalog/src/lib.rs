//! Transitional commercial module manifest.
//!
//! Boundary debt under H-2026-06-15-02: do not use this crate as new
//! marquee-app plan, price, or catalog authority. See
//! kb/Architecture/Commercial Authority Inventory.md.

use capability_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct CatalogModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "catalog",
    display_name: "Catalog",
    suite: ModuleSuite::CommercialCore,
    crate_name: "prio-catalog",
    purpose: "Commercial catalog of products, plans, pricing, bundles, and sellable offers.",
    dependencies: &[],
    owned_objects: &["product", "service", "plan", "sku", "price", "bundle"],
    api: ApiSurface {
        grpc_package: "prio.catalog.v1",
        grpc_service: "CatalogService",
        openapi_tag: "Catalog",
        openapi_base_path: "/v1/catalog",
        graphql_query_root: "CatalogQuery",
        graphql_mutation_root: "CatalogMutation",
    },
};

impl ModuleManifest for CatalogModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
