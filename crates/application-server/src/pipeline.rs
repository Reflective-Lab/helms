//! Pipeline coordinator — chains truth executions with output→input mapping.
//!
//! The showcase pipeline: score-inbound-fit → qualify-inbound-lead → schedule-strategic-meetings
//!
//! Each step is a distinct convergence run. The coordinator:
//! 1. Reads seed data (Parquet)
//! 2. Executes step N
//! 3. Extracts relevant outputs from step N's projection
//! 4. Maps them to step N+1's inputs
//! 5. Returns per-step results for live visibility

use std::collections::HashMap;
use std::path::Path;

use application_kernel::Actor as CrmActor;
use application_storage::{AppRuntimeStores, KernelStore};
use serde::{Deserialize, Serialize};
use tonic::Status;

use crate::truth_runtime::{TruthExecutionArtifacts, TruthProjection, execute_truth};

// ── Pipeline Types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PipelineResult {
    pub steps: Vec<PipelineStepResult>,
    pub status: PipelineStatus,
    pub prospect_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipelineStepResult {
    pub truth_key: String,
    pub status: StepStatus,
    pub cycles: Option<u32>,
    pub stop_reason: Option<String>,
    pub fact_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PipelineStatus {
    Completed,
    BlockedAtStep { step: usize, reason: String },
    Failed { step: usize, error: String },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum StepStatus {
    Completed,
    Blocked { reason: String },
    Skipped,
    Failed { error: String },
}

// ── Pipeline Input ──────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ShowcasePipelineInput {
    pub prospect_name: String,
    pub visitor_id: String,
    pub usage_events_json: String,
    pub inbound_summary: String,
    pub meeting_count: u32,
    pub window_start: String,
    pub window_end: String,
    pub calendar_slots_json: Option<String>,
    pub industry: Option<String>,
    pub website: Option<String>,
    pub contact_name: Option<String>,
    pub contact_title: Option<String>,
    pub contact_email: Option<String>,
}

// ── Pipeline Execution ──────────────────────────────────────────────

pub async fn run_showcase_pipeline<S: KernelStore>(
    store: &S,
    runtime_stores: &AppRuntimeStores,
    input: ShowcasePipelineInput,
    actor: CrmActor,
) -> PipelineResult {
    let mut steps = Vec::new();
    let prospect_name = input.prospect_name.clone();

    // ── Step 1: Score inbound fit ───────────────────────────────────

    let score_inputs = build_score_inputs(&input);
    let score_result = execute_truth(
        store,
        runtime_stores,
        "score-inbound-fit",
        score_inputs,
        actor.clone(),
        true,
    )
    .await;

    let (score_artifacts, fit_score_bps) = match score_result {
        Ok(artifacts) => {
            let fit_score = extract_fit_score(&artifacts);
            let step = step_result_from_artifacts("score-inbound-fit", &artifacts);
            let blocked = matches!(step.status, StepStatus::Blocked { .. });
            steps.push(step);
            if blocked {
                return PipelineResult {
                    steps,
                    status: PipelineStatus::BlockedAtStep {
                        step: 0,
                        reason: "score-inbound-fit blocked for review".into(),
                    },
                    prospect_name,
                };
            }
            (artifacts, fit_score)
        }
        Err(e) => {
            steps.push(PipelineStepResult {
                truth_key: "score-inbound-fit".into(),
                status: StepStatus::Failed {
                    error: e.message().to_string(),
                },
                cycles: None,
                stop_reason: None,
                fact_count: None,
            });
            return PipelineResult {
                steps,
                status: PipelineStatus::Failed {
                    step: 0,
                    error: e.message().to_string(),
                },
                prospect_name,
            };
        }
    };

    // ── Step 2: Qualify inbound lead ────────────────────────────────

    let org_id = score_artifacts
        .projection
        .as_ref()
        .and_then(|p| p.organization.as_ref())
        .map(|org| org.id.to_string());

    let qualify_inputs = build_qualify_inputs(&input, org_id.as_deref(), fit_score_bps);
    let qualify_result = execute_truth(
        store,
        runtime_stores,
        "qualify-inbound-lead",
        qualify_inputs,
        actor.clone(),
        true,
    )
    .await;

    let qualify_artifacts = match qualify_result {
        Ok(artifacts) => {
            let step = step_result_from_artifacts("qualify-inbound-lead", &artifacts);
            let blocked = matches!(step.status, StepStatus::Blocked { .. });
            steps.push(step);
            if blocked {
                return PipelineResult {
                    steps,
                    status: PipelineStatus::BlockedAtStep {
                        step: 1,
                        reason: "qualify-inbound-lead blocked for review".into(),
                    },
                    prospect_name,
                };
            }
            artifacts
        }
        Err(e) => {
            steps.push(PipelineStepResult {
                truth_key: "qualify-inbound-lead".into(),
                status: StepStatus::Failed {
                    error: e.message().to_string(),
                },
                cycles: None,
                stop_reason: None,
                fact_count: None,
            });
            return PipelineResult {
                steps,
                status: PipelineStatus::Failed {
                    step: 1,
                    error: e.message().to_string(),
                },
                prospect_name,
            };
        }
    };

    // ── Step 3: Schedule strategic meetings ─────────────────────────

    let schedule_inputs = build_schedule_inputs(&input, org_id.as_deref(), fit_score_bps);
    let schedule_result = execute_truth(
        store,
        runtime_stores,
        "schedule-strategic-meetings",
        schedule_inputs,
        actor,
        true,
    )
    .await;

    match schedule_result {
        Ok(artifacts) => {
            let step = step_result_from_artifacts("schedule-strategic-meetings", &artifacts);
            let blocked = matches!(step.status, StepStatus::Blocked { .. });
            steps.push(step);
            if blocked {
                return PipelineResult {
                    steps,
                    status: PipelineStatus::BlockedAtStep {
                        step: 2,
                        reason: "schedule-strategic-meetings blocked for confirmation".into(),
                    },
                    prospect_name,
                };
            }
        }
        Err(e) => {
            steps.push(PipelineStepResult {
                truth_key: "schedule-strategic-meetings".into(),
                status: StepStatus::Failed {
                    error: e.message().to_string(),
                },
                cycles: None,
                stop_reason: None,
                fact_count: None,
            });
            return PipelineResult {
                steps,
                status: PipelineStatus::Failed {
                    step: 2,
                    error: e.message().to_string(),
                },
                prospect_name,
            };
        }
    }

    PipelineResult {
        steps,
        status: PipelineStatus::Completed,
        prospect_name,
    }
}

// ── Input Builders ──────────────────────────────────────────────────

fn build_score_inputs(input: &ShowcasePipelineInput) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("organization_name".into(), input.prospect_name.clone());
    m.insert("visitor_id".into(), input.visitor_id.clone());
    m.insert("usage_events_json".into(), input.usage_events_json.clone());
    if let Some(ref industry) = input.industry {
        m.insert("industry".into(), industry.clone());
    }
    if let Some(ref website) = input.website {
        m.insert("website".into(), website.clone());
    }
    m
}

fn build_qualify_inputs(
    input: &ShowcasePipelineInput,
    org_id: Option<&str>,
    fit_score_bps: Option<u16>,
) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("organization_name".into(), input.prospect_name.clone());
    m.insert("inbound_summary".into(), input.inbound_summary.clone());
    if let Some(org_id) = org_id {
        m.insert("organization_id".into(), org_id.to_string());
    }
    if let Some(fit) = fit_score_bps {
        // Convert bps (0-10000) to 0-100 scale for qualify
        m.insert("fit_score".into(), (fit / 100).to_string());
    }
    if let Some(ref industry) = input.industry {
        m.insert("industry".into(), industry.clone());
    }
    if let Some(ref website) = input.website {
        m.insert("website".into(), website.clone());
    }
    if let Some(ref name) = input.contact_name {
        m.insert("contact_name".into(), name.clone());
    }
    if let Some(ref title) = input.contact_title {
        m.insert("contact_title".into(), title.clone());
    }
    if let Some(ref email) = input.contact_email {
        m.insert("contact_email".into(), email.clone());
    }
    m
}

fn build_schedule_inputs(
    input: &ShowcasePipelineInput,
    org_id: Option<&str>,
    fit_score_bps: Option<u16>,
) -> HashMap<String, String> {
    let prospect_seed = serde_json::json!([{
        "organization_id": org_id,
        "name": input.prospect_name,
        "contact_name": input.contact_name,
        "contact_email": input.contact_email,
        "fit_score_bps": fit_score_bps,
        "pipeline_stage": "qualified",
        "last_contact_days_ago": 0,
        "estimated_value": null,
        "territory": null,
        "segment": input.industry,
        "tags": ["pipeline-showcase"]
    }]);

    let mut m = HashMap::new();
    m.insert(
        "intent_text".into(),
        format!(
            "Book {} meetings with qualified prospects",
            input.meeting_count
        ),
    );
    m.insert("requested_count".into(), input.meeting_count.to_string());
    m.insert("window_start".into(), input.window_start.clone());
    m.insert("window_end".into(), input.window_end.clone());
    m.insert("prospects_json".into(), prospect_seed.to_string());
    if let Some(ref slots) = input.calendar_slots_json {
        m.insert("calendar_slots_json".into(), slots.clone());
    }
    m
}

// ── Output Extraction ───────────────────────────────────────────────

fn extract_fit_score(artifacts: &TruthExecutionArtifacts) -> Option<u16> {
    let facts = artifacts.projection.as_ref()?.facts.as_slice();
    for fact in facts {
        if fact.statement.contains("fit score") || fact.statement.contains("Inbound fit score") {
            // Extract bps from statement like "Inbound fit score 7500 bps (high-fit)..."
            let words: Vec<&str> = fact.statement.split_whitespace().collect();
            for (i, w) in words.iter().enumerate() {
                if *w == "bps" && i > 0 {
                    if let Ok(score) = words[i - 1].parse::<u16>() {
                        return Some(score);
                    }
                }
            }
            // Fallback: use confidence_bps as proxy
            return Some(fact.confidence_bps as u16);
        }
    }
    None
}

fn step_result_from_artifacts(
    truth_key: &str,
    artifacts: &TruthExecutionArtifacts,
) -> PipelineStepResult {
    let stop_reason = format!("{:?}", artifacts.result.stop_reason);
    let is_blocked = stop_reason.contains("Blocked") || stop_reason.contains("HumanIntervention");

    PipelineStepResult {
        truth_key: truth_key.into(),
        status: if is_blocked {
            StepStatus::Blocked {
                reason: stop_reason.clone(),
            }
        } else {
            StepStatus::Completed
        },
        cycles: Some(artifacts.result.cycles),
        stop_reason: Some(stop_reason),
        fact_count: Some(artifacts.result.integrity.fact_count),
    }
}

// ── Seed Data Loader ────────────────────────────────────────────────

/// Load behavioral events from seed Parquet for a specific prospect.
/// Returns the events as a JSON string suitable for score-inbound-fit input.
pub fn load_prospect_events_from_seed(
    seed_dir: &Path,
    prospect_id: &str,
) -> Result<String, String> {
    use polars::prelude::*;

    let path = seed_dir.join("behavior_events.parquet");
    let parquet_path = path
        .to_str()
        .ok_or_else(|| format!("invalid parquet path: {}", path.display()))?;
    let df = LazyFrame::scan_parquet(PlPath::new(parquet_path), Default::default())
        .map_err(|e| format!("failed to read parquet: {e}"))?
        .filter(col("prospect_id").eq(lit(prospect_id)))
        .collect()
        .map_err(|e| format!("failed to filter prospect: {e}"))?;

    let mut events = Vec::new();
    let rows = df.height();
    for i in 0..rows {
        let visitor_id = df
            .column("prospect_id")
            .map_err(|e| e.to_string())?
            .str()
            .map_err(|e| e.to_string())?
            .get(i)
            .unwrap_or("");
        let timestamp = df
            .column("timestamp")
            .map_err(|e| e.to_string())?
            .i64()
            .map_err(|e| e.to_string())?
            .get(i)
            .unwrap_or(0);
        let event_type = df
            .column("event_types")
            .map_err(|e| e.to_string())?
            .str()
            .map_err(|e| e.to_string())?
            .get(i)
            .unwrap_or("");
        let page = df
            .column("page_sections")
            .map_err(|e| e.to_string())?
            .str()
            .map_err(|e| e.to_string())?
            .get(i)
            .unwrap_or("");

        events.push(serde_json::json!({
            "visitor_id": visitor_id,
            "timestamp": timestamp,
            "event_type": event_type,
            "page": page,
        }));
    }

    serde_json::to_string(&events).map_err(|e| format!("json serialization failed: {e}"))
}

/// Load account context from seed Parquet for a specific prospect.
pub fn load_prospect_context_from_seed(
    seed_dir: &Path,
    prospect_id: &str,
) -> Result<ShowcasePipelineInput, String> {
    use polars::prelude::*;

    let path = seed_dir.join("account_context.parquet");
    let parquet_path = path
        .to_str()
        .ok_or_else(|| format!("invalid parquet path: {}", path.display()))?;
    let df = LazyFrame::scan_parquet(PlPath::new(parquet_path), Default::default())
        .map_err(|e| format!("failed to read account_context parquet: {e}"))?
        .filter(col("prospect_id").eq(lit(prospect_id)))
        .collect()
        .map_err(|e| format!("failed to filter prospect: {e}"))?;

    if df.height() == 0 {
        return Err(format!(
            "prospect '{prospect_id}' not found in account_context"
        ));
    }

    let name = df
        .column("company_name")
        .ok()
        .and_then(|c| c.str().ok())
        .and_then(|s| s.get(0))
        .unwrap_or(prospect_id)
        .to_string();

    let industry = df
        .column("industry")
        .ok()
        .and_then(|c| c.str().ok())
        .and_then(|s| s.get(0))
        .map(ToString::to_string);

    let events_json = load_prospect_events_from_seed(seed_dir, prospect_id)?;

    Ok(ShowcasePipelineInput {
        prospect_name: name,
        visitor_id: prospect_id.to_string(),
        usage_events_json: events_json,
        inbound_summary: format!("Inbound inquiry from {prospect_id} via website demo request"),
        meeting_count: 1,
        window_start: "2026-04-21".into(),
        window_end: "2026-04-25".into(),
        calendar_slots_json: None,
        industry,
        website: None,
        contact_name: None,
        contact_title: None,
        contact_email: None,
    })
}
