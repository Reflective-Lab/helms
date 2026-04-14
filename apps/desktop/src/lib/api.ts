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

const apiBaseUrl = import.meta.env.PUBLIC_CRM_API_BASE_URL || 'http://127.0.0.1:8080'

function isTauriRuntime() {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

async function requestJson<T>(path: string, init?: RequestInit): Promise<T> {
	const response = await fetch(`${apiBaseUrl}${path}`, init)
	if (!response.ok) {
		const body = await response.text()
		throw new Error(body || `Request failed with ${response.status}`)
	}
	return response.json() as Promise<T>
}

export function getOperatorDashboard() {
	return invoke<OperatorDashboard>('operator_dashboard')
}

export function getTruthCatalog() {
	return invoke<TruthListItem[]>('list_truths')
}

export function getTruthDetail(key: string) {
	return invoke<TruthDetailItem>('get_truth_detail', { key })
}

export function getWorkbenchApps() {
	return invoke<WorkbenchAppManifest[]>('list_workbench_apps')
}

export async function getTruthCatalogItem(key: string) {
	const truths = await getTruthCatalog()
	return truths.find((truth) => truth.key === key) ?? null
}

export function getOrganizations() {
	return invoke<OrganizationListItem[]>('list_organizations')
}

export function getOpportunities() {
	return invoke<OpportunityListItem[]>('list_opportunities')
}

export function getSubscriptions(organizationId?: string) {
	return invoke<SubscriptionListItem[]>('list_subscriptions', { organizationId })
}

export function getCatalogItems(activeOnly = false) {
	return invoke<CatalogItemListItem[]>('list_catalog_items', { activeOnly })
}

export function getWorkflowCases() {
	return invoke<WorkflowCaseListItem[]>('list_workflow_cases')
}

export function getApprovals() {
	return invoke<ApprovalListItem[]>('list_approvals')
}

export function getSystemProfile() {
	return invoke<SystemProfile>('system_profile')
}

export function getAccountSummary(orgId: string) {
	return invoke<AccountWorkspaceSummary>('account_summary', { orgId })
}

export function executeTruth(key: string, inputs: TruthExecutionInputs) {
	return invoke<TruthExecutionSession>('execute_truth', { key, inputs })
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
