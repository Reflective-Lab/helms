//! CRM Workflow module — lead/case state machines as a HelmModule.
//!
//! Moved from helms/crates/application-server/src/service.rs (WorkflowGrpc).

use application_kernel::{WorkflowCaseAdvance, WorkflowCaseCreate};
use application_storage::{AppKernelStore, InMemoryKernelStore, KernelStore};
use async_trait::async_trait;
use runway_app_host::HelmModule;
use tonic::{Request, Response, Status};

use crate::proto::{common as pb, workflow as workflow_pb};
use crate::shared::{
    actor_from_proto, parse_uuid, proto_workflow_case, record_ref_from_proto, status_from_storage,
    workflow_priority_from_proto, workflow_state_from_proto,
};

// ---------------------------------------------------------------------------
// gRPC service struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct WorkflowGrpc<S = InMemoryKernelStore> {
    store: S,
}

impl<S> WorkflowGrpc<S> {
    #[allow(dead_code)]
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

#[tonic::async_trait]
impl<S> workflow_pb::workflow_service_server::WorkflowService for WorkflowGrpc<S>
where
    S: KernelStore,
{
    async fn create_workflow_case(
        &self,
        request: Request<workflow_pb::CreateWorkflowCaseRequest>,
    ) -> Result<Response<pb::WorkflowCase>, Status> {
        let request = request.into_inner();
        let related_to = request
            .related_to
            .into_iter()
            .map(record_ref_from_proto)
            .collect::<Result<Vec<_>, _>>()?;
        let workflow_case = self
            .store
            .write(|kernel| {
                kernel.create_workflow_case(
                    WorkflowCaseCreate {
                        title: request.title,
                        priority: workflow_priority_from_proto(request.priority),
                        owner_user_id: request.owner_user_id,
                        related_to,
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_workflow_case(workflow_case)))
    }

    async fn advance_workflow_case(
        &self,
        request: Request<workflow_pb::AdvanceWorkflowCaseRequest>,
    ) -> Result<Response<pb::WorkflowCase>, Status> {
        let request = request.into_inner();
        let workflow_case_id = parse_uuid(&request.workflow_case_id)?;
        let workflow_case = self
            .store
            .write(|kernel| {
                kernel.advance_workflow_case(
                    WorkflowCaseAdvance {
                        workflow_case_id,
                        state: workflow_state_from_proto(request.state),
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_workflow_case(workflow_case)))
    }
}

// ---------------------------------------------------------------------------
// HelmModule wrapper
// ---------------------------------------------------------------------------

pub struct WorkflowModule {}

impl WorkflowModule {
    pub fn new(_store: AppKernelStore) -> Self {
        Self {}
    }

    #[allow(dead_code)]
    pub fn in_memory() -> Self {
        Self::new(AppKernelStore::Memory(InMemoryKernelStore::default_local()))
    }
}

#[async_trait]
impl HelmModule for WorkflowModule {
    fn module_id(&self) -> &'static str {
        "crm.workflow"
    }

    async fn init(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
