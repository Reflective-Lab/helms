//! CRM Facts module — immutable audit log as a HelmModule.
//!
//! Moved from helms/crates/application-server/src/service.rs (FactsGrpc).

use application_kernel::FactRecord;
use application_storage::{AppKernelStore, InMemoryKernelStore, KernelStore};
use async_trait::async_trait;
use runway_app_host::HelmModule;
use tonic::{Request, Response, Status};

use crate::proto::{common as pb, facts as facts_pb};
use crate::shared::{
    actor_from_proto, clamp_bps, parse_optional_uuid, proto_fact, record_ref_from_proto,
    status_from_storage,
};

// ---------------------------------------------------------------------------
// gRPC service struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FactsGrpc<S = InMemoryKernelStore> {
    store: S,
}

impl<S> FactsGrpc<S> {
    #[allow(dead_code)]
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

#[tonic::async_trait]
impl<S> facts_pb::facts_service_server::FactsService for FactsGrpc<S>
where
    S: KernelStore,
{
    async fn record_fact(
        &self,
        request: Request<facts_pb::RecordFactRequest>,
    ) -> Result<Response<pb::Fact>, Status> {
        let request = request.into_inner();
        let related_to = request
            .related_to
            .into_iter()
            .map(record_ref_from_proto)
            .collect::<Result<Vec<_>, _>>()?;
        let confidence_bps = clamp_bps(request.confidence_bps)?;
        let source_note_id = parse_optional_uuid(request.source_note_id)?;
        let fact = self
            .store
            .write(|kernel| {
                kernel.record_fact(
                    FactRecord {
                        statement: request.statement,
                        confidence_bps,
                        related_to,
                        source_note_id,
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_fact(fact)))
    }
}

// ---------------------------------------------------------------------------
// HelmModule wrapper
// ---------------------------------------------------------------------------

pub struct FactsModule {}

impl FactsModule {
    pub fn new(_store: AppKernelStore) -> Self {
        Self {}
    }

    #[allow(dead_code)]
    pub fn in_memory() -> Self {
        Self::new(AppKernelStore::Memory(InMemoryKernelStore::default_local()))
    }
}

#[async_trait]
impl HelmModule for FactsModule {
    fn module_id(&self) -> &'static str {
        "crm.facts"
    }

    async fn init(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
