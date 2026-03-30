import { invoke } from '@tauri-apps/api/core'

import type {
	AccountWorkspaceSummary,
	ApprovalListItem,
	CatalogItemListItem,
	OperatorDashboard,
	OperatorShellData,
	OpportunityListItem,
	OrganizationListItem,
	SubscriptionListItem,
	SystemProfile,
	TruthExecutionInputs,
	TruthExecutionSession,
	TruthListItem,
	WorkflowCaseListItem
} from '$lib/types'

export function getOperatorDashboard() {
	return invoke<OperatorDashboard>('operator_dashboard')
}

export function getTruthCatalog() {
	return invoke<TruthListItem[]>('list_truths')
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

export async function loadOperatorShell(): Promise<OperatorShellData> {
	const [dashboard, truths, organizations, opportunities, workflows, approvals, profile] =
		await Promise.all([
			getOperatorDashboard(),
			getTruthCatalog(),
			getOrganizations(),
			getOpportunities(),
			getWorkflowCases(),
			getApprovals(),
			getSystemProfile()
		])

	return {
		dashboard,
		truths,
		organizations,
		opportunities,
		workflows,
		approvals,
		profile
	}
}
