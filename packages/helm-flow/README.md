# @reflective/helm-flow

Helm Flow — Presentation/Workflow Runtime + Reusable UI Components

## What It Is

A headless orchestration layer for governed presentation flows.

**Owned by Helm:**
- Flow state machine (RunState, progress, phase tracking)
- Deterministic pacing and timing choreography
- HITL (Human In The Loop) approval form
- Document intake and readiness display
- Replay system with adapter-driven load/save/run

**NOT owned by Helm:**
- Business logic (vendor selection, policy evaluation, etc.)
- Result rendering (shortlist, policy inspection, learning display)
- Domain-specific forms and layouts
- Provider/LLM mechanics
- Recording infrastructure (Tauri, filesystem, etc.)

## Core Exports

### `FlowPlayer` — Headless Orchestrator

```ts
const player = new FlowPlayer({
  phases: [
    { name: 'Analysis', steps: [...] },
    { name: 'HITL Gate', steps: [...], gateName: 'hitl' },
    { name: 'Promotion', steps: [...] },
  ],
})

player.start()
player.scheduleSteps(6, (index) => console.log(`Step ${index}`))
// ... domain logic runs ...
player.pauseAtGate('hitl', () => openHitlForm())
// ... user approves ...
player.approveGate()
player.finish()

const state = player.getState() // { runState, activeStepIndex, progressPercent, ... }
```

State machine: `bootstrap` → `running` → `gate-review` → `hitl` → `finished`

### `ReplayRunner` — Session Playback

```ts
const replayer = new ReplayRunner({
  loadSession: async () => fetch('/session.json'),
  saveSession: async (session) => { /* save */ },
  runStage: async (stage, inputs) => { /* domain-specific */ },
})

const session = await replayer.ensureSession()
const run = await replayer.takeRun(session, 'analysis')
await replayer.playDelay(run)  // Compressed thinking delay
const result = run.result  // Domain layer deserializes
```

### `HitlGate.svelte` — Approval Form

```svelte
<HitlGate
  decisionSummary={{ candidate: 'Anthropic', reason: 'Budget delta within threshold' }}
  bind:approverName
  bind:approvalNote
  bind:delegateToPolicy
  policyPreview={cedarPolicy}
  onApprove={handleApprove}
/>
```

No domain assumptions. Domain layer provides decision content.

### `DocumentIntake.svelte` — Document Collection

```svelte
<DocumentIntake
  bind:documents
  bind:fastLoadEnabled
  bind:executableReady
  expectedDocs={[
    { title: 'RFI/RFP', purpose: '...', info: '...' },
    // ...
  ]}
  onFilesSelected={(files) => handleFiles(files)}
/>
```

Generic pattern reusable across decision types.

## Types

```ts
// State
type RunState = 'bootstrap' | 'running' | 'gate-review' | 'hitl' | 'finished'
type RunMode = 'mock' | 'live' | 'replay'
interface FlowState { runState, activeStepIndex, progressPercent, currentPhase }

// Flow definition
interface FlowStep { id, label, detail, agent?, purpose? }
interface FlowPhase { name, steps[], gateName?, gateReason? }

// Replay
interface DemoSession { schema_version, recorded_at, source_hash, mode, runs[] }
interface DemoRun { stage, result, compressed_delay_ms, original_elapsed_ms? }
interface ReplayAdapter { loadSession(), saveSession(), runStage() }
```

## Usage in Hackathon (Phase 1)

1. **Import types** — Remove vendor-selection type definitions from AIProviderEvaluation.svelte
2. **Use FlowPlayer** — Replace pushStep, completeThrough, scheduleAnalysisSteps, clearRunTimers
3. **Use HitlGate** — Replace HITL form markup; keep Cedar semantics local
4. **Use DocumentIntake** — Replace document dropbox markup; keep sample data local
5. **Keep local** — All vendor-selection business logic, result rendering, experience aggregation

## Future Phases

**Phase 2:** ReplayRunner full integration (currently adapter-based, not wired)
**Phase 3:** Result view primitives (TimelinePanel, DecisionCard, EvidenceList)
**Phase 4:** Experience store UI (once usage pattern is clear across demos)

## License

Reflective Labs
