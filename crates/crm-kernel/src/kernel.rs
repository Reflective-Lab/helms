use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::{KernelError, KernelResult};
use crate::events::DomainEvent;
use crate::model::{
    AccountSummary, Activity, ActivityOutcome, Actor, AgentRun, Approval, AuditEntry, CatalogItem,
    CommunicationChannel, CommunicationDirection, CommunicationEvent, Conversation, Document,
    DocumentStatus, Fact, FieldDefinition, Intent, Job, Lead, Money, Note, ObjectDefinition,
    ObjectDefinitionKind, OfferQuote, Opportunity, OpportunityStage, OrderSubscription,
    Organization, OrganizationLifecycle, PermissionGrant, Person, Policy, ProposedFact, RecordKind,
    RecordRef, Relationship, RelationshipCardinality, RelationshipDefinition, RelationshipType,
    Role, Task, TimelineEntry, TimelineEntryKind, ViewDefinition, ViewLayout, WorkflowCase,
    WorkflowDefinition, WorkflowPriority, WorkflowRun, WorkflowState, Workspace, WorkspaceMember,
};

#[derive(Debug, Clone)]
pub struct OrganizationUpsert {
    pub organization_id: Option<Uuid>,
    pub name: String,
    pub external_key: Option<String>,
    pub website: Option<String>,
    pub industry: Option<String>,
    pub lifecycle: OrganizationLifecycle,
    pub owner_user_id: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PersonUpsert {
    pub person_id: Option<Uuid>,
    pub organization_id: Option<Uuid>,
    pub full_name: String,
    pub title: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub linkedin_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RelationshipLink {
    pub from: RecordRef,
    pub to: RecordRef,
    pub relationship_type: RelationshipType,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpportunityCreate {
    pub organization_id: Uuid,
    pub primary_contact_id: Option<Uuid>,
    pub name: String,
    pub value: Money,
    pub confidence_bps: u16,
    pub next_step: Option<String>,
    pub expected_close_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct OpportunityAdvance {
    pub opportunity_id: Uuid,
    pub stage: OpportunityStage,
    pub next_step: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ActivityAppend {
    pub subject: String,
    pub details: String,
    pub related_to: Vec<RecordRef>,
    pub outcome: ActivityOutcome,
    pub occurred_at: Option<DateTime<Utc>>,
    pub next_action_due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NoteAppend {
    pub subject: String,
    pub body: String,
    pub related_to: Vec<RecordRef>,
}

#[derive(Debug, Clone)]
pub struct DocumentAttach {
    pub title: String,
    pub media_type: String,
    pub uri: String,
    pub status: DocumentStatus,
    pub related_to: Vec<RecordRef>,
}

#[derive(Debug, Clone)]
pub struct CommunicationRecord {
    pub channel: CommunicationChannel,
    pub direction: CommunicationDirection,
    pub subject: Option<String>,
    pub summary: String,
    pub counterpart: String,
    pub related_to: Vec<RecordRef>,
    pub occurred_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct WorkflowCaseCreate {
    pub title: String,
    pub priority: WorkflowPriority,
    pub owner_user_id: Option<String>,
    pub related_to: Vec<RecordRef>,
}

#[derive(Debug, Clone)]
pub struct WorkflowCaseAdvance {
    pub workflow_case_id: Uuid,
    pub state: WorkflowState,
}

#[derive(Debug, Clone)]
pub struct PermissionGrantInput {
    pub subject: String,
    pub role: String,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct FactRecord {
    pub statement: String,
    pub confidence_bps: u16,
    pub related_to: Vec<RecordRef>,
    pub source_note_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct ObjectDefinitionUpsert {
    pub object_definition_id: Option<Uuid>,
    pub key: String,
    pub display_name: String,
    pub kind: ObjectDefinitionKind,
    pub fields: Vec<FieldDefinition>,
    pub relationships: Vec<RelationshipDefinition>,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct ViewDefinitionUpsert {
    pub view_definition_id: Option<Uuid>,
    pub object_key: String,
    pub name: String,
    pub layout: ViewLayout,
    pub filter_expression: Option<String>,
    pub sort_expression: Option<String>,
    pub visible_fields: Vec<String>,
    pub group_by: Option<String>,
    pub favorite: bool,
    pub owner_user_id: Option<String>,
}

#[derive(Debug, Default)]
pub struct CrmKernel {
    pub organizations: HashMap<Uuid, Organization>,
    pub people: HashMap<Uuid, Person>,
    pub relationships: HashMap<Uuid, Relationship>,
    pub leads: HashMap<Uuid, Lead>,
    pub opportunities: HashMap<Uuid, Opportunity>,
    pub conversations: HashMap<Uuid, Conversation>,
    pub activities: HashMap<Uuid, Activity>,
    pub tasks: HashMap<Uuid, Task>,
    pub notes: HashMap<Uuid, Note>,
    pub quotes: HashMap<Uuid, OfferQuote>,
    pub orders: HashMap<Uuid, OrderSubscription>,
    pub documents: HashMap<Uuid, Document>,
    pub facts: HashMap<Uuid, Fact>,
    pub intents: HashMap<Uuid, Intent>,
    pub workflow_cases: HashMap<Uuid, WorkflowCase>,
    pub communication_events: HashMap<Uuid, CommunicationEvent>,
    pub permission_grants: HashMap<Uuid, PermissionGrant>,
    pub catalog_items: HashMap<Uuid, CatalogItem>,
    pub object_definitions: HashMap<Uuid, ObjectDefinition>,
    pub view_definitions: HashMap<Uuid, ViewDefinition>,
    pub workspaces: HashMap<Uuid, Workspace>,
    pub workspace_members: HashMap<Uuid, WorkspaceMember>,
    pub roles: HashMap<Uuid, Role>,
    pub jobs: HashMap<Uuid, Job>,
    pub proposed_facts: HashMap<Uuid, ProposedFact>,
    pub approvals: HashMap<Uuid, Approval>,
    pub agent_runs: HashMap<Uuid, AgentRun>,
    pub policies: HashMap<Uuid, Policy>,
    pub workflow_definitions: HashMap<Uuid, WorkflowDefinition>,
    pub workflow_runs: HashMap<Uuid, WorkflowRun>,
    pub audit_trail: Vec<AuditEntry>,
    pub timeline: Vec<TimelineEntry>,
    pub pending_events: Vec<DomainEvent>,
}

impl CrmKernel {
    #[must_use]
    pub fn drain_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.pending_events)
    }

    pub fn upsert_organization(
        &mut self,
        command: OrganizationUpsert,
        actor: Actor,
    ) -> KernelResult<Organization> {
        let now = Utc::now();
        let id = command.organization_id.unwrap_or_else(Uuid::new_v4);
        let organization = if let Some(existing) = self.organizations.get(&id) {
            Organization {
                id,
                name: required("organization.name", &command.name)?,
                external_key: optional_trimmed(command.external_key),
                website: optional_trimmed(command.website),
                industry: optional_trimmed(command.industry),
                lifecycle: command.lifecycle,
                owner_user_id: optional_trimmed(command.owner_user_id),
                tags: normalize_tags(command.tags),
                created_at: existing.created_at,
                updated_at: now,
            }
        } else {
            Organization {
                id,
                name: required("organization.name", &command.name)?,
                external_key: optional_trimmed(command.external_key),
                website: optional_trimmed(command.website),
                industry: optional_trimmed(command.industry),
                lifecycle: command.lifecycle,
                owner_user_id: optional_trimmed(command.owner_user_id),
                tags: normalize_tags(command.tags),
                created_at: now,
                updated_at: now,
            }
        };

        self.organizations.insert(id, organization.clone());
        self.record_event(DomainEvent::OrganizationUpserted {
            organization: organization.clone(),
            actor: actor.clone(),
        });
        let record = RecordRef {
            kind: RecordKind::Organization,
            id,
        };
        self.record_audit(
            "organization.upserted",
            Some(record),
            actor.clone(),
            &[("name", organization.name.clone())],
        );
        self.push_timeline(TimelineEntry {
            id: Uuid::new_v4(),
            kind: TimelineEntryKind::Audit,
            anchor: Some(record),
            headline: format!("Organization saved: {}", organization.name),
            body: organization
                .website
                .clone()
                .unwrap_or_else(|| "Organization context updated".to_string()),
            actor,
            occurred_at: now,
            related_to: vec![record],
        });

        Ok(organization)
    }

    pub fn upsert_person(&mut self, command: PersonUpsert, actor: Actor) -> KernelResult<Person> {
        if let Some(organization_id) = command.organization_id {
            self.ensure_record_exists(RecordRef {
                kind: RecordKind::Organization,
                id: organization_id,
            })?;
        }

        let now = Utc::now();
        let id = command.person_id.unwrap_or_else(Uuid::new_v4);
        let person = if let Some(existing) = self.people.get(&id) {
            Person {
                id,
                organization_id: command.organization_id,
                full_name: required("person.full_name", &command.full_name)?,
                title: optional_trimmed(command.title),
                email: optional_trimmed(command.email),
                phone: optional_trimmed(command.phone),
                linkedin_url: optional_trimmed(command.linkedin_url),
                created_at: existing.created_at,
                updated_at: now,
            }
        } else {
            Person {
                id,
                organization_id: command.organization_id,
                full_name: required("person.full_name", &command.full_name)?,
                title: optional_trimmed(command.title),
                email: optional_trimmed(command.email),
                phone: optional_trimmed(command.phone),
                linkedin_url: optional_trimmed(command.linkedin_url),
                created_at: now,
                updated_at: now,
            }
        };

        self.people.insert(id, person.clone());
        self.record_event(DomainEvent::PersonUpserted {
            person: person.clone(),
            actor: actor.clone(),
        });
        let record = RecordRef {
            kind: RecordKind::Person,
            id,
        };
        self.record_audit(
            "person.upserted",
            Some(record),
            actor.clone(),
            &[("full_name", person.full_name.clone())],
        );
        self.push_timeline(TimelineEntry {
            id: Uuid::new_v4(),
            kind: TimelineEntryKind::Audit,
            anchor: Some(record),
            headline: format!("Contact saved: {}", person.full_name),
            body: person
                .title
                .clone()
                .unwrap_or_else(|| "Contact context updated".to_string()),
            actor,
            occurred_at: now,
            related_to: related_with_org(record, person.organization_id),
        });

        Ok(person)
    }

    pub fn link_relationship(
        &mut self,
        command: RelationshipLink,
        actor: Actor,
    ) -> KernelResult<Relationship> {
        if command.from == command.to {
            return Err(KernelError::Invariant(
                "relationship endpoints must be distinct".to_string(),
            ));
        }

        self.ensure_record_exists(command.from)?;
        self.ensure_record_exists(command.to)?;

        let relationship = Relationship {
            id: Uuid::new_v4(),
            from: command.from,
            to: command.to,
            relationship_type: command.relationship_type,
            label: optional_trimmed(command.label),
            created_at: Utc::now(),
        };

        self.relationships
            .insert(relationship.id, relationship.clone());
        self.record_event(DomainEvent::RelationshipLinked {
            relationship: relationship.clone(),
            actor: actor.clone(),
        });
        let record = RecordRef {
            kind: RecordKind::Relationship,
            id: relationship.id,
        };
        self.record_audit(
            "relationship.linked",
            Some(record),
            actor.clone(),
            &[
                ("from", relationship.from.id.to_string()),
                ("to", relationship.to.id.to_string()),
            ],
        );
        self.push_timeline(TimelineEntry {
            id: Uuid::new_v4(),
            kind: TimelineEntryKind::Audit,
            anchor: Some(record),
            headline: "Relationship linked".to_string(),
            body: relationship
                .label
                .clone()
                .unwrap_or_else(|| format!("{:?}", relationship.relationship_type)),
            actor,
            occurred_at: relationship.created_at,
            related_to: vec![relationship.from, relationship.to],
        });

        Ok(relationship)
    }

    pub fn create_opportunity(
        &mut self,
        command: OpportunityCreate,
        actor: Actor,
    ) -> KernelResult<Opportunity> {
        self.ensure_record_exists(RecordRef {
            kind: RecordKind::Organization,
            id: command.organization_id,
        })?;
        if let Some(contact_id) = command.primary_contact_id {
            self.ensure_record_exists(RecordRef {
                kind: RecordKind::Person,
                id: contact_id,
            })?;
        }
        validate_money(&command.value)?;
        validate_confidence(command.confidence_bps)?;

        let now = Utc::now();
        let opportunity = Opportunity {
            id: Uuid::new_v4(),
            organization_id: command.organization_id,
            primary_contact_id: command.primary_contact_id,
            name: required("opportunity.name", &command.name)?,
            stage: OpportunityStage::Qualifying,
            value: command.value,
            confidence_bps: command.confidence_bps,
            next_step: optional_trimmed(command.next_step),
            expected_close_at: command.expected_close_at,
            created_at: now,
            updated_at: now,
        };

        self.opportunities
            .insert(opportunity.id, opportunity.clone());
        self.record_event(DomainEvent::OpportunityCreated {
            opportunity: opportunity.clone(),
            actor: actor.clone(),
        });
        let record = RecordRef {
            kind: RecordKind::Opportunity,
            id: opportunity.id,
        };
        self.record_audit(
            "opportunity.created",
            Some(record),
            actor.clone(),
            &[("name", opportunity.name.clone())],
        );
        self.push_timeline(TimelineEntry {
            id: Uuid::new_v4(),
            kind: TimelineEntryKind::Audit,
            anchor: Some(record),
            headline: format!("Opportunity opened: {}", opportunity.name),
            body: opportunity
                .next_step
                .clone()
                .unwrap_or_else(|| "Pipeline item created".to_string()),
            actor,
            occurred_at: now,
            related_to: opportunity_related_to(&opportunity),
        });

        Ok(opportunity)
    }

    pub fn advance_opportunity(
        &mut self,
        command: OpportunityAdvance,
        actor: Actor,
    ) -> KernelResult<Opportunity> {
        let opportunity = self
            .opportunities
            .get_mut(&command.opportunity_id)
            .ok_or_else(|| KernelError::NotFound {
                kind: "opportunity",
                id: command.opportunity_id.to_string(),
            })?;

        if opportunity.stage.is_closed() && opportunity.stage != command.stage {
            return Err(KernelError::Invariant(
                "closed opportunities require a dedicated reopen flow".to_string(),
            ));
        }

        let previous_stage = opportunity.stage;
        opportunity.stage = command.stage;
        opportunity.next_step = optional_trimmed(command.next_step);
        opportunity.updated_at = Utc::now();
        let updated = opportunity.clone();

        self.record_event(DomainEvent::OpportunityStageChanged {
            opportunity: updated.clone(),
            previous_stage,
            actor: actor.clone(),
        });
        let record = RecordRef {
            kind: RecordKind::Opportunity,
            id: updated.id,
        };
        self.record_audit(
            "opportunity.advanced",
            Some(record),
            actor.clone(),
            &[("stage", format!("{:?}", updated.stage))],
        );
        self.push_timeline(TimelineEntry {
            id: Uuid::new_v4(),
            kind: TimelineEntryKind::Audit,
            anchor: Some(record),
            headline: format!("Opportunity moved to {:?}", updated.stage),
            body: updated
                .next_step
                .clone()
                .unwrap_or_else(|| updated.name.clone()),
            actor,
            occurred_at: updated.updated_at,
            related_to: opportunity_related_to(&updated),
        });

        Ok(updated)
    }

    pub fn append_activity(
        &mut self,
        command: ActivityAppend,
        actor: Actor,
    ) -> KernelResult<Activity> {
        let related_to = self.validate_related(command.related_to)?;
        let occurred_at = command.occurred_at.unwrap_or_else(Utc::now);
        let activity = Activity {
            id: Uuid::new_v4(),
            subject: required("activity.subject", &command.subject)?,
            details: required("activity.details", &command.details)?,
            actor: actor.clone(),
            related_to: related_to.clone(),
            outcome: command.outcome,
            occurred_at,
            next_action_due_at: command.next_action_due_at,
        };

        self.activities.insert(activity.id, activity.clone());
        self.record_event(DomainEvent::ActivityAppended {
            activity: activity.clone(),
            actor: actor.clone(),
        });
        let record = RecordRef {
            kind: RecordKind::Activity,
            id: activity.id,
        };
        self.record_audit(
            "activity.logged",
            Some(record),
            actor.clone(),
            &[("subject", activity.subject.clone())],
        );
        self.push_timeline(TimelineEntry {
            id: Uuid::new_v4(),
            kind: TimelineEntryKind::Activity,
            anchor: related_to.first().copied(),
            headline: activity.subject.clone(),
            body: activity.details.clone(),
            actor,
            occurred_at,
            related_to,
        });

        Ok(activity)
    }

    pub fn append_note(&mut self, command: NoteAppend, actor: Actor) -> KernelResult<Note> {
        let related_to = self.validate_related(command.related_to)?;
        let note = Note {
            id: Uuid::new_v4(),
            subject: required("note.subject", &command.subject)?,
            body: required("note.body", &command.body)?,
            author: actor.clone(),
            related_to: related_to.clone(),
            promoted_to_fact: false,
            created_at: Utc::now(),
        };

        self.notes.insert(note.id, note.clone());
        self.record_event(DomainEvent::NoteAppended {
            note: note.clone(),
            actor: actor.clone(),
        });
        let record = RecordRef {
            kind: RecordKind::Note,
            id: note.id,
        };
        self.record_audit(
            "note.added",
            Some(record),
            actor.clone(),
            &[("subject", note.subject.clone())],
        );
        self.push_timeline(TimelineEntry {
            id: Uuid::new_v4(),
            kind: TimelineEntryKind::Note,
            anchor: related_to.first().copied(),
            headline: note.subject.clone(),
            body: note.body.clone(),
            actor,
            occurred_at: note.created_at,
            related_to,
        });

        Ok(note)
    }

    pub fn attach_document(
        &mut self,
        command: DocumentAttach,
        actor: Actor,
    ) -> KernelResult<Document> {
        let related_to = self.validate_related(command.related_to)?;
        let document = Document {
            id: Uuid::new_v4(),
            title: required("document.title", &command.title)?,
            media_type: required("document.media_type", &command.media_type)?,
            uri: required("document.uri", &command.uri)?,
            status: command.status,
            uploaded_by: actor.clone(),
            related_to: related_to.clone(),
            created_at: Utc::now(),
        };

        self.documents.insert(document.id, document.clone());
        self.record_event(DomainEvent::DocumentAttached {
            document: document.clone(),
            actor: actor.clone(),
        });
        let record = RecordRef {
            kind: RecordKind::Document,
            id: document.id,
        };
        self.record_audit(
            "document.attached",
            Some(record),
            actor.clone(),
            &[("title", document.title.clone())],
        );
        self.push_timeline(TimelineEntry {
            id: Uuid::new_v4(),
            kind: TimelineEntryKind::Document,
            anchor: related_to.first().copied(),
            headline: format!("Document attached: {}", document.title),
            body: document.uri.clone(),
            actor,
            occurred_at: document.created_at,
            related_to,
        });

        Ok(document)
    }

    pub fn record_communication(
        &mut self,
        command: CommunicationRecord,
        actor: Actor,
    ) -> KernelResult<CommunicationEvent> {
        let related_to = self.validate_related(command.related_to)?;
        let event = CommunicationEvent {
            id: Uuid::new_v4(),
            channel: command.channel,
            direction: command.direction,
            subject: optional_trimmed(command.subject),
            summary: required("communication.summary", &command.summary)?,
            counterpart: required("communication.counterpart", &command.counterpart)?,
            actor: actor.clone(),
            related_to: related_to.clone(),
            occurred_at: command.occurred_at.unwrap_or_else(Utc::now),
        };

        self.communication_events.insert(event.id, event.clone());
        self.record_event(DomainEvent::CommunicationRecorded {
            event: event.clone(),
            actor: actor.clone(),
        });
        let record = RecordRef {
            kind: RecordKind::CommunicationEvent,
            id: event.id,
        };
        self.record_audit(
            "communication.recorded",
            Some(record),
            actor.clone(),
            &[("counterpart", event.counterpart.clone())],
        );
        self.push_timeline(TimelineEntry {
            id: Uuid::new_v4(),
            kind: TimelineEntryKind::Communication,
            anchor: related_to.first().copied(),
            headline: event
                .subject
                .clone()
                .unwrap_or_else(|| format!("{:?} interaction", event.channel)),
            body: event.summary.clone(),
            actor,
            occurred_at: event.occurred_at,
            related_to,
        });

        Ok(event)
    }

    pub fn create_workflow_case(
        &mut self,
        command: WorkflowCaseCreate,
        actor: Actor,
    ) -> KernelResult<WorkflowCase> {
        let related_to = self.validate_related(command.related_to)?;
        let now = Utc::now();
        let case = WorkflowCase {
            id: Uuid::new_v4(),
            title: required("workflow_case.title", &command.title)?,
            state: WorkflowState::Open,
            priority: command.priority,
            owner_user_id: optional_trimmed(command.owner_user_id),
            related_to: related_to.clone(),
            opened_at: now,
            updated_at: now,
        };

        self.workflow_cases.insert(case.id, case.clone());
        self.record_event(DomainEvent::WorkflowCaseCreated {
            workflow_case: case.clone(),
            actor: actor.clone(),
        });
        let record = RecordRef {
            kind: RecordKind::WorkflowCase,
            id: case.id,
        };
        self.record_audit(
            "workflow_case.opened",
            Some(record),
            actor.clone(),
            &[("title", case.title.clone())],
        );
        self.push_timeline(TimelineEntry {
            id: Uuid::new_v4(),
            kind: TimelineEntryKind::Audit,
            anchor: related_to.first().copied(),
            headline: format!("Workflow case opened: {}", case.title),
            body: format!("{:?}", case.priority),
            actor,
            occurred_at: now,
            related_to,
        });

        Ok(case)
    }

    pub fn advance_workflow_case(
        &mut self,
        command: WorkflowCaseAdvance,
        actor: Actor,
    ) -> KernelResult<WorkflowCase> {
        let case = self
            .workflow_cases
            .get_mut(&command.workflow_case_id)
            .ok_or_else(|| KernelError::NotFound {
                kind: "workflow_case",
                id: command.workflow_case_id.to_string(),
            })?;

        if case.state.is_terminal() && case.state != command.state {
            return Err(KernelError::Invariant(
                "completed workflow cases require a dedicated reopen flow".to_string(),
            ));
        }

        let previous_state = case.state;
        case.state = command.state;
        case.updated_at = Utc::now();
        let updated = case.clone();
        self.record_event(DomainEvent::WorkflowCaseStateChanged {
            workflow_case: updated.clone(),
            previous_state,
            actor: actor.clone(),
        });
        let record = RecordRef {
            kind: RecordKind::WorkflowCase,
            id: updated.id,
        };
        self.record_audit(
            "workflow_case.advanced",
            Some(record),
            actor.clone(),
            &[("state", format!("{:?}", updated.state))],
        );
        self.push_timeline(TimelineEntry {
            id: Uuid::new_v4(),
            kind: TimelineEntryKind::Audit,
            anchor: updated.related_to.first().copied(),
            headline: format!("Workflow case moved to {:?}", updated.state),
            body: updated.title.clone(),
            actor,
            occurred_at: updated.updated_at,
            related_to: updated.related_to.clone(),
        });

        Ok(updated)
    }

    pub fn grant_permission(
        &mut self,
        command: PermissionGrantInput,
        actor: Actor,
    ) -> KernelResult<PermissionGrant> {
        let grant = PermissionGrant {
            id: Uuid::new_v4(),
            subject: required("permission.subject", &command.subject)?,
            role: required("permission.role", &command.role)?,
            scope: required("permission.scope", &command.scope)?,
            granted_by: actor.clone(),
            created_at: Utc::now(),
        };

        self.permission_grants.insert(grant.id, grant.clone());
        self.record_event(DomainEvent::PermissionGranted {
            grant: grant.clone(),
            actor: actor.clone(),
        });
        self.record_audit(
            "permission.granted",
            Some(RecordRef {
                kind: RecordKind::PermissionGrant,
                id: grant.id,
            }),
            actor,
            &[("scope", grant.scope.clone())],
        );

        Ok(grant)
    }

    pub fn record_fact(&mut self, command: FactRecord, actor: Actor) -> KernelResult<Fact> {
        let related_to = self.validate_related(command.related_to)?;
        validate_confidence(command.confidence_bps)?;

        if let Some(note_id) = command.source_note_id {
            let note = self
                .notes
                .get_mut(&note_id)
                .ok_or_else(|| KernelError::NotFound {
                    kind: "note",
                    id: note_id.to_string(),
                })?;
            note.promoted_to_fact = true;
        }

        let fact = Fact {
            id: Uuid::new_v4(),
            statement: required("fact.statement", &command.statement)?,
            confidence_bps: command.confidence_bps,
            promoted_by: actor.clone(),
            source_note_id: command.source_note_id,
            related_to: related_to.clone(),
            created_at: Utc::now(),
        };

        self.facts.insert(fact.id, fact.clone());
        self.record_event(DomainEvent::FactRecorded {
            fact: fact.clone(),
            actor: actor.clone(),
        });
        let record = RecordRef {
            kind: RecordKind::Fact,
            id: fact.id,
        };
        self.record_audit(
            "fact.recorded",
            Some(record),
            actor.clone(),
            &[("statement", fact.statement.clone())],
        );
        self.push_timeline(TimelineEntry {
            id: Uuid::new_v4(),
            kind: TimelineEntryKind::Fact,
            anchor: related_to.first().copied(),
            headline: "Fact promoted".to_string(),
            body: fact.statement.clone(),
            actor,
            occurred_at: fact.created_at,
            related_to,
        });

        Ok(fact)
    }

    pub fn upsert_object_definition(
        &mut self,
        command: ObjectDefinitionUpsert,
        actor: Actor,
    ) -> KernelResult<ObjectDefinition> {
        let now = Utc::now();
        let id = command.object_definition_id.unwrap_or_else(Uuid::new_v4);
        let fields = validate_field_definitions(command.fields)?;
        let relationships = validate_relationship_definitions(command.relationships)?;
        let definition = if let Some(existing) = self.object_definitions.get(&id) {
            ObjectDefinition {
                id,
                key: required("object_definition.key", &command.key)?,
                display_name: required("object_definition.display_name", &command.display_name)?,
                kind: command.kind,
                fields,
                relationships,
                active: command.active,
                created_at: existing.created_at,
                updated_at: now,
            }
        } else {
            ObjectDefinition {
                id,
                key: required("object_definition.key", &command.key)?,
                display_name: required("object_definition.display_name", &command.display_name)?,
                kind: command.kind,
                fields,
                relationships,
                active: command.active,
                created_at: now,
                updated_at: now,
            }
        };

        self.object_definitions.insert(id, definition.clone());
        self.record_event(DomainEvent::ObjectDefinitionUpserted {
            definition: definition.clone(),
            actor: actor.clone(),
        });
        self.record_audit(
            "metadata.object_definition.upserted",
            None,
            actor,
            &[("key", definition.key.clone())],
        );
        Ok(definition)
    }

    pub fn upsert_view_definition(
        &mut self,
        command: ViewDefinitionUpsert,
        actor: Actor,
    ) -> KernelResult<ViewDefinition> {
        if !self
            .object_definitions
            .values()
            .any(|definition| definition.key == command.object_key)
        {
            return Err(KernelError::Validation(format!(
                "view.object_key references unknown object {}",
                command.object_key
            )));
        }

        let now = Utc::now();
        let id = command.view_definition_id.unwrap_or_else(Uuid::new_v4);
        let visible_fields = command
            .visible_fields
            .into_iter()
            .filter_map(|field| {
                let trimmed = field.trim().to_string();
                (!trimmed.is_empty()).then_some(trimmed)
            })
            .collect::<Vec<_>>();

        let view = if let Some(existing) = self.view_definitions.get(&id) {
            ViewDefinition {
                id,
                object_key: required("view.object_key", &command.object_key)?,
                name: required("view.name", &command.name)?,
                layout: command.layout,
                filter_expression: optional_trimmed(command.filter_expression),
                sort_expression: optional_trimmed(command.sort_expression),
                visible_fields,
                group_by: optional_trimmed(command.group_by),
                favorite: command.favorite,
                owner_user_id: optional_trimmed(command.owner_user_id),
                created_at: existing.created_at,
                updated_at: now,
            }
        } else {
            ViewDefinition {
                id,
                object_key: required("view.object_key", &command.object_key)?,
                name: required("view.name", &command.name)?,
                layout: command.layout,
                filter_expression: optional_trimmed(command.filter_expression),
                sort_expression: optional_trimmed(command.sort_expression),
                visible_fields,
                group_by: optional_trimmed(command.group_by),
                favorite: command.favorite,
                owner_user_id: optional_trimmed(command.owner_user_id),
                created_at: now,
                updated_at: now,
            }
        };

        self.view_definitions.insert(id, view.clone());
        self.record_event(DomainEvent::ViewDefinitionUpserted {
            view: view.clone(),
            actor: actor.clone(),
        });
        self.record_audit(
            "metadata.view_definition.upserted",
            None,
            actor,
            &[("name", view.name.clone())],
        );
        Ok(view)
    }

    pub fn get_account_summary(
        &self,
        organization_id: Uuid,
        timeline_limit: usize,
    ) -> KernelResult<AccountSummary> {
        let organization = self
            .organizations
            .get(&organization_id)
            .cloned()
            .ok_or_else(|| KernelError::NotFound {
                kind: "organization",
                id: organization_id.to_string(),
            })?;

        let contacts = self
            .people
            .values()
            .filter(|person| person.organization_id == Some(organization_id))
            .cloned()
            .collect::<Vec<_>>();

        let opportunities = self
            .opportunities
            .values()
            .filter(|opportunity| opportunity.organization_id == organization_id)
            .cloned()
            .collect::<Vec<_>>();

        let mut related_ids = HashSet::from([organization_id]);
        related_ids.extend(contacts.iter().map(|person| person.id));
        related_ids.extend(opportunities.iter().map(|opportunity| opportunity.id));

        let workflow_cases = self
            .workflow_cases
            .values()
            .filter(|case| {
                case.related_to
                    .iter()
                    .any(|reference| related_ids.contains(&reference.id))
            })
            .cloned()
            .collect::<Vec<_>>();

        related_ids.extend(workflow_cases.iter().map(|case| case.id));

        let facts = self
            .facts
            .values()
            .filter(|fact| {
                fact.related_to
                    .iter()
                    .any(|reference| related_ids.contains(&reference.id))
            })
            .cloned()
            .collect::<Vec<_>>();

        let documents = self
            .documents
            .values()
            .filter(|document| {
                document
                    .related_to
                    .iter()
                    .any(|reference| related_ids.contains(&reference.id))
            })
            .cloned()
            .collect::<Vec<_>>();

        let scope = format!("organization:{organization_id}");
        let permissions = self
            .permission_grants
            .values()
            .filter(|grant| grant.scope == scope || grant.scope == "workspace")
            .cloned()
            .collect::<Vec<_>>();

        let mut recent_timeline = self
            .timeline
            .iter()
            .filter(|entry| {
                entry
                    .related_to
                    .iter()
                    .any(|reference| related_ids.contains(&reference.id))
            })
            .cloned()
            .collect::<Vec<_>>();
        recent_timeline.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
        recent_timeline.truncate(timeline_limit);

        Ok(AccountSummary {
            organization,
            contacts,
            opportunities,
            workflow_cases,
            facts,
            documents,
            permissions,
            recent_timeline,
        })
    }

    #[must_use]
    pub fn list_organizations(&self) -> Vec<Organization> {
        let mut items = self.organizations.values().cloned().collect::<Vec<_>>();
        items.sort_by(|left, right| left.name.cmp(&right.name));
        items
    }

    #[must_use]
    pub fn list_people(&self, organization_id: Option<Uuid>) -> Vec<Person> {
        let mut items = self
            .people
            .values()
            .filter(|person| organization_id.map_or(true, |id| person.organization_id == Some(id)))
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| left.full_name.cmp(&right.full_name));
        items
    }

    #[must_use]
    pub fn list_opportunities(&self, organization_id: Option<Uuid>) -> Vec<Opportunity> {
        let mut items = self
            .opportunities
            .values()
            .filter(|opportunity| {
                organization_id.map_or(true, |id| opportunity.organization_id == id)
            })
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        items
    }

    #[must_use]
    pub fn list_timeline(&self, anchors: &[RecordRef], limit: usize) -> Vec<TimelineEntry> {
        let anchor_ids = anchors
            .iter()
            .map(|reference| reference.id)
            .collect::<HashSet<_>>();
        let mut items = self
            .timeline
            .iter()
            .filter(|entry| {
                anchor_ids.is_empty()
                    || entry
                        .related_to
                        .iter()
                        .any(|reference| anchor_ids.contains(&reference.id))
            })
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
        items.truncate(limit);
        items
    }

    #[must_use]
    pub fn list_object_definitions(&self) -> Vec<ObjectDefinition> {
        let mut items = self
            .object_definitions
            .values()
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| left.key.cmp(&right.key));
        items
    }

    #[must_use]
    pub fn list_view_definitions(&self, object_key: Option<&str>) -> Vec<ViewDefinition> {
        let mut items = self
            .view_definitions
            .values()
            .filter(|view| object_key.map_or(true, |key| view.object_key == key))
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| left.name.cmp(&right.name));
        items
    }

    fn record_audit(
        &mut self,
        action: &str,
        record: Option<RecordRef>,
        actor: Actor,
        detail: &[(&str, String)],
    ) {
        let entry = AuditEntry {
            id: Uuid::new_v4(),
            action: action.to_string(),
            record,
            actor,
            detail: detail
                .iter()
                .map(|(key, value)| ((*key).to_string(), value.clone()))
                .collect::<BTreeMap<_, _>>(),
            occurred_at: Utc::now(),
        };
        self.audit_trail.push(entry.clone());
        self.record_event(DomainEvent::AuditRecorded { entry });
    }

    fn push_timeline(&mut self, entry: TimelineEntry) {
        self.timeline.push(entry.clone());
        self.record_event(DomainEvent::TimelineEntryRecorded { entry });
    }

    fn record_event(&mut self, event: DomainEvent) {
        self.pending_events.push(event);
    }

    fn validate_related(&self, related_to: Vec<RecordRef>) -> KernelResult<Vec<RecordRef>> {
        if related_to.is_empty() {
            return Err(KernelError::Validation(
                "at least one related record is required".to_string(),
            ));
        }

        for reference in &related_to {
            self.ensure_record_exists(*reference)?;
        }

        Ok(related_to)
    }

    fn ensure_record_exists(&self, reference: RecordRef) -> KernelResult<()> {
        let exists = match reference.kind {
            RecordKind::Organization => self.organizations.contains_key(&reference.id),
            RecordKind::Person => self.people.contains_key(&reference.id),
            RecordKind::Relationship => self.relationships.contains_key(&reference.id),
            RecordKind::Lead => self.leads.contains_key(&reference.id),
            RecordKind::Opportunity => self.opportunities.contains_key(&reference.id),
            RecordKind::Conversation => self.conversations.contains_key(&reference.id),
            RecordKind::Activity => self.activities.contains_key(&reference.id),
            RecordKind::Task => self.tasks.contains_key(&reference.id),
            RecordKind::OfferQuote => self.quotes.contains_key(&reference.id),
            RecordKind::OrderSubscription => self.orders.contains_key(&reference.id),
            RecordKind::Document => self.documents.contains_key(&reference.id),
            RecordKind::Fact => self.facts.contains_key(&reference.id),
            RecordKind::Intent => self.intents.contains_key(&reference.id),
            RecordKind::WorkflowCase => self.workflow_cases.contains_key(&reference.id),
            RecordKind::CommunicationEvent => self.communication_events.contains_key(&reference.id),
            RecordKind::PermissionGrant => self.permission_grants.contains_key(&reference.id),
            RecordKind::AuditEntry => self
                .audit_trail
                .iter()
                .any(|entry| entry.id == reference.id),
            RecordKind::Note => self.notes.contains_key(&reference.id),
            RecordKind::CatalogItem => self.catalog_items.contains_key(&reference.id),
        };

        if exists {
            Ok(())
        } else {
            Err(KernelError::NotFound {
                kind: "record",
                id: reference.id.to_string(),
            })
        }
    }
}

fn required(field: &str, value: &str) -> KernelResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(KernelError::Validation(format!("{field} cannot be empty")))
    } else {
        Ok(trimmed.to_string())
    }
}

fn optional_trimmed(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut normalized = tags
        .into_iter()
        .filter_map(|tag| {
            let trimmed = tag.trim().to_lowercase();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn validate_money(value: &Money) -> KernelResult<()> {
    if value.currency_code.trim().is_empty() {
        return Err(KernelError::Validation(
            "money.currency_code cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_confidence(confidence_bps: u16) -> KernelResult<()> {
    if confidence_bps > 10_000 {
        Err(KernelError::Validation(
            "confidence_bps must be between 0 and 10000".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn validate_field_definitions(fields: Vec<FieldDefinition>) -> KernelResult<Vec<FieldDefinition>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(fields.len());
    for field in fields {
        let key = required("field_definition.key", &field.key)?;
        if !seen.insert(key.clone()) {
            return Err(KernelError::Validation(format!(
                "duplicate field key: {key}"
            )));
        }
        normalized.push(FieldDefinition {
            id: field.id,
            key,
            label: required("field_definition.label", &field.label)?,
            field_type: field.field_type,
            required: field.required,
            options: field
                .options
                .into_iter()
                .filter_map(|option| {
                    let trimmed = option.trim().to_string();
                    (!trimmed.is_empty()).then_some(trimmed)
                })
                .collect(),
            relation_object_key: optional_trimmed(field.relation_object_key),
            active: field.active,
        });
    }
    Ok(normalized)
}

fn validate_relationship_definitions(
    relationships: Vec<RelationshipDefinition>,
) -> KernelResult<Vec<RelationshipDefinition>> {
    relationships
        .into_iter()
        .map(|relationship| {
            Ok(RelationshipDefinition {
                id: relationship.id,
                target_object_key: required(
                    "relationship_definition.target_object_key",
                    &relationship.target_object_key,
                )?,
                cardinality: match relationship.cardinality {
                    RelationshipCardinality::OneToOne => RelationshipCardinality::OneToOne,
                    RelationshipCardinality::OneToMany => RelationshipCardinality::OneToMany,
                    RelationshipCardinality::ManyToMany => RelationshipCardinality::ManyToMany,
                },
                label: required("relationship_definition.label", &relationship.label)?,
            })
        })
        .collect()
}

fn opportunity_related_to(opportunity: &Opportunity) -> Vec<RecordRef> {
    let mut related = vec![RecordRef {
        kind: RecordKind::Opportunity,
        id: opportunity.id,
    }];
    related.push(RecordRef {
        kind: RecordKind::Organization,
        id: opportunity.organization_id,
    });
    if let Some(contact_id) = opportunity.primary_contact_id {
        related.push(RecordRef {
            kind: RecordKind::Person,
            id: contact_id,
        });
    }
    related
}

fn related_with_org(record: RecordRef, organization_id: Option<Uuid>) -> Vec<RecordRef> {
    let mut related = vec![record];
    if let Some(organization_id) = organization_id {
        related.push(RecordRef {
            kind: RecordKind::Organization,
            id: organization_id,
        });
    }
    related
}
