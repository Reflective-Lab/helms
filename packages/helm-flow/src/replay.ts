/**
 * Replay logic with injected storage/runner callbacks.
 *
 * Helm doesn't know about Tauri, filesystem, providers, or HTTP.
 * Domain layer provides load/save/run adapters.
 *
 * This enables:
 * - Web-based replay (localStorage/IndexedDB)
 * - Desktop replay (Tauri filesystem)
 * - Remote replay (API calls)
 * - Live mode (same adapter interface, different implementation)
 */

import type { DemoSession, DemoRun, ReplayAdapter } from './types'

export class ReplayRunner {
  private adapter: ReplayAdapter
  private session: DemoSession | null = null
  private cursor: Record<string, number> = {}

  constructor(adapter: ReplayAdapter) {
    this.adapter = adapter
  }

  /**
   * Load or fetch the replay session.
   * Caches it so multiple calls don't re-fetch.
   */
  async ensureSession(): Promise<DemoSession> {
    if (this.session) return this.session
    this.session = await this.adapter.loadSession()
    return this.session
  }

  /**
   * Record a new session.
   * Adapter implements the actual recording mechanism (Tauri, API, etc.).
   */
  async recordSession(): Promise<DemoSession> {
    // Adapter-specific recording logic. Helm just coordinates.
    const session = await this.adapter.loadSession()
    this.session = session
    return session
  }

  /**
   * Get the next recorded run for a given stage.
   * Maintains cursor to advance through multiple runs of same stage.
   *
   * @param session - The loaded session
   * @param stage - Stage name (normalized by caller)
   * @throws If stage not found at current cursor position
   */
  async takeRun(session: DemoSession, stage: string): Promise<DemoRun> {
    const normalized = this.normalizeStage(stage)
    const offset = this.cursor[normalized] ?? 0

    // Find all runs matching this stage
    const matches = session.runs.filter((run) => this.normalizeStage(run.stage) === normalized)

    const recorded = matches[offset]
    if (!recorded) {
      throw new Error(
        `Recorded session does not include stage '${stage}' at position ${offset + 1}.`
      )
    }

    // Advance cursor for next call
    this.cursor[normalized] = offset + 1
    return recorded
  }

  /**
   * Simulate thinking delay during playback.
   *
   * Even in replay, delays are deterministic—not instant.
   * - Mocked: can be instant (caller chooses)
   * - Replay: compressed thinking delay (recorded.compressed_delay_ms)
   *
   * @param recorded - The run to delay for
   */
  async playDelay(recorded: DemoRun): Promise<void> {
    // Clamp delay to reasonable bounds (900ms–6500ms)
    const delay = Math.max(900, Math.min(6500, recorded.compressed_delay_ms || 1800))

    return new Promise<void>((resolve) => {
      setTimeout(resolve, delay)
    })
  }

  /**
   * Reset playback cursor.
   * Allows replaying the same session multiple times.
   */
  resetCursor(): void {
    this.cursor = {}
  }

  /**
   * Normalize stage name.
   * Hook for domain layer to implement stage mapping.
   *
   * Example (vendor-selection):
   *   "before-hitl" → "analysis"
   *   "promote" → "approved"
   *   "advisory" → "negative-control"
   *
   * Default: no mapping (return as-is).
   * Override or call domain-specific normalization before takeRun.
   */
  private normalizeStage(stage: string): string {
    return stage
  }
}
