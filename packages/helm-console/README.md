# @reflective/helm-console

Shared receipt-backed operator-console primitives for app-owned adapters.

The package is the Helm home for this operator-console pattern:

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

It does not own app meaning or ship app profiles. Apps, showcases, and tests own
nouns, command payloads, decision rules, evidence schema, custom product views,
and concrete `ConsoleAdapter` values.

## Usage

```ts
import {
  HelmConsoleClient,
  type ConsoleAdapter,
} from '@reflective/helm-console'

const adapter: ConsoleAdapter = {
  appId: 'example',
  displayName: 'Example',
  routePrefix: '/example',
  subjectRefKind: 'example://',
  nouns: {
    run: 'run',
    spec: 'spec',
    event: 'event',
    artifact: 'artifact',
  },
  connection: { defaultBaseUrl: '/example' },
  run: {
    load: { id: 'example.load', label: 'Load run', path: '/runs/{id}' },
  },
  controls: [],
  aids: [],
  artifacts: [],
  safety: [],
}

const client = new HelmConsoleClient(adapter, {
  baseUrl: '/example',
  bearerToken: 'dev',
})

const view = await client.read(adapter.run.load, {
  id: runId,
})
```

```svelte
<script lang="ts">
  import ReceiptBackedConsole from '@reflective/helm-console/ReceiptBackedConsole.svelte'
  import { adapter } from './console-adapter'
</script>

<ReceiptBackedConsole {adapter} />
```

## Review Rule

Every mutating control must map to an app API command with declared authority:

- `chain-recorded`
- `receipt-bearing`
- `derived-recompute`

No component in this package should create hidden process state. Svelte local
state is for selection, forms, and connection preferences only.
