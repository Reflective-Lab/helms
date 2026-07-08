//! `PackResolver` and `IntentOverlay` — inversion boundary for content-side
//! capability bindings (Seam B T3 keystone, RFL-172).
//!
//! # Rationale
//!
//! `truth-catalog` is the *mechanism* crate: it owns `TruthDefinition`,
//! `TruthCatalog`, `TruthConvergeBinding`, and the admission machinery.
//! Historically the mechanism reached directly into
//! `capability_registry::find_module` (to map module suites to pack IDs) and
//! per-truth overlay tables (to apply `context`/`constraints`/`authority` to
//! compiled `IntentPacket`s), coupling the mechanism to CRM content.
//!
//! `PackResolver` and `IntentOverlay` invert this dependency: the mechanism
//! receives behaviour from the content side via trait objects at call sites,
//! rather than importing content crates at all.  The content-side
//! implementations (`CrmPackResolver`, `CrmIntentOverlay`) will live in
//! `crm-truths` once T4 completes the move.  Until then, legacy shims
//! (`LegacyResolver`, `LegacyOverlay`) keep `cargo check --workspace` green
//! while the new path exists; they are the only place in the mechanism where
//! `capability_registry` and `capability_core` are imported.

use organism_pack::IntentPacket;

use crate::{TruthDefinition, TruthModuleTouch};

/// Error produced when a [`PackResolver`] encounters a module key that is not
/// present in the capability registry.
///
/// The `Display` output matches the former `panic!` at `converge.rs:606`:
///
/// > `truth '{truth_key}' references unknown module '{module_key}'`
///
/// The `truth_key` field is empty when the error is returned from
/// [`PackResolver::pack_ids_for`] (the resolver does not know which truth is
/// being built); it is filled in by [`crate::converge::TruthConvergeBinding::build`].
#[derive(Debug, thiserror::Error)]
#[error("truth '{truth_key}' references unknown module '{module_key}'")]
pub struct UnknownModule {
    /// The truth that references the unknown module.  Filled in by the
    /// calling `build()` context; may be empty when the error comes directly
    /// from a resolver.
    pub truth_key: String,
    /// The module key that could not be resolved.
    pub module_key: String,
}

/// Resolves a set of [`TruthModuleTouch`] entries to their Converge pack IDs.
///
/// Implement this trait on the content side (e.g. `CrmPackResolver` in
/// `crm-truths`) to supply the module → pack mapping without importing
/// capability crates into the mechanism.
///
/// # Contract
///
/// * The returned `Vec` is deduped and preserves insertion order.
/// * Every `module_key` in `modules` must be resolvable; return
///   [`UnknownModule`] on the first unresolvable key.
/// * The `truth_key` field of the returned error should be left empty; the
///   caller (`build()`) fills it in from the [`TruthDefinition`].
pub trait PackResolver {
    /// Return the deduped, ordered pack IDs for the given module touches.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownModule`] when any `module_key` in `modules` cannot be
    /// resolved.
    fn pack_ids_for(
        &self,
        modules: &[TruthModuleTouch],
    ) -> Result<Vec<&'static str>, UnknownModule>;
}

/// Applies content-side overlay fields to an [`IntentPacket`] that has been
/// compiled from a truth's `.feature` source.
///
/// Implement on the content side (e.g. `CrmIntentOverlay` in `crm-truths`) to
/// supply per-truth `context`, `constraints`, `authority`, and `expires`
/// overrides without encoding CRM specifics in the mechanism.
///
/// # Safety
///
/// The `Send + Sync` bound is required because overlay instances are shared
/// across async boundaries in the Helms runtime.
pub trait IntentOverlay: Send + Sync {
    /// Mutate `intent` in-place with content-specific fields for `def`.
    fn apply(&self, def: &TruthDefinition, intent: &mut IntentPacket);
}
