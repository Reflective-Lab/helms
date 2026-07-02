//! Runway app-host mount test — coordination + governed-jobs live on one host.

use std::sync::Arc;
use std::time::Duration;

use application_storage::{AppKernelStore, AppRuntimeStores, InMemoryKernelStore};
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use converge_core::integrity::{ContentHash, IntegrityProof, MerkleRoot};
use converge_core::{
    ContextState, ConvergeResult, Criterion, CriterionId, CriterionOutcome, CriterionResult,
    StopReason,
};
use helm_coordination::{CoordinationModuleState, mount_live_modules};
use helm_governed_jobs::{GovernedJobsModuleState, JobStreamState};
use helm_truth_execution::{
    TruthBody, TruthExecutionArtifacts, TruthExecutionModule, dispatcher::TruthExecutionContext,
};
use http_body_util::BodyExt;
use runway_app_host::{
    AppExecutionPacket, MountKind, MountedModule, RouteOwner, RouteRegistration, RunwayAppHost,
};
use runway_storage::StorageKit;
use tower::ServiceExt;

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

fn live_job_state() -> Arc<JobStreamState> {
    let hub = runway_app_host::EventHub::with_capacity(256);
    let truths = Arc::new(TruthExecutionModule::new().register(Arc::new(ImmediateTruth)));
    Arc::new(JobStreamState {
        store: AppKernelStore::Memory(InMemoryKernelStore::default_local()),
        runtime_stores: AppRuntimeStores::default(),
        truths,
        hub: hub.handle(),
        app_id: "test.host-mount".into(),
        gate_timeout: Duration::from_secs(30),
        ..JobStreamState::default()
    })
}

fn host_packet() -> AppExecutionPacket {
    AppExecutionPacket::new(
        "test.coordination-host",
        "Coordination Host Mount Test",
        "Pins live coordination + governed-jobs on RunwayAppHost",
        "",
    )
    .with_mounted_module(MountedModule {
        module_id: "helm.governed-jobs".into(),
        mount_kind: MountKind::Mounted,
        routes: vec![RouteRegistration {
            method: "POST".into(),
            path: "/v1/jobs/{key}/stream".into(),
            owner: RouteOwner::HelmModule,
        }],
    })
    .with_mounted_module(MountedModule {
        module_id: "helm.coordination".into(),
        mount_kind: MountKind::Mounted,
        routes: vec![
            RouteRegistration {
                method: "POST".into(),
                path: "/v1/coordination/sessions".into(),
                owner: RouteOwner::HelmModule,
            },
            RouteRegistration {
                method: "GET".into(),
                path: "/v1/coordination/stream".into(),
                owner: RouteOwner::HelmModule,
            },
        ],
    })
}

#[tokio::test]
async fn runway_host_mounts_live_coordination_and_governed_jobs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage = StorageKit::local(dir.path()).await.expect("local storage");

    let state = live_job_state();
    let (jobs, coordination) = mount_live_modules(state, "test.coordination");

    assert_eq!(jobs.module_state(), GovernedJobsModuleState::Live);
    assert_eq!(coordination.module_state(), CoordinationModuleState::Live);

    let router = RunwayAppHost::builder(host_packet())
        .with_storage(storage)
        .mount(jobs)
        .mount(coordination)
        .build()
        .await
        .expect("host builds")
        .into_router();

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/coordination/sessions")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "actor_id": "alice",
                        "display_name": "Alice",
                        "workspace_id": "ws-mount"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["principal"]["actor_id"], "alice");
}
