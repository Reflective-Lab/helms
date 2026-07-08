//! Observable contract pins for upstream SSE/sequencing migration (QF-2026-06-26-01).
//!
//! coordination_test.rs covers gate flows; this file pins globally-monotonic
//! sequences when coordination shares a governed-jobs hub.

use std::sync::Arc;
use std::time::Duration;

use application_kernel::ActorKind;
use application_storage::{AppKernelStore, AppRuntimeStores, InMemoryKernelStore};
use async_trait::async_trait;
use converge_core::integrity::{ContentHash, IntegrityProof, MerkleRoot};
use converge_core::{
    ContextState, ConvergeResult, Criterion, CriterionId, CriterionOutcome, CriterionResult,
    StopReason,
};
use helm_coordination::{CoordinationService, PrincipalClaim};
use helm_governed_jobs::JobStreamState;
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionError, TruthExecutionModule,
    dispatcher::TruthExecutionContext,
};
use runway_app_host::{EventEnvelope, EventHub};

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
    ) -> Result<TruthExecutionArtifacts, TruthExecutionError> {
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
    let hub = EventHub::with_capacity(256);
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

fn claim(actor: &str) -> PrincipalClaim {
    PrincipalClaim {
        actor_id: Some(actor.to_string()),
        display_name: Some(actor.to_string()),
        kind: Some(ActorKind::Human),
        workspace_id: Some("ws-char".to_string()),
    }
}

async fn wait_for_event(
    rx: &mut tokio::sync::broadcast::Receiver<EventEnvelope>,
    expected_type: &str,
    deadline: Duration,
) -> EventEnvelope {
    let fut = async {
        loop {
            match rx.recv().await {
                Ok(env) if env.r#type == expected_type => return env,
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => panic!("channel closed"),
            }
        }
    };
    tokio::time::timeout(deadline, fut).await.expect("deadline")
}

#[tokio::test]
async fn shared_hub_interleaves_job_and_coordination_sequences() {
    let state = live_state();
    let service = CoordinationService::new(state.hub.clone(), "test.coordination")
        .with_job_state(state.clone());

    let mut rx = state.hub.subscribe();

    state.hub.publish(job_event("job.started"));
    let started = wait_for_event(&mut rx, "job.started", Duration::from_secs(5)).await;
    assert_eq!(started.sequence, 1);

    service
        .open_session(&claim("alice"))
        .expect("session opens");
    let session = wait_for_event(&mut rx, "session.opened", Duration::from_secs(5)).await;
    assert_eq!(session.sequence, 2);
    assert!(session.sequence > started.sequence);
}

fn job_event(ty: &str) -> EventEnvelope {
    EventEnvelope {
        event_id: uuid::Uuid::new_v4(),
        sequence: 0,
        r#type: ty.into(),
        schema_version: 1,
        occurred_at: chrono::Utc::now(),
        app_id: "test.governed-jobs".into(),
        run_id: Some("run-coord-seq".into()),
        job_id: Some("job-1".into()),
        correlation_id: None,
        actor: None,
        payload: serde_json::json!({ "workspace_id": "ws-char" }),
    }
}
