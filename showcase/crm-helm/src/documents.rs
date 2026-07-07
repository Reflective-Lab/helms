//! CRM Documents module — attachments as a HelmModule.
//!
//! Moved from helms/crates/application-server/src/service.rs (DocumentsGrpc).

use application_kernel::{DocumentAttach, NoteAppend};
use application_storage::{AppKernelStore, InMemoryKernelStore, KernelStore};
use async_trait::async_trait;
use runway_app_host::HelmModule;
use tonic::{Request, Response, Status};

use crate::proto::{common as pb, documents as documents_pb};
use crate::shared::{
    actor_from_proto, document_status_from_proto, proto_document, proto_note,
    record_ref_from_proto, status_from_storage,
};

// ---------------------------------------------------------------------------
// gRPC service struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct DocumentsGrpc<S = InMemoryKernelStore> {
    store: S,
}

impl<S> DocumentsGrpc<S> {
    #[allow(dead_code)]
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

#[tonic::async_trait]
impl<S> documents_pb::documents_service_server::DocumentsService for DocumentsGrpc<S>
where
    S: KernelStore,
{
    async fn append_note(
        &self,
        request: Request<documents_pb::AppendNoteRequest>,
    ) -> Result<Response<pb::Note>, Status> {
        let request = request.into_inner();
        let related_to = request
            .related_to
            .into_iter()
            .map(record_ref_from_proto)
            .collect::<Result<Vec<_>, _>>()?;
        let note = self
            .store
            .write(|kernel| {
                kernel.append_note(
                    NoteAppend {
                        subject: request.subject,
                        body: request.body,
                        related_to,
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_note(note)))
    }

    async fn attach_document(
        &self,
        request: Request<documents_pb::AttachDocumentRequest>,
    ) -> Result<Response<pb::Document>, Status> {
        let request = request.into_inner();
        let related_to = request
            .related_to
            .into_iter()
            .map(record_ref_from_proto)
            .collect::<Result<Vec<_>, _>>()?;
        let document = self
            .store
            .write(|kernel| {
                kernel.attach_document(
                    DocumentAttach {
                        title: request.title,
                        media_type: request.media_type,
                        uri: request.uri,
                        status: document_status_from_proto(request.status),
                        related_to,
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_document(document)))
    }
}

// ---------------------------------------------------------------------------
// HelmModule wrapper
// ---------------------------------------------------------------------------

pub struct DocumentsModule {}

impl DocumentsModule {
    pub fn new(_store: AppKernelStore) -> Self {
        Self {}
    }

    #[allow(dead_code)]
    pub fn in_memory() -> Self {
        Self::new(AppKernelStore::Memory(InMemoryKernelStore::default_local()))
    }
}

#[async_trait]
impl HelmModule for DocumentsModule {
    fn module_id(&self) -> &'static str {
        "crm.documents"
    }

    async fn init(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
