use capability_core::ModuleSuite;
use capability_registry::find_module;
use truth_catalog::{TruthModuleTouch, resolve::{PackResolver, UnknownModule}};

const FOUNDATION_PACK_ID: &str = "prio-foundation-pack";
const RELATIONSHIP_PACK_ID: &str = "prio-relationship-pack";
const COMMERCIAL_PACK_ID: &str = "prio-commercial-pack";
const REVENUE_PACK_ID: &str = "prio-revenue-pack";
const WORK_PACK_ID: &str = "prio-work-pack";
const TRUST_PACK_ID: &str = "trust";
const KNOWLEDGE_PACK_ID: &str = "knowledge";

pub struct CrmPackResolver;

impl PackResolver for CrmPackResolver {
    fn pack_ids_for(&self, modules: &[TruthModuleTouch]) -> Result<Vec<&'static str>, UnknownModule> {
        let mut pack_ids: Vec<&'static str> = Vec::new();
        for touch in modules {
            let module = find_module(touch.module_key).ok_or_else(|| UnknownModule {
                truth_key: String::new(),
                module_key: touch.module_key.to_owned(),
            })?;
            let pack_id = match module.suite {
                ModuleSuite::Foundation => FOUNDATION_PACK_ID,
                ModuleSuite::RelationshipCore => RELATIONSHIP_PACK_ID,
                ModuleSuite::CommercialCore => COMMERCIAL_PACK_ID,
                ModuleSuite::UsageRevenueCore => REVENUE_PACK_ID,
                ModuleSuite::WorkCore => WORK_PACK_ID,
                ModuleSuite::TrustCore => TRUST_PACK_ID,
                ModuleSuite::IntelligenceCore => KNOWLEDGE_PACK_ID,
            };
            if !pack_ids.contains(&pack_id) {
                pack_ids.push(pack_id);
            }
        }
        Ok(pack_ids)
    }
}
