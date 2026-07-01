# Design: Mobile SSE Resilience — Gap #5
**Date:** 2026-07-01
**Status:** Approved for implementation
**Crates touched:** `helm-session-contracts`, `helm-session-host`
**Crates not touched:** `helm-client`, `helm-coordination`, `runway-app-host`

---

## Problem

The `EventHub` already provides cursor-based replay: a client that reconnects
with its last cursor receives all events it missed. This handles
Informational and Advisory pushes correctly — they are fire-and-forget and
cursor replay is sufficient.

Disruptive and Preemptive pushes require more. The server needs to know the
client did not just *receive* the SSE bytes but *acted* on them — spawned a
formation or paused the active loop. Without that signal:

- A mobile client that goes to background after receiving a Preemptive push
  but before `SpawnFormation` executes will reconnect with a cursor that
  shows the event as delivered, so cursor replay skips it. The push is
  silently lost.
- The server has no way to distinguish "client received and acted" from
  "client received and crashed."
- No completion signal exists, so the server cannot correlate which group
  findings a participant actually resolved.

---

## Out of Scope

- Persistent delivery state (follows gap #2 — durable coordination state).
  The `DeliveryTable` is in-memory; it survives reconnects within a process
  lifetime but not restarts. Gap #2 will migrate it to `DocumentStore`.
- Server-side retry loops (Approach B). Replay happens at subscribe time
  only.
- Escalation count on re-delivery (Approach C extension). Can be added as a
  field later without breaking the contract.
- Changes to `ClientHelm` logic — `handle_push()`, `SeverityRouter`, and
  `LoopRegistry` are unchanged.
- Temperature submission endpoint consuming `triggered_by` (server side). The
  `triggered_by` field is structurally complete on the client and wire. The server
  admission endpoint that processes temperature signals (not yet implemented in
  `helm-session-host`) will call `apply_completion_ack(produced_output: true)` when
  `triggered_by` is present. Until that endpoint exists, completion acks for
  formations that produce temperature output must use `POST /ack/completion` directly.

---

## Participant Identity

### `ParticipantId`

A new newtype in `helm-session-contracts`. Stable for the life of a device
installation.

```rust
// helm-session-contracts/src/participant.rs
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParticipantId(String);

impl ParticipantId {
    pub fn from_string(s: impl Into<String>) -> Self { Self(s.into()) }
    pub fn as_str(&self) -> &str { &self.0 }
}
```

**Derivation rule (native layer, not server):**
```
participant_id = sha256_hex(firebase_uid + ":" + device_install_id)
```

- `firebase_uid` — stable for the life of the Firebase account.
- `device_install_id` — a UUID written once on first app launch to
  `UserDefaults` (iOS) / `SharedPreferences` (Android), never regenerated.
- Concatenated and hashed so neither component is exposed on the wire.

The server validates that the asserted `ParticipantId` is a member of the
session but never generates or stores the raw components.

---

## Delivery Tracking

### Which events are tracked

Only **Disruptive** and **Preemptive** `SessionPush` events. Informational
and Advisory are fire-and-forget; cursor replay is sufficient for them.
`GatedDecision` events are not tracked here — gate state is tracked by the
existing `DecisionLedger`.

### `DeliveryRecord`

```rust
pub struct DeliveryRecord {
    /// EventHub sequence number when the push was published.
    pub delivered_at_version: u64,
    /// When the client acked receipt and confirmed it is acting on the push.
    pub delivery_acked_at_ms: Option<u64>,
    /// When the client acked formation completion triggered by this push.
    pub completed_acked_at_ms: Option<u64>,
    /// Whether the completed formation produced a temperature signal.
    /// None until completed_acked_at_ms is set.
    pub produced_output: Option<bool>,
}
```

### `DeliveryTable`

Owned by `SharedSessionStore`, keyed by
`(session_id, ParticipantId, FindingId)`.

```rust
pub struct DeliveryTable {
    records: HashMap<(String, ParticipantId, FindingId), DeliveryRecord>,
}
```

Methods:
- `record_delivery(session_id, participant_id, finding_id, version)` — called
  by `publish_push` for Disruptive/Preemptive only.
- `apply_delivery_ack(session_id, participant_id, finding_id, now_ms)` — sets
  `delivery_acked_at_ms`. No-ops if already set (idempotent).
- `apply_completion_ack(session_id, participant_id, finding_id, produced_output, now_ms)`
  — sets `completed_acked_at_ms`. No-ops if already set (idempotent).
- `unacked_for_participant(session_id, participant_id) -> Vec<(FindingId, u64)>`
  — returns `(finding_id, delivered_at_version)` for all records with
  `delivery_acked_at_ms = None`. Used at subscribe time for pull replay.

`SharedSessionStore` exposes `DeliveryTable` access through its existing
mutex guard pattern so all mutations are serialised.

---

## Ack Paths

### Delivery ack

The native layer calls this **immediately** after `ClientHelm::handle_push()`
returns `SpawnFormation` or `PauseAndInject`. It does not wait for the
formation to complete.

```
POST /session/{session_id}/ack/delivery
Content-Type: application/json

{
  "participant_id": "<ParticipantId>",
  "finding_id": "<FindingId>"
}
```

Response: `204 No Content`. Idempotent — duplicate acks are silently accepted.

The endpoint is mounted on `helm-session-host`'s HTTP router. `helm-session-host`
validates that the `finding_id` has a tracked record for this session before
writing; unknown finding IDs return `404`.

### Completion ack — with temperature signal

`triggered_by: Option<FindingId>` is added to `PendingSubmission` in
`helm-session-contracts`. When the native layer drains submissions and POSTs
a temperature signal, it includes the `FindingId` of the push that triggered
the formation if one exists. The existing temperature submission endpoint reads
this field and calls `apply_completion_ack(produced_output: true)`.

No new HTTP endpoint. No change to `ClientHelm`.

### Completion ack — without temperature signal

When `formation_completed()` produces no temperature signal (output has no
`temperature` field), the native layer posts a standalone completion ack:

```
POST /session/{session_id}/ack/completion
Content-Type: application/json

{
  "participant_id": "<ParticipantId>",
  "finding_id": "<FindingId>",
  "produced_output": false
}
```

Response: `204 No Content`. Idempotent.

---

## Pull Replay at Subscribe Time

The SSE subscribe request gains two optional query parameters:

```
GET /session/{session_id}/stream?participant_id=<id>&cursor=<seq>
```

If `participant_id` is supplied, before going live the server:

1. Fetches `unacked_for_participant(session_id, participant_id)`.
2. Filters to records where `delivered_at_version <= cursor` (i.e., the
   cursor would skip them, but they were never acked).
3. Re-publishes each matching `SessionPush` onto the hub. The re-published
   events get fresh `event_id` and new sequence numbers and flow through the
   live stream the subscriber is about to open. The `finding_id` and payload
   are identical to the original. `DeliveryRecord.delivered_at_version` always
   holds the **original** publish sequence — not the re-publish sequence. This
   means that if the client receives the re-published event but crashes before
   sending the delivery ack, the next reconnect will still find
   `delivered_at_version <= new_cursor` and re-publish again. The loop
   continues until the delivery ack lands.
4. The subscriber then opens live from its cursor position, receiving both
   the re-published unacked events and any new events.

Events after the cursor are replayed by the existing cursor machinery
unchanged.

If `participant_id` is absent, subscribe behaves exactly as today — no change
for non-mobile clients or tests that don't assert participant identity.

---

## Changes by Crate

### `helm-session-contracts`

| Change | Detail |
|---|---|
| New file `src/participant.rs` | `ParticipantId` newtype |
| New file `src/ack.rs` | `DeliveryAck`, `CompletionAck` wire structs for the two POST bodies |
| Modified `src/lib.rs` | Re-export `ParticipantId`, `DeliveryAck`, `CompletionAck` |
| Modified temperature submission type | Add `triggered_by: Option<FindingId>` to `PendingSubmission` |

### `helm-session-host`

| Change | Detail |
|---|---|
| New file `src/delivery.rs` | `DeliveryRecord`, `DeliveryTable` |
| Modified `src/store.rs` | Embed `DeliveryTable`; expose `record_delivery`, `apply_delivery_ack`, `apply_completion_ack`, `unacked_for_participant` through `SharedSessionStore` |
| Modified `src/service.rs` | `publish_push` calls `record_delivery` for Disruptive/Preemptive urgencies |
| Modified `src/http.rs` | Add `POST /ack/delivery`, `POST /ack/completion`; add `participant_id` query param to SSE subscribe; pull-replay logic |
| Modified `src/module.rs` | Mount new ack routes |

### `helm-client`

No changes.

---

## Error Handling

| Case | Behaviour |
|---|---|
| Ack for unknown `finding_id` | `404 Not Found` — client should not retry |
| Ack for Informational/Advisory finding | `404 Not Found` — only Disruptive/Preemptive are tracked |
| Duplicate delivery ack | `204` — idempotent, first write wins |
| Duplicate completion ack | `204` — idempotent, first write wins |
| `participant_id` not a session member | `403 Forbidden` |
| Pull replay finds no unacked events | Proceeds to live stream immediately, no error |

---

## Testing

- `delivery_table_records_disruptive_and_preemptive_only` — Informational and
  Advisory pushes produce no `DeliveryRecord`.
- `delivery_ack_marks_record_and_is_idempotent` — double-ack is a no-op.
- `completion_ack_with_and_without_output` — both paths write correctly.
- `pull_replay_re_publishes_unacked_findings_at_subscribe` — subscriber
  receives re-published finding before live events.
- `pull_replay_skips_findings_after_cursor` — findings the client has not yet
  seen at all are not double-replayed.
- `pull_replay_no_op_when_participant_id_absent` — existing subscribe
  behaviour is unchanged.
- `temperature_submission_with_triggered_by_acks_completion` — posting a
  temperature signal with `triggered_by` set closes the completion record.
