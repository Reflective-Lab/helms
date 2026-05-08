# HITL Admission Gate

## Status

Decision adopted 2026-05-08. Implementation pending; truth-catalog's
`admit_truth_intent` and the four migrated executors do not yet enforce the
gate. This document records where the gate belongs when it lands.

## Decision

The Human-in-the-Loop gate for irreversible intents sits at the **truth-catalog
admission boundary** — i.e. inside or immediately above
`truth_catalog::admission::admit_truth_intent`, before
`organism_runtime::Runtime::admit_intent` is called.

The gate does NOT live inside organism's `Runtime::admit_intent`.

## Why pre-admission, not single-ingress at the kernel

The 2026-05-07 organism-1.7.0 → 1.8.0 handoff offered two viable shapes for
the gate:

1. **Single-ingress** wrapping `Runtime::admit_intent` — every admission
   passes through one HITL check.
2. **Pre-admission** at the truth-catalog boundary — the gate fires when an
   IntentPacket is staged for execution, before kernel admission.

We chose (2) because:

- **Keeps the kernel pure.** `Runtime::admit_intent` is mechanism — typed
  validation, receipt issuance, ContextState mutation. HITL is policy. Mixing
  them would push helms-specific approval flows into organism, growing the
  upstream surface.
- **Truth-catalog is the natural choke point.** Every IntentPacket helms
  executes today flows through `compile_intent_for_truth` →
  `admit_truth_intent`. Putting the gate one level up means one place to
  enforce, not one-per-executor.
- **Cheap rejection.** Irreversible intents that lack approval get rejected
  before any kernel admission cost (validation, ContextState mutation, receipt
  issuance).

## Rules

These follow from the decision and bind future implementation.

1. **The gate lives in helms.** Either `truth-catalog` or a thin wrapper in
   `workbench-backend`. Not in organism, axiom, or converge.

2. **Every `Reversibility::Irreversible` intent requires HITL approval before
   admission.** No exceptions, no global override. The check is per-call.

3. **Tournaments are forbidden for irreversible intents.** Even with a cached
   HITL approval. Racing candidates means committing on the winner before the
   human has reviewed the *specific* execution path. One and only one execution
   path for irreversible intents — single-shot, approved, run.

4. **Approval is per-execution, not cached globally.** A truth approved
   yesterday is not approved today. The approval is bound to a specific intent
   instance (`IntentPacket::id`), not to the truth definition.

5. **The gate's failure mode is rejection, not bypass.** If the approval store
   is unreachable, reject the admission. Never default to "approved" on
   infrastructure failure.

## Implementation TODOs

Open questions when this gets wired:

- **Where exactly does the gate fire?** Two options:
  (a) Inside `admit_truth_intent` between `compile_intent_for_truth` and
  `Runtime::admit_intent` — single function, single behavior.
  (b) In a wrapper `admit_truth_intent_with_hitl` that callers opt into —
  bypassable, more explicit.
  Pick (a) unless there's a concrete reason to allow bypass.

- **What counts as human approval?** workbench-backend already has `Approval`
  types and an approval store. The gate should consume those, not invent a new
  shape. Spec out the contract: which `Approval` proves which intent's HITL.

- **How does rejection surface to the caller?** A new error variant in
  `AdmitTruthError` (e.g. `RequiresHumanApproval { intent_id, reason }`).
  Truth executors translate it to a meaningful status — likely
  `Status::failed_precondition` with an actionable message.

- **Detection of `Reversibility::Irreversible`.** Today the IntentPacket carries
  `reversibility: Reversibility`. Axiom's compile_intent defaults to
  `Reversible`; a truth opts into `Irreversible` by adding
  `"reversibility: irreversible"` as a constraint in its `.truths` source. The
  gate reads `intent.reversibility` post-compile.

- **Auditability.** Every HITL gate decision (approved, rejected, denied for
  missing approval) should produce a `UserExperienceEvent` so the audit ledger
  records the human-in-the-loop step. The three new variants from the
  2026-05-06 Converge handoff are the natural carriers.

## Cross-references

- Organism handoff (2026-05-07) — original "two viable shapes" framing.
- `truth-catalog/src/admission.rs` — where the gate will likely live.
- `application-server/src/truth_runtime/qualify_inbound_lead.rs` — the
  canonical executor pattern that the gate has to flow through cleanly.
- `kb/Architecture/Capability Binding.md` — adjacent: what an admitted intent
  resolves to.
