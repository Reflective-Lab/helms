# Foundation Contracts

Helm is the operator-facing product layer. It composes lower layers; it does not
redefine them.

## Use These Surfaces First

| Need | Start here | Add when needed | Avoid by default |
|---|---|---|---|
| Truth authoring and validation | Axiom CLI/library | live LLM validation through Converge provider surfaces | product-local truth parsers or validators |
| Governed execution in-process | `converge-kernel` | `converge-model`, `converge-pack` | `converge-core` |
| Governed execution out-of-process | `converge-client` | `converge-protocol` for typed wire access | runtime internals |
| Capability contracts for chat and routing | `converge-provider-api` | `converge-provider` for ready-made adapters | direct vendor HTTP spread across product code |
| Reusable reasoning and planning | `organism-pack`, `organism-runtime` | `organism-domain`, `organism-intelligence`, `organism-notes` | Organism phase crates |

## Extension Locations

Converge v3.8 extracts implementation-heavy capabilities out of the foundation
repository. Helm should resolve those capabilities from extension repositories
and keep the foundation dependencies focused on contracts.

| Capability | Current location | Helm dependency rule |
|---|---|---|
| Policy gates / Cedar PDP | `/Users/kpernyer/dev/extensions/arbiter` | Import through the `arbiter` package; alias to `converge-policy` only for transitional code that still uses `converge_policy`. |
| Native optimization solvers | `/Users/kpernyer/dev/extensions/ferrox` | Treat as an optional solver extension; do not reintroduce OR-Tools into Helm or Converge foundation crates. |
| Provider adapters / external tools | `/Users/kpernyer/dev/extensions/manifold` | Planned home for concrete adapters; keep Helm coupled to capability contracts, not vendor types. |
| Knowledge and recall | `/Users/kpernyer/dev/extensions/mnemos` | Existing `converge_knowledge` imports may be satisfied by aliasing the `mnemos` package during migration. |
| Analytics and ML pipelines | `/Users/kpernyer/dev/extensions/prism` | Existing `converge_analytics` imports may be satisfied by aliasing the `prism` package during migration. |

## What Helm Owns

- operator-facing UX
- application state and projections
- product-specific truth composition
- app-local storage, APIs, and workflows
- composition of Axiom, Organism, and Converge into a usable product

## What Helm Does Not Own

- the convergence loop or promotion gate
- the reusable planning loop
- generic OCR, web, social, or note primitives
- generic provider contracts or adapters
- truth compilation or validation semantics that belong in Axiom

## Practical Rule

If a lower-layer public surface already solves the problem, consume it.

If the need is generic but missing, add it to the correct lower layer.

Only keep it in Helm if it is truly product-specific.

## References

- `~/dev/work/converge/kb/Architecture/Golden Path Matrix.md`
- [[Architecture/Converge Application]]
- [[Architecture/Naming Migration Map]]
