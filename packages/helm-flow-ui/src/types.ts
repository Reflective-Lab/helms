/**
 * Helm Flow UI — Common types for orchestration
 *
 * These types are shared across all domain-specific implementations.
 * Domain-specific types (e.g., DDReport, BrandComposition) extend these.
 */

export type RunMode = "mock" | "live" | "replay";

/**
 * Status of a replay session (available, recorded_at, run_count).
 * Domains extend this with domain-specific fields.
 */
export interface ReplayStatus {
  available: boolean;
  run_count: number;
  recorded_at?: string | null;
  error?: string | null;
}
