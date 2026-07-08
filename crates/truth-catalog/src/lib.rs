//! # truth-catalog — Mechanism Seam (RFL-172)
//!
//! `truth-catalog` is the *mechanism* crate for Helms' executable truth layer.
//! It owns the structural types and runtime machinery that govern how truths
//! are defined, catalogued, compiled to organism `IntentPacket`s, and
//! admitted into the Converge reasoning kernel.
//!
//! ## Mechanism / content inversion (Seam B)
//!
//! Historically, truth definitions (the CRM `TRUTHS` const, per-truth
//! evaluators, overlay tables, and the capability-registry-based pack resolver)
//! lived alongside the mechanism in a single crate.  RFL-172 inverted this:
//!
//! | Layer | Crate | Contains |
//! |-------|-------|---------|
//! | **Mechanism** | `truth-catalog` *(this crate)* | `TruthDefinition`, `TruthCatalog`, `TruthKey`, `TruthConvergeBinding`, `PackResolver`, `IntentOverlay`, admission, orchestration |
//! | **Content** | `crm-truths` | `TRUTHS` const, 24 `.feature` files, 13 CRM evaluators, `CrmPackResolver`, `CrmIntentOverlay` |
//!
//! The inversion means `truth-catalog` carries **zero** `capability_registry` /
//! `capability_core` imports — the content side injects those via traits at
//! call sites.
//!
//! ## Core types
//!
//! - [`TruthDefinition`] — the static descriptor for a single executable truth
//!   (key, kind, summary, modules, gherkin source).
//! - [`TruthCatalog`] — a borrowing view over a `&[TruthDefinition]` slice with
//!   typed query methods (`find`, `by_kind`, `for_module`).  Construct with
//!   `TruthCatalog::new(crm_truths::TRUTHS)` for the CRM catalog, or over a
//!   synthetic slice in tests.
//! - [`TruthKey`] — a validated, kebab-case newtype for truth identifiers.
//!   Enforces parse-don't-validate at the runtime string crossing (HTTP keys,
//!   CLI args).  Construct via [`TruthKey::parse`] or the [`FromStr`] impl.
//! - [`TruthConvergeBinding`] — a truth mapped to Converge's typed model
//!   (intent, pack IDs, approval points).  Built via
//!   [`TruthConvergeBinding::build`] with an injected [`PackResolver`].
//!
//! ## Injection points
//!
//! Content-side behaviour enters the mechanism via two traits:
//!
//! - [`resolve::PackResolver`] — maps `TruthModuleTouch` entries to Converge
//!   pack IDs without importing capability crates.
//! - [`resolve::IntentOverlay`] — applies per-truth `context`, `constraints`,
//!   `authority`, and `expires` overrides to compiled `IntentPacket`s.
//!
//! Mounting binaries inject `crm_truths::CrmPackResolver` and
//! `crm_truths::CrmIntentOverlay` at construction time.
//!
//! ## Lineage
//!
//! - RFL-171 (Seam A): `helm-module-contracts` split extracted the
//!   `HelmModule` / `HelmModuleState` contract boundary.
//! - RFL-172 (Seam B, this crate): mechanism/content split; `crm-truths`
//!   created as the content implementor.  Task sequence:
//!   T1 scaffold → T2 TruthKey+TruthCatalog → T3 PackResolver+IntentOverlay →
//!   T4 content move → T5 governed-jobs injection → T6 workbench repoint →
//!   T7 quality suite → T8 docs.
//!
//! [`FromStr`]: std::str::FromStr

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
