mod http_api;
mod proto;
mod service;
mod truth_runtime;

use anyhow::Result;
use application_storage::{AppConfig, open_kernel_store, open_runtime_stores};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tonic::transport::Server;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::http_api::{HttpState, app_router};
use crate::proto::{
    conversations::conversations_service_server::ConversationsServiceServer,
    documents::documents_service_server::DocumentsServiceServer,
    facts::facts_service_server::FactsServiceServer,
    identity::identity_service_server::IdentityServiceServer,
    metadata::metadata_service_server::MetadataServiceServer,
    modules::module_registry_service_server::ModuleRegistryServiceServer,
    opportunities::opportunities_service_server::OpportunitiesServiceServer,
    parties::parties_service_server::PartiesServiceServer,
    truths::truth_catalog_service_server::TruthCatalogServiceServer,
    workflow::workflow_service_server::WorkflowServiceServer,
};
use crate::service::{
    ConversationsGrpc, DocumentsGrpc, FactsGrpc, IdentityGrpc, MetadataGrpc, ModuleRegistryGrpc,
    OpportunitiesGrpc, PartiesGrpc, TruthCatalogGrpc, WorkflowGrpc,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let grpc_addr: SocketAddr = std::env::var("CRM_GRPC_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50051".to_string())
        .parse()?;
    let http_addr: SocketAddr = std::env::var("CRM_HTTP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8081".to_string())
        .parse()?;

    let config = AppConfig::from_env();
    let store = open_kernel_store(config.clone()).await?;
    let runtime_stores = open_runtime_stores(&config).await?;
    let http_state = HttpState::new(
        config,
        store.clone(),
        runtime_stores.clone(),
        std::env::var("CRM_BILLING_INGRESS_TOKEN").ok(),
    );

    let http_app = app_router(http_state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let identity_service = IdentityGrpc::new(store.clone());
    let parties_service = PartiesGrpc::new(store.clone());
    let opportunities_service = OpportunitiesGrpc::new(store.clone());
    let conversations_service = ConversationsGrpc::new(store.clone());
    let documents_service = DocumentsGrpc::new(store.clone());
    let workflow_service = WorkflowGrpc::new(store.clone());
    let facts_service = FactsGrpc::new(store.clone());
    let metadata_service = MetadataGrpc::new(store.clone());
    let module_registry_service = ModuleRegistryGrpc::new();
    let truth_catalog_service = TruthCatalogGrpc::new(store, runtime_stores);

    info!("starting gRPC server on {}", grpc_addr);
    info!("starting HTTP server on {}", http_addr);

    let grpc = async move {
        Server::builder()
            .add_service(IdentityServiceServer::new(identity_service))
            .add_service(PartiesServiceServer::new(parties_service))
            .add_service(OpportunitiesServiceServer::new(opportunities_service))
            .add_service(ConversationsServiceServer::new(conversations_service))
            .add_service(DocumentsServiceServer::new(documents_service))
            .add_service(WorkflowServiceServer::new(workflow_service))
            .add_service(FactsServiceServer::new(facts_service))
            .add_service(MetadataServiceServer::new(metadata_service))
            .add_service(ModuleRegistryServiceServer::new(module_registry_service))
            .add_service(TruthCatalogServiceServer::new(truth_catalog_service))
            .serve(grpc_addr)
            .await
            .map_err(anyhow::Error::from)
    };

    let http = async move {
        let listener = TcpListener::bind(http_addr).await?;
        axum::serve(listener, http_app).await?;
        Result::<()>::Ok(())
    };

    tokio::try_join!(grpc, http)?;
    Ok(())
}
