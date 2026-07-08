#![allow(clippy::result_large_err)]

//! CRM Helm Showcase
//!
//! Demonstrates how Runtime Runway + Helm modules compose into a thin app binary.
//! Phase 6b wires the 7 CRM gRPC modules extracted from helms/application-server.

mod conversations;
mod documents;
mod facts;
mod metadata;
mod opportunities;
mod parties;
mod proto;
mod shared;
mod truths;
mod workbench;
mod workflow;

use std::sync::Arc;

use application_storage::{AppKernelStore, InMemoryKernelStore};
use runway_app_host::{
    AppExecutionPacket, BoundaryRegistration, BoundaryStatus, ContractLayer, MountKind,
    MountedModule, RunwayAppHost,
};
use runway_storage::StorageKit;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let packet = AppExecutionPacket::new(
        "crm-helm",
        "CRM Helm Showcase",
        "CRM gRPC services composed via HelmModule — Phase 6b showcase",
        "/crm",
    )
    .with_mounted_module(MountedModule::new("crm.parties", MountKind::Mounted))
    .with_mounted_module(MountedModule::new("crm.opportunities", MountKind::Mounted))
    .with_mounted_module(MountedModule::new("crm.conversations", MountKind::Mounted))
    .with_mounted_module(MountedModule::new("crm.documents", MountKind::Mounted))
    .with_mounted_module(MountedModule::new("crm.workflow", MountKind::Mounted))
    .with_mounted_module(MountedModule::new("crm.facts", MountKind::Mounted))
    .with_mounted_module(MountedModule::new("crm.metadata", MountKind::Mounted))
    .with_boundary(BoundaryRegistration::new(
        ContractLayer::Helm,
        vec![
            "crm.parties".to_string(),
            "crm.opportunities".to_string(),
            "crm.conversations".to_string(),
            "crm.documents".to_string(),
            "crm.workflow".to_string(),
            "crm.facts".to_string(),
            "crm.metadata".to_string(),
        ],
        BoundaryStatus::Mounted,
    ));

    // Shared in-memory kernel store — all 7 modules share one store instance
    // so writes in one service are visible to reads in another.
    let store = AppKernelStore::Memory(InMemoryKernelStore::default_local());

    let storage = StorageKit::local("crm-helm-local.db").await?;

    let host = RunwayAppHost::builder(packet)
        .with_storage(storage)
        .mount(Arc::new(parties::PartiesModule::new(store.clone())))
        .mount(Arc::new(opportunities::OpportunitiesModule::new(
            store.clone(),
        )))
        .mount(Arc::new(conversations::ConversationsModule::new(
            store.clone(),
        )))
        .mount(Arc::new(documents::DocumentsModule::new(store.clone())))
        .mount(Arc::new(workflow::WorkflowModule::new(store.clone())))
        .mount(Arc::new(facts::FactsModule::new(store.clone())))
        .mount(Arc::new(metadata::MetadataModule::new(store)))
        .build()
        .await?;

    // TODO(Phase 9/truth-execution): mount TruthCatalog module once
    // feat/helm-truth-execution is merged.

    host.serve().await
}
