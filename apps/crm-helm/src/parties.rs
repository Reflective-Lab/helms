//! CRM Parties module — accounts/contacts CRUD as a HelmModule.
//!
//! Moved from helms/crates/application-server/src/service.rs (PartiesGrpc).

use application_kernel::{CrmKernel, OrganizationUpsert, PersonUpsert, RelationshipLink};
use application_storage::{AppKernelStore, InMemoryKernelStore, KernelStore};
use async_trait::async_trait;
use runway_app_host::HelmModule;
use tonic::{Request, Response, Status};

use crate::proto::{common as pb, parties as parties_pb};
use crate::shared::{
    actor_from_proto, default_limit, organization_lifecycle_from_proto, parse_optional_uuid,
    parse_uuid, proto_account_summary, proto_organization, proto_person, proto_relationship,
    record_ref_from_proto, relationship_type_from_proto, status_from_storage,
};

// ---------------------------------------------------------------------------
// gRPC service struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PartiesGrpc<S = InMemoryKernelStore> {
    store: S,
}

impl<S> PartiesGrpc<S> {
    #[allow(dead_code)]
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

#[tonic::async_trait]
impl<S> parties_pb::parties_service_server::PartiesService for PartiesGrpc<S>
where
    S: KernelStore,
{
    async fn upsert_organization(
        &self,
        request: Request<parties_pb::UpsertOrganizationRequest>,
    ) -> Result<Response<pb::Organization>, Status> {
        let request = request.into_inner();
        let organization_id = parse_optional_uuid(request.organization_id)?;
        let organization = self
            .store
            .write(|kernel| {
                kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id,
                        name: request.name,
                        external_key: request.external_key,
                        website: request.website,
                        industry: request.industry,
                        lifecycle: organization_lifecycle_from_proto(request.lifecycle),
                        owner_user_id: request.owner_user_id,
                        tags: request.tags,
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_organization(organization)))
    }

    async fn upsert_person(
        &self,
        request: Request<parties_pb::UpsertPersonRequest>,
    ) -> Result<Response<pb::Person>, Status> {
        let request = request.into_inner();
        let person_id = parse_optional_uuid(request.person_id)?;
        let organization_id = parse_optional_uuid(request.organization_id)?;
        let person = self
            .store
            .write(|kernel| {
                kernel.upsert_person(
                    PersonUpsert {
                        person_id,
                        organization_id,
                        full_name: request.full_name,
                        title: request.title,
                        email: request.email,
                        phone: request.phone,
                        linkedin_url: request.linkedin_url,
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_person(person)))
    }

    async fn link_relationship(
        &self,
        request: Request<parties_pb::LinkRelationshipRequest>,
    ) -> Result<Response<pb::Relationship>, Status> {
        let request = request.into_inner();
        let from = request
            .from
            .ok_or_else(|| Status::invalid_argument("from is required"))
            .and_then(record_ref_from_proto)?;
        let to = request
            .to
            .ok_or_else(|| Status::invalid_argument("to is required"))
            .and_then(record_ref_from_proto)?;
        let relationship = self
            .store
            .write(|kernel| {
                kernel.link_relationship(
                    RelationshipLink {
                        from,
                        to,
                        relationship_type: relationship_type_from_proto(request.relationship_type),
                        label: request.label,
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_relationship(relationship)))
    }

    async fn get_account_summary(
        &self,
        request: Request<parties_pb::GetAccountSummaryRequest>,
    ) -> Result<Response<pb::AccountSummary>, Status> {
        let request = request.into_inner();
        let organization_id = parse_uuid(&request.organization_id)?;
        let timeline_limit = default_limit(request.timeline_limit, 25);
        let summary = self
            .store
            .read(|kernel| kernel.get_account_summary(organization_id, timeline_limit))
            .map_err(status_from_storage)?
            .map_err(crate::shared::status_from_kernel)?;
        Ok(Response::new(proto_account_summary(summary)))
    }

    async fn list_organizations(
        &self,
        _request: Request<parties_pb::ListOrganizationsRequest>,
    ) -> Result<Response<parties_pb::ListOrganizationsResponse>, Status> {
        let organizations = self
            .store
            .read(CrmKernel::list_organizations)
            .map_err(status_from_storage)?;
        Ok(Response::new(parties_pb::ListOrganizationsResponse {
            organizations: organizations.into_iter().map(proto_organization).collect(),
        }))
    }

    async fn list_people(
        &self,
        request: Request<parties_pb::ListPeopleRequest>,
    ) -> Result<Response<parties_pb::ListPeopleResponse>, Status> {
        let organization_id = parse_optional_uuid(request.into_inner().organization_id)?;
        let people = self
            .store
            .read(|kernel| kernel.list_people(organization_id))
            .map_err(status_from_storage)?;
        Ok(Response::new(parties_pb::ListPeopleResponse {
            people: people.into_iter().map(proto_person).collect(),
        }))
    }
}

// ---------------------------------------------------------------------------
// HelmModule wrapper
// ---------------------------------------------------------------------------

pub struct PartiesModule {}

impl PartiesModule {
    pub fn new(_store: AppKernelStore) -> Self {
        Self {}
    }

    #[allow(dead_code)]
    pub fn in_memory() -> Self {
        Self::new(AppKernelStore::Memory(InMemoryKernelStore::default_local()))
    }
}

#[async_trait]
impl HelmModule for PartiesModule {
    fn module_id(&self) -> &'static str {
        "crm.parties"
    }

    async fn init(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
