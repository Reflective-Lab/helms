//! CRM truth content over the truth-catalog mechanism — app-side forever, never foundation.
//!
//! This crate owns the CRM-specific truth definitions and binds them to the
//! mechanism crate (`truth-catalog`). It is intentionally placed at the
//! application layer and must never be promoted to a foundation dependency.
//!
//! The mechanism/content split is:
//! - `truth-catalog` — generic mechanism: `TruthKey`, `TruthCatalog`, `TruthDefinition`, etc.
//! - `crm-truths` (this crate) — CRM content: the `TRUTHS` const, pack resolver, evaluators,
//!   integrity and snapshot tests, and all `.feature` files.
