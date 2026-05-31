# Operator Control Common Module

Helm owns the operator-control read models that sit above Axiom reports and
app transcripts. This module is not a new truth engine and not a promotion
authority. It is the common control-plane surface that lets Helm show what is
ready, what is missing, and which receipt chain explains the current state.

The first code slice lives in `crates/prio-agent-ops`.

This module should be hostable inside the Runtime Runway app execution container. The
current Helm `application-server` remains a useful reference host, but it should
not become the permanent generic backend for every marquee app. Runtime Runway owns the
server/container substrate; Helm owns the operator-control and governed-job
semantics that the container mounts.

The first host-facing list lives at
`GET /v1/workbench/operator-control/previews`. It returns the current
`JobReadinessPacket` previews, each with the packet's matching
`OperatorLedgerEntry` and the receipt-family catalog Helm can render before
app-specific receipt payloads are standardized. The singular
`GET /v1/workbench/operator-control/preview` endpoint remains as a compatibility
view over the first packet. The current first packet is Tally escrow-release
readiness: it shows buyer authorization, release-condition evidence,
policy-gate evidence, idempotency, custody receipt, and double-release guard
coverage while keeping release authority inside Tally.

The second packet is Quorum adaptive inquiry readiness. It demonstrates the
same list contract on a softer, sensemaking-shaped job: the inquiry question,
participant consent, signal mass, and adaptive probe are present, while
competing hypotheses and skewed role coverage keep the packet blocked. Helm can
show the operator what to inspect next, but it does not declare quorum, approve
synthesis, or convert sensemaking into organizational action.

The next packets widen the contract across the app portfolio:

- Fathom carries temporal-evidence windows and filing disagreement without
  granting narrative authority.
- Warden carries compliance verdicts, shadow-rule diffs, and remediation blocks
  without granting compliance override authority.
- Plumb carries strategy anchors, execution telemetry, and Prism-backed fuzzy
  drift traces from its Organism Suggestor path without granting revision
  authority. A trace may include both linguistic memberships/rule activations
  and a typed defuzzified score for sorting or dashboard summaries.
- Atlas carries integration-candidate evidence, owner-gate gaps, and writeback
  guards without granting repository writeback authority.

## First Shared Types

`JobReadinessPacket` is the generic read model repeated across the app probes:

- package id, truth version, domain hint, job key, and app subject ref;
- adapter receipt id and adapter status;
- optional verifier verdict;
- clause-level evidence readiness;
- optional fuzzy-readiness trace from app-owned Suggestors, including an
  optional typed defuzzified score when the app produces one;
- verifier forbidden actions;
- operator actions;
- `authorizes_domain_action: false`.

The packet is content-addressed. Its id changes when evidence readiness,
forbidden actions, or operator actions change. It rejects construction if a
caller tries to mark the packet as domain-action authority.

`OperatorLedgerEntry` is the deterministic journal entry for operator-control
receipts:

- record kind;
- receipt family;
- source ref;
- package id, truth version, and domain hint;
- payload hash;
- backlink ids;
- summary;
- `authority_effect: none`.

The entry stores hashes and backlinks, not raw app transcripts. It rejects raw
payloads that are not `sha256:` hashes.

## Receipt Families

The Axiom app probes now justify three Helm-owned receipt families:

| Family | Examples |
|---|---|
| Long-running job | approval, decision, plan, execution, action, outcome |
| Temporal evidence | corpus snapshot, evidence window, disagreement, analyst review, narrative claim |
| Content publication | canonical story, claim review, editorial approval, publication boundary |

The common mechanics are shared: deterministic ids, payload hashes, backlinks,
and no authority effect. Domain payload details stay app-local until a real
Helm ledger module hosts them.

## Boundary

Helm may:

- compose Axiom reports, adapter receipts, app subject refs, and operator
  actions into readiness views;
- journal receipt chains with deterministic ids and payload hashes;
- show missing evidence, concerns, approvals, and next actions.

Helm must not:

- promote facts;
- mutate source JTBDs or generated truths;
- select Organism formations;
- run Mosaic specialist cores;
- bypass an app's domain state machine;
- treat a readiness packet or ledger receipt as domain authority.

## Cross-Repo Source

This module follows the Axiom contract probe trail:

- Tally through Folio repeated `ObservationAdapterReceipt`.
- Tally through Folio repeated `JobReadinessPacket`.
- Warden, Triage, Plumb, and Catalyst repeated long-running job receipts.
- Fathom proved temporal-evidence receipts.
- Folio proved content/publication receipts.

That is enough evidence to stop probing and implement the shared Helm
operator-control mechanics.
