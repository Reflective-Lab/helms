# Operator Coordination

Helm's operator surface is headless: it ships as mountable `HelmModule`s hosted
by Runtime Runway, not as a desktop-only app. `helm-coordination`
(`crates/helm-coordination`) makes that headless surface usable by **several
operators at once** under an **optimistic** model.

Optimistic means: any operator with authority may act, presence and claims are
advisory hints rather than locks, and the engine detects and resolves conflicting
or duplicate decisions *after the fact* instead of serializing operators behind a
pessimistic lease.

## Why this module exists

Before coordination, the headless surface had three gaps:

- Gate approvals ran as `Actor::system()` and were an unattributed
  first-caller-wins `HashMap<String, JobGateWaiter>` in
  `helm-governed-jobs/src/job_stream.rs`.
- Identity was self-declared (`body.actor: Option<String>`, unverified) and
  `EventEnvelope.actor` was always `None`.
- `Workspace` / `WorkspaceMember` / `Role` existed in `application-kernel` but
  were unused for coordination.

`helm-coordination` adds verified-at-the-seam identity, sessions, presence with
advisory soft-claims, and a conflict-safe decision ledger, then rewires the
governed-jobs gate flow to run through it and emit attributed events.

## Boundary

This respects the split documented in
[Helm Surface Model](Helm%20Surface%20Model.md) and
[Runway Execution Container Boundary](Runway%20Execution%20Container%20Boundary.md):

- **Runtime Runway authenticates.** Helm does not.
- **Helm consumes identity** through the `PrincipalResolver` seam and owns
  session, presence, and approval semantics.
- Coordination is **non-authority** over domain and commercial state
  (`HP-READINESS-IS-NON-AUTHORITY`). A soft-claim never grants rights; the
  decision ledger only attributes and de-duplicates, it does not promote.

## Core types

- `OperatorPrincipal { actor_id, display_name, kind, workspace_id }` — an
  identified operator scoped to a workspace; projects to `application_kernel::Actor`.
- `PrincipalResolver` — the identity seam. Ships `RequestActorResolver` (trusts
  the self-declared request actor, the current platform behavior). A
  `RunwayClaimsResolver` backed by verified `HostContext` claims is the
  documented follow-up; swapping it changes no callers.
- `SessionRegistry` — `open` / `heartbeat` / `close`, workspace-scoped, with a
  heartbeat-lease sweep for stale sessions (default 300s).
- `PresenceRegistry` — who is focused on which `SubjectRef`; advisory
  `claim` / `release`. No locking.
- `DecisionLedger` — append-once-per-gate `ref_id`. First decision is
  **recorded**; an identical later decision is **idempotent** (returns the
  original receipt, no second side-effect); a divergent decision is a
  **conflict** (rejected, `409`, original preserved).
- `AuthorityResolver` — `can_decide(principal, subject)`. Ships
  `PermissiveAuthority`; a `RoleAuthority` backed by the kernel
  `Role`/`WorkspaceMember` scaffolding is the documented follow-up.

## Routes (`/v1/coordination/`)

- `POST /sessions`, `POST /sessions/{id}/heartbeat`, `DELETE /sessions/{id}`,
  `GET /sessions?workspace=`
- `GET /presence?workspace=&subject_kind=&subject_id=`,
  `POST /presence/focus`, `POST /presence/claim`, `POST /presence/release`
- `GET /stream?workspace=` — SSE over the shared event hub, scoped by workspace,
  replaying coordination + attributed job/gate events
- `POST /gates/{ref_id}/decision` — the new front door for gate approvals:
  authority-checked, deduped, optimistic

## Events on the shared hub

Coordination reuses `runway_app_host::{EventEnvelope, EventHubHandle}` and stamps
`EventEnvelope.actor`. Event types: `session.opened`, `session.closed`,
`presence.joined`, `presence.left`, `presence.focus_changed`, `claim.acquired`,
`claim.released`, `decision.recorded`, `decision.conflict`, `decision.denied`.
Each carries `workspace_id` and a `principal` summary in its payload so the
coordination stream can scope by workspace.

## Integration with helm-governed-jobs

`POST /gates/{ref_id}/decision` is the gatekeeper. It (1) resolves the principal,
(2) checks the `AuthorityResolver`, (3) records in the `DecisionLedger`
(idempotent-or-conflict), and (4) **only on a fresh accepted decision** calls
`JobStreamState::signal_gate(ref_id, decision)`. Idempotent repeats and conflicts
never re-signal the waiter.

Attribution: the governed-jobs `Publisher` now stamps `EventEnvelope.actor` from
the run's initiating principal, threaded through `run_job_task` via
`JobRunTask.initiator` (falling back to a system actor for automation-initiated
runs). The **approver** attribution — who decided a gate — is the authoritative
`decision.recorded` event emitted by coordination; `gate.approved` in the job
stream is the run-time echo attributed to the run initiator.

## Module wiring

`CoordinationModule` implements `HelmModule` (`module_id = "helm.coordination"`)
and the Helm readiness contract. It is `Live` only when wired to a
`JobStreamState` (so it can drive real gate decisions); otherwise it is a default
shell whose presence/session routes still work. Apps mount it alongside
`GovernedJobsModule`, sharing the same `EventHubHandle` and sequence counter so
coordination and job events ride one globally-monotonic stream.

## Deferred follow-ups

Called out, not built in the first increment:

- `RunwayClaimsResolver` — needs Runtime Runway to expose verified claims via
  `HostContext`.
- `RoleAuthority` — backed by the kernel `Role`/`WorkspaceMember` scaffolding.
- Durable (non-in-memory) coordination state — sessions, presence, and the
  ledger are in-memory for this slice.

## Related

- [Helm Surface Model](Helm%20Surface%20Model.md)
- [Runway Execution Container Boundary](Runway%20Execution%20Container%20Boundary.md)
- [HITL Admission Gate](HITL%20Admission%20Gate.md)
- [Operator Control Common Module](Operator%20Control%20Common%20Module.md)
