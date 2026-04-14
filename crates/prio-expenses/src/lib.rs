pub mod receipt_extractor;

use prio_module_core::{ApiSurface, CapabilityModule, ModuleManifest, ModuleSuite};

pub struct ExpensesModule;

pub const MODULE: CapabilityModule = CapabilityModule {
    key: "expenses",
    display_name: "Expenses",
    suite: ModuleSuite::WorkCore,
    crate_name: "prio-expenses",
    purpose: "Employee expenses, receipts, mileage, per diem, review workflow, and export readiness.",
    dependencies: &["parties", "documents", "workflow", "approvals", "policies"],
    owned_objects: &[
        "expense_report",
        "expense_item",
        "receipt_capture",
        "mileage_claim",
        "per_diem_claim",
        "export_batch",
    ],
    api: ApiSurface {
        grpc_package: "prio.expenses.v1",
        grpc_service: "ExpensesService",
        openapi_tag: "Expenses",
        openapi_base_path: "/v1/expenses",
        graphql_query_root: "ExpensesQuery",
        graphql_mutation_root: "ExpensesMutation",
    },
};

impl ModuleManifest for ExpensesModule {
    fn module() -> CapabilityModule {
        MODULE
    }
}
