pub mod admission;
pub mod catalog;
mod converge;
pub mod intent_compile;
pub mod key;
pub mod orchestration;
mod organism;
pub mod resolve;

pub use catalog::TruthCatalog;
pub use converge::{TruthConvergeBinding, to_converge_truth};
pub use key::{InvalidTruthKey, TruthKey};
pub use organism::TruthOrganismBinding;
pub use resolve::{IntentOverlay, PackResolver, UnknownModule};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TruthKind {
    Job,
    Policy,
    ModuleLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TruthModuleTouch {
    pub module_key: &'static str,
    pub responsibility: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TruthDefinition {
    pub key: &'static str,
    pub display_name: &'static str,
    pub kind: TruthKind,
    pub summary: &'static str,
    pub feature_path: &'static str,
    pub actor_roles: &'static [&'static str],
    pub approval_points: &'static [&'static str],
    pub desired_outcomes: &'static [&'static str],
    pub guardrails: &'static [&'static str],
    pub modules: &'static [TruthModuleTouch],
    pub gherkin: &'static str,
}
