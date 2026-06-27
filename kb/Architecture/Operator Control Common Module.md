# Operator Control Common Module

Helm owns the operator-control read models that sit above Axiom reports and
app transcripts. This module is not a new truth engine and not a promotion
authority. It is the common control-plane surface that lets Helm show what is
ready, what is missing, and which receipt chain explains the current state.

The public app-facing import surface now lives in `crates/helm-operator-control`.
That crate re-exports the packet, ledger, receipt-family, hash, and ledger-entry
helpers that downstream apps need. `crates/prio-agent-ops` is still the legacy
implementation crate for this slice and should not be treated as the marquee-app
contract.

This module should be hostable inside the Runtime Runway app execution container. The
current Helm `application-server` remains a useful reference host, but it should
not become the permanent generic backend for every marquee app. Runtime Runway owns the
server/container substrate; Helm owns the operator-control and governed-job
semantics that the container mounts.

## Mount Liveness Contract

Helm modules expose a small liveness vocabulary and the
`HelmModuleReadiness` trait through `crates/helm-module-contracts`:

- `shell-default`: routes may exist, but the module is serving default, static,
  demo, or otherwise incomplete state;
- `live`: the module is backed by live app evidence or executable truth wiring.

This vocabulary exists so Runtime Runway's manifest verifier can reject a
manifest that presents a default Helm shell as live. Route reachability is not
enough.

RR D1 check 2 reads Runway's `HelmModule::module_state()` on mounted modules.
Helm derives that value from `HelmModuleReadiness::module_state()` and exposes
the bridged `Shell` / `Live` value through the Runway mount trait. A mounted
router is not proof of live module state.

Current module behavior:

- `OperatorControlModule::new(...)`, `with_store(...)`, and `with_truths(...)`
  report `shell-default` until the caller explicitly supplies complete live
  readiness evidence.
- `OperatorControlModule::with_live_readiness_evidence(...)` reports `live`
  only when the evidence marker contains process receipt, integrity proof,
  adapter receipt, and Axiom report.
- `OperatorControlModule::with_live_readiness_feed(...)` is the live handoff
  contract. The feed is app-owned and returns already-derived
  `JobReadinessPacket` / `OperatorLedgerEntry` snapshots plus the same evidence
  completeness marker. The module reports `live` only when the feed evidence is
  complete and the feed returns at least one snapshot.
- `GovernedJobsModule::new()` reports `shell-default` because no truth bodies
  are registered.
- `GovernedJobsModule::with_state(...)` reports `live` only when its
  `TruthExecutionModule` registry has at least one registered truth body.

`HelmModuleStatus` is the host-facing shape RR can inspect:

```json
{
  "module_id": "helm.operator-control",
  "state": "shell-default",
  "reason": "default/static operator-control shell; live app evidence is not fully wired",
  "registered_truths": 0,
  "live_requirements": [
    "process_receipt",
    "integrity_proof",
    "adapter_receipt",
    "axiom_report"
  ],
  "missing_live_requirements": [
    "process_receipt",
    "integrity_proof",
    "adapter_receipt",
    "axiom_report"
  ]
}
```

For `live` modules, `state` serializes as `"live"` and
`missing_live_requirements` is omitted when empty. Tests in
`helm-module-contracts`, `helm-operator-control`, and `helm-governed-jobs`
pin this shape.

For app mounts, `mount_kind: "planned"` remains correct until the app wires the
live operator-control evidence feed and Runtime Runway's verifier checks the
module status. Helm readiness remains advisory even when the module reports
`live`; it never authorizes domain action, commerce action, deployment, claim
refresh, or app writeback.

The live feed boundary is packet-based, not domain-state-based. Apps own process
receipts, integrity proofs, adapter receipts, Axiom reports, subject refs, and
packet construction. Helm consumes the packet/ledger snapshots and renders
operator readiness. Helm must not read raw app transcripts, entitlement state,
deployment state, or app write authority through this feed.

The first host-facing list lives at
`GET /v1/workbench/operator-control/previews`. It returns injected live
`JobReadinessPacket` previews, each with the packet's matching
`OperatorLedgerEntry` and the receipt-family catalog Helm can render before
app-specific receipt payloads are standardized. Without an injected live feed it
returns an empty list. The singular
`GET /v1/workbench/operator-control/preview` endpoint remains as a compatibility
view over the first live packet and returns an operator-control error when no
live packet is supplied. Helm no longer synthesizes a static app portfolio.

Import rule: marquee apps should depend on `helm-operator-control` for
`JobReadinessPacket`, `OperatorLedgerEntry`, `ReceiptFamily`, and the
`job_readiness_packet_*` helpers. New app code importing `prio-agent-ops`
directly for operator-control contracts is transitional boundary debt and should
be rejected during review.

App examples and portfolios belong in app repos, showcases, or arena tests.
Helm's invariant is the contract shape: packet plus ledger entries plus receipt
family metadata, with `authorizes_domain_action: false`.

## First Shared Types

`JobReadinessPacket` is the generic read model repeated across the app probes:

- package id, truth version, domain hint, job key, and app `SubjectRef`;
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

The app `SubjectRef` is the same Converge subject identity carried by
Converge-backed proposals and facts. Helm may display it and use it for packet
correlation, audit backlinks, and replay lookup. Helm must not infer approval,
writeback, readiness, or domain authority from the ref alone; those remain in
the readiness clauses, receipts, app state machine, and Converge promotion
outcome.

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

- compose Axiom reports, adapter receipts, app `SubjectRef`s, and operator
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

- app probes repeated `ObservationAdapterReceipt`;
- app probes repeated `JobReadinessPacket`;
- long-running app probes repeated governed job receipts;
- temporal-evidence probes proved temporal receipt handling;
- content/publication probes proved publication receipt handling.

That is enough evidence to stop probing and implement the shared Helm
operator-control mechanics.
