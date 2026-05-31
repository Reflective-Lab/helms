# EXP-002 code preservation

This directory preserves the Rust source that backed EXP-002 ("Code generation as a convergence step"). See `experiments/EXP-002.md` for the full hypothesis, method, and outcome (Confirmed, 2026-04-19).

## What's here

- `generate_data_transformer.rs` — the original 482-line test that exercised the three-suggestor pattern (CodegenGapSuggestor → CodeGenSuggestor → CodeVerifierSuggestor) end-to-end with stubbed LLM output. Confirmed all four falsification criteria as NOT falsified.

## Provenance

Originally lived at `crates/application-server/src/truth_runtime/generate_data_transformer.rs` under a `#[cfg(test)]` gate. Deleted with the rest of `application-server` during Phase 9 of the Runtime Runway/Helm app-host boundary refactor (commit `af4cd23`, 2026-05-30). Recovered from `git show a63811c^:crates/application-server/src/truth_runtime/generate_data_transformer.rs`.

## How to revive

The original test compiled inside `application-server` and used:

- `converge-pack` for `Suggestor`, `Context`, `AgentEffect`, `ContextKey`
- `sha2` for content hashing
- `async-trait` for the `Suggestor` trait
- Helm's `application-storage` types for the test harness

To run again, create a new test crate (e.g. `experiments/code/EXP-002/Cargo.toml`) declaring the same deps as workspace path-deps, with this file as `src/lib.rs` or under `tests/`. Match the workspace's converge version (currently `3.9.x`).

The Gherkin spec the test references is preserved at `truths/jobs/generate_data_transformer.feature` and is still part of the helms workspace.

## Why this is artifact-only

EXP-002's outcome is documented as Confirmed. The test demonstrated the pattern works; subsequent work moved toward implementing the `WasmSuggestor` adapter that would let verified Wasm modules execute inside follow-on convergence loops. The 482-line file is the proof-of-record for the convergence-step thesis, not active runtime code. Reviving it makes sense if someone wants to:

- Re-run the proof against a newer `converge-pack` version to verify the pattern still holds
- Extend it from stubbed LLM output to a real LLM-driven CodeGenSuggestor
- Use it as the template for a sibling experiment (EXP-003 etc.)

If you're picking it up for any of those, expect 30–60 minutes of harness re-wiring.
