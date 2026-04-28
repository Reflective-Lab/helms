/**
 * Helm Flow — Presentation/Workflow Runtime Types
 *
 * Core types for flow orchestration, pacing, replay, and HITL.
 * Domain layers extend these with business-specific shapes.
 */

/**
 * Run state machine for governed workflow.
 * Tracks: initialization → execution → gates → completion
 */
export type RunState = 'bootstrap' | 'running' | 'gate-review' | 'hitl' | 'finished'

/**
 * Execution mode for demonstration/testing.
 * - mock: deterministic mocked agents, no provider calls
 * - live: real provider execution
 * - replay: pre-recorded session playback with compressed thinking delays
 */
export type RunMode = 'mock' | 'live' | 'replay'

/**
 * Single step in a flow phase.
 * Represents a unit of work: one agent, one action, one outcome.
 */
export interface FlowStep {
  id: string
  label: string
  detail: string
  agent?: string
  purpose?: string
}

/**
 * Named group of flow steps with optional gate.
 * Flow progresses through phases; gates pause between them.
 */
export interface FlowPhase {
  name: string
  steps: FlowStep[]
  gateName?: string  // "hitl" | "policy-check" | etc.
  gateReason?: string
}

/**
 * Current runtime state of the flow.
 * Updated by FlowPlayer; consumed by UI layer.
 */
export interface FlowState {
  runState: RunState
  activeStepIndex: number
  activeLabelText: string
  progressPercent: number
  currentPhase?: FlowPhase
}

/**
 * Configuration for FlowPlayer choreography.
 */
export interface FlowPlayerConfig {
  phases: FlowPhase[]
  stepDelayMs?: number     // Default 1650ms — pacing for step progression
  reviewPauseMs?: number   // Default 1100ms — pause before gate decisions
}

/**
 * Single recorded run in a replay session.
 * Contains stage name, result (domain-specific), and timing metadata.
 */
export interface DemoRun {
  stage: string
  result: unknown  // Domain layer deserializes this
  compressed_delay_ms: number
  original_elapsed_ms?: number | null
}

/**
 * Recorded session for replay.
 * Contract between recording (domain-specific) and playback (headless).
 */
export interface DemoSession {
  schema_version: number
  recorded_at: string
  source_hash: string
  mode: RunMode
  runs: DemoRun[]
}

/**
 * Status of available replay session.
 * Helm queries this to show UI options.
 */
export interface ReplayStatus {
  available: boolean
  run_count: number
  mode?: RunMode | null
  recorded_at?: string | null
  source_hash?: string | null
  source_matches: boolean
  error?: string | null
}

/**
 * Adapter pattern for replay load/save/run.
 * Domain layer implements; Helm calls these methods.
 */
export interface ReplayAdapter {
  loadSession(): Promise<DemoSession>
  saveSession(session: DemoSession): Promise<void>
  runStage(stage: string, inputs: Record<string, string>): Promise<unknown>
  recordingInProgress?: boolean
  buildingOfflineBackup?: boolean
}
