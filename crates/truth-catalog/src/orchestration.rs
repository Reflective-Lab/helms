//! Tournament orchestration policy primitives (handoff §6).
//!
//! The selection-side decisions — which templates to race, which to skip,
//! which to refuse outright — live here. Actual execution
//! (`compile_and_run_formation` / `FormationTournament`) is gated on
//! helms wiring its `SuggestorDescriptorCatalog` and
//! `ExecutableSuggestorCatalog`, which the 2026-05-07 handoff explicitly
//! left as open architectural questions.
//!
//! What this module does today:
//! - Defines [`AutoRunOptions`] — count-based cost model per the revised
//!   handoff (`max_candidates` + `relative_cutoff`; per-step cost telemetry
//!   is not a 1.8.0 commitment from organism)
//! - Refuses tournaments for `Reversibility::Irreversible` intents per the
//!   HITL Admission Gate ADR (see `kb/Architecture/HITL Admission Gate.md`)
//! - Filters alternates by composite-score cutoff and the `max_candidates`
//!   cap, surfacing every exclusion in the result
//! - Returns owned [`CandidateSlate`] data so the catalog used during
//!   selection can be dropped immediately
//!
//! What it does NOT do (yet):
//! - Compile / instantiate / run formations. That step needs the
//!   executable catalog story to land first.

use organism_pack::{IntentPacket, Reversibility};
use organism_runtime::GuruError;
use organism_runtime::guru::SelectionTrace;

use converge_kernel::formation::SuggestorCapability;

use crate::admission::select_formation_for_intent;

/// Policy options controlling tournament-of-templates orchestration.
///
/// Defaults to single-shot (`race_alternates: false`) per the revised
/// handoff: auto-tournament is a power tool, not a free upgrade. Callers
/// opt in explicitly when racing makes sense.
#[derive(Debug, Clone)]
pub struct AutoRunOptions {
    /// Race the alternates? Default: `false`. Single-shot is the cheap
    /// path; tournaments are forbidden outright when
    /// `intent.reversibility == Reversibility::Irreversible`.
    pub race_alternates: bool,
    /// Hard cap on candidates considered (incl. primary). Clamped to at
    /// least 1 internally to keep the primary in the race.
    pub max_candidates: usize,
    /// Skip alternates whose composite match score is below
    /// `(primary_score * relative_cutoff)`. Clamped to `[0.0, 1.0]`; the
    /// primary itself is never filtered out.
    pub relative_cutoff: f64,
}

impl Default for AutoRunOptions {
    fn default() -> Self {
        Self {
            race_alternates: false,
            max_candidates: 3,
            relative_cutoff: 0.85,
        }
    }
}

/// The primary template the guru picked plus alternates that survived the
/// policy filters, the auditable [`SelectionTrace`], and a record of every
/// candidate dropped (with the reason why).
#[derive(Debug, Clone)]
pub struct CandidateSlate {
    pub primary_template_id: String,
    pub alternate_template_ids: Vec<String>,
    pub trace: SelectionTrace,
    pub excluded: Vec<ExcludedCandidate>,
}

/// One alternate the guru returned that this slate ultimately did not
/// race. Surfaced so audits can see exactly why a candidate didn't compete
/// (per the revised handoff's "surface partial failure" guidance).
#[derive(Debug, Clone)]
pub struct ExcludedCandidate {
    pub template_id: String,
    pub reason: ExclusionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusionReason {
    /// Race not requested — only the primary runs.
    SingleShotRequested,
    /// Composite match score under `primary_score * relative_cutoff`.
    BelowRelativeCutoff,
    /// Slate already at `max_candidates`.
    BeyondMaxCandidates,
}

#[derive(Debug, thiserror::Error)]
pub enum AutoRunError {
    /// The intent declares `Reversibility::Irreversible`, which forbids
    /// tournaments — even with a cached HITL approval. Single-shot,
    /// approved, run is the only path for irreversibles.
    #[error(
        "tournaments are forbidden for irreversible intents; use single-shot via race_alternates: false"
    )]
    IrreversibleCannotRace,
    /// FormationGuru couldn't pick a primary at all (no catalog match).
    #[error("formation selection failed: {0}")]
    Selection(#[from] GuruError),
}

/// Prepare a [`CandidateSlate`] from `intent` per the policy `opts`.
///
/// This is the auto-mode "front half" — it picks who would race without
/// running anything. Callers hand the slate to the executor once helms's
/// suggestor catalogs are wired (handoff §6 still-open infrastructure).
///
/// Behavior:
/// 1. If `race_alternates && intent.reversibility == Irreversible`, refuse
///    with [`AutoRunError::IrreversibleCannotRace`] before any selection
///    work happens (cheap rejection beats expensive races).
/// 2. Run `select_formation_for_intent` to get the primary + alternates +
///    trace.
/// 3. If `!race_alternates`, return the primary alone; alternates land in
///    `excluded` with [`ExclusionReason::SingleShotRequested`].
/// 4. Otherwise filter alternates: drop those below the relative cutoff,
///    drop anything past `max_candidates`. Every drop is surfaced in
///    `excluded`.
///
/// # Errors
///
/// [`AutoRunError::IrreversibleCannotRace`] when the policy/intent combo
/// is forbidden, [`AutoRunError::Selection`] when no template matches.
pub fn prepare_candidates(
    intent: &IntentPacket,
    capabilities: &[SuggestorCapability],
    opts: &AutoRunOptions,
) -> Result<CandidateSlate, AutoRunError> {
    if opts.race_alternates && intent.reversibility == Reversibility::Irreversible {
        return Err(AutoRunError::IrreversibleCannotRace);
    }

    let selection = select_formation_for_intent(intent, capabilities)?;

    if !opts.race_alternates {
        let excluded = selection
            .alternate_template_ids
            .into_iter()
            .map(|template_id| ExcludedCandidate {
                template_id,
                reason: ExclusionReason::SingleShotRequested,
            })
            .collect();
        return Ok(CandidateSlate {
            primary_template_id: selection.primary_template_id,
            alternate_template_ids: Vec::new(),
            trace: selection.trace,
            excluded,
        });
    }

    let max = opts.max_candidates.max(1);
    let cutoff_factor = opts.relative_cutoff.clamp(0.0, 1.0);

    let trace = &selection.trace;
    let primary_id = selection.primary_template_id.clone();
    let primary_score = trace
        .scores
        .iter()
        .find(|s| s.template_id == primary_id)
        .map(|s| f64::from(s.composite))
        .unwrap_or(0.0);
    let cutoff = primary_score * cutoff_factor;

    let mut included = vec![primary_id.clone()];
    let mut excluded: Vec<ExcludedCandidate> = Vec::new();

    for alt_id in selection.alternate_template_ids {
        if included.len() >= max {
            excluded.push(ExcludedCandidate {
                template_id: alt_id,
                reason: ExclusionReason::BeyondMaxCandidates,
            });
            continue;
        }
        let alt_score = trace
            .scores
            .iter()
            .find(|s| s.template_id == alt_id)
            .map(|s| f64::from(s.composite))
            .unwrap_or(0.0);
        if alt_score < cutoff {
            excluded.push(ExcludedCandidate {
                template_id: alt_id,
                reason: ExclusionReason::BelowRelativeCutoff,
            });
            continue;
        }
        included.push(alt_id);
    }

    // Primary is always at index 0 of `included`; the rest are accepted alternates.
    let alternate_template_ids = included.split_off(1);
    Ok(CandidateSlate {
        primary_template_id: primary_id,
        alternate_template_ids,
        trace: selection.trace,
        excluded,
    })
}

