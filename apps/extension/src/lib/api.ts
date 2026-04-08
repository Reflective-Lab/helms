import type {
	Organization,
	TimelineEntry,
	TruthCatalogItem,
	TruthExecutionResponse,
	WorkflowCase
} from './types'

const BASE = 'http://localhost:8081'

async function get<T>(path: string): Promise<T> {
	const res = await fetch(`${BASE}${path}`)
	if (!res.ok) throw new Error(`${res.status} ${res.statusText}`)
	return res.json()
}

async function post<T>(path: string, body: unknown): Promise<T> {
	const res = await fetch(`${BASE}${path}`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(body)
	})
	if (!res.ok) throw new Error(`${res.status} ${res.statusText}`)
	return res.json()
}

export function getTruths(): Promise<TruthCatalogItem[]> {
	return get('/v1/truths')
}

export function getTimeline(limit = 20): Promise<TimelineEntry[]> {
	return get(`/v1/timeline?limit=${limit}`)
}

export function getWorkflowCases(): Promise<WorkflowCase[]> {
	return get('/v1/workflow/cases')
}

export function getOrganizations(): Promise<Organization[]> {
	return get('/v1/organizations')
}

export function executeTruth(
	key: string,
	inputs: Record<string, string>
): Promise<TruthExecutionResponse> {
	return post(`/v1/truths/${key}/execute`, {
		inputs,
		persist_projection: true
	})
}

export async function healthCheck(): Promise<boolean> {
	try {
		await get('/health')
		return true
	} catch {
		return false
	}
}
