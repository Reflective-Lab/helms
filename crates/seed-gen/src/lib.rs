//! seed-gen library target.
//!
//! Exposes the Parquet seed-loading utilities extracted from
//! `helm-operator-control` (RFL-154 T5b) so they can be consumed by a
//! mounting app without depending on the `seed-gen` binary.
//!
//! See [`showcase_seed`] for usage notes and the known column-name mismatch.

pub mod showcase_seed;
