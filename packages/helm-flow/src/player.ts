/**
 * Headless flow orchestrator.
 *
 * Owns: state transitions, step progression, pacing, timers.
 * Does NOT own: domain logic, result rendering, decision semantics.
 *
 * FlowPlayer drives the entire choreography without knowing about
 * vendors, policies, providers, or business outcomes.
 */

import type { FlowState, FlowStep, FlowPhase, FlowPlayerConfig, RunState } from './types'

export class FlowPlayer {
  private runState: RunState = 'bootstrap'
  private activeStepIndex = 0
  private phases: FlowPhase[]
  private timers: ReturnType<typeof setTimeout>[] = []
  private config: FlowPlayerConfig

  constructor(config: FlowPlayerConfig) {
    this.config = config
    this.phases = config.phases
  }

  /**
   * Get current runtime state.
   * UI layer subscribes to this to render accordingly.
   */
  getState(): FlowState {
    return {
      runState: this.runState,
      activeStepIndex: this.activeStepIndex,
      activeLabelText: this.activeStep()?.label ?? '',
      progressPercent: this.calculateProgress(),
      currentPhase: this.currentPhase(),
    }
  }

  /**
   * Get the currently active step.
   */
  activeStep(): FlowStep | null {
    const allSteps = this.phases.flatMap((p) => p.steps)
    return allSteps[this.activeStepIndex] ?? null
  }

  /**
   * Get the phase containing the currently active step.
   */
  currentPhase(): FlowPhase | undefined {
    let offset = 0
    for (const phase of this.phases) {
      if (this.activeStepIndex < offset + phase.steps.length) {
        return phase
      }
      offset += phase.steps.length
    }
    return undefined
  }

  /**
   * Total number of steps across all phases.
   */
  totalSteps(): number {
    return this.phases.reduce((sum, p) => sum + p.steps.length, 0)
  }

  /**
   * Start the flow from bootstrap.
   * Resets state and transitions to 'running'.
   */
  start(): void {
    this.reset()
    this.runState = 'running'
    this.clearTimers()
  }

  /**
   * Schedule step progression with deterministic pacing.
   *
   * Even for "mocked" flows, delays are real—just deterministic.
   * This ensures presentation is watchable, not instant.
   *
   * @param stepCount - Number of steps to schedule
   * @param onStepChange - Callback when step advances (for external state updates)
   */
  scheduleSteps(stepCount: number, onStepChange: (index: number) => void): void {
    const delayMs = this.config.stepDelayMs ?? 1650

    for (let i = 0; i < stepCount; i++) {
      const timer = setTimeout(() => {
        // Safety: only advance if still running
        if (this.runState !== 'running') return
        this.activeStepIndex = i
        onStepChange(i)
      }, i * delayMs)

      this.timers.push(timer)
    }
  }

  /**
   * Pause before a gate (HITL, policy check, etc).
   * Transitions to 'gate-review' and calls onGate callback after pause.
   *
   * @param gateName - Name of the gate ("hitl", "policy-check", etc.)
   * @param onGate - Callback when pause completes
   */
  pauseAtGate(gateName: string, onGate: () => void): void {
    this.runState = 'gate-review'
    const pauseMs = this.config.reviewPauseMs ?? 1100

    const timer = setTimeout(() => {
      onGate()
    }, pauseMs)

    this.timers.push(timer)
  }

  /**
   * User approves gate decision.
   * Transitions from 'gate-review' back to 'running'.
   */
  approveGate(): void {
    if (this.runState !== 'gate-review' && this.runState !== 'hitl') return
    this.clearTimers()
    this.runState = 'running'
  }

  /**
   * Finish the flow.
   * Transitions to 'finished' and clears all timers.
   */
  finish(): void {
    this.clearTimers()
    this.runState = 'finished'
  }

  /**
   * Reset to bootstrap state.
   * Called by start() or when restarting the flow.
   */
  reset(): void {
    this.clearTimers()
    this.runState = 'bootstrap'
    this.activeStepIndex = 0
  }

  /**
   * Cancel all pending timers.
   * Called on cleanup or state transitions.
   */
  private clearTimers(): void {
    this.timers.forEach(clearTimeout)
    this.timers = []
  }

  /**
   * Calculate progress percentage.
   * Minimum 8% to show visual indication even at start.
   */
  private calculateProgress(): number {
    const total = this.totalSteps()
    if (total === 0) return 8
    return Math.max(8, (this.activeStepIndex / total) * 100)
  }
}
