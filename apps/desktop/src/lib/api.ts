import { invoke } from '@tauri-apps/api/core'

import type {
	AccountWorkspaceSummary,
	AppleNotesImportReport,
	AppleNotesPublishReport,
	ApprovalListItem,
	CatalogItemListItem,
	NoteCleanupReport,
	NoteValueReport,
	ExpenseItem,
	ExpenseOcrRun,
	ExpenseReport,
	ExpenseReceiptSample,
	OperatorDashboard,
	OperatorShellData,
	OpportunityListItem,
	OrganizationListItem,
	SubscriptionListItem,
	SystemProfile,
	TruthDetailItem,
	TruthExecutionInputs,
	TruthExecutionSession,
	TruthListItem,
	VaultImportReport,
	VaultNote,
	VaultTreeEntry,
	WebSnapshotCaptureReport,
	WorkbenchAppManifest,
	WorkflowCaseListItem
} from '$lib/types'

const apiBaseUrl = import.meta.env.PUBLIC_CRM_API_BASE_URL || 'http://127.0.0.1:8081'

function isTauriRuntime() {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

const workbenchBackendMode =
	import.meta.env.PUBLIC_DESKTOP_BACKEND_MODE || (isTauriRuntime() ? 'embedded' : 'remote')

async function requestJson<T>(path: string, init?: RequestInit): Promise<T> {
	const response = await fetch(`${apiBaseUrl}${path}`, init)
	if (!response.ok) {
		const body = await response.text()
		throw new Error(body || `Request failed with ${response.status}`)
	}
	return response.json() as Promise<T>
}

function useRemoteWorkbench() {
	return workbenchBackendMode === 'remote'
}

function workbenchPath(path: string, query?: Record<string, string | boolean | undefined>) {
	const search = new URLSearchParams()
	for (const [key, value] of Object.entries(query ?? {})) {
		if (value !== undefined) {
			search.set(key, String(value))
		}
	}

	const encodedPath = `/v1/workbench${path}`
	const queryString = search.toString()
	return queryString ? `${encodedPath}?${queryString}` : encodedPath
}

function requestWorkbenchJson<T>(
	path: string,
	query?: Record<string, string | boolean | undefined>,
	init?: RequestInit
) {
	return requestJson<T>(workbenchPath(path, query), init)
}

function postWorkbenchJson<T>(path: string, body: unknown) {
	return requestWorkbenchJson<T>(path, undefined, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json'
		},
		body: JSON.stringify(body)
	})
}

export function getOperatorDashboard() {
	return useRemoteWorkbench()
		? requestWorkbenchJson<OperatorDashboard>('/dashboard')
		: invoke<OperatorDashboard>('operator_dashboard')
}

export function getTruthCatalog() {
	return useRemoteWorkbench()
		? requestWorkbenchJson<TruthListItem[]>('/truths')
		: invoke<TruthListItem[]>('list_truths')
}

export function getTruthDetail(key: string) {
	return useRemoteWorkbench()
		? requestWorkbenchJson<TruthDetailItem>(`/truths/${encodeURIComponent(key)}`)
		: invoke<TruthDetailItem>('get_truth_detail', { key })
}

export function getWorkbenchApps() {
	return useRemoteWorkbench()
		? requestWorkbenchJson<WorkbenchAppManifest[]>('/apps')
		: invoke<WorkbenchAppManifest[]>('list_workbench_apps')
}

export async function getTruthCatalogItem(key: string) {
	const truths = await getTruthCatalog()
	return truths.find((truth) => truth.key === key) ?? null
}

export function getOrganizations() {
	return useRemoteWorkbench()
		? requestWorkbenchJson<OrganizationListItem[]>('/organizations')
		: invoke<OrganizationListItem[]>('list_organizations')
}

export function getOpportunities() {
	return useRemoteWorkbench()
		? requestWorkbenchJson<OpportunityListItem[]>('/opportunities')
		: invoke<OpportunityListItem[]>('list_opportunities')
}

export function getSubscriptions(organizationId?: string) {
	return useRemoteWorkbench()
		? requestWorkbenchJson<SubscriptionListItem[]>('/subscriptions', {
				organization_id: organizationId
			})
		: invoke<SubscriptionListItem[]>('list_subscriptions', { organizationId })
}

export function getCatalogItems(activeOnly = false) {
	return useRemoteWorkbench()
		? requestWorkbenchJson<CatalogItemListItem[]>('/catalog', { active_only: activeOnly })
		: invoke<CatalogItemListItem[]>('list_catalog_items', { activeOnly })
}

export function getWorkflowCases() {
	return useRemoteWorkbench()
		? requestWorkbenchJson<WorkflowCaseListItem[]>('/workflow/cases')
		: invoke<WorkflowCaseListItem[]>('list_workflow_cases')
}

export function getApprovals() {
	return useRemoteWorkbench()
		? requestWorkbenchJson<ApprovalListItem[]>('/approvals')
		: invoke<ApprovalListItem[]>('list_approvals')
}

export function getSystemProfile() {
	return useRemoteWorkbench()
		? requestWorkbenchJson<SystemProfile>('/system/profile')
		: invoke<SystemProfile>('system_profile')
}

export function getAccountSummary(orgId: string) {
	return useRemoteWorkbench()
		? requestWorkbenchJson<AccountWorkspaceSummary>(
				`/organizations/${encodeURIComponent(orgId)}/summary`
			)
		: invoke<AccountWorkspaceSummary>('account_summary', { orgId })
}

export function executeTruth(key: string, inputs: TruthExecutionInputs) {
	return useRemoteWorkbench()
		? postWorkbenchJson<TruthExecutionSession>(`/truths/${encodeURIComponent(key)}/execute`, {
				inputs
			})
		: invoke<TruthExecutionSession>('execute_truth', { key, inputs })
}

export function getExpenseReports() {
	return isTauriRuntime()
		? invoke<ExpenseReport[]>('list_expense_reports')
		: requestJson<ExpenseReport[]>('/v1/expenses/reports')
}

export function getExpenseItems(reportId?: string) {
	return isTauriRuntime()
		? invoke<ExpenseItem[]>('list_expense_items', { reportId })
		: requestJson<ExpenseItem[]>('/v1/expenses/items')
}

export function getReceiptSamples() {
	return isTauriRuntime()
		? invoke<ExpenseReceiptSample[]>('list_receipt_samples')
		: Promise.resolve<ExpenseReceiptSample[]>([])
}

export function compareReceiptOcr(sampleId: string) {
	return isTauriRuntime()
		? invoke<ExpenseOcrRun[]>('compare_receipt_ocr', { sampleId })
		: Promise.resolve<ExpenseOcrRun[]>([])
}

export function getNoteVaultRoot() {
	return invoke<string>('get_note_vault_root')
}

export function listNotes() {
	return invoke<VaultTreeEntry[]>('list_notes')
}

export function readNote(path: string) {
	return invoke<VaultNote>('read_note', { path })
}

export function saveNote(path: string, body: string) {
	return invoke<VaultNote>('save_note', { path, body })
}

export function createNote(title: string, parentDir?: string) {
	return invoke<VaultNote>('create_note', { title, parentDir })
}

export function moveNote(fromPath: string, toPath: string) {
	return invoke<VaultNote>('move_note', { fromPath, toPath })
}

export function importMarkdownTree(sourceDir: string) {
	return invoke<VaultImportReport>('import_markdown_tree', { sourceDir })
}

export function importAppleNotes() {
	return invoke<AppleNotesImportReport>('import_apple_notes')
}

export function publishAppleNotes(runId?: string) {
	return invoke<AppleNotesPublishReport>('publish_apple_notes', { runId })
}

export function captureNoteUrl(url: string) {
	return invoke<WebSnapshotCaptureReport>('capture_note_url', { url })
}

export function analyzeNoteCleanup() {
	return invoke<NoteCleanupReport>('analyze_note_cleanup')
}

export function analyzeNoteValue() {
	return invoke<NoteValueReport>('analyze_note_value')
}

export async function loadOperatorShell(): Promise<OperatorShellData> {
	const [apps, dashboard, truths, organizations, opportunities, workflows, approvals, profile] =
		await Promise.all([
			getWorkbenchApps(),
			getOperatorDashboard(),
			getTruthCatalog(),
			getOrganizations(),
			getOpportunities(),
			getWorkflowCases(),
			getApprovals(),
			getSystemProfile()
		])

	return {
		apps,
		dashboard,
		truths,
		organizations,
		opportunities,
		workflows,
		approvals,
		profile
	}
}
