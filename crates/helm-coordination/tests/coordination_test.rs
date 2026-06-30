//! Two-operator coordination integration tests.
//!
//! These drive the optimistic gate-decision flow end-to-end against a real
//! `helm-governed-jobs` run loop: two operators decide the same gate; the first
//! decision signals the waiter and completes the job, an identical second call
//! is idempotent (no second side-effect), and a divergent call is a conflict.
//!
//! Attribution note: the governed-job `gate.approved` event is attributed to the
//! *run initiator*. The authoritative *approver* attribution lives in the
//! coordination `decision.recorded` event, which these tests assert.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use application_kernel::ActorKind;
use application_storage::{AppKernelStore, AppRuntimeStores, InMemoryKernelStore};
use async_trait::async_trait;
use converge_core::ApprovalPointId;
use converge_core::integrity::{ContentHash, IntegrityProof, MerkleRoot};
use converge_core::{
    ContextState, ConvergeResult, Criterion, CriterionId, CriterionOutcome, CriterionResult,
    StopReason,
};
use helm_coordination::{
    AuthorityResolver, CoordinationError, CoordinationService, DecisionOutcome, GateDecisionKind,
    OperatorPrincipal, PrincipalClaim, SubjectRef,
};
use helm_governed_jobs::{JobRunTask, JobStreamState, run_job_task};
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionModule, dispatcher::TruthExecutionContext,
};
use runway_app_host::{EventEnvelope, EventHub};

const TRUTH_KEY: &str = "score-inbound-fit";

/// A truth that blocks at a HITL gate on first execution and is met on the
/// second (post-approval) execution, so an approval completes the job.
struct CompletingGateTruth {
    calls: AtomicUsize,
}

impl CompletingGateTruth {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl TruthBody for CompletingGateTruth {
    fn key(&self) -> &'static str {
        TRUTH_KEY
    }

    async fn execute(
        &self,
        _ctx: TruthExecutionContext,
    ) -> Result<TruthExecutionArtifacts, tonic::Status> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let result = if call == 0 {
            blocked_result()
        } else {
            met_result()
        };
        Ok(TruthExecutionArtifacts {
            result,
            experience_events: vec![],
            projection: None,
            runtime_scope_id: "test-scope".into(),
        })
    }
}

fn integrity() -> IntegrityProof {
    IntegrityProof {
        merkle_root: MerkleRoot(ContentHash([0u8; 32])),
        clock_time: 0,
        fact_count: 0,
    }
}

fn blocked_result() -> ConvergeResult {
    ConvergeResult {
        context: ContextState::default(),
        cycles: 1,
        converged: false,
        stop_reason: StopReason::human_intervention_required(
            vec![CriterionId::new("gate-criterion")],
            vec![],
        ),
        criteria_outcomes: vec![CriterionOutcome {
            criterion: Criterion::required(CriterionId::new("gate-criterion"), "gate criterion"),
            result: CriterionResult::Blocked {
                reason: "requires operator approval".into(),
                approval_ref: Some(ApprovalPointId::new("gate-ref")),
            },
        }],
        integrity: integrity(),
    }
}

fn met_result() -> ConvergeResult {
    ConvergeResult {
        context: ContextState::default(),
        cycles: 2,
        converged: true,
        stop_reason: StopReason::Converged,
        criteria_outcomes: vec![CriterionOutcome {
            criterion: Criterion::required(CriterionId::new("gate-criterion"), "gate criterion"),
            result: CriterionResult::Met { evidence: vec![] },
        }],
        integrity: integrity(),
    }
}

fn live_state() -> Arc<JobStreamState> {
    let hub = EventHub::with_capacity(256);
    let truths =
        Arc::new(TruthExecutionModule::new().register(Arc::new(CompletingGateTruth::new())));
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
        workspace_id: Some("ws-1".to_string()),
    }
}

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
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => return None,
            }
        }
    };
    tokio::time::timeout(deadline, fut).await.ok().flatten()
}

#[tokio::test]
async fn two_operators_optimistic_gate_decision() {
    let state = live_state();
    let service = CoordinationService::new(state.hub.clone(), "test.coordination")
        .with_job_state(state.clone());

    let mut rx = state.hub.subscribe();

    // Start a system-initiated run that will park at the HITL gate.
    let state_clone = state.clone();
    let job = tokio::spawn(async move {
        run_job_task(JobRunTask {
            state: state_clone,
            run_id: "run-coord-1".to_string(),
            truth_key: TRUTH_KEY.to_string(),
            app_id: "test.governed-jobs".to_string(),
            inputs: Default::default(),
            initiator: None,
        })
        .await;
    });

    let gate_paused = wait_for_event(&mut rx, "gate.paused", Duration::from_secs(5))
        .await
        .expect("gate.paused should fire");
    let ref_id = gate_paused.payload["ref_id"]
        .as_str()
        .expect("ref_id present")
        .to_string();

    // Operator A approves — first decision is recorded and drives the job.
    let outcome = service
        .decide_gate(&ref_id, &claim("alice"), GateDecisionKind::Approve, None)
        .expect("alice decision resolves");
    assert!(matches!(outcome, DecisionOutcome::Recorded(_)));

    // The decision.recorded event attributes the approval to alice.
    let recorded = wait_for_event(&mut rx, "decision.recorded", Duration::from_secs(5))
        .await
        .expect("decision.recorded should fire");
    assert_eq!(recorded.actor.as_deref(), Some("ws-1:alice"));
    assert_eq!(recorded.payload["principal"]["actor_id"], "alice");

    // The approval signals the waiter and the job completes.
    wait_for_event(&mut rx, "gate.approved", Duration::from_secs(5))
        .await
        .expect("gate.approved should fire");
    wait_for_event(&mut rx, "job.completed", Duration::from_secs(5))
        .await
        .expect("job.completed should fire");
    tokio::time::timeout(Duration::from_secs(5), job)
        .await
        .expect("job task completes")
        .expect("job task does not panic");

    // Operator B agrees afterward — idempotent, no second side-effect.
    let outcome = service
        .decide_gate(&ref_id, &claim("bob"), GateDecisionKind::Approve, None)
        .expect("bob agreeing decision resolves");
    match outcome {
        DecisionOutcome::Idempotent(record) => assert_eq!(record.principal.actor_id, "alice"),
        other => panic!("expected idempotent, got {other:?}"),
    }

    // Operator B diverges — conflict, rejected, original preserved.
    let outcome = service
        .decide_gate(&ref_id, &claim("bob"), GateDecisionKind::Reject, None)
        .expect("bob divergent decision resolves to an outcome");
    match outcome {
        DecisionOutcome::Conflict {
            existing,
            attempted,
            ..
        } => {
            assert_eq!(existing.decision, GateDecisionKind::Approve);
            assert_eq!(attempted, GateDecisionKind::Reject);
        }
        other => panic!("expected conflict, got {other:?}"),
    }
    let conflict = wait_for_event(&mut rx, "decision.conflict", Duration::from_secs(5))
        .await
        .expect("decision.conflict should fire");
    assert_eq!(conflict.payload["existing_actor"], "alice");
}

struct DenyAll;

impl AuthorityResolver for DenyAll {
    fn can_decide(&self, _principal: &OperatorPrincipal, _subject: &SubjectRef) -> bool {
        false
    }
}

#[tokio::test]
async fn authority_denied_blocks_decision() {
    let hub = EventHub::with_capacity(64);
    let service = CoordinationService::new(hub.handle(), "test.coordination")
        .with_authority(Arc::new(DenyAll));

    let result = service.decide_gate("g1", &claim("alice"), GateDecisionKind::Approve, None);
    assert!(matches!(
        result,
        Err(CoordinationError::AuthorityDenied { .. })
    ));
    // Nothing recorded; a later authorized resolver would still see a clean gate.
    assert!(
        service
            .decide_gate("g1", &claim("alice"), GateDecisionKind::Approve, None)
            .is_err()
    );
}

#[tokio::test]
async fn session_lifecycle_through_service() {
    let hub = EventHub::with_capacity(64);
    let service = CoordinationService::new(hub.handle(), "test.coordination");

    let session = service.open_session(&claim("alice")).expect("open");
    assert_eq!(service.list_sessions("ws-1").len(), 1);
    service.heartbeat(session.id).expect("heartbeat");
    service.close_session(session.id).expect("close");
    assert!(service.list_sessions("ws-1").is_empty());
}

mod http {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use helm_coordination::CoordinationModule;
    use http_body_util::BodyExt;
    use runway_app_host::HelmModule;
    use tower::ServiceExt;

    fn router(service: CoordinationService) -> axum::Router {
        Arc::new(CoordinationModule::new(Arc::new(service))).router()
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn open_session_returns_created() {
        let hub = EventHub::with_capacity(64);
        let app = router(CoordinationService::new(hub.handle(), "test.coordination"));

        let request = Request::builder()
            .method("POST")
            .uri("/v1/coordination/sessions")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&claim("alice")).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let value = body_json(response).await;
        assert_eq!(value["principal"]["actor_id"], "alice");
    }

    #[tokio::test]
    async fn divergent_decision_returns_conflict() {
        let hub = EventHub::with_capacity(64);
        let app = router(CoordinationService::new(hub.handle(), "test.coordination"));

        let approve = Request::builder()
            .method("POST")
            .uri("/v1/coordination/gates/g1/decision")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({
                    "decision": "approve",
                    "actor_id": "alice",
                    "workspace_id": "ws-1"
                })
                .to_string(),
            ))
            .unwrap();
        let response = app.clone().oneshot(approve).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_json(response).await["status"], "recorded");

        let reject = Request::builder()
            .method("POST")
            .uri("/v1/coordination/gates/g1/decision")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({
                    "decision": "reject",
                    "actor_id": "bob",
                    "workspace_id": "ws-1"
                })
                .to_string(),
            ))
            .unwrap();
        let response = app.oneshot(reject).await.unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(body_json(response).await["status"], "conflict");
    }
}
