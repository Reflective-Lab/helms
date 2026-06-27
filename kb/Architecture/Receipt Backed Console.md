---
source: codex
type: architecture
---

# Receipt Backed Console

Helm owns the shared operator-console layer for apps. The first package is
`packages/helm-console`, which carries the generic receipt-backed console
pattern without shipping app-specific profiles.

## Boundary

Helm owns:

- console shell and four-pane UX pattern;
- command cards and authority labels;
- event timeline and realtime client glue;
- receipt and proof artifact panels;
- `ConsoleAdapter` descriptor schema;
- no-UI-only-process-state review convention.

Runway owns auth, route prefixing, storage/event-log, telemetry, and
deployment. Apps own domain nouns, payloads, decision rules, evidence meaning,
and custom panels. Converge owns stream semantics and replay. Mosaic
specialists own derived aids.

## Four Panes

Every serious app console should expose:

1. **Run / Controls** — commands that map to real app APIs.
2. **Events / Stream** — live and replayable event timeline.
3. **Aids / Intelligence** — analytic, memory, solver, policy, provider, and
   prediction outputs framed as aids.
4. **Artifacts / Proof** — receipts, final artifacts, integrity proofs, and
   resolver-backed citations.

The app can rename the panes, but the responsibilities stay stable.

## First Package

`@reflective/helm-console` exports:

- `ConsoleAdapter`, `ConsoleCommandDescriptor`, and related types;
- `HelmConsoleClient` for route-prefix, bearer, command, read, and fetch-SSE
  handling;
- `ReceiptBackedConsole.svelte`, `CommandCard.svelte`,
  `EventTimeline.svelte`, `ConnectionBar.svelte`, and
  `ProofArtifactPanel.svelte`.

## Why This Belongs In Helm

The pattern is operator-facing and app-facing. It is not a lower-level server
primitive. Runway should not know how an operator console presents a receipt.
Apps should not copy command/timeline/proof mechanics. Helm is the
trust-transfer surface where reusable operator UX belongs, while concrete
`ConsoleAdapter` values stay in app, showcase, or test configuration.

## Review Rules

- Mutating controls must declare authority:
  `chain-recorded`, `receipt-bearing`, or `derived-recompute`.
- Derived aids cannot silently execute commands or become hidden authority.
- Missing live credentials must fail honestly.
- Proof artifacts must preserve negative evidence and unresolved work.
- Local component state may store drafts, selected ids, and connection
  preferences only.

## Extraction Rule

Do not overfit to any one app. Keep app-specific panels and route profiles in
apps until at least two apps use the same component shape. Promote only stable
mechanics into `helm-console`.
