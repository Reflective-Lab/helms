use prio_agent_ops::MODULE as AGENT_OPS;
use std::collections::BTreeMap;

use capability_core::{CapabilityModule, ModuleSuite};
use prio_approvals::MODULE as APPROVALS;
use prio_audit::MODULE as AUDIT;
use prio_catalog::MODULE as CATALOG;
use prio_conversations::MODULE as CONVERSATIONS;
use prio_documents::MODULE as DOCUMENTS;
use prio_entitlements::MODULE as ENTITLEMENTS;
use prio_expenses::MODULE as EXPENSES;
use prio_facts::MODULE as FACTS;
use prio_identity::MODULE as IDENTITY;
use prio_intents::MODULE as INTENTS;
use prio_ledger::MODULE as LEDGER;
use prio_memory::MODULE as MEMORY;
use prio_metering::MODULE as METERING;
use prio_opportunities::MODULE as OPPORTUNITIES;
use prio_parties::MODULE as PARTIES;
use prio_payments::MODULE as PAYMENTS;
use prio_policies::MODULE as POLICIES;
use prio_subscriptions::MODULE as SUBSCRIPTIONS;
use prio_tasks::MODULE as TASKS;
use prio_workflow::MODULE as WORKFLOW;

pub const MODULES: &[CapabilityModule] = &[
    CATALOG,
    IDENTITY,
    PARTIES,
    CONVERSATIONS,
    OPPORTUNITIES,
    TASKS,
    DOCUMENTS,
    EXPENSES,
    SUBSCRIPTIONS,
    METERING,
    LEDGER,
    ENTITLEMENTS,
    PAYMENTS,
    WORKFLOW,
    APPROVALS,
    POLICIES,
    FACTS,
    AUDIT,
    INTENTS,
    MEMORY,
    AGENT_OPS,
];

#[must_use]
pub fn find_module(key: &str) -> Option<CapabilityModule> {
    MODULES.iter().copied().find(|module| module.key == key)
}

#[must_use]
pub fn all_modules() -> Vec<CapabilityModule> {
    MODULES.to_vec()
}

#[must_use]
pub fn modules_by_suite() -> BTreeMap<ModuleSuite, Vec<CapabilityModule>> {
    let mut grouped = BTreeMap::<ModuleSuite, Vec<CapabilityModule>>::new();
    for module in MODULES {
        grouped.entry(module.suite).or_default().push(*module);
    }
    grouped
}
