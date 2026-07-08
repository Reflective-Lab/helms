//! HITL gate path tests for helm-governed-jobs.
//!
//! Phase 4b preserved the reject + timeout flows from the original
//! application-server RealtimeHub implementation. These tests prove the
//! preservation by exercising both paths against the new EventHubHandle wiring.
//!
//! # How the job is driven
//!
//! `run_job_task` is the internal async function that the HTTP route spawns.
//! It is exposed as `pub` so integration tests can call it directly without
//! going through the HTTP layer (which adds catalog validation noise).
//!
//! # Gate-requiring truth body
//!
//! `GateRequiringTruth` returns a `TruthExecutionArtifacts` whose first (and
//! only) criterion has `CriterionResult::Blocked { approval_ref: Some("gate-ref") }`.
//! That causes `blocked_gate()` in `job_stream.rs` to detect the gate and park
//! the job task on a `oneshot` waiter.

use std::sync::Arc;
use std::time::Duration;

use application_storage::{AppKernelStore, AppRuntimeStores, InMemoryKernelStore};
use async_trait::async_trait;
use converge_core::integrity::{ContentHash, IntegrityProof, MerkleRoot};
use converge_core::{
    ApprovalPointId, ContextState, ConvergeResult, Criterion, CriterionId, CriterionOutcome,
    CriterionResult, StopReason,
};
use helm_governed_jobs::{GateDecision, JobRunTask, JobStreamState, run_job_task};
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionError, TruthExecutionModule,
    dispatcher::TruthExecutionContext,
};
use runway_app_host::{EventEnvelope, EventHub};
use truth_catalog::{TruthCatalog, TruthDefinition, TruthKind};

// ── Stub truth body ────────────────────────────────────────────────────────────

/// A `TruthBody` that always pauses at a HITL gate identified by `GATE_REF`.
///
/// On first call (pre-gate) it returns a `Blocked` criterion.
/// On second call (post-gate, after approval) it would return `Met` —
/// but neither the reject nor timeout paths ever trigger the second call,
/// so we always return `Blocked` here to keep the stub simple.
const GATE_REF: &str = "gate-ref";

/// The truth key used by the test.  Supplied via the fixture catalog injected
/// into `JobStreamState` — no longer requires the old global TRUTHS slice.
const TRUTH_KEY: &str = "score-inbound-fit";

/// Minimal gherkin sufficient for axiom to compile an `IntentPacket`.
const FIXTURE_GHERKIN_SCORE: &str = "Feature: Score inbound fit\n\n  Intent:\n    Outcome: score inbound lead fit for mechanism tests\n\n  Scenario: Score\n    Given a test lead exists\n    Then fit is scored";

const FIXTURE_TRUTHS: &[TruthDefinition] = &[TruthDefinition {
    key: "score-inbound-fit",
    display_name: "Score inbound fit",
    kind: TruthKind::Job,
    summary: "Fixture truth for mechanism tests.",
    feature_path: "fixture",
    actor_roles: &[],
    approval_points: &[],
    desired_outcomes: &[],
    guardrails: &[],
    modules: &[],
    gherkin: FIXTURE_GHERKIN_SCORE,
}];

struct GateRequiringTruth;

#[async_trait]
impl TruthBody for GateRequiringTruth {
    fn key(&self) -> &'static str {
        TRUTH_KEY
    }

    async fn execute(
        &self,
        _ctx: TruthExecutionContext,
    ) -> Result<TruthExecutionArtifacts, TruthExecutionError> {
        let result = ConvergeResult {
            context: ContextState::default(),
            cycles: 1,
            converged: false,
            stop_reason: StopReason::human_intervention_required(
                vec![CriterionId::new("gate-criterion")],
                vec![],
            ),
            criteria_outcomes: vec![CriterionOutcome {
                criterion: Criterion::required(
                    CriterionId::new("gate-criterion"),
                    "gate criterion",
                ),
                result: CriterionResult::Blocked {
                    reason: "requires operator approval".into(),
                    approval_ref: Some(ApprovalPointId::new(GATE_REF)),
                },
            }],
            integrity: IntegrityProof {
                merkle_root: MerkleRoot(ContentHash([0u8; 32])),
                clock_time: 0,
                fact_count: 0,
            },
        };
        Ok(TruthExecutionArtifacts {
            result,
            experience_events: vec![],
            projection: None,
            runtime_scope_id: "test-scope".into(),
        })
    }
}

// ── Helper: build state with configured gate timeout ──────────────────────────

fn state_with_timeout(timeout: Duration) -> Arc<JobStreamState> {
    let hub = EventHub::with_capacity(128);
    let truths = Arc::new(TruthExecutionModule::new().register(Arc::new(GateRequiringTruth)));
    Arc::new(JobStreamState {
        store: AppKernelStore::Memory(InMemoryKernelStore::default_local()),
        runtime_stores: AppRuntimeStores::default(),
        truths,
        hub: hub.handle(),
        app_id: "test.governed-jobs".into(),
        gate_timeout: timeout,
        catalog: TruthCatalog::new(FIXTURE_TRUTHS),
        ..JobStreamState::default()
    })
}

// ── Helper: drain the subscriber until we see an event of the expected type ──

async fn wait_for_event(
    rx: &mut tokio::sync::broadcast::Receiver<EventEnvelope>,
    expected_type: &str,
    deadline: Duration,
) -> Option<EventEnvelope> {
    let fut = async {
        loop {
            match rx.recv().await {
                Ok(env) if env.r#type == expected_type => return Some(env),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
    };
    tokio::time::timeout(deadline, fut).await.ok().flatten()
}

// ── gate_rejected test ────────────────────────────────────────────────────────

#[tokio::test]
async fn gate_rejected_emits_rejection_event_and_fails_job() {
    let state = state_with_timeout(Duration::from_secs(30));
    let hub = state.hub.clone();
    // Subscribe before spawning so we don't miss early events.
    let mut rx = hub.subscribe();

    let run_id = "test-run-reject-1".to_string();

    let state_clone = state.clone();
    let run_id_clone = run_id.clone();
    let job_task = tokio::spawn(async move {
        run_job_task(JobRunTask {
            state: state_clone,
            run_id: run_id_clone,
            truth_key: TRUTH_KEY.to_string(),
            app_id: "test.governed-jobs".to_string(),
            inputs: Default::default(),
            initiator: None,
        })
        .await;
    });

    // Wait for gate.paused — confirms the job is parked at the HITL gate.
    let gate_paused = wait_for_event(&mut rx, "gate.paused", Duration::from_secs(5))
        .await
        .expect("gate.paused should fire within 5s");

    // Extract the scoped ref_id that was registered as the gate waiter.
    let ref_id = gate_paused.payload["ref_id"]
        .as_str()
        .expect("gate.paused payload must contain ref_id")
        .to_string();

    // Reject the gate.
    let signalled = state.signal_gate(&ref_id, GateDecision::Rejected);
    assert!(
        signalled,
        "signal_gate should find the waiter and signal it"
    );

    // Assert event sequence: gate.rejected then job.failed.
    let _gate_rejected = wait_for_event(&mut rx, "gate.rejected", Duration::from_secs(5))
        .await
        .expect("gate.rejected should fire after rejection signal");

    let _job_failed = wait_for_event(&mut rx, "job.failed", Duration::from_secs(5))
        .await
        .expect("job.failed should fire after gate rejection");

    // The job task should complete (not hang).
    tokio::time::timeout(Duration::from_secs(5), job_task)
        .await
        .expect("job task should complete within deadline")
        .expect("job task should not panic");
}

// ── gate_timeout test ─────────────────────────────────────────────────────────

#[tokio::test]
async fn gate_timeout_fires_after_configured_duration() {
    // 300ms timeout — fast enough for CI, long enough to avoid spurious failures.
    let state = state_with_timeout(Duration::from_millis(300));
    let hub = state.hub.clone();
    let mut rx = hub.subscribe();

    let run_id = "test-run-timeout-1".to_string();

    let state_clone = state.clone();
    let run_id_clone = run_id.clone();
    let job_task = tokio::spawn(async move {
        run_job_task(JobRunTask {
            state: state_clone,
            run_id: run_id_clone,
            truth_key: TRUTH_KEY.to_string(),
            app_id: "test.governed-jobs".to_string(),
            inputs: Default::default(),
            initiator: None,
        })
        .await;
    });

    // Wait for gate.paused — confirms the job is parked.
    let _gate_paused = wait_for_event(&mut rx, "gate.paused", Duration::from_secs(5))
        .await
        .expect("gate.paused should fire within 5s");

    // Deliberately send NO signal.  The configured 300ms timeout should fire.

    // gate.timeout should arrive within 300ms + 2s generous buffer.
    let _gate_timeout = wait_for_event(&mut rx, "gate.timeout", Duration::from_secs(3))
        .await
        .expect("gate.timeout should fire within 3s with 300ms gate_timeout configured");

    let _job_failed = wait_for_event(&mut rx, "job.failed", Duration::from_secs(2))
        .await
        .expect("job.failed should follow gate.timeout");

    // The job task should complete after the timeout.
    tokio::time::timeout(Duration::from_secs(5), job_task)
        .await
        .expect("job task should complete within deadline")
        .expect("job task should not panic");
}
