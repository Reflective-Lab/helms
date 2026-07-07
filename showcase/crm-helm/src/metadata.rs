//! CRM Metadata module — schema definitions as a HelmModule.
//!
//! Moved from helms/crates/application-server/src/service.rs (MetadataGrpc).

use application_kernel::{Actor, CrmKernel, ObjectDefinitionUpsert, ViewDefinitionUpsert};
use application_storage::{AppKernelStore, InMemoryKernelStore, KernelStore};
use async_trait::async_trait;
use runway_app_host::HelmModule;
use tonic::{Request, Response, Status};

use crate::proto::{common as pb, metadata as metadata_pb};
use crate::shared::{
    field_definition_from_proto, object_definition_kind_from_proto, parse_optional_uuid,
    proto_object_definition, proto_view_definition, relationship_definition_from_proto,
    status_from_storage, view_layout_from_proto,
};

// ---------------------------------------------------------------------------
// gRPC service struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct MetadataGrpc<S = InMemoryKernelStore> {
    store: S,
}

impl<S> MetadataGrpc<S> {
    #[allow(dead_code)]
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

#[tonic::async_trait]
impl<S> metadata_pb::metadata_service_server::MetadataService for MetadataGrpc<S>
where
    S: KernelStore,
{
    async fn upsert_object_definition(
        &self,
        request: Request<metadata_pb::UpsertObjectDefinitionRequest>,
    ) -> Result<Response<pb::ObjectDefinition>, Status> {
        let request = request.into_inner();
        let object_definition_id = parse_optional_uuid(request.object_definition_id)?;
        let fields = request
            .fields
            .into_iter()
            .map(field_definition_from_proto)
            .collect::<Result<Vec<_>, _>>()?;
        let relationships = request
            .relationships
            .into_iter()
            .map(relationship_definition_from_proto)
            .collect::<Result<Vec<_>, _>>()?;
        let definition = self
            .store
            .write(|kernel| {
                kernel.upsert_object_definition(
                    ObjectDefinitionUpsert {
                        object_definition_id,
                        key: request.key,
                        display_name: request.display_name,
                        kind: object_definition_kind_from_proto(request.kind),
                        fields,
                        relationships,
                        active: request.active,
                    },
                    Actor::system(),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_object_definition(definition)))
    }

    async fn upsert_view_definition(
        &self,
        request: Request<metadata_pb::UpsertViewDefinitionRequest>,
    ) -> Result<Response<pb::ViewDefinition>, Status> {
        let request = request.into_inner();
        let view_definition_id = parse_optional_uuid(request.view_definition_id)?;
        let view = self
            .store
            .write(|kernel| {
                kernel.upsert_view_definition(
                    ViewDefinitionUpsert {
                        view_definition_id,
                        object_key: request.object_key,
                        name: request.name,
                        layout: view_layout_from_proto(request.layout),
                        filter_expression: request.filter_expression,
                        sort_expression: request.sort_expression,
                        visible_fields: request.visible_fields,
                        group_by: request.group_by,
                        favorite: request.favorite,
                        owner_user_id: request.owner_user_id,
                    },
                    Actor::system(),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_view_definition(view)))
    }

    async fn list_object_definitions(
        &self,
        _request: Request<metadata_pb::ListObjectDefinitionsRequest>,
    ) -> Result<Response<metadata_pb::ListObjectDefinitionsResponse>, Status> {
        let objects = self
            .store
            .read(CrmKernel::list_object_definitions)
            .map_err(status_from_storage)?;
        Ok(Response::new(metadata_pb::ListObjectDefinitionsResponse {
            objects: objects.into_iter().map(proto_object_definition).collect(),
        }))
    }

    async fn list_view_definitions(
        &self,
        request: Request<metadata_pb::ListViewDefinitionsRequest>,
    ) -> Result<Response<metadata_pb::ListViewDefinitionsResponse>, Status> {
        let request = request.into_inner();
        let object_key = request.object_key.as_deref();
        let views = self
            .store
            .read(|kernel| kernel.list_view_definitions(object_key))
            .map_err(status_from_storage)?;
        Ok(Response::new(metadata_pb::ListViewDefinitionsResponse {
            views: views.into_iter().map(proto_view_definition).collect(),
        }))
    }
}

// ---------------------------------------------------------------------------
// HelmModule wrapper
// ---------------------------------------------------------------------------

pub struct MetadataModule {}

impl MetadataModule {
    pub fn new(_store: AppKernelStore) -> Self {
        Self {}
    }

    #[allow(dead_code)]
    pub fn in_memory() -> Self {
        Self::new(AppKernelStore::Memory(InMemoryKernelStore::default_local()))
    }
}

#[async_trait]
impl HelmModule for MetadataModule {
    fn module_id(&self) -> &'static str {
        "crm.metadata"
    }

    async fn init(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
