//! Shared contracts for Helm modules mounted into Runtime Runway.
//!
//! These types are intentionally small. Runtime Runway owns the generic host
//! verifier, but Helm modules need a stable vocabulary for reporting whether a
//! mounted module is only a default shell or has live app evidence wired.

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

#[cfg(test)]
mod tests {
    use super::{HelmModuleState, HelmModuleStatus};
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
}
