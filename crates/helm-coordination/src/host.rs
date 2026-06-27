// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! App-host wiring — mount coordination alongside governed-jobs on one hub.

use std::sync::Arc;

use helm_governed_jobs::{GovernedJobsModule, JobStreamState};

use crate::{CoordinationModule, CoordinationService};

/// Build live [`GovernedJobsModule`] and [`CoordinationModule`] instances that
/// share `state`'s hub (and its monotonic sequence stream).
#[must_use]
pub fn mount_live_modules(
    state: Arc<JobStreamState>,
    coordination_app_id: impl Into<String>,
) -> (Arc<GovernedJobsModule>, Arc<CoordinationModule>) {
    let jobs = Arc::new(GovernedJobsModule::with_shared_state(state.clone()));
    let coordination = Arc::new(CoordinationModule::new(Arc::new(
        CoordinationService::new(state.hub.clone(), coordination_app_id).with_job_state(state),
    )));
    (jobs, coordination)
}
