mod capabilities;
mod error;
mod events;
mod kernel;
mod model;

pub use capabilities::{
    ConversationsCommands, DocumentsCommands, FactsCommands, IdentityCommands, MetadataCommands,
    OpportunitiesCommands, PartiesCommands, WorkflowCommands,
};
pub use error::{KernelError, KernelResult};
pub use events::DomainEvent;
pub use kernel::{
    ActivityAppend, CommunicationRecord, CrmKernel, DocumentAttach, FactRecord, NoteAppend,
    ObjectDefinitionUpsert, OpportunityAdvance, OpportunityCreate, OrganizationUpsert,
    PermissionGrantInput, PersonUpsert, RelationshipLink, ViewDefinitionUpsert,
    WorkflowCaseAdvance, WorkflowCaseCreate,
};
pub use model::*;

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::{
        ActivityAppend, ActivityOutcome, Actor, ActorKind, CrmKernel, DomainEvent, FactRecord,
        Money, ObjectDefinitionKind, ObjectDefinitionUpsert, OpportunityCreate,
        OrganizationLifecycle, OrganizationUpsert, PersonUpsert, RecordKind, RecordRef,
        ViewDefinitionUpsert, ViewLayout,
    };

    fn human() -> Actor {
        Actor {
            actor_id: "user-1".to_string(),
            display_name: "Kenneth".to_string(),
            kind: ActorKind::Human,
        }
    }

    #[test]
    fn fact_promotion_marks_source_note() {
        let actor = human();
        let mut kernel = CrmKernel::default();
        let organization = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: None,
                    name: "Aprio".to_string(),
                    external_key: None,
                    website: None,
                    industry: None,
                    lifecycle: OrganizationLifecycle::Prospect,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("organization should be created");

        let note = kernel
            .append_note(
                crate::NoteAppend {
                    subject: "Buying signal".to_string(),
                    body: "Champion asked for implementation timing.".to_string(),
                    related_to: vec![RecordRef {
                        kind: RecordKind::Organization,
                        id: organization.id,
                    }],
                },
                actor.clone(),
            )
            .expect("note should be created");

        kernel
            .record_fact(
                FactRecord {
                    statement: "The buyer is actively evaluating implementation timing."
                        .to_string(),
                    confidence_bps: 8_500,
                    related_to: vec![RecordRef {
                        kind: RecordKind::Organization,
                        id: organization.id,
                    }],
                    source_note_id: Some(note.id),
                },
                actor,
            )
            .expect("fact should be recorded");

        assert!(
            kernel
                .notes
                .get(&note.id)
                .expect("note exists")
                .promoted_to_fact
        );
    }

    #[test]
    fn account_summary_collects_related_records() {
        let actor = human();
        let mut kernel = CrmKernel::default();
        let organization = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: None,
                    name: "Northwind".to_string(),
                    external_key: None,
                    website: None,
                    industry: Some("software".to_string()),
                    lifecycle: OrganizationLifecycle::Active,
                    owner_user_id: Some("owner-1".to_string()),
                    tags: vec!["priority".to_string()],
                },
                actor.clone(),
            )
            .expect("organization should be created");

        let person = kernel
            .upsert_person(
                PersonUpsert {
                    person_id: None,
                    organization_id: Some(organization.id),
                    full_name: "Alice Doe".to_string(),
                    title: Some("CTO".to_string()),
                    email: None,
                    phone: None,
                    linkedin_url: None,
                },
                actor.clone(),
            )
            .expect("person should be created");

        let opportunity = kernel
            .create_opportunity(
                OpportunityCreate {
                    organization_id: organization.id,
                    primary_contact_id: Some(person.id),
                    name: "Expansion deal".to_string(),
                    value: Money {
                        currency_code: "USD".to_string(),
                        amount_minor: 120_000_00,
                    },
                    confidence_bps: 6_000,
                    next_step: Some("Send architecture note".to_string()),
                    expected_close_at: None,
                },
                actor.clone(),
            )
            .expect("opportunity should be created");

        kernel
            .append_activity(
                ActivityAppend {
                    subject: "Discovery call".to_string(),
                    details: "Validated JTBD and approval path.".to_string(),
                    related_to: vec![
                        RecordRef {
                            kind: RecordKind::Organization,
                            id: organization.id,
                        },
                        RecordRef {
                            kind: RecordKind::Opportunity,
                            id: opportunity.id,
                        },
                    ],
                    outcome: ActivityOutcome::Completed,
                    occurred_at: None,
                    next_action_due_at: None,
                },
                actor,
            )
            .expect("activity should be created");

        let summary = kernel
            .get_account_summary(organization.id, 10)
            .expect("summary should resolve");

        assert_eq!(summary.organization.name, "Northwind");
        assert_eq!(summary.contacts.len(), 1);
        assert_eq!(summary.opportunities.len(), 1);
        assert!(!summary.recent_timeline.is_empty());
    }

    #[test]
    fn list_people_filters_by_account() {
        let actor = human();
        let mut kernel = CrmKernel::default();
        let first = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: Some(Uuid::new_v4()),
                    name: "First".to_string(),
                    external_key: None,
                    website: None,
                    industry: None,
                    lifecycle: OrganizationLifecycle::Prospect,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("first org");
        let second = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: Some(Uuid::new_v4()),
                    name: "Second".to_string(),
                    external_key: None,
                    website: None,
                    industry: None,
                    lifecycle: OrganizationLifecycle::Prospect,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("second org");

        kernel
            .upsert_person(
                PersonUpsert {
                    person_id: None,
                    organization_id: Some(first.id),
                    full_name: "Alice".to_string(),
                    title: None,
                    email: None,
                    phone: None,
                    linkedin_url: None,
                },
                actor.clone(),
            )
            .expect("alice");

        kernel
            .upsert_person(
                PersonUpsert {
                    person_id: None,
                    organization_id: Some(second.id),
                    full_name: "Bob".to_string(),
                    title: None,
                    email: None,
                    phone: None,
                    linkedin_url: None,
                },
                actor,
            )
            .expect("bob");

        assert_eq!(kernel.list_people(Some(first.id)).len(), 1);
    }

    #[test]
    fn metadata_requires_object_before_view() {
        let actor = human();
        let mut kernel = CrmKernel::default();

        kernel
            .upsert_object_definition(
                ObjectDefinitionUpsert {
                    object_definition_id: None,
                    key: "usage_event".to_string(),
                    display_name: "Usage Event".to_string(),
                    kind: ObjectDefinitionKind::Custom,
                    fields: vec![],
                    relationships: vec![],
                    active: true,
                },
                actor.clone(),
            )
            .expect("object definition");

        kernel
            .upsert_view_definition(
                ViewDefinitionUpsert {
                    view_definition_id: None,
                    object_key: "usage_event".to_string(),
                    name: "High intent visits".to_string(),
                    layout: ViewLayout::Table,
                    filter_expression: Some("path startsWith /pricing".to_string()),
                    sort_expression: None,
                    visible_fields: vec!["session_id".to_string(), "path".to_string()],
                    group_by: None,
                    favorite: true,
                    owner_user_id: None,
                },
                actor,
            )
            .expect("view definition");

        assert_eq!(kernel.list_object_definitions().len(), 1);
        assert_eq!(kernel.list_view_definitions(Some("usage_event")).len(), 1);
    }

    #[test]
    fn organization_upsert_emits_domain_audit_and_timeline_events() {
        let actor = human();
        let mut kernel = CrmKernel::default();

        let organization = kernel
            .upsert_organization(
                OrganizationUpsert {
                    organization_id: None,
                    name: "Converge".to_string(),
                    external_key: None,
                    website: Some("https://converge.zone".to_string()),
                    industry: None,
                    lifecycle: OrganizationLifecycle::Prospect,
                    owner_user_id: None,
                    tags: vec![],
                },
                actor.clone(),
            )
            .expect("organization should be created");

        let events = kernel.drain_events();
        assert_eq!(events.len(), 3);
        assert!(matches!(
            &events[0],
            DomainEvent::OrganizationUpserted {
                organization: event_org,
                actor: event_actor
            } if event_org.id == organization.id && event_actor.actor_id == actor.actor_id
        ));
        assert!(matches!(&events[1], DomainEvent::AuditRecorded { .. }));
        assert!(matches!(
            &events[2],
            DomainEvent::TimelineEntryRecorded { .. }
        ));
    }
}
