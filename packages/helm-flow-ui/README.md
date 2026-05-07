# Helm Flow UI

Reusable UI patterns and utilities for building orchestrated workflows with Helm Flow.

## Purpose

When building multiple domain-specific applications that orchestrate long-running operations (vendor evaluation, due diligence, content composition, etc.), the UI orchestration patterns are identical:

1. **Mode selection** — Choose between mock (deterministic), live (real execution), or replay (recorded session)
2. **Flow visualization** — Display progress through phases and steps
3. **Replay management** — Record, build offline backups, clear recordings
4. **State management** — Track execution mode, recording progress, errors

This package provides reusable, domain-agnostic components and utilities so each application only needs to implement domain-specific logic (how to invoke backends, what data to collect, how to display results).

## Architecture

```
Domain Application
  ├── [domain]-flow.svelte        (uses FlowContainer)
  ├── [domain]-adapter.ts         (extends BaseReplayAdapter)
  └── [domain]-types.ts           (extends ReplayStatus, defines results)

@reflective/helm-flow-ui
  ├── FlowContainer.svelte        (mode selection, replay controls, progress viz)
  ├── BaseReplayAdapter           (Tauri detection, session management, invocation helpers)
  ├── types.ts                    (RunMode, ReplayStatus)
  └── spinner.ts                  (verb utilities)
```

## Usage

### 1. Extend `BaseReplayAdapter`

```typescript
// my-adapter.ts
import { BaseReplayAdapter } from "@reflective/helm-flow-ui";
import type { DemoSession } from "@reflective/helm-flow";

export class MyDomainAdapter extends BaseReplayAdapter {
  async loadSession(): Promise<DemoSession> {
    return await this.invokeTauri("load_my_session");
  }

  async saveSession(session: DemoSession): Promise<void> {
    await this.invokeTauri("save_my_session", { session });
  }

  async runStage(stage: string, inputs: Record<string, string>): Promise<MyResult> {
    return await this.invokeTauri("run_my_stage", { stage, inputs });
  }
}
```

### 2. Use `FlowContainer` in your component

```svelte
<script lang="ts">
  import { FlowPlayer, ReplayRunner } from "@reflective/helm-flow";
  import FlowContainer from "@reflective/helm-flow-ui/FlowContainer.svelte";
  import { randomVerb } from "@reflective/helm-flow-ui";
  import { MyDomainAdapter } from "./my-adapter";

  const flowPlayer = new FlowPlayer({
    phases: [{ name: "Work", steps: [...] }],
    stepDelayMs: 1500,
  });

  let flowState = $state(flowPlayer.getState());
  let runMode = $state("mock");
  let spinnerVerb = $state(randomVerb());
  let spinnerInterval = $state(null);
  // ... other state

  async function runAnalysis() {
    flowPlayer.reset();
    const adapter = new MyDomainAdapter();
    
    if (runMode === "replay") {
      const runner = new ReplayRunner(adapter as any);
      // ... replay logic
    } else {
      // ... mock/live logic
    }
  }
</script>

<FlowContainer
  {flowPlayer}
  {flowState}
  {runMode}
  {spinnerVerb}
  {spinnerInterval}
  {replayAvailable}
  {recordingReplay}
  {buildingOffline}
  onRunAnalysis={runAnalysis}
  onModeChange={(mode) => { runMode = mode }}
  onRecord={() => adapter.recordSession()}
  onBuildOffline={() => adapter.buildOfflineSession()}
  onClear={() => adapter.clearSession()}
/>
```

## Components

### `FlowContainer.svelte`

Domain-agnostic UI for orchestrating Helm Flow operations.

**Props:**
- `flowPlayer: FlowPlayer` — The flow orchestrator
- `flowState: FlowState` — Current flow state (from `flowPlayer.getState()`)
- `runMode: RunMode` — "mock" | "live" | "replay"
- `spinnerVerb: string` — Current loading verb (rotate via `randomVerb()`)
- `spinnerInterval: ReturnType<typeof setInterval> | null` — Interval for verb rotation
- `replayAvailable: boolean` — Whether replay session exists
- `replayStatus?: string` — Display text for replay status
- `recordingReplay: boolean` — Currently recording
- `buildingOffline: boolean` — Currently building offline backup
- `error?: string` — Error message to display
- `onRunAnalysis?: () => Promise<void>` — Callback when analysis starts
- `onModeChange?: (mode: RunMode) => void` — Callback when mode changes
- `onRecord?: () => Promise<void>` — Record a session
- `onBuildOffline?: () => Promise<void>` — Build offline backup
- `onClear?: () => Promise<void>` — Clear recorded session

## Classes

### `BaseReplayAdapter`

Abstract base class for domain-specific replay adapters. Encapsulates:
- Tauri vs. HTTP runtime detection (`this.isTauri`)
- Recording/offline backup flags (`recordingInProgress`, `buildingOfflineBackup`)
- Helper methods: `invokeTauri<T>()`, `fetchJSON<T>()`, `getReplayStatus()`

**Must implement:**
- `loadSession(): Promise<DemoSession>` — Load replay session
- `saveSession(session): Promise<void>` — Save replay session
- `runStage(stage, inputs): Promise<T>` — Execute a stage

## Utilities

### `randomVerb(): string`

Return a random verb from `SPINNER_VERBS`. Call when starting an operation, then rotate every 2000ms for visual feedback during loading.

### `seededVerb(seed: string, tick: number = 0): string`

Return a deterministic verb based on a seed (useful for tests and reproducible behavior).

## Types

### `RunMode`

```typescript
type RunMode = "mock" | "live" | "replay";
```

### `ReplayStatus`

```typescript
interface ReplayStatus {
  available: boolean;
  run_count: number;
  recorded_at?: string | null;
  error?: string | null;
}
```

Domains extend this with domain-specific fields (e.g., `DDReplayStatus`, `BrandReplayStatus`).

## FAQ

**Q: How much state management do I need in my component?**

A: At minimum:
- `flowState` (from `flowPlayer.getState()`)
- `runMode` (tracking selected mode)
- `spinnerVerb` (rotated by interval)
- `recordingReplay` / `buildingOffline` (tracking recording status)
- `replayAvailable` (whether replay session exists)

**Q: Can I customize the UI styling?**

A: Yes. `FlowContainer.svelte` uses CSS custom properties for colors:
- `--active-bg` (default: #ccff00)
- `--active-color` (default: #07090d)
- `--border-color` (default: #ccc)
- `--error-color` (default: #ff6b6b)
- `--error-bg` (default: rgba(255, 107, 107, 0.1))
- `--success-color` (default: #5ad363)

Set these in your app's CSS to customize.

**Q: How do I add domain-specific error handling?**

A: Extend `BaseReplayAdapter.getReplayStatus()` in your adapter, or override error handling in your component before passing `error` to `FlowContainer`.

**Q: Do I need to use `FlowContainer`?**

A: No. You can build your own UI and just use `BaseReplayAdapter` and utilities. `FlowContainer` is a reasonable default that eliminates duplicated orchestration UI.
