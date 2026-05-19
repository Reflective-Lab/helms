// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT
// See LICENSE file in the project root for full license information.

//! Helm application plugin runtime.
//!
//! This crate owns sandboxed WASM guest execution for Helm application plugins.
//! Converge remains the typed convergence kernel; Helm owns plugin lifecycle,
//! module verification, quotas, and the adapter that projects plugin outputs
//! into Converge proposals or invariants.

#![forbid(unsafe_code)]

pub mod adapter;
pub mod contract;
pub mod engine;
pub mod host;
pub mod integration;
pub mod signing;
pub mod store;
