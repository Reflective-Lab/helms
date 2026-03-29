use serde::{Deserialize, Serialize};

use crate::model::{
    Activity, Actor, AuditEntry, CatalogItem, CommunicationEvent, Document, Entitlement, Fact,
    LedgerEntry, Note, ObjectDefinition, Opportunity, OpportunityStage, OrderSubscription,
    Organization, PermissionGrant, Person, Relationship, SubscriptionStatus, TimelineEntry,
    ViewDefinition, WorkflowCase, WorkflowState,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DomainEvent {
    OrganizationUpserted {
        organization: Organization,
        actor: Actor,
    },
    PersonUpserted {
        person: Person,
        actor: Actor,
    },
    RelationshipLinked {
        relationship: Relationship,
        actor: Actor,
    },
    OpportunityCreated {
        opportunity: Opportunity,
        actor: Actor,
    },
    OpportunityStageChanged {
        opportunity: Opportunity,
        previous_stage: OpportunityStage,
        actor: Actor,
    },
    ActivityAppended {
        activity: Activity,
        actor: Actor,
    },
    NoteAppended {
        note: Note,
        actor: Actor,
    },
    DocumentAttached {
        document: Document,
        actor: Actor,
    },
    CommunicationRecorded {
        event: CommunicationEvent,
        actor: Actor,
    },
    WorkflowCaseCreated {
        workflow_case: WorkflowCase,
        actor: Actor,
    },
    WorkflowCaseStateChanged {
        workflow_case: WorkflowCase,
        previous_state: WorkflowState,
        actor: Actor,
    },
    PermissionGranted {
        grant: PermissionGrant,
        actor: Actor,
    },
    CatalogItemUpserted {
        catalog_item: CatalogItem,
        actor: Actor,
    },
    OrderSubscriptionCreated {
        subscription: OrderSubscription,
        actor: Actor,
    },
    OrderSubscriptionStateChanged {
        subscription: OrderSubscription,
        previous_status: SubscriptionStatus,
        actor: Actor,
    },
    OrderSubscriptionPlanChanged {
        subscription: OrderSubscription,
        previous_catalog_item_id: Option<uuid::Uuid>,
        previous_value: crate::Money,
        actor: Actor,
    },
    EntitlementsGranted {
        entitlements: Vec<Entitlement>,
        actor: Actor,
    },
    EntitlementsReplaced {
        subscription_id: uuid::Uuid,
        entitlements: Vec<Entitlement>,
        actor: Actor,
    },
    EntitlementAdjusted {
        entitlement: Entitlement,
        actor: Actor,
    },
    LedgerEntryAppended {
        entry: LedgerEntry,
        actor: Actor,
    },
    FactRecorded {
        fact: Fact,
        actor: Actor,
    },
    ObjectDefinitionUpserted {
        definition: ObjectDefinition,
        actor: Actor,
    },
    ViewDefinitionUpserted {
        view: ViewDefinition,
        actor: Actor,
    },
    AuditRecorded {
        entry: AuditEntry,
    },
    TimelineEntryRecorded {
        entry: TimelineEntry,
    },
}
