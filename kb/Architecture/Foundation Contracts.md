# Foundation Contracts

Helm is the operator-facing product layer. It composes lower layers; it does not
redefine them.

## Use These Surfaces First

| Need | Start here | Add when needed | Avoid by default |
|---|---|---|---|
| Truth authoring and validation | Axiom CLI/library | live LLM validation through Converge provider surfaces | product-local truth parsers or validators |
| Governed execution in-process | `converge-kernel` | `converge-model`, `converge-pack` | `converge-core` |
| Governed execution out-of-process | `converge-client` | `converge-protocol` for typed wire access | runtime internals |
| App server execution container | Runtime Runway app execution container | Helm mounted as operator-control/job module | app-owned HTTP/gRPC/GraphQL servers |
| Capability contracts for chat and routing | `converge-provider` | Manifold adapters for concrete provider implementations | direct vendor HTTP spread across product code |
| Reusable reasoning and planning | `organism-pack`, `organism-runtime` | `organism-domain`, `organism-intelligence`, `organism-notes` | Organism phase crates |
| Application plugin execution | `helm-plugin-runtime` | Axiom-produced WASM/manifests and Converge contracts | embedding plugin machinery in Converge |

## WASM Plugin Boundary

WASM is an artifact format, not a reason for Helm, Axiom, and Converge to blur
ownership. The shared contract is:

| Layer | Owns | Must not own |
|---|---|---|
| Axiom | Predicate extraction, Rust source generation, WASM compilation, manifest generation, hashes, lineage, replay metadata, and proof obligations. | Plugin hosting, tenant runtime policy, authority recompute, fact promotion, or specialist execution. |
| Helm | Plugin install/upgrade/revoke, signing policy, sandbox host policy, quotas, tenant configuration, app-facing lifecycle, and adapters that map sandbox output into Converge-facing contracts. | Truth semantics, Axiom compilation rules, Converge promotion gates, or lower-layer specialist cores. |
| Converge | Kernel execution, proposal promotion, stop reasons, invariant decision semantics, HITL pauses, evidence refs, trace links, and integrity proof. | Wasmtime/Cranelift embedding, application plugin lifecycle, tenant plugin policy, or Axiom parser internals. |

Flow:

```text
JTBD or .truths source
  -> Axiom validates, extracts predicates, and compiles WASM + manifest
  -> Helm installs and runs the artifact in `helm-plugin-runtime`
  -> Helm adapts sandbox output into Converge proposals or invariant verdicts
  -> Converge recomputes authority, checks promotion gates, and records integrity
```

Helm must not treat a successful plugin execution as a promoted fact. Plugin
output is evidence, a proposal, or an invariant verdict until Converge accepts
it through public kernel/pack contracts.

## Extension Locations

Converge v3.9 keeps implementation-heavy capabilities out of the foundation
repository. Helm should resolve those capabilities from extension repositories
and keep the foundation dependencies focused on contracts.

Local own-stack dependencies use the checked-out path as the version source of
truth. Do not add stale `version = ...` gates to Axiom, Converge, Organism,
Atelier, or Mosaic path dependencies inside Helm; release compatibility belongs
to tagged release branches, not day-to-day local composition.

| Capability | Current location | Helm dependency rule |
|---|---|---|
| Policy gates / Cedar PDP | `/Users/kpernyer/dev/reflective/mosaic-extensions/arbiter-policy` | Use Arbiter through Organism formations or explicit policy contracts. Do not build local policy engines. |
| Provider adapters / external tools / storage / vector / search / fetch / feed | `/Users/kpernyer/dev/reflective/mosaic-extensions/manifold-adapters` | Keep Helm coupled to capability contracts, not vendor types. Do not spread direct vendor HTTP across product code. |
| Source-specific connectors | `/Users/kpernyer/dev/reflective/mosaic-extensions/embassy-ports` | Use Embassy when the external source identity is part of the type. Do not hide source semantics behind ad hoc product connectors. |
| Knowledge, recall, memory | `/Users/kpernyer/dev/reflective/mosaic-extensions/mnemos-knowledge` | Use Mnemos for recall and evidence seeding. Do not create product-local vector recall or memory layers. |
| Closed-form analytics and fuzzy inference | `/Users/kpernyer/dev/reflective/mosaic-extensions/prism-analytics` | Use Prism for regression, fuzzy inference, ranking, forecasting, anomaly detection, classification, and feature extraction. |
| Trained models and model-training pipelines | `/Users/kpernyer/dev/reflective/mosaic-extensions/crucible-models` | Use Crucible for trained artifacts, training loops, registry/deployment agents, and classifier Suggestors. |
| Native optimization solvers | `/Users/kpernyer/dev/reflective/mosaic-extensions/ferrox-solvers` | Use Ferrox for scheduling, routing, allocation, feasibility, and solver-backed optimization. Do not reintroduce OR-Tools or local optimizers into Helm. |

## No Local Specialist Cores

This is a hard boundary. Helm may compose specialist capabilities, configure
them, render their results, and decide host policy. Helm must not implement
reusable specialist cores that already belong to Mosaic.

Forbidden local cores include:

- model-training pipelines
- regression engines
- fuzzy-logic engines
- generic ranking frameworks
- forecasting engines
- anomaly detection engines
- optimization, scheduling, routing, allocation, or feasibility solvers
- Cedar or policy evaluation engines
- vector recall, memory, or knowledge retrieval systems
- generic provider adapters
- source-specific connectors

Allowed Helm work:

- product-specific Truth catalog and overlays
- UI surfaces, projections, API endpoints, operator flows
- tenant policy choices, thresholds, credentials, and cost caps
- application plugin lifecycle, signing, quotas, and sandbox host policy
- executable factory registration and capability assembly
- thin glue that maps Helm data into an Organism/Mosaic contract and maps the
  result back into Helm projections

If Helm appears to need a reusable specialist core, first check Organism
formation support and the Mosaic bench. If the capability is missing, record an
upstream gap in the correct owner instead of normalizing a local implementation.

## Formation Rule

For non-trivial automated decisions, Helm should prefer the Organism formation
path over direct local orchestration:

```text
Axiom Truth -> IntentPacket
  -> Organism selects a Formation
  -> Mosaic-backed Suggestors participate in Converge's fixed-point loop
  -> Converge promotes governed facts or stops honestly
  -> Helm renders, approves, redirects, and writes back
```

The selection trace should explain why Arbiter, Manifold, Embassy, Mnemos,
Prism, and Ferrox were used or intentionally omitted.

## What Helm Owns

- operator-facing UX
- application state and projections
- product-specific truth composition
- app-local storage, APIs, and workflows
- operator-control and governed-job modules mounted into the Runtime Runway host
- application plugin runtime and sandbox policy
- composition of Axiom, Organism, and Converge into a usable product

## What Helm Does Not Own

- the convergence loop or promotion gate
- the reusable planning loop
- generic OCR, web, social, or note primitives
- generic provider contracts or adapters
- truth compilation or validation semantics that belong in Axiom
- the Converge runtime or promotion gate
- the generic app server container, deployment host, auth, secrets, telemetry,
  health, or storage/event-log substrate that belongs in Runtime Runway
- regression, fuzzy logic, ranking, forecasting, anomaly detection, or ML
  implementations that belong in Prism
- optimization, scheduling, routing, allocation, or feasibility implementations
  that belong in Ferrox
- policy evaluation, authorization, approval-gate, memory, recall, provider, or
  source-connector implementations that belong in Mosaic

## Practical Rule

If a lower-layer public surface already solves the problem, consume it.

If the need is generic but missing, add it to the correct lower layer.

Only keep it in Helm if it is truly product-specific.

## References

- `~/dev/reflective/bedrock-platform/converge/kb/Architecture/Golden Path Matrix.md`
- [[Architecture/Converge Application]]
- [[Architecture/Naming Migration Map]]
