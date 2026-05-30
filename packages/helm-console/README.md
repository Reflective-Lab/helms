# @reflective/helm-console

Shared receipt-backed operator-console primitives for Marquee apps.

The package is the Helm home for the pattern first proven in Quorum M4:

```text
going-in spec -> command -> durable event/receipt -> live projection
  -> derived aid -> explicit transition -> artifact/proof
```

## What It Owns

- `ConsoleAdapter` descriptors for app-specific route bindings.
- A small `HelmConsoleClient` for route-prefix, bearer auth, commands, reads,
  and fetch-SSE streams.
- Reusable Svelte components for command cards, event timelines, connection
  bars, and proof artifact panels.
- Starter adapters for Quorum, Atlas, and Warden.

It does not own app meaning. Apps still own nouns, command payloads, decision
rules, evidence schema, and custom product views.

## First App Profiles

| App | Profile | Purpose |
|---|---|---|
| Quorum | `quorumConsoleAdapter` | live inquiry controls, events, aids, process receipt |
| Atlas | `atlasConsoleAdapter` | acquisition readiness room, Quorum uncertainty, budget checks |
| Warden | `wardenConsoleAdapter` | rule/gate reads, shadow-analysis boundary, audit-pack proof |

## Usage

```ts
import {
  HelmConsoleClient,
  quorumConsoleAdapter,
} from '@reflective/helm-console'

const client = new HelmConsoleClient(quorumConsoleAdapter, {
  baseUrl: '/quorum',
  bearerToken: 'dev',
})

const view = await client.read(quorumConsoleAdapter.run.load, {
  id: inquiryId,
})
```

```svelte
<script lang="ts">
  import ReceiptBackedConsole from '@reflective/helm-console/ReceiptBackedConsole.svelte'
  import { quorumConsoleAdapter } from '@reflective/helm-console/profiles'
</script>

<ReceiptBackedConsole adapter={quorumConsoleAdapter} />
```

## Review Rule

Every mutating control must map to an app API command with declared authority:

- `chain-recorded`
- `receipt-bearing`
- `derived-recompute`

No component in this package should create hidden process state. Svelte local
state is for selection, forms, and connection preferences only.
