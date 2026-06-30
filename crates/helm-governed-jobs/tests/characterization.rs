//! Observable contract pins for upstream SSE/sequencing migration (QF-2026-06-26-01).
//!
//! gate_test.rs covers HITL flows; this file pins monotonic sequence stamping at
//! the helm-governed-jobs boundary via the job run publisher.

use std::sync::Arc;
use std::time::Duration;

use application_storage::{AppKernelStore, AppRuntimeStores, InMemoryKernelStore};
use async_trait::async_trait;
use converge_core::integrity::{ContentHash, IntegrityProof, MerkleRoot};
use converge_core::{
    ContextState, ConvergeResult, Criterion, CriterionId, CriterionOutcome, CriterionResult,
    StopReason,
};
use helm_governed_jobs::{JobRunTask, JobStreamState, run_job_task};
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionModule, dispatcher::TruthExecutionContext,
};
use runway_app_host::EventEnvelope;

const TRUTH_KEY: &str = "score-inbound-fit";

struct ImmediateTruth;

#[async_trait]
impl TruthBody for ImmediateTruth {
    fn key(&self) -> &'static str {
        TRUTH_KEY
    }

    async fn execute(
        &self,
        _ctx: TruthExecutionContext,
    ) -> Result<TruthExecutionArtifacts, tonic::Status> {
        Ok(TruthExecutionArtifacts {
            result: ConvergeResult {
                context: ContextState::default(),
                cycles: 1,
                converged: true,
                stop_reason: StopReason::Converged,
                criteria_outcomes: vec![CriterionOutcome {
                    criterion: Criterion::required(CriterionId::new("ok"), "ok"),
                    result: CriterionResult::Met { evidence: vec![] },
                }],
                integrity: IntegrityProof {
                    merkle_root: MerkleRoot(ContentHash([0u8; 32])),
                    clock_time: 0,
                    fact_count: 0,
                },
            },
            experience_events: vec![],
            projection: None,
            runtime_scope_id: "test-scope".into(),
        })
    }
}

fn live_state() -> Arc<JobStreamState> {
    let hub = runway_app_host::EventHub::with_capacity(128);
    let truths = Arc::new(TruthExecutionModule::new().register(Arc::new(ImmediateTruth)));
    Arc::new(JobStreamState {
        store: AppKernelStore::Memory(InMemoryKernelStore::default_local()),
        runtime_stores: AppRuntimeStores::default(),
        truths,
        hub: hub.handle(),
        app_id: "test.governed-jobs".into(),
        gate_timeout: Duration::from_secs(30),
        ..JobStreamState::default()
    })
}

async fn wait_for_event(
    rx: &mut tokio::sync::broadcast::Receiver<EventEnvelope>,
    expected_type: &str,
) -> EventEnvelope {
    loop {
        match rx.recv().await {
            Ok(env) if env.r#type == expected_type => return env,
            Ok(_) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => panic!("channel closed waiting for {expected_type}"),
        }
    }
}

#[tokio::test]
async fn job_run_events_are_monotonic_from_one() {
    let state = live_state();
    let mut rx = state.hub.subscribe();

    let state_clone = state.clone();
    let job = tokio::spawn(async move {
        run_job_task(JobRunTask {
            state: state_clone,
            run_id: "run-seq".into(),
            truth_key: TRUTH_KEY.into(),
            app_id: "test.governed-jobs".into(),
            inputs: Default::default(),
            initiator: None,
        })
        .await;
    });

    let started = wait_for_event(&mut rx, "job.started").await;
    let completed = wait_for_event(&mut rx, "job.completed").await;
    assert_eq!(started.sequence, 1);
    assert!(completed.sequence > started.sequence);

    tokio::time::timeout(Duration::from_secs(5), job)
        .await
        .expect("job completes")
        .expect("job task ok");
}
