use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModuleSuite {
    Foundation,
    RelationshipCore,
    CommercialCore,
    UsageRevenueCore,
    WorkCore,
    TrustCore,
    IntelligenceCore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ApiSurface {
    pub grpc_package: &'static str,
    pub grpc_service: &'static str,
    pub openapi_tag: &'static str,
    pub openapi_base_path: &'static str,
    pub graphql_query_root: &'static str,
    pub graphql_mutation_root: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CapabilityModule {
    pub key: &'static str,
    pub display_name: &'static str,
    pub suite: ModuleSuite,
    pub crate_name: &'static str,
    pub purpose: &'static str,
    pub dependencies: &'static [&'static str],
    pub owned_objects: &'static [&'static str],
    pub api: ApiSurface,
}

pub trait ModuleManifest {
    fn module() -> CapabilityModule;
}
