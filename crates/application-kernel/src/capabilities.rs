use uuid::Uuid;

use crate::{
    AccountSummary, Activity, ActivityAppend, Actor, CatalogItem, CatalogItemUpsert,
    CommunicationEvent, CommunicationRecord, CreditGrantApplication, CreditGrantApply, CrmKernel,
    Document, DocumentAttach, Entitlement, Fact, FactRecord, KernelResult, LedgerEntry, Note,
    NoteAppend, ObjectDefinition, ObjectDefinitionUpsert, Opportunity, OpportunityAdvance,
    OpportunityCreate, OrderSubscription, Organization, OrganizationUpsert, PermissionGrant,
    PermissionGrantInput, Person, PersonUpsert, RecordRef, Relationship, RelationshipLink,
    SubscriptionActivate, SubscriptionActivation, SubscriptionCreate, SubscriptionPlanChange,
    SubscriptionPlanChangeResult, SubscriptionSuspend, SubscriptionSuspension, TimelineEntry,
    ViewDefinition, ViewDefinitionUpsert, WorkflowCase, WorkflowCaseAdvance, WorkflowCaseCreate,
    WorkflowState,
};

pub trait PartiesCommands {
    fn upsert_organization(
        &mut self,
        command: OrganizationUpsert,
        actor: Actor,
    ) -> KernelResult<Organization>;
    fn upsert_person(&mut self, command: PersonUpsert, actor: Actor) -> KernelResult<Person>;
    fn link_relationship(
        &mut self,
        command: RelationshipLink,
        actor: Actor,
    ) -> KernelResult<Relationship>;
    fn get_account_summary(
        &self,
        organization_id: Uuid,
        timeline_limit: usize,
    ) -> KernelResult<AccountSummary>;
    fn list_organizations(&self) -> Vec<Organization>;
    fn list_people(&self, organization_id: Option<Uuid>) -> Vec<Person>;
}

pub trait OpportunitiesCommands {
    fn create_opportunity(
        &mut self,
        command: OpportunityCreate,
        actor: Actor,
    ) -> KernelResult<Opportunity>;
    fn advance_opportunity(
        &mut self,
        command: OpportunityAdvance,
        actor: Actor,
    ) -> KernelResult<Opportunity>;
    fn list_opportunities(&self, organization_id: Option<Uuid>) -> Vec<Opportunity>;
}

pub trait ConversationsCommands {
    fn append_activity(&mut self, command: ActivityAppend, actor: Actor) -> KernelResult<Activity>;
    fn record_communication(
        &mut self,
        command: CommunicationRecord,
        actor: Actor,
    ) -> KernelResult<CommunicationEvent>;
    fn list_timeline(&self, anchors: &[crate::RecordRef], limit: usize) -> Vec<TimelineEntry>;
}

pub trait DocumentsCommands {
    fn append_note(&mut self, command: NoteAppend, actor: Actor) -> KernelResult<Note>;
    fn attach_document(&mut self, command: DocumentAttach, actor: Actor) -> KernelResult<Document>;
}

pub trait WorkflowCommands {
    fn create_workflow_case(
        &mut self,
        command: WorkflowCaseCreate,
        actor: Actor,
    ) -> KernelResult<WorkflowCase>;
    fn advance_workflow_case(
        &mut self,
        command: WorkflowCaseAdvance,
        actor: Actor,
    ) -> KernelResult<WorkflowCase>;
    fn list_workflow_cases(&self, state: Option<WorkflowState>) -> Vec<WorkflowCase>;
}

pub trait IdentityCommands {
    fn grant_permission(
        &mut self,
        command: PermissionGrantInput,
        actor: Actor,
    ) -> KernelResult<PermissionGrant>;
}

pub trait FactsCommands {
    fn record_fact(&mut self, command: FactRecord, actor: Actor) -> KernelResult<Fact>;
    fn list_timeline(&self, anchors: &[RecordRef], limit: usize) -> Vec<TimelineEntry>;
}

pub trait RevenueCommands {
    fn upsert_catalog_item(
        &mut self,
        command: CatalogItemUpsert,
        actor: Actor,
    ) -> KernelResult<CatalogItem>;
    fn create_order_subscription(
        &mut self,
        command: SubscriptionCreate,
        actor: Actor,
    ) -> KernelResult<OrderSubscription>;
    fn activate_subscription(
        &mut self,
        command: SubscriptionActivate,
        actor: Actor,
    ) -> KernelResult<SubscriptionActivation>;
    fn suspend_subscription(
        &mut self,
        command: SubscriptionSuspend,
        actor: Actor,
    ) -> KernelResult<SubscriptionSuspension>;
    fn change_subscription_plan(
        &mut self,
        command: SubscriptionPlanChange,
        actor: Actor,
    ) -> KernelResult<SubscriptionPlanChangeResult>;
    fn apply_credit_grant(
        &mut self,
        command: CreditGrantApply,
        actor: Actor,
    ) -> KernelResult<CreditGrantApplication>;
    fn get_subscription(&self, id: Uuid) -> KernelResult<OrderSubscription>;
    fn list_subscriptions(&self, organization_id: Option<Uuid>) -> Vec<OrderSubscription>;
    fn list_catalog_items(&self, active_only: bool) -> Vec<CatalogItem>;
    fn list_entitlements(&self, organization_id: Option<Uuid>) -> Vec<Entitlement>;
    fn list_ledger_entries(&self, organization_id: Option<Uuid>) -> Vec<LedgerEntry>;
}

pub trait MetadataCommands {
    fn upsert_object_definition(
        &mut self,
        command: ObjectDefinitionUpsert,
        actor: Actor,
    ) -> KernelResult<ObjectDefinition>;
    fn upsert_view_definition(
        &mut self,
        command: ViewDefinitionUpsert,
        actor: Actor,
    ) -> KernelResult<ViewDefinition>;
    fn list_object_definitions(&self) -> Vec<ObjectDefinition>;
    fn list_view_definitions(&self, object_key: Option<&str>) -> Vec<ViewDefinition>;
}

impl PartiesCommands for CrmKernel {
    fn upsert_organization(
        &mut self,
        command: OrganizationUpsert,
        actor: Actor,
    ) -> KernelResult<Organization> {
        CrmKernel::upsert_organization(self, command, actor)
    }

    fn upsert_person(&mut self, command: PersonUpsert, actor: Actor) -> KernelResult<Person> {
        CrmKernel::upsert_person(self, command, actor)
    }

    fn link_relationship(
        &mut self,
        command: RelationshipLink,
        actor: Actor,
    ) -> KernelResult<Relationship> {
        CrmKernel::link_relationship(self, command, actor)
    }

    fn get_account_summary(
        &self,
        organization_id: Uuid,
        timeline_limit: usize,
    ) -> KernelResult<AccountSummary> {
        CrmKernel::get_account_summary(self, organization_id, timeline_limit)
    }

    fn list_organizations(&self) -> Vec<Organization> {
        CrmKernel::list_organizations(self)
    }

    fn list_people(&self, organization_id: Option<Uuid>) -> Vec<Person> {
        CrmKernel::list_people(self, organization_id)
    }
}

impl OpportunitiesCommands for CrmKernel {
    fn create_opportunity(
        &mut self,
        command: OpportunityCreate,
        actor: Actor,
    ) -> KernelResult<Opportunity> {
        CrmKernel::create_opportunity(self, command, actor)
    }

    fn advance_opportunity(
        &mut self,
        command: OpportunityAdvance,
        actor: Actor,
    ) -> KernelResult<Opportunity> {
        CrmKernel::advance_opportunity(self, command, actor)
    }

    fn list_opportunities(&self, organization_id: Option<Uuid>) -> Vec<Opportunity> {
        CrmKernel::list_opportunities(self, organization_id)
    }
}

impl ConversationsCommands for CrmKernel {
    fn append_activity(&mut self, command: ActivityAppend, actor: Actor) -> KernelResult<Activity> {
        CrmKernel::append_activity(self, command, actor)
    }

    fn record_communication(
        &mut self,
        command: CommunicationRecord,
        actor: Actor,
    ) -> KernelResult<CommunicationEvent> {
        CrmKernel::record_communication(self, command, actor)
    }

    fn list_timeline(&self, anchors: &[crate::RecordRef], limit: usize) -> Vec<TimelineEntry> {
        CrmKernel::list_timeline(self, anchors, limit)
    }
}

impl DocumentsCommands for CrmKernel {
    fn append_note(&mut self, command: NoteAppend, actor: Actor) -> KernelResult<Note> {
        CrmKernel::append_note(self, command, actor)
    }

    fn attach_document(&mut self, command: DocumentAttach, actor: Actor) -> KernelResult<Document> {
        CrmKernel::attach_document(self, command, actor)
    }
}

impl WorkflowCommands for CrmKernel {
    fn create_workflow_case(
        &mut self,
        command: WorkflowCaseCreate,
        actor: Actor,
    ) -> KernelResult<WorkflowCase> {
        CrmKernel::create_workflow_case(self, command, actor)
    }

    fn advance_workflow_case(
        &mut self,
        command: WorkflowCaseAdvance,
        actor: Actor,
    ) -> KernelResult<WorkflowCase> {
        CrmKernel::advance_workflow_case(self, command, actor)
    }

    fn list_workflow_cases(&self, state: Option<WorkflowState>) -> Vec<WorkflowCase> {
        CrmKernel::list_workflow_cases(self, state)
    }
}

impl IdentityCommands for CrmKernel {
    fn grant_permission(
        &mut self,
        command: PermissionGrantInput,
        actor: Actor,
    ) -> KernelResult<PermissionGrant> {
        CrmKernel::grant_permission(self, command, actor)
    }
}

impl FactsCommands for CrmKernel {
    fn record_fact(&mut self, command: FactRecord, actor: Actor) -> KernelResult<Fact> {
        CrmKernel::record_fact(self, command, actor)
    }

    fn list_timeline(&self, anchors: &[RecordRef], limit: usize) -> Vec<TimelineEntry> {
        CrmKernel::list_timeline(self, anchors, limit)
    }
}

impl RevenueCommands for CrmKernel {
    fn upsert_catalog_item(
        &mut self,
        command: CatalogItemUpsert,
        actor: Actor,
    ) -> KernelResult<CatalogItem> {
        CrmKernel::upsert_catalog_item(self, command, actor)
    }

    fn create_order_subscription(
        &mut self,
        command: SubscriptionCreate,
        actor: Actor,
    ) -> KernelResult<OrderSubscription> {
        CrmKernel::create_order_subscription(self, command, actor)
    }

    fn activate_subscription(
        &mut self,
        command: SubscriptionActivate,
        actor: Actor,
    ) -> KernelResult<SubscriptionActivation> {
        CrmKernel::activate_subscription(self, command, actor)
    }

    fn suspend_subscription(
        &mut self,
        command: SubscriptionSuspend,
        actor: Actor,
    ) -> KernelResult<SubscriptionSuspension> {
        CrmKernel::suspend_subscription(self, command, actor)
    }

    fn change_subscription_plan(
        &mut self,
        command: SubscriptionPlanChange,
        actor: Actor,
    ) -> KernelResult<SubscriptionPlanChangeResult> {
        CrmKernel::change_subscription_plan(self, command, actor)
    }

    fn apply_credit_grant(
        &mut self,
        command: CreditGrantApply,
        actor: Actor,
    ) -> KernelResult<CreditGrantApplication> {
        CrmKernel::apply_credit_grant(self, command, actor)
    }

    fn get_subscription(&self, id: Uuid) -> KernelResult<OrderSubscription> {
        CrmKernel::get_subscription(self, id)
    }

    fn list_subscriptions(&self, organization_id: Option<Uuid>) -> Vec<OrderSubscription> {
        CrmKernel::list_subscriptions(self, organization_id)
    }

    fn list_catalog_items(&self, active_only: bool) -> Vec<CatalogItem> {
        CrmKernel::list_catalog_items(self, active_only)
    }

    fn list_entitlements(&self, organization_id: Option<Uuid>) -> Vec<Entitlement> {
        CrmKernel::list_entitlements(self, organization_id)
    }

    fn list_ledger_entries(&self, organization_id: Option<Uuid>) -> Vec<LedgerEntry> {
        CrmKernel::list_ledger_entries(self, organization_id)
    }
}

impl MetadataCommands for CrmKernel {
    fn upsert_object_definition(
        &mut self,
        command: ObjectDefinitionUpsert,
        actor: Actor,
    ) -> KernelResult<ObjectDefinition> {
        CrmKernel::upsert_object_definition(self, command, actor)
    }

    fn upsert_view_definition(
        &mut self,
        command: ViewDefinitionUpsert,
        actor: Actor,
    ) -> KernelResult<ViewDefinition> {
        CrmKernel::upsert_view_definition(self, command, actor)
    }

    fn list_object_definitions(&self) -> Vec<ObjectDefinition> {
        CrmKernel::list_object_definitions(self)
    }

    fn list_view_definitions(&self, object_key: Option<&str>) -> Vec<ViewDefinition> {
        CrmKernel::list_view_definitions(self, object_key)
    }
}
