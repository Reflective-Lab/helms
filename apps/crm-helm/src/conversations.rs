//! CRM Conversations module — communication threads as a HelmModule.
//!
//! Moved from helms/crates/application-server/src/service.rs (ConversationsGrpc).

use application_kernel::{ActivityAppend, CommunicationRecord};
use application_storage::{AppKernelStore, InMemoryKernelStore, KernelStore};
use async_trait::async_trait;
use runway_app_host::HelmModule;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::proto::{common as pb, conversations as conversations_pb};
use crate::shared::{
    activity_outcome_from_proto, actor_from_proto, communication_channel_from_proto,
    communication_direction_from_proto, datetime_from_proto, default_limit, proto_activity,
    proto_communication_event, proto_timeline_entry, record_ref_from_proto, status_from_storage,
};

// ---------------------------------------------------------------------------
// gRPC service struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ConversationsGrpc<S = InMemoryKernelStore> {
    store: S,
}

impl<S> ConversationsGrpc<S> {
    #[allow(dead_code)]
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

#[tonic::async_trait]
impl<S> conversations_pb::conversations_service_server::ConversationsService
    for ConversationsGrpc<S>
where
    S: KernelStore,
{
    type StreamTimelineStream = ReceiverStream<Result<pb::TimelineEntry, Status>>;

    async fn append_activity(
        &self,
        request: Request<conversations_pb::AppendActivityRequest>,
    ) -> Result<Response<pb::Activity>, Status> {
        let request = request.into_inner();
        let related_to = request
            .related_to
            .into_iter()
            .map(record_ref_from_proto)
            .collect::<Result<Vec<_>, _>>()?;
        let activity = self
            .store
            .write(|kernel| {
                kernel.append_activity(
                    ActivityAppend {
                        subject: request.subject,
                        details: request.details,
                        related_to,
                        outcome: activity_outcome_from_proto(request.outcome),
                        occurred_at: request.occurred_at.and_then(datetime_from_proto),
                        next_action_due_at: request
                            .next_action_due_at
                            .and_then(datetime_from_proto),
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_activity(activity)))
    }

    async fn record_communication(
        &self,
        request: Request<conversations_pb::RecordCommunicationRequest>,
    ) -> Result<Response<pb::CommunicationEvent>, Status> {
        let request = request.into_inner();
        let related_to = request
            .related_to
            .into_iter()
            .map(record_ref_from_proto)
            .collect::<Result<Vec<_>, _>>()?;
        let event = self
            .store
            .write(|kernel| {
                kernel.record_communication(
                    CommunicationRecord {
                        channel: communication_channel_from_proto(request.channel),
                        direction: communication_direction_from_proto(request.direction),
                        subject: request.subject,
                        summary: request.summary,
                        counterpart: request.counterpart,
                        related_to,
                        occurred_at: request.occurred_at.and_then(datetime_from_proto),
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_communication_event(event)))
    }

    async fn stream_timeline(
        &self,
        request: Request<conversations_pb::StreamTimelineRequest>,
    ) -> Result<Response<Self::StreamTimelineStream>, Status> {
        let request = request.into_inner();
        let anchors = request
            .anchors
            .into_iter()
            .map(record_ref_from_proto)
            .collect::<Result<Vec<_>, _>>()?;
        let entries = self
            .store
            .read(|kernel| kernel.list_timeline(&anchors, default_limit(request.limit, 50)))
            .map_err(status_from_storage)?;

        let (tx, rx) = mpsc::channel(16);
        tokio::spawn(async move {
            for entry in entries {
                if tx.send(Ok(proto_timeline_entry(entry))).await.is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

// ---------------------------------------------------------------------------
// HelmModule wrapper
// ---------------------------------------------------------------------------

pub struct ConversationsModule {}

impl ConversationsModule {
    pub fn new(_store: AppKernelStore) -> Self {
        Self {}
    }

    #[allow(dead_code)]
    pub fn in_memory() -> Self {
        Self::new(AppKernelStore::Memory(InMemoryKernelStore::default_local()))
    }
}

#[async_trait]
impl HelmModule for ConversationsModule {
    fn module_id(&self) -> &'static str {
        "crm.conversations"
    }

    async fn init(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
