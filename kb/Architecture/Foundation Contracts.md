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
