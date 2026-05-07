/**
 * Base Replay Adapter for Helm Flow UI
 *
 * Encapsulates common orchestration logic across all domain-specific adapters:
 * - Tauri vs. HTTP runtime detection
 * - Recording and offline backup flags
 * - Session management wrappers
 *
 * Domains extend this with their own loadSession(), saveSession(), runStage() implementations.
 */

import type { ReplayAdapter, DemoSession } from "@reflective/helm-flow";

/**
 * Base class for domain-specific replay adapters.
 *
 * Domains extend this and implement:
 * - loadSession(): Load domain-specific session format
 * - saveSession(session): Save domain-specific session
 * - runStage(stage, inputs): Execute domain-specific stage logic
 */
export abstract class BaseReplayAdapter implements ReplayAdapter {
	/** Detected runtime environment: true if Tauri, false if HTTP/browser */
	protected isTauri: boolean;

	/** Flag: currently recording a session */
	recordingInProgress = false;

	/** Flag: currently building offline backup */
	buildingOfflineBackup = false;

	constructor() {
		this.isTauri = Boolean((window as any).__TAURI_INTERNALS__);
	}

	/**
	 * Load the replay session from storage (Tauri or HTTP).
	 * Domains implement this with domain-specific types.
	 */
	abstract loadSession(): Promise<DemoSession>;

	/**
	 * Save the replay session to storage (Tauri or HTTP).
	 * Domains implement this with domain-specific types.
	 */
	abstract saveSession(session: DemoSession): Promise<void>;

	/**
	 * Run a stage (for mock or live modes).
	 * Domains implement this with domain-specific stage logic and Tauri/HTTP calls.
	 */
	abstract runStage(
		stage: string,
		inputs: Record<string, string>
	): Promise<any>;

	/**
	 * Helper: Get status of replay availability.
	 * Domains can extend this to add domain-specific fields.
	 */
	async getReplayStatus(): Promise<{
		available: boolean;
		run_count: number;
		recorded_at?: string | null;
		error?: string | null;
	}> {
		try {
			const session = await this.loadSession();
			return {
				available: true,
				run_count: (session as any).runs?.length ?? 1,
				recorded_at: (session as any).recorded_at ?? null,
			};
		} catch (e) {
			return {
				available: false,
				run_count: 0,
				error: e instanceof Error ? e.message : String(e),
			};
		}
	}

	/**
	 * Helper: Invoke a Tauri command (for use by domain adapters).
	 * Detects if we're in Tauri and uses the appropriate invocation.
	 */
	protected async invokeTauri<T>(
		command: string,
		payload?: Record<string, any>
	): Promise<T> {
		if (!this.isTauri) {
			throw new Error(
				`Tauri invocation "${command}" requested but not in Tauri environment`
			);
		}

		const { invoke } = await import("@tauri-apps/api/core");
		return await invoke<T>(command, payload);
	}

	/**
	 * Helper: Fetch from HTTP endpoint (for use by domain adapters).
	 * Returns parsed JSON response.
	 */
	protected async fetchJSON<T>(
		url: string,
		options?: RequestInit
	): Promise<T> {
		const response = await fetch(url, {
			...options,
			headers: {
				"Content-Type": "application/json",
				...options?.headers,
			},
		});

		if (!response.ok) {
			const body = await response.text();
			throw new Error(body || `HTTP ${response.status}`);
		}

		return await response.json();
	}
}
