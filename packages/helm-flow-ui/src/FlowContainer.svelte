<script lang="ts">
  /**
   * Flow Container — Reusable UI orchestration shell
   *
   * Provides:
   * - Mode selector (Mock / Live / Replay buttons)
   * - Replay management controls (Record, Build Offline, Clear)
   * - Flow visualization (step progress with phase grouping)
   * - Spinner verb rotation during execution
   *
   * Domain-agnostic: all domain knowledge comes from props.
   *
   * Usage:
   * ```svelte
   * <FlowContainer
   *   {flowPlayer}
   *   {flowState}
   *   {runMode}
   *   {spinnerVerb}
   *   {spinnerInterval}
   *   {replayAvailable}
   *   {replayStatus}
   *   {recordingReplay}
   *   {buildingOffline}
   *   {error}
   *   onRunAnalysis={handleRun}
   *   onModeChange={(mode) => { runMode = mode }}
   *   onRecord={handleRecord}
   *   onBuildOffline={handleBuildOffline}
   *   onClear={handleClear}
   * />
   * ```
   */

  import type { FlowPlayer, FlowState } from "@reflective/helm-flow";
  import type { RunMode } from "./types";

  interface Props {
    flowPlayer: FlowPlayer;
    flowState: FlowState;
    runMode: RunMode;
    spinnerVerb: string;
    spinnerInterval: ReturnType<typeof setInterval> | null;
    replayAvailable: boolean;
    replayStatus?: string;
    recordingReplay: boolean;
    buildingOffline: boolean;
    error?: string;
    onRunAnalysis?: () => Promise<void>;
    onModeChange?: (mode: RunMode) => void;
    onRecord?: () => Promise<void>;
    onBuildOffline?: () => Promise<void>;
    onClear?: () => Promise<void>;
  }

  let {
    flowPlayer,
    flowState,
    runMode = "mock",
    spinnerVerb,
    spinnerInterval,
    replayAvailable = false,
    replayStatus = "",
    recordingReplay = false,
    buildingOffline = false,
    error = "",
    onRunAnalysis = () => Promise.resolve(),
    onModeChange = () => {},
    onRecord = () => Promise.resolve(),
    onBuildOffline = () => Promise.resolve(),
    onClear = () => Promise.resolve(),
  }: Props = $props();

  const isRunning = flowState.runState === "running";
  const isFinished = flowState.runState === "finished";
  const showingFlow = isRunning || isFinished;
</script>

<div class="flow-container">
  <!-- Mode selector -->
  <div class="mode-selector">
    <label class="mode-label">Mode:</label>
    <div class="mode-buttons">
      <button
        class="mode-button {runMode === 'mock' ? 'active' : ''}"
        onclick={() => onModeChange("mock")}
        disabled={isRunning}
      >
        Mock
      </button>
      <button
        class="mode-button {runMode === 'live' ? 'active' : ''}"
        onclick={() => onModeChange("live")}
        disabled={isRunning}
      >
        Live
      </button>
      <button
        class="mode-button {runMode === 'replay' ? 'active' : ''}"
        onclick={() => onModeChange("replay")}
        disabled={!replayAvailable || isRunning}
        title={replayAvailable ? "Replay recorded session" : "No replay available"}
      >
        Replay {#if replayStatus}({replayStatus}){/if}
      </button>
    </div>
  </div>

  <!-- Replay management controls -->
  {#if runMode !== "replay"}
    <div class="replay-controls">
      <button
        class="control-button"
        onclick={onRecord}
        disabled={recordingReplay || isRunning}
        title="Record live run for replay"
      >
        {recordingReplay ? "Recording..." : "Record Replay"}
      </button>
      <button
        class="control-button"
        onclick={onBuildOffline}
        disabled={buildingOffline || isRunning}
        title="Build offline backup of recorded session"
      >
        {buildingOffline ? "Building..." : "Offline Backup"}
      </button>
      {#if replayAvailable}
        <button
          class="control-button danger"
          onclick={onClear}
          disabled={isRunning}
          title="Delete recorded session"
        >
          Clear Replay
        </button>
      {/if}
    </div>
  {/if}

  <!-- Flow visualization -->
  {#if showingFlow}
    <div class="flow-visualization">
      {#each flowState.currentPhase?.steps ?? [] as step, i}
        {@const isActive = i === flowState.activeStepIndex}
        <div
          class="step {isActive ? 'active' : i < flowState.activeStepIndex ? 'completed' : 'pending'}"
        >
          <div class="step-indicator">
            {#if isActive}
              <div class="spinner"></div>
            {:else if i < flowState.activeStepIndex}
              <span class="checkmark">✓</span>
            {:else}
              <span class="number">{i + 1}</span>
            {/if}
          </div>
          <div class="step-content">
            <p class="step-label">{step.label}</p>
            <p class="step-detail">{step.detail}</p>
          </div>
        </div>
      {/each}
    </div>

    {#if isRunning}
      <div class="spinner-display">
        <span class="spinner-dot"></span>
        <span class="spinner-text">{spinnerVerb}...</span>
      </div>
    {/if}
  {/if}

  <!-- Error display -->
  {#if error}
    <div class="error-message">
      {error}
    </div>
  {/if}
</div>

<style>
  .flow-container {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .mode-selector {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .mode-label {
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .mode-buttons {
    display: flex;
    gap: 0.5rem;
  }

  .mode-button {
    padding: 0.25rem 0.75rem;
    font-size: 0.75rem;
    font-weight: 600;
    border: 1px solid var(--border-color, #ccc);
    border-radius: 0.25rem;
    background: transparent;
    cursor: pointer;
    transition: all 160ms;
  }

  .mode-button.active {
    background: var(--active-bg, #ccff00);
    color: var(--active-color, #07090d);
  }

  .mode-button:hover:not(:disabled) {
    border-color: var(--active-bg, #ccff00);
  }

  .mode-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .replay-controls {
    display: flex;
    gap: 0.5rem;
  }

  .control-button {
    padding: 0.25rem 0.75rem;
    font-size: 0.75rem;
    font-weight: 600;
    border: 1px solid var(--border-color, #ccc);
    border-radius: 0.25rem;
    background: transparent;
    cursor: pointer;
    transition: all 160ms;
  }

  .control-button:hover:not(:disabled) {
    border-color: var(--active-bg, #ccff00);
  }

  .control-button.danger {
    color: var(--error-color, #ff6b6b);
    border-color: var(--error-color, #ff6b6b);
  }

  .control-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .flow-visualization {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .step {
    display: flex;
    gap: 0.75rem;
    padding: 0.75rem;
    border: 1px solid var(--border-color, #ccc);
    border-radius: 0.5rem;
    transition: all 160ms;
  }

  .step.active {
    border-color: var(--active-bg, #ccff00);
    background: var(--active-bg, #ccff0010);
  }

  .step.completed {
    border-color: var(--border-color, #ccc);
    background: transparent;
  }

  .step.pending {
    border-color: var(--border-color, #ccc);
    background: transparent;
    opacity: 0.5;
  }

  .step-indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.5rem;
    height: 1.5rem;
    border: 1px solid currentColor;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .spinner {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background: var(--active-bg, #ccff00);
    animation: pulse 1s infinite;
  }

  .checkmark {
    font-size: 0.75rem;
    font-weight: 700;
    color: var(--success-color, #5ad363);
  }

  .number {
    font-size: 0.75rem;
  }

  .step-content {
    flex: 1;
  }

  .step-label {
    margin: 0;
    font-weight: 600;
    font-size: 0.9rem;
  }

  .step-detail {
    margin: 0.25rem 0 0 0;
    font-size: 0.8rem;
    opacity: 0.7;
  }

  .spinner-display {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.9rem;
  }

  .spinner-dot {
    display: inline-block;
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background: var(--active-bg, #ccff00);
    animation: pulse 1s infinite;
  }

  .error-message {
    padding: 0.75rem;
    border-radius: 0.5rem;
    border: 1px solid var(--error-color, #ff6b6b);
    background: var(--error-bg, rgba(255, 107, 107, 0.1));
    color: var(--error-color, #ff6b6b);
    font-size: 0.85rem;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }
</style>
