# Operator Control Common Module

Helm owns the operator-control read models that sit above Axiom reports and
app transcripts. This module is not a new truth engine and not a promotion
authority. It is the common control-plane surface that lets Helm show what is
ready, what is missing, and which receipt chain explains the current state.

The first code slice lives in `crates/prio-agent-ops`.

The first host-facing preview lives at
`GET /v1/workbench/operator-control/preview`. It returns a
`JobReadinessPacket`, the packet's matching `OperatorLedgerEntry`, and the
receipt-family catalog Helm can render before app-specific receipt payloads are
standardized.

## First Shared Types

`JobReadinessPacket` is the generic read model repeated across the app probes:

- package id, truth version, domain hint, job key, and app subject ref;
- adapter receipt id and adapter status;
- optional verifier verdict;
- clause-level evidence readiness;
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
