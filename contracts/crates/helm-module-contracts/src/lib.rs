//! Shared contracts for Helm modules mounted into Runtime Runway.
//!
//! Defines both the readiness-reporting vocabulary (`HelmModuleReadiness`,
//! `HelmModuleState`, `HelmModuleStatus`) and the mounting contract
//! (`HelmModule`, `ModuleState`) so the interface lives in a neutral crate
//! that both helms crates and Runtime Runway can consume without creating a
//! foundation→substrate dependency (RP-LAYERING, RFL-128).
//!
//! The [`operator_receipts`] submodule owns the full operator-control receipt
//! vocabulary (all 18 types, hashing helpers, and `OperatorControlError`),
//! promoted here from `prio-agent-ops` as part of RFL-154.
//!
//! The [`operator_preview`] submodule provides read-only view types over the
//! receipts vocabulary (`OperatorControlPreview`, `OperatorControlPreviewBacking`,
//! `OperatorReceiptFamilyView`, `operator_receipt_families()`), promoted here
//! from `workbench-backend` as part of RFL-154 T2.

pub mod operator_preview;
pub mod operator_receipts;

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HelmModuleState {
    /// Routes may exist, but the module is serving default/demo/static state.
    ShellDefault,
    /// The module is backed by live app evidence or executable truth wiring.
    Live,
}

impl HelmModuleState {
    pub const fn is_live(self) -> bool {
        matches!(self, Self::Live)
    }
}

pub trait HelmModuleReadiness {
    fn module_state(&self) -> HelmModuleState;

    fn readiness_status(&self) -> HelmModuleStatus;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelmModuleStatus {
    pub module_id: String,
    pub state: HelmModuleState,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered_truths: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub live_requirements: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_live_requirements: Vec<String>,
}

impl HelmModuleStatus {
    pub fn new(
        module_id: impl Into<String>,
        state: HelmModuleState,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            module_id: module_id.into(),
            state,
            reason: reason.into(),
            registered_truths: None,
            live_requirements: Vec::new(),
            missing_live_requirements: Vec::new(),
        }
    }

    pub fn with_registered_truths(mut self, count: usize) -> Self {
        self.registered_truths = Some(count);
        self
    }

    pub fn with_live_requirements<I, S>(mut self, requirements: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.live_requirements = requirements.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_missing_live_requirements<I, S>(mut self, requirements: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.missing_live_requirements = requirements.into_iter().map(Into::into).collect();
        self
    }
}

// ── Mounting contract ─────────────────────────────────────────────────────────
//
// `HelmModule` and `ModuleState` live here so the contract is neutral — neither
// helms crates nor Runtime Runway need to depend on the other to share it.
// Extracted from `runway-app-host::module` (RFL-128; RP-LAYERING).

/// Whether a mounted module is wired to live state or is still a default shell.
///
/// The D1 manifest verifier reconciles this against the manifest's
/// `mounted_modules[].mount_kind`: a module the manifest marks `Mounted` must
/// report `Live`, otherwise `serve()` fails. The default is `Shell` so silence
/// fails closed — a module that forgets to report its state is treated as
/// not-yet-wired, never as a live claim that passes the gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleState {
    /// Mounted but not wired to live state (default-shell).
    Shell,
    /// Wired to live state.
    Live,
}

/// Contract for a Helm module that can be mounted into Runtime Runway
/// (`RunwayAppHostBuilder::mount`).
///
/// All methods have default no-op implementations so implementors only need to
/// override what they provide. `module_id` is the one required method.
///
/// RP-LAYERING (RFL-128): this trait is defined here (a neutral foundation
/// crate) so that both helms crates and `runway-app-host` consume it without
/// either side depending on the other.
#[async_trait]
pub trait HelmModule: Send + Sync + 'static {
    fn module_id(&self) -> &'static str;

    /// Called once during host `build()`. Override to register services,
    /// validate config, or log readiness evidence. The host aborts startup on
    /// `Err`. Default implementation is a no-op.
    async fn init(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Axum router to merge into the host. Default returns an empty router.
    fn router(self: Arc<Self>) -> axum::Router {
        axum::Router::new()
    }

    /// Whether this module is wired to live state. The D1 verifier treats a
    /// manifest-declared `Mounted` module that reports `Shell` as the
    /// planned-vs-mounted lie; `serve()` will be rejected. Defaults to `Shell`
    /// (fails closed).
    fn module_state(&self) -> ModuleState {
        ModuleState::Shell
    }
}

#[cfg(test)]
mod tests {
    use super::{HelmModule, HelmModuleReadiness, HelmModuleState, HelmModuleStatus, ModuleState};
    use serde_json::json;

    #[test]
    fn shell_default_serializes_as_contract_value() {
        let value = serde_json::to_value(HelmModuleState::ShellDefault).unwrap();
        assert_eq!(value, "shell-default");
    }

    #[test]
    fn live_state_reports_live() {
        assert!(HelmModuleState::Live.is_live());
        assert!(!HelmModuleState::ShellDefault.is_live());
    }

    #[test]
    fn status_carries_missing_requirements() {
        let status = HelmModuleStatus::new(
            "helm.operator-control",
            HelmModuleState::ShellDefault,
            "live evidence is not wired",
        )
        .with_registered_truths(0)
        .with_live_requirements(["process_receipt", "integrity_proof"])
        .with_missing_live_requirements(["process_receipt"]);

        assert_eq!(status.registered_truths, Some(0));
        assert_eq!(status.missing_live_requirements, vec!["process_receipt"]);
    }

    #[test]
    fn status_serializes_verifier_contract_shape() {
        let status = HelmModuleStatus::new(
            "helm.operator-control",
            HelmModuleState::ShellDefault,
            "live evidence is not wired",
        )
        .with_registered_truths(0)
        .with_live_requirements(["process_receipt", "integrity_proof"])
        .with_missing_live_requirements(["process_receipt"]);

        let value = serde_json::to_value(status).unwrap();
        assert_eq!(
            value,
            json!({
                "module_id": "helm.operator-control",
                "state": "shell-default",
                "reason": "live evidence is not wired",
                "registered_truths": 0,
                "live_requirements": ["process_receipt", "integrity_proof"],
                "missing_live_requirements": ["process_receipt"]
            })
        );
    }

    #[test]
    fn live_status_omits_empty_missing_requirements() {
        let status = HelmModuleStatus::new(
            "helm.governed-jobs",
            HelmModuleState::Live,
            "truth registry is populated",
        )
        .with_registered_truths(3)
        .with_live_requirements(["truth_registry"]);

        let value = serde_json::to_value(status).unwrap();
        assert_eq!(value["state"], "live");
        assert_eq!(value["live_requirements"], json!(["truth_registry"]));
        assert!(value.get("missing_live_requirements").is_none());
    }

    #[test]
    fn readiness_trait_exposes_state_and_status() {
        struct TestModule;

        impl HelmModuleReadiness for TestModule {
            fn module_state(&self) -> HelmModuleState {
                HelmModuleState::ShellDefault
            }

            fn readiness_status(&self) -> HelmModuleStatus {
                HelmModuleStatus::new("helm.test", self.module_state(), "test shell")
            }
        }

        let module = TestModule;
        assert_eq!(module.module_state(), HelmModuleState::ShellDefault);
        assert_eq!(module.readiness_status().module_id, "helm.test");
    }

    #[test]
    fn module_state_shell_is_default() {
        use std::sync::Arc;

        struct MinimalModule;

        impl HelmModule for MinimalModule {
            fn module_id(&self) -> &'static str {
                "helm.test-minimal"
            }
        }

        assert_eq!(
            Arc::new(MinimalModule).module_state(),
            ModuleState::Shell,
            "default module_state must be Shell (fails closed)"
        );
    }

    #[tokio::test]
    async fn module_init_default_is_noop() {
        use std::sync::Arc;

        struct MinimalModule;

        impl HelmModule for MinimalModule {
            fn module_id(&self) -> &'static str {
                "helm.test-minimal"
            }
        }

        Arc::new(MinimalModule)
            .init()
            .await
            .expect("init should succeed");
    }
}
