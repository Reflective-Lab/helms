//! CRM Opportunities module — sales pipeline as a HelmModule.
//!
//! Moved from helms/crates/application-server/src/service.rs (OpportunitiesGrpc).

use application_kernel::{Money, OpportunityAdvance, OpportunityCreate};
use application_storage::{AppKernelStore, InMemoryKernelStore, KernelStore};
use async_trait::async_trait;
use runway_app_host::HelmModule;
use tonic::{Request, Response, Status};

use crate::proto::{common as pb, opportunities as opportunities_pb};
use crate::shared::{
    actor_from_proto, clamp_bps, datetime_from_proto, opportunity_stage_from_proto,
    parse_optional_uuid, parse_uuid, proto_opportunity, status_from_storage,
};

// ---------------------------------------------------------------------------
// gRPC service struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct OpportunitiesGrpc<S = InMemoryKernelStore> {
    store: S,
}

impl<S> OpportunitiesGrpc<S> {
    #[allow(dead_code)]
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

#[tonic::async_trait]
impl<S> opportunities_pb::opportunities_service_server::OpportunitiesService
    for OpportunitiesGrpc<S>
where
    S: KernelStore,
{
    async fn create_opportunity(
        &self,
        request: Request<opportunities_pb::CreateOpportunityRequest>,
    ) -> Result<Response<pb::Opportunity>, Status> {
        let request = request.into_inner();
        let value = request
            .value
            .ok_or_else(|| Status::invalid_argument("value is required"))?;
        let organization_id = parse_uuid(&request.organization_id)?;
        let primary_contact_id = parse_optional_uuid(request.primary_contact_id)?;
        let confidence_bps = clamp_bps(request.confidence_bps)?;
        let expected_close_at = request.expected_close_at.and_then(datetime_from_proto);
        let opportunity = self
            .store
            .write(|kernel| {
                kernel.create_opportunity(
                    OpportunityCreate {
                        organization_id,
                        primary_contact_id,
                        name: request.name,
                        value: Money {
                            currency_code: value.currency_code,
                            amount_minor: value.amount_minor,
                        },
                        confidence_bps,
                        next_step: request.next_step,
                        expected_close_at,
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_opportunity(opportunity)))
    }

    async fn advance_opportunity_stage(
        &self,
        request: Request<opportunities_pb::AdvanceOpportunityStageRequest>,
    ) -> Result<Response<pb::Opportunity>, Status> {
        let request = request.into_inner();
        let opportunity_id = parse_uuid(&request.opportunity_id)?;
        let opportunity = self
            .store
            .write(|kernel| {
                kernel.advance_opportunity(
                    OpportunityAdvance {
                        opportunity_id,
                        stage: opportunity_stage_from_proto(request.stage),
                        next_step: request.next_step,
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_opportunity(opportunity)))
    }

    async fn list_opportunities(
        &self,
        request: Request<opportunities_pb::ListOpportunitiesRequest>,
    ) -> Result<Response<opportunities_pb::ListOpportunitiesResponse>, Status> {
        let organization_id = parse_optional_uuid(request.into_inner().organization_id)?;
        let opportunities = self
            .store
            .read(|kernel| kernel.list_opportunities(organization_id))
            .map_err(status_from_storage)?;
        Ok(Response::new(opportunities_pb::ListOpportunitiesResponse {
            opportunities: opportunities.into_iter().map(proto_opportunity).collect(),
        }))
    }
}

// ---------------------------------------------------------------------------
// HelmModule wrapper
// ---------------------------------------------------------------------------

pub struct OpportunitiesModule {}

impl OpportunitiesModule {
    pub fn new(_store: AppKernelStore) -> Self {
        Self {}
    }

    #[allow(dead_code)]
    pub fn in_memory() -> Self {
        Self::new(AppKernelStore::Memory(InMemoryKernelStore::default_local()))
    }
}

#[async_trait]
impl HelmModule for OpportunitiesModule {
    fn module_id(&self) -> &'static str {
        "crm.opportunities"
    }

    async fn init(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
