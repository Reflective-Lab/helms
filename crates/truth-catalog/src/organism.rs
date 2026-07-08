use organism_pack::IntentBinding;
use organism_runtime::ReadinessReport;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TruthOrganismBinding {
    pub truth_key: &'static str,
    pub blueprint: Option<&'static str>,
    pub binding: IntentBinding,
    pub readiness: ReadinessReport,
}

impl TruthOrganismBinding {
    #[must_use]
    pub fn pack_names(&self) -> Vec<String> {
        self.binding
            .packs
            .iter()
            .map(|pack| pack.pack_name.clone())
            .collect()
    }
}
