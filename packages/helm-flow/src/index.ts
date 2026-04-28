/**
 * @reflective/helm-flow
 *
 * Helm Flow — Presentation/Workflow Runtime + Reusable UI Components
 *
 * Exports:
 * - FlowPlayer: Headless flow orchestrator (state machine, pacing, choreography)
 * - ReplayRunner: Replay playback with injected adapters
 * - HitlGate: Generic HITL approval form
 * - DocumentIntake: Document collection and readiness
 * - Types: RunState, RunMode, FlowStep, FlowPhase, FlowState, DemoSession, etc.
 */

// Types
export type {
  RunState,
  RunMode,
  FlowStep,
  FlowPhase,
  FlowState,
  FlowPlayerConfig,
  DemoRun,
  DemoSession,
  ReplayStatus,
  ReplayAdapter,
} from './types'

// Classes
export { FlowPlayer } from './player'
export { ReplayRunner } from './replay'

// Components (Svelte) — import directly from .svelte files:
// import HitlGate from '@reflective/helm-flow/src/HitlGate.svelte'
// import DocumentIntake from '@reflective/helm-flow/src/DocumentIntake.svelte'
