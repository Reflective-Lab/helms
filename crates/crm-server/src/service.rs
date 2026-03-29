use chrono::{DateTime, Utc};
use converge_core::CriterionResult;
use crm_kernel::{
    ActivityAppend, ActivityOutcome, Actor, ActorKind, CommunicationChannel,
    CommunicationDirection, CommunicationRecord, CrmKernel, DocumentAttach, DocumentStatus,
    Entitlement, EntitlementValue, Fact, FactRecord, FieldDefinition, FieldType, LedgerEntry,
    LedgerEntryKind, Money, NoteAppend, ObjectDefinition, ObjectDefinitionKind,
    ObjectDefinitionUpsert, Opportunity, OpportunityAdvance, OpportunityCreate, OpportunityStage,
    OrderSubscription, Organization, OrganizationLifecycle, OrganizationUpsert,
    PermissionGrantInput, Person, PersonUpsert, RecordKind, RecordRef, Relationship,
    RelationshipCardinality, RelationshipDefinition, RelationshipLink, SubscriptionStatus,
    TimelineEntry, ViewDefinition, ViewDefinitionUpsert, ViewLayout, WorkflowCase,
    WorkflowCaseAdvance, WorkflowCaseCreate, WorkflowPriority, WorkflowState,
};
use crm_storage::{InMemoryKernelStore, KernelStore, StorageError};
use prio_module_core::{CapabilityModule, ModuleSuite};
use prio_modules::all_modules;
use prio_truths::{
    TruthDefinition, TruthKind as CatalogTruthKind, all_truths, converge_binding_for_truth,
    find_truth,
};
use prost_types::Timestamp;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::proto::{
    common as pb, conversations as conversations_pb, documents as documents_pb, facts as facts_pb,
    identity as identity_pb, metadata as metadata_pb, modules as modules_pb,
    opportunities as opportunities_pb, parties as parties_pb, truths as truths_pb,
    workflow as workflow_pb,
};
use crate::truth_runtime::{TruthExecutionArtifacts, TruthProjection, execute_truth};

#[derive(Clone)]
pub struct IdentityGrpc<S = InMemoryKernelStore> {
    store: S,
}

#[derive(Clone)]
pub struct PartiesGrpc<S = InMemoryKernelStore> {
    store: S,
}

#[derive(Clone)]
pub struct OpportunitiesGrpc<S = InMemoryKernelStore> {
    store: S,
}

#[derive(Clone)]
pub struct ConversationsGrpc<S = InMemoryKernelStore> {
    store: S,
}

#[derive(Clone)]
pub struct DocumentsGrpc<S = InMemoryKernelStore> {
    store: S,
}

#[derive(Clone)]
pub struct WorkflowGrpc<S = InMemoryKernelStore> {
    store: S,
}

#[derive(Clone)]
pub struct FactsGrpc<S = InMemoryKernelStore> {
    store: S,
}

#[derive(Clone)]
pub struct MetadataGrpc<S = InMemoryKernelStore> {
    store: S,
}

#[derive(Clone, Default)]
pub struct ModuleRegistryGrpc;

#[derive(Clone)]
pub struct TruthCatalogGrpc<S = InMemoryKernelStore> {
    store: S,
}

macro_rules! impl_new_store {
    ($name:ident) => {
        impl<S> $name<S> {
            #[must_use]
            pub fn new(store: S) -> Self {
                Self { store }
            }
        }
    };
}

impl_new_store!(IdentityGrpc);
impl_new_store!(PartiesGrpc);
impl_new_store!(OpportunitiesGrpc);
impl_new_store!(ConversationsGrpc);
impl_new_store!(DocumentsGrpc);
impl_new_store!(WorkflowGrpc);
impl_new_store!(FactsGrpc);
impl_new_store!(MetadataGrpc);

impl ModuleRegistryGrpc {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl_new_store!(TruthCatalogGrpc);

#[tonic::async_trait]
impl<S> identity_pb::identity_service_server::IdentityService for IdentityGrpc<S>
where
    S: KernelStore,
{
    async fn grant_permission(
        &self,
        request: Request<identity_pb::GrantPermissionRequest>,
    ) -> Result<Response<pb::PermissionGrant>, Status> {
        let request = request.into_inner();
        let grant = self
            .store
            .write(|kernel| {
                kernel.grant_permission(
                    PermissionGrantInput {
                        subject: request.subject,
                        role: request.role,
                        scope: request.scope,
                    },
                    actor_from_proto(request.actor),
                )
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(proto_permission_grant(grant)))
    }

    async fn list_permissions(
        &self,
        request: Request<identity_pb::ListPermissionsRequest>,
    ) -> Result<Response<identity_pb::ListPermissionsResponse>, Status> {
        let scope = request.into_inner().scope;
        let permissions = self
            .store
            .read(|kernel| {
                let mut items = kernel
                    .permission_grants
                    .values()
                    .filter(|grant| {
                        scope
                            .as_deref()
                            .is_none_or(|expected| grant.scope == expected)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
                items
            })
            .map_err(status_from_storage)?;
        Ok(Response::new(identity_pb::ListPermissionsResponse {
            permissions: permissions
                .into_iter()
                .map(proto_permission_grant)
                .collect(),
        }))
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
            .map_err(status_from_kernel)?;
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

#[tonic::async_trait]
impl modules_pb::module_registry_service_server::ModuleRegistryService for ModuleRegistryGrpc {
    async fn list_modules(
        &self,
        _request: Request<modules_pb::ListModulesRequest>,
    ) -> Result<Response<modules_pb::ListModulesResponse>, Status> {
        let mut modules = all_modules();
        modules.sort_by(|left, right| {
            left.suite
                .cmp(&right.suite)
                .then_with(|| left.key.cmp(right.key))
        });
        Ok(Response::new(modules_pb::ListModulesResponse {
            modules: modules.into_iter().map(proto_module_info).collect(),
        }))
    }
}

#[tonic::async_trait]
impl<S> truths_pb::truth_catalog_service_server::TruthCatalogService for TruthCatalogGrpc<S>
where
    S: KernelStore,
{
    async fn list_truths(
        &self,
        request: Request<truths_pb::ListTruthsRequest>,
    ) -> Result<Response<truths_pb::ListTruthsResponse>, Status> {
        let request = request.into_inner();
        let kind_filter = truth_kind_filter_from_proto(request.kind);
        let module_key = request.module_key.trim();

        let mut truths = all_truths()
            .into_iter()
            .filter(|truth| kind_filter.is_none_or(|kind| truth.kind == kind))
            .filter(|truth| {
                module_key.is_empty()
                    || truth
                        .modules
                        .iter()
                        .any(|touch| touch.module_key == module_key)
            })
            .collect::<Vec<_>>();

        truths.sort_by(|left, right| {
            truth_kind_rank(left.kind)
                .cmp(&truth_kind_rank(right.kind))
                .then_with(|| left.key.cmp(right.key))
        });

        Ok(Response::new(truths_pb::ListTruthsResponse {
            truths: truths
                .into_iter()
                .map(|truth| proto_truth_info(truth, request.include_gherkin))
                .collect(),
        }))
    }

    async fn get_truth(
        &self,
        request: Request<truths_pb::GetTruthRequest>,
    ) -> Result<Response<truths_pb::TruthInfo>, Status> {
        let request = request.into_inner();
        let key = request.key.trim();
        if key.is_empty() {
            return Err(Status::invalid_argument("truth key is required"));
        }

        let truth =
            find_truth(key).ok_or_else(|| Status::not_found(format!("truth not found: {key}")))?;
        Ok(Response::new(proto_truth_info(truth, true)))
    }

    async fn execute_truth(
        &self,
        request: Request<truths_pb::ExecuteTruthRequest>,
    ) -> Result<Response<truths_pb::ExecuteTruthResponse>, Status> {
        let request = request.into_inner();
        let key = request.key.trim();
        if key.is_empty() {
            return Err(Status::invalid_argument("truth key is required"));
        }

        let truth =
            find_truth(key).ok_or_else(|| Status::not_found(format!("truth not found: {key}")))?;
        let execution = execute_truth(
            &self.store,
            key,
            request.inputs,
            actor_from_proto(request.actor),
            request.persist_projection,
        )?;

        Ok(Response::new(proto_execute_truth_response(
            truth, execution,
        )))
    }
}

fn status_from_storage(error: StorageError) -> Status {
    match error {
        StorageError::LockPoisoned => Status::internal("storage lock poisoned"),
        StorageError::Kernel(error) => status_from_kernel(error),
    }
}

fn status_from_kernel(error: crm_kernel::KernelError) -> Status {
    match error {
        crm_kernel::KernelError::Validation(message) => Status::invalid_argument(message),
        crm_kernel::KernelError::NotFound { kind, id } => {
            Status::not_found(format!("{kind} not found: {id}"))
        }
        crm_kernel::KernelError::Invariant(message) => Status::failed_precondition(message),
    }
}

fn actor_from_proto(actor: Option<pb::Actor>) -> Actor {
    actor.map_or_else(Actor::system, |actor| Actor {
        actor_id: actor.actor_id,
        display_name: actor.display_name,
        kind: match pb::ActorKind::try_from(actor.kind).unwrap_or(pb::ActorKind::System) {
            pb::ActorKind::Human => ActorKind::Human,
            pb::ActorKind::Agent => ActorKind::Agent,
            pb::ActorKind::System | pb::ActorKind::Unspecified => ActorKind::System,
        },
    })
}

fn proto_actor(actor: Actor) -> pb::Actor {
    pb::Actor {
        actor_id: actor.actor_id,
        display_name: actor.display_name,
        kind: match actor.kind {
            ActorKind::Human => pb::ActorKind::Human as i32,
            ActorKind::Agent => pb::ActorKind::Agent as i32,
            ActorKind::System => pb::ActorKind::System as i32,
        },
    }
}

fn record_ref_from_proto(reference: pb::RecordRef) -> Result<RecordRef, Status> {
    Ok(RecordRef {
        kind: match pb::RecordKind::try_from(reference.kind) {
            Ok(pb::RecordKind::Organization) => RecordKind::Organization,
            Ok(pb::RecordKind::Person) => RecordKind::Person,
            Ok(pb::RecordKind::Relationship) => RecordKind::Relationship,
            Ok(pb::RecordKind::Lead) => RecordKind::Lead,
            Ok(pb::RecordKind::Opportunity) => RecordKind::Opportunity,
            Ok(pb::RecordKind::Conversation) => RecordKind::Conversation,
            Ok(pb::RecordKind::Activity) => RecordKind::Activity,
            Ok(pb::RecordKind::Task) => RecordKind::Task,
            Ok(pb::RecordKind::OfferQuote) => RecordKind::OfferQuote,
            Ok(pb::RecordKind::OrderSubscription) => RecordKind::OrderSubscription,
            Ok(pb::RecordKind::Document) => RecordKind::Document,
            Ok(pb::RecordKind::Fact) => RecordKind::Fact,
            Ok(pb::RecordKind::Intent) => RecordKind::Intent,
            Ok(pb::RecordKind::WorkflowCase) => RecordKind::WorkflowCase,
            Ok(pb::RecordKind::CommunicationEvent) => RecordKind::CommunicationEvent,
            Ok(pb::RecordKind::PermissionGrant) => RecordKind::PermissionGrant,
            Ok(pb::RecordKind::AuditEntry) => RecordKind::AuditEntry,
            Ok(pb::RecordKind::Note) => RecordKind::Note,
            Ok(pb::RecordKind::CatalogItem) => RecordKind::CatalogItem,
            Ok(pb::RecordKind::Unspecified) | Err(_) => {
                return Err(Status::invalid_argument("record kind is required"));
            }
        },
        id: parse_uuid(&reference.record_id)?,
    })
}

fn proto_record_ref(reference: RecordRef) -> pb::RecordRef {
    pb::RecordRef {
        kind: match reference.kind {
            RecordKind::Organization => pb::RecordKind::Organization as i32,
            RecordKind::Person => pb::RecordKind::Person as i32,
            RecordKind::Relationship => pb::RecordKind::Relationship as i32,
            RecordKind::Lead => pb::RecordKind::Lead as i32,
            RecordKind::Opportunity => pb::RecordKind::Opportunity as i32,
            RecordKind::Conversation => pb::RecordKind::Conversation as i32,
            RecordKind::Activity => pb::RecordKind::Activity as i32,
            RecordKind::Task => pb::RecordKind::Task as i32,
            RecordKind::OfferQuote => pb::RecordKind::OfferQuote as i32,
            RecordKind::OrderSubscription => pb::RecordKind::OrderSubscription as i32,
            RecordKind::Document => pb::RecordKind::Document as i32,
            RecordKind::Fact => pb::RecordKind::Fact as i32,
            RecordKind::Intent => pb::RecordKind::Intent as i32,
            RecordKind::WorkflowCase => pb::RecordKind::WorkflowCase as i32,
            RecordKind::CommunicationEvent => pb::RecordKind::CommunicationEvent as i32,
            RecordKind::PermissionGrant => pb::RecordKind::PermissionGrant as i32,
            RecordKind::AuditEntry => pb::RecordKind::AuditEntry as i32,
            RecordKind::Note => pb::RecordKind::Note as i32,
            RecordKind::CatalogItem => pb::RecordKind::CatalogItem as i32,
        },
        record_id: reference.id.to_string(),
    }
}

fn proto_timestamp(value: DateTime<Utc>) -> Option<Timestamp> {
    Some(Timestamp {
        seconds: value.timestamp(),
        nanos: value.timestamp_subsec_nanos() as i32,
    })
}

fn datetime_from_proto(value: Timestamp) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(value.seconds, value.nanos as u32)
}

fn parse_uuid(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value).map_err(|_| Status::invalid_argument(format!("invalid uuid: {value}")))
}

fn parse_optional_uuid(value: Option<String>) -> Result<Option<Uuid>, Status> {
    value
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .map(|value| parse_uuid(&value))
        .transpose()
}

fn clamp_bps(value: u32) -> Result<u16, Status> {
    u16::try_from(value)
        .map_err(|_| Status::invalid_argument("bps value is out of range"))
        .and_then(|value| {
            if value > 10_000 {
                Err(Status::invalid_argument(
                    "bps value must be between 0 and 10000",
                ))
            } else {
                Ok(value)
            }
        })
}

fn default_limit(value: u32, fallback: usize) -> usize {
    if value == 0 { fallback } else { value as usize }
}

fn organization_lifecycle_from_proto(value: i32) -> OrganizationLifecycle {
    match pb::OrganizationLifecycle::try_from(value).unwrap_or(pb::OrganizationLifecycle::Prospect)
    {
        pb::OrganizationLifecycle::Active => OrganizationLifecycle::Active,
        pb::OrganizationLifecycle::Dormant => OrganizationLifecycle::Dormant,
        pb::OrganizationLifecycle::Partner => OrganizationLifecycle::Partner,
        pb::OrganizationLifecycle::Prospect | pb::OrganizationLifecycle::Unspecified => {
            OrganizationLifecycle::Prospect
        }
    }
}

fn opportunity_stage_from_proto(value: i32) -> OpportunityStage {
    match pb::OpportunityStage::try_from(value).unwrap_or(pb::OpportunityStage::Qualifying) {
        pb::OpportunityStage::Discovery => OpportunityStage::Discovery,
        pb::OpportunityStage::Proposal => OpportunityStage::Proposal,
        pb::OpportunityStage::Negotiation => OpportunityStage::Negotiation,
        pb::OpportunityStage::ClosedWon => OpportunityStage::ClosedWon,
        pb::OpportunityStage::ClosedLost => OpportunityStage::ClosedLost,
        pb::OpportunityStage::Qualifying | pb::OpportunityStage::Unspecified => {
            OpportunityStage::Qualifying
        }
    }
}

fn activity_outcome_from_proto(value: i32) -> ActivityOutcome {
    match pb::ActivityOutcome::try_from(value).unwrap_or(pb::ActivityOutcome::Completed) {
        pb::ActivityOutcome::Waiting => ActivityOutcome::Waiting,
        pb::ActivityOutcome::Blocked => ActivityOutcome::Blocked,
        pb::ActivityOutcome::Completed | pb::ActivityOutcome::Unspecified => {
            ActivityOutcome::Completed
        }
    }
}

fn document_status_from_proto(value: i32) -> DocumentStatus {
    match pb::DocumentStatus::try_from(value).unwrap_or(pb::DocumentStatus::Draft) {
        pb::DocumentStatus::Verified => DocumentStatus::Verified,
        pb::DocumentStatus::Archived => DocumentStatus::Archived,
        pb::DocumentStatus::Draft | pb::DocumentStatus::Unspecified => DocumentStatus::Draft,
    }
}

fn communication_channel_from_proto(value: i32) -> CommunicationChannel {
    match pb::CommunicationChannel::try_from(value).unwrap_or(pb::CommunicationChannel::Email) {
        pb::CommunicationChannel::Phone => CommunicationChannel::Phone,
        pb::CommunicationChannel::Meeting => CommunicationChannel::Meeting,
        pb::CommunicationChannel::Chat => CommunicationChannel::Chat,
        pb::CommunicationChannel::Sms => CommunicationChannel::Sms,
        pb::CommunicationChannel::Email | pb::CommunicationChannel::Unspecified => {
            CommunicationChannel::Email
        }
    }
}

fn communication_direction_from_proto(value: i32) -> CommunicationDirection {
    match pb::CommunicationDirection::try_from(value).unwrap_or(pb::CommunicationDirection::Inbound)
    {
        pb::CommunicationDirection::Outbound => CommunicationDirection::Outbound,
        pb::CommunicationDirection::Internal => CommunicationDirection::Internal,
        pb::CommunicationDirection::Inbound | pb::CommunicationDirection::Unspecified => {
            CommunicationDirection::Inbound
        }
    }
}

fn workflow_priority_from_proto(value: i32) -> WorkflowPriority {
    match pb::WorkflowPriority::try_from(value).unwrap_or(pb::WorkflowPriority::Medium) {
        pb::WorkflowPriority::Low => WorkflowPriority::Low,
        pb::WorkflowPriority::High => WorkflowPriority::High,
        pb::WorkflowPriority::Critical => WorkflowPriority::Critical,
        pb::WorkflowPriority::Medium | pb::WorkflowPriority::Unspecified => {
            WorkflowPriority::Medium
        }
    }
}

fn workflow_state_from_proto(value: i32) -> WorkflowState {
    match pb::WorkflowState::try_from(value).unwrap_or(pb::WorkflowState::Open) {
        pb::WorkflowState::AwaitingApproval => WorkflowState::AwaitingApproval,
        pb::WorkflowState::WaitingExternal => WorkflowState::WaitingExternal,
        pb::WorkflowState::Blocked => WorkflowState::Blocked,
        pb::WorkflowState::Done => WorkflowState::Done,
        pb::WorkflowState::Open | pb::WorkflowState::Unspecified => WorkflowState::Open,
    }
}

fn relationship_type_from_proto(value: i32) -> crm_kernel::RelationshipType {
    match pb::RelationshipType::try_from(value).unwrap_or(pb::RelationshipType::Other) {
        pb::RelationshipType::Employment => crm_kernel::RelationshipType::Employment,
        pb::RelationshipType::Champion => crm_kernel::RelationshipType::Champion,
        pb::RelationshipType::DecisionMaker => crm_kernel::RelationshipType::DecisionMaker,
        pb::RelationshipType::Partner => crm_kernel::RelationshipType::Partner,
        pb::RelationshipType::Competitor => crm_kernel::RelationshipType::Competitor,
        pb::RelationshipType::Other | pb::RelationshipType::Unspecified => {
            crm_kernel::RelationshipType::Other
        }
    }
}

fn object_definition_kind_from_proto(value: i32) -> ObjectDefinitionKind {
    match pb::ObjectDefinitionKind::try_from(value).unwrap_or(pb::ObjectDefinitionKind::Custom) {
        pb::ObjectDefinitionKind::Standard => ObjectDefinitionKind::Standard,
        pb::ObjectDefinitionKind::Custom | pb::ObjectDefinitionKind::Unspecified => {
            ObjectDefinitionKind::Custom
        }
    }
}

fn view_layout_from_proto(value: i32) -> ViewLayout {
    match pb::ViewLayout::try_from(value).unwrap_or(pb::ViewLayout::Table) {
        pb::ViewLayout::Kanban => ViewLayout::Kanban,
        pb::ViewLayout::Calendar => ViewLayout::Calendar,
        pb::ViewLayout::Table | pb::ViewLayout::Unspecified => ViewLayout::Table,
    }
}

fn field_type_from_proto(value: i32) -> FieldType {
    match pb::FieldType::try_from(value).unwrap_or(pb::FieldType::Text) {
        pb::FieldType::LongText => FieldType::LongText,
        pb::FieldType::Number => FieldType::Number,
        pb::FieldType::Currency => FieldType::Currency,
        pb::FieldType::Boolean => FieldType::Boolean,
        pb::FieldType::Date => FieldType::Date,
        pb::FieldType::DateTime => FieldType::DateTime,
        pb::FieldType::Email => FieldType::Email,
        pb::FieldType::Phone => FieldType::Phone,
        pb::FieldType::Url => FieldType::Url,
        pb::FieldType::Select => FieldType::Select,
        pb::FieldType::MultiSelect => FieldType::MultiSelect,
        pb::FieldType::Relation => FieldType::Relation,
        pb::FieldType::Text | pb::FieldType::Unspecified => FieldType::Text,
    }
}

fn relationship_cardinality_from_proto(value: i32) -> RelationshipCardinality {
    match pb::RelationshipCardinality::try_from(value)
        .unwrap_or(pb::RelationshipCardinality::OneToMany)
    {
        pb::RelationshipCardinality::OneToOne => RelationshipCardinality::OneToOne,
        pb::RelationshipCardinality::ManyToMany => RelationshipCardinality::ManyToMany,
        pb::RelationshipCardinality::OneToMany | pb::RelationshipCardinality::Unspecified => {
            RelationshipCardinality::OneToMany
        }
    }
}

fn proto_money(value: Money) -> pb::Money {
    pb::Money {
        currency_code: value.currency_code,
        amount_minor: value.amount_minor,
    }
}

fn proto_subscription_status(value: SubscriptionStatus) -> i32 {
    match value {
        SubscriptionStatus::Draft => pb::SubscriptionStatus::Draft as i32,
        SubscriptionStatus::PendingActivation => pb::SubscriptionStatus::PendingActivation as i32,
        SubscriptionStatus::Active => pb::SubscriptionStatus::Active as i32,
        SubscriptionStatus::Suspended => pb::SubscriptionStatus::Suspended as i32,
        SubscriptionStatus::Cancelled => pb::SubscriptionStatus::Cancelled as i32,
    }
}

fn proto_order_subscription(value: OrderSubscription) -> pb::OrderSubscription {
    pb::OrderSubscription {
        id: value.id.to_string(),
        organization_id: value.organization_id.to_string(),
        quote_id: value.quote_id.map(|id| id.to_string()),
        catalog_item_id: value.catalog_item_id.map(|id| id.to_string()),
        status: proto_subscription_status(value.status),
        value: Some(proto_money(value.value)),
        started_at: proto_timestamp(value.started_at),
        activated_at: value.activated_at.and_then(proto_timestamp),
    }
}

fn proto_entitlement_value(value: EntitlementValue) -> pb::EntitlementValue {
    pb::EntitlementValue {
        kind: Some(match value {
            EntitlementValue::FeatureFlag(flag) => pb::entitlement_value::Kind::FeatureFlag(flag),
            EntitlementValue::Quota(quota) => pb::entitlement_value::Kind::Quota(quota),
            EntitlementValue::Credits(credits) => pb::entitlement_value::Kind::Credits(credits),
            EntitlementValue::Text(text) => pb::entitlement_value::Kind::Text(text),
        }),
    }
}

fn proto_entitlement(value: Entitlement) -> pb::Entitlement {
    pb::Entitlement {
        id: value.id.to_string(),
        organization_id: value.organization_id.to_string(),
        subscription_id: value.subscription_id.to_string(),
        catalog_item_id: value.catalog_item_id.to_string(),
        key: value.key,
        value: Some(proto_entitlement_value(value.value)),
        created_at: proto_timestamp(value.created_at),
    }
}

fn proto_ledger_entry(value: LedgerEntry) -> pb::LedgerEntry {
    pb::LedgerEntry {
        id: value.id.to_string(),
        organization_id: value.organization_id.to_string(),
        subscription_id: value.subscription_id.to_string(),
        kind: match value.kind {
            LedgerEntryKind::OpeningBalance => pb::LedgerEntryKind::OpeningBalance as i32,
            LedgerEntryKind::CreditGrant => pb::LedgerEntryKind::CreditGrant as i32,
            LedgerEntryKind::Debit => pb::LedgerEntryKind::Debit as i32,
            LedgerEntryKind::Adjustment => pb::LedgerEntryKind::Adjustment as i32,
        },
        amount: Some(proto_money(value.amount)),
        description: value.description,
        created_at: proto_timestamp(value.created_at),
    }
}

fn field_definition_from_proto(field: pb::FieldDefinition) -> Result<FieldDefinition, Status> {
    Ok(FieldDefinition {
        id: parse_optional_uuid(Some(field.id))?.unwrap_or_else(Uuid::new_v4),
        key: field.key,
        label: field.label,
        field_type: field_type_from_proto(field.field_type),
        required: field.r#required,
        options: field.options,
        relation_object_key: field.relation_object_key,
        active: field.active,
    })
}

fn relationship_definition_from_proto(
    definition: pb::RelationshipDefinition,
) -> Result<RelationshipDefinition, Status> {
    Ok(RelationshipDefinition {
        id: parse_optional_uuid(Some(definition.id))?.unwrap_or_else(Uuid::new_v4),
        target_object_key: definition.target_object_key,
        cardinality: relationship_cardinality_from_proto(definition.cardinality),
        label: definition.label,
    })
}

fn proto_organization(value: Organization) -> pb::Organization {
    pb::Organization {
        id: value.id.to_string(),
        name: value.name,
        external_key: value.external_key,
        website: value.website,
        industry: value.industry,
        lifecycle: match value.lifecycle {
            OrganizationLifecycle::Prospect => pb::OrganizationLifecycle::Prospect as i32,
            OrganizationLifecycle::Active => pb::OrganizationLifecycle::Active as i32,
            OrganizationLifecycle::Dormant => pb::OrganizationLifecycle::Dormant as i32,
            OrganizationLifecycle::Partner => pb::OrganizationLifecycle::Partner as i32,
        },
        owner_user_id: value.owner_user_id,
        tags: value.tags,
        created_at: proto_timestamp(value.created_at),
        updated_at: proto_timestamp(value.updated_at),
    }
}

fn proto_person(value: Person) -> pb::Person {
    pb::Person {
        id: value.id.to_string(),
        organization_id: value.organization_id.map(|id| id.to_string()),
        full_name: value.full_name,
        title: value.title,
        email: value.email,
        phone: value.phone,
        linkedin_url: value.linkedin_url,
        created_at: proto_timestamp(value.created_at),
        updated_at: proto_timestamp(value.updated_at),
    }
}

fn proto_relationship(value: Relationship) -> pb::Relationship {
    pb::Relationship {
        id: value.id.to_string(),
        from: Some(proto_record_ref(value.from)),
        to: Some(proto_record_ref(value.to)),
        relationship_type: match value.relationship_type {
            crm_kernel::RelationshipType::Employment => pb::RelationshipType::Employment as i32,
            crm_kernel::RelationshipType::Champion => pb::RelationshipType::Champion as i32,
            crm_kernel::RelationshipType::DecisionMaker => {
                pb::RelationshipType::DecisionMaker as i32
            }
            crm_kernel::RelationshipType::Partner => pb::RelationshipType::Partner as i32,
            crm_kernel::RelationshipType::Competitor => pb::RelationshipType::Competitor as i32,
            crm_kernel::RelationshipType::Other => pb::RelationshipType::Other as i32,
        },
        label: value.label,
        created_at: proto_timestamp(value.created_at),
    }
}

fn proto_opportunity(value: Opportunity) -> pb::Opportunity {
    pb::Opportunity {
        id: value.id.to_string(),
        organization_id: value.organization_id.to_string(),
        primary_contact_id: value.primary_contact_id.map(|id| id.to_string()),
        name: value.name,
        stage: match value.stage {
            OpportunityStage::Qualifying => pb::OpportunityStage::Qualifying as i32,
            OpportunityStage::Discovery => pb::OpportunityStage::Discovery as i32,
            OpportunityStage::Proposal => pb::OpportunityStage::Proposal as i32,
            OpportunityStage::Negotiation => pb::OpportunityStage::Negotiation as i32,
            OpportunityStage::ClosedWon => pb::OpportunityStage::ClosedWon as i32,
            OpportunityStage::ClosedLost => pb::OpportunityStage::ClosedLost as i32,
        },
        value: Some(pb::Money {
            currency_code: value.value.currency_code,
            amount_minor: value.value.amount_minor,
        }),
        confidence_bps: u32::from(value.confidence_bps),
        next_step: value.next_step,
        expected_close_at: value.expected_close_at.and_then(proto_timestamp),
        created_at: proto_timestamp(value.created_at),
        updated_at: proto_timestamp(value.updated_at),
    }
}

fn proto_activity(value: crm_kernel::Activity) -> pb::Activity {
    pb::Activity {
        id: value.id.to_string(),
        subject: value.subject,
        details: value.details,
        actor: Some(proto_actor(value.actor)),
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
        outcome: match value.outcome {
            ActivityOutcome::Completed => pb::ActivityOutcome::Completed as i32,
            ActivityOutcome::Waiting => pb::ActivityOutcome::Waiting as i32,
            ActivityOutcome::Blocked => pb::ActivityOutcome::Blocked as i32,
        },
        occurred_at: proto_timestamp(value.occurred_at),
        next_action_due_at: value.next_action_due_at.and_then(proto_timestamp),
    }
}

fn proto_note(value: crm_kernel::Note) -> pb::Note {
    pb::Note {
        id: value.id.to_string(),
        subject: value.subject,
        body: value.body,
        author: Some(proto_actor(value.author)),
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
        promoted_to_fact: value.promoted_to_fact,
        created_at: proto_timestamp(value.created_at),
    }
}

fn proto_document(value: crm_kernel::Document) -> pb::Document {
    pb::Document {
        id: value.id.to_string(),
        title: value.title,
        media_type: value.media_type,
        uri: value.uri,
        status: match value.status {
            DocumentStatus::Draft => pb::DocumentStatus::Draft as i32,
            DocumentStatus::Verified => pb::DocumentStatus::Verified as i32,
            DocumentStatus::Archived => pb::DocumentStatus::Archived as i32,
        },
        uploaded_by: Some(proto_actor(value.uploaded_by)),
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
        created_at: proto_timestamp(value.created_at),
    }
}

fn proto_communication_event(value: crm_kernel::CommunicationEvent) -> pb::CommunicationEvent {
    pb::CommunicationEvent {
        id: value.id.to_string(),
        channel: match value.channel {
            CommunicationChannel::Email => pb::CommunicationChannel::Email as i32,
            CommunicationChannel::Phone => pb::CommunicationChannel::Phone as i32,
            CommunicationChannel::Meeting => pb::CommunicationChannel::Meeting as i32,
            CommunicationChannel::Chat => pb::CommunicationChannel::Chat as i32,
            CommunicationChannel::Sms => pb::CommunicationChannel::Sms as i32,
        },
        direction: match value.direction {
            CommunicationDirection::Inbound => pb::CommunicationDirection::Inbound as i32,
            CommunicationDirection::Outbound => pb::CommunicationDirection::Outbound as i32,
            CommunicationDirection::Internal => pb::CommunicationDirection::Internal as i32,
        },
        subject: value.subject,
        summary: value.summary,
        counterpart: value.counterpart,
        actor: Some(proto_actor(value.actor)),
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
        occurred_at: proto_timestamp(value.occurred_at),
    }
}

fn proto_workflow_case(value: WorkflowCase) -> pb::WorkflowCase {
    pb::WorkflowCase {
        id: value.id.to_string(),
        title: value.title,
        state: match value.state {
            WorkflowState::Open => pb::WorkflowState::Open as i32,
            WorkflowState::AwaitingApproval => pb::WorkflowState::AwaitingApproval as i32,
            WorkflowState::WaitingExternal => pb::WorkflowState::WaitingExternal as i32,
            WorkflowState::Blocked => pb::WorkflowState::Blocked as i32,
            WorkflowState::Done => pb::WorkflowState::Done as i32,
        },
        priority: match value.priority {
            WorkflowPriority::Low => pb::WorkflowPriority::Low as i32,
            WorkflowPriority::Medium => pb::WorkflowPriority::Medium as i32,
            WorkflowPriority::High => pb::WorkflowPriority::High as i32,
            WorkflowPriority::Critical => pb::WorkflowPriority::Critical as i32,
        },
        owner_user_id: value.owner_user_id,
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
        opened_at: proto_timestamp(value.opened_at),
        updated_at: proto_timestamp(value.updated_at),
    }
}

fn proto_fact(value: Fact) -> pb::Fact {
    pb::Fact {
        id: value.id.to_string(),
        statement: value.statement,
        confidence_bps: u32::from(value.confidence_bps),
        promoted_by: Some(proto_actor(value.promoted_by)),
        source_note_id: value.source_note_id.map(|id| id.to_string()),
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
        created_at: proto_timestamp(value.created_at),
    }
}

fn proto_permission_grant(value: crm_kernel::PermissionGrant) -> pb::PermissionGrant {
    pb::PermissionGrant {
        id: value.id.to_string(),
        subject: value.subject,
        role: value.role,
        scope: value.scope,
        granted_by: Some(proto_actor(value.granted_by)),
        created_at: proto_timestamp(value.created_at),
    }
}

fn proto_timeline_entry(value: TimelineEntry) -> pb::TimelineEntry {
    pb::TimelineEntry {
        id: value.id.to_string(),
        kind: match value.kind {
            crm_kernel::TimelineEntryKind::Activity => pb::TimelineEntryKind::Activity as i32,
            crm_kernel::TimelineEntryKind::Note => pb::TimelineEntryKind::Note as i32,
            crm_kernel::TimelineEntryKind::Document => pb::TimelineEntryKind::Document as i32,
            crm_kernel::TimelineEntryKind::Communication => {
                pb::TimelineEntryKind::Communication as i32
            }
            crm_kernel::TimelineEntryKind::Fact => pb::TimelineEntryKind::Fact as i32,
            crm_kernel::TimelineEntryKind::Audit => pb::TimelineEntryKind::Audit as i32,
        },
        anchor: value.anchor.map(proto_record_ref),
        headline: value.headline,
        body: value.body,
        actor: Some(proto_actor(value.actor)),
        occurred_at: proto_timestamp(value.occurred_at),
        related_to: value.related_to.into_iter().map(proto_record_ref).collect(),
    }
}

fn proto_account_summary(value: crm_kernel::AccountSummary) -> pb::AccountSummary {
    pb::AccountSummary {
        organization: Some(proto_organization(value.organization)),
        contacts: value.contacts.into_iter().map(proto_person).collect(),
        opportunities: value
            .opportunities
            .into_iter()
            .map(proto_opportunity)
            .collect(),
        workflow_cases: value
            .workflow_cases
            .into_iter()
            .map(proto_workflow_case)
            .collect(),
        facts: value.facts.into_iter().map(proto_fact).collect(),
        documents: value.documents.into_iter().map(proto_document).collect(),
        permissions: value
            .permissions
            .into_iter()
            .map(proto_permission_grant)
            .collect(),
        recent_timeline: value
            .recent_timeline
            .into_iter()
            .map(proto_timeline_entry)
            .collect(),
    }
}

fn proto_field_definition(value: FieldDefinition) -> pb::FieldDefinition {
    pb::FieldDefinition {
        id: value.id.to_string(),
        key: value.key,
        label: value.label,
        field_type: match value.field_type {
            FieldType::Text => pb::FieldType::Text as i32,
            FieldType::LongText => pb::FieldType::LongText as i32,
            FieldType::Number => pb::FieldType::Number as i32,
            FieldType::Currency => pb::FieldType::Currency as i32,
            FieldType::Boolean => pb::FieldType::Boolean as i32,
            FieldType::Date => pb::FieldType::Date as i32,
            FieldType::DateTime => pb::FieldType::DateTime as i32,
            FieldType::Email => pb::FieldType::Email as i32,
            FieldType::Phone => pb::FieldType::Phone as i32,
            FieldType::Url => pb::FieldType::Url as i32,
            FieldType::Select => pb::FieldType::Select as i32,
            FieldType::MultiSelect => pb::FieldType::MultiSelect as i32,
            FieldType::Relation => pb::FieldType::Relation as i32,
        },
        r#required: value.required,
        options: value.options,
        relation_object_key: value.relation_object_key,
        active: value.active,
    }
}

fn proto_relationship_definition(value: RelationshipDefinition) -> pb::RelationshipDefinition {
    pb::RelationshipDefinition {
        id: value.id.to_string(),
        target_object_key: value.target_object_key,
        cardinality: match value.cardinality {
            RelationshipCardinality::OneToOne => pb::RelationshipCardinality::OneToOne as i32,
            RelationshipCardinality::OneToMany => pb::RelationshipCardinality::OneToMany as i32,
            RelationshipCardinality::ManyToMany => pb::RelationshipCardinality::ManyToMany as i32,
        },
        label: value.label,
    }
}

fn proto_object_definition(value: ObjectDefinition) -> pb::ObjectDefinition {
    pb::ObjectDefinition {
        id: value.id.to_string(),
        key: value.key,
        display_name: value.display_name,
        kind: match value.kind {
            ObjectDefinitionKind::Standard => pb::ObjectDefinitionKind::Standard as i32,
            ObjectDefinitionKind::Custom => pb::ObjectDefinitionKind::Custom as i32,
        },
        fields: value
            .fields
            .into_iter()
            .map(proto_field_definition)
            .collect(),
        relationships: value
            .relationships
            .into_iter()
            .map(proto_relationship_definition)
            .collect(),
        active: value.active,
        created_at: proto_timestamp(value.created_at),
        updated_at: proto_timestamp(value.updated_at),
    }
}

fn proto_view_definition(value: ViewDefinition) -> pb::ViewDefinition {
    pb::ViewDefinition {
        id: value.id.to_string(),
        object_key: value.object_key,
        name: value.name,
        layout: match value.layout {
            ViewLayout::Table => pb::ViewLayout::Table as i32,
            ViewLayout::Kanban => pb::ViewLayout::Kanban as i32,
            ViewLayout::Calendar => pb::ViewLayout::Calendar as i32,
        },
        filter_expression: value.filter_expression,
        sort_expression: value.sort_expression,
        visible_fields: value.visible_fields,
        group_by: value.group_by,
        favorite: value.favorite,
        owner_user_id: value.owner_user_id,
        created_at: proto_timestamp(value.created_at),
        updated_at: proto_timestamp(value.updated_at),
    }
}

fn proto_module_info(module: CapabilityModule) -> modules_pb::ModuleInfo {
    modules_pb::ModuleInfo {
        key: module.key.to_string(),
        display_name: module.display_name.to_string(),
        suite: match module.suite {
            ModuleSuite::Foundation => modules_pb::ModuleSuite::Foundation as i32,
            ModuleSuite::RelationshipCore => modules_pb::ModuleSuite::RelationshipCore as i32,
            ModuleSuite::CommercialCore => modules_pb::ModuleSuite::CommercialCore as i32,
            ModuleSuite::UsageRevenueCore => modules_pb::ModuleSuite::UsageRevenueCore as i32,
            ModuleSuite::WorkCore => modules_pb::ModuleSuite::WorkCore as i32,
            ModuleSuite::TrustCore => modules_pb::ModuleSuite::TrustCore as i32,
            ModuleSuite::IntelligenceCore => modules_pb::ModuleSuite::IntelligenceCore as i32,
        },
        crate_name: module.crate_name.to_string(),
        purpose: module.purpose.to_string(),
        dependencies: module
            .dependencies
            .iter()
            .map(|item| (*item).to_string())
            .collect(),
        owned_objects: module
            .owned_objects
            .iter()
            .map(|item| (*item).to_string())
            .collect(),
        api: Some(modules_pb::ModuleApiSurface {
            grpc_package: module.api.grpc_package.to_string(),
            grpc_service: module.api.grpc_service.to_string(),
            openapi_tag: module.api.openapi_tag.to_string(),
            openapi_base_path: module.api.openapi_base_path.to_string(),
            graphql_query_root: module.api.graphql_query_root.to_string(),
            graphql_mutation_root: module.api.graphql_mutation_root.to_string(),
        }),
    }
}

fn truth_kind_filter_from_proto(value: i32) -> Option<CatalogTruthKind> {
    match truths_pb::TruthKind::try_from(value).unwrap_or(truths_pb::TruthKind::Unspecified) {
        truths_pb::TruthKind::Job => Some(CatalogTruthKind::Job),
        truths_pb::TruthKind::Policy => Some(CatalogTruthKind::Policy),
        truths_pb::TruthKind::ModuleLocal => Some(CatalogTruthKind::ModuleLocal),
        truths_pb::TruthKind::Unspecified => None,
    }
}

fn truth_kind_rank(kind: CatalogTruthKind) -> u8 {
    match kind {
        CatalogTruthKind::Job => 0,
        CatalogTruthKind::Policy => 1,
        CatalogTruthKind::ModuleLocal => 2,
    }
}

fn proto_truth_kind(kind: CatalogTruthKind) -> i32 {
    match kind {
        CatalogTruthKind::Job => truths_pb::TruthKind::Job as i32,
        CatalogTruthKind::Policy => truths_pb::TruthKind::Policy as i32,
        CatalogTruthKind::ModuleLocal => truths_pb::TruthKind::ModuleLocal as i32,
    }
}

fn proto_truth_info(truth: TruthDefinition, include_gherkin: bool) -> truths_pb::TruthInfo {
    let converge = converge_binding_for_truth(truth.key).map(|binding| {
        let intent_id = binding.intent.id.as_str().to_string();
        let request = binding.intent.request.clone();
        truths_pb::ConvergeBinding {
            runtime: binding.runtime.to_string(),
            intent_id,
            request,
            intent_kind: binding.intent_kind_name().to_string(),
            pack_ids: binding.pack_ids.iter().map(ToString::to_string).collect(),
            required_success_criteria: binding.required_success_criteria(),
            hard_constraints: binding.hard_constraints(),
            approval_points: binding
                .approval_points
                .iter()
                .map(ToString::to_string)
                .collect(),
        }
    });

    truths_pb::TruthInfo {
        key: truth.key.to_string(),
        display_name: truth.display_name.to_string(),
        kind: proto_truth_kind(truth.kind),
        summary: truth.summary.to_string(),
        feature_path: truth.feature_path.to_string(),
        actor_roles: truth.actor_roles.iter().map(ToString::to_string).collect(),
        approval_points: truth
            .approval_points
            .iter()
            .map(ToString::to_string)
            .collect(),
        desired_outcomes: truth
            .desired_outcomes
            .iter()
            .map(ToString::to_string)
            .collect(),
        guardrails: truth.guardrails.iter().map(ToString::to_string).collect(),
        modules: truth
            .modules
            .iter()
            .map(|touch| truths_pb::TruthModuleTouch {
                module_key: touch.module_key.to_string(),
                responsibility: touch.responsibility.to_string(),
            })
            .collect(),
        gherkin: if include_gherkin {
            truth.gherkin.to_string()
        } else {
            String::new()
        },
        converge,
    }
}

fn proto_execute_truth_response(
    truth: TruthDefinition,
    execution: TruthExecutionArtifacts,
) -> truths_pb::ExecuteTruthResponse {
    let TruthExecutionArtifacts {
        result,
        experience_events,
        projection,
    } = execution;
    let context_fact_ids = result
        .context
        .all_keys()
        .into_iter()
        .flat_map(|key| {
            result
                .context
                .get(key)
                .iter()
                .map(|fact| fact.id.clone())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let criteria_outcomes = result
        .criteria_outcomes
        .into_iter()
        .map(proto_criterion_outcome)
        .collect::<Vec<_>>();
    let projection = projection.unwrap_or(TruthProjection {
        organization: None,
        person: None,
        opportunity: None,
        subscription: None,
        entitlements: Vec::new(),
        ledger_entries: Vec::new(),
        documents: Vec::new(),
        workflow_cases: Vec::new(),
        facts: Vec::new(),
        domain_event_kinds: Vec::new(),
    });

    truths_pb::ExecuteTruthResponse {
        truth: Some(proto_truth_info(truth, true)),
        execution: Some(truths_pb::TruthExecution {
            intent_id: format!("truth:{}", truth.key),
            converged: result.converged,
            cycles: result.cycles,
            stop_reason: stop_reason_name(&result.stop_reason).to_string(),
            criteria_outcomes,
            experience_event_kinds: experience_events
                .into_iter()
                .map(|event| format!("{:?}", event.kind()))
                .collect(),
            context_fact_ids,
        }),
        organization: projection.organization.map(proto_organization),
        person: projection.person.map(proto_person),
        opportunity: projection.opportunity.map(proto_opportunity),
        projected_subscription: projection.subscription.map(proto_order_subscription),
        projected_entitlements: projection
            .entitlements
            .into_iter()
            .map(proto_entitlement)
            .collect(),
        projected_ledger_entries: projection
            .ledger_entries
            .into_iter()
            .map(proto_ledger_entry)
            .collect(),
        projected_facts: projection.facts.into_iter().map(proto_fact).collect(),
        projected_event_kinds: projection
            .domain_event_kinds
            .into_iter()
            .map(ToString::to_string)
            .collect(),
        projected_documents: projection
            .documents
            .into_iter()
            .map(proto_document)
            .collect(),
        projected_workflow_cases: projection
            .workflow_cases
            .into_iter()
            .map(proto_workflow_case)
            .collect(),
    }
}

fn proto_criterion_outcome(
    outcome: converge_core::CriterionOutcome,
) -> truths_pb::CriterionOutcome {
    let (status, evidence_fact_ids, detail, approval_ref) = match outcome.result {
        CriterionResult::Met { evidence } => (
            truths_pb::CriterionStatus::Met as i32,
            evidence
                .into_iter()
                .map(|fact_id| fact_id.to_string())
                .collect(),
            None,
            None,
        ),
        CriterionResult::Unmet { reason } => (
            truths_pb::CriterionStatus::Unmet as i32,
            Vec::new(),
            Some(reason),
            None,
        ),
        CriterionResult::Blocked {
            reason,
            approval_ref,
        } => (
            truths_pb::CriterionStatus::Blocked as i32,
            Vec::new(),
            Some(reason),
            approval_ref,
        ),
        CriterionResult::Indeterminate => (
            truths_pb::CriterionStatus::Indeterminate as i32,
            Vec::new(),
            None,
            None,
        ),
    };

    truths_pb::CriterionOutcome {
        criterion_id: outcome.criterion.id,
        description: outcome.criterion.description,
        required: outcome.criterion.required,
        status,
        evidence_fact_ids,
        detail,
        approval_ref,
    }
}

fn stop_reason_name(stop_reason: &converge_core::StopReason) -> &'static str {
    match stop_reason {
        converge_core::StopReason::Converged => "converged",
        converge_core::StopReason::CriteriaMet { .. } => "criteria-met",
        converge_core::StopReason::UserCancelled => "user-cancelled",
        converge_core::StopReason::HumanInterventionRequired { .. } => {
            "human-intervention-required"
        }
        converge_core::StopReason::CycleBudgetExhausted { .. } => "cycle-budget-exhausted",
        converge_core::StopReason::FactBudgetExhausted { .. } => "fact-budget-exhausted",
        converge_core::StopReason::TokenBudgetExhausted { .. } => "token-budget-exhausted",
        converge_core::StopReason::TimeBudgetExhausted { .. } => "time-budget-exhausted",
        converge_core::StopReason::InvariantViolated { .. } => "invariant-violated",
        converge_core::StopReason::PromotionRejected { .. } => "promotion-rejected",
        converge_core::StopReason::Error { .. } => "error",
        converge_core::StopReason::AgentRefused { .. } => "agent-refused",
        converge_core::StopReason::HitlGatePending { .. } => "hitl-gate-pending",
        _ => "unknown",
    }
}
