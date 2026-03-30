<script lang="ts">
	import { onMount } from 'svelte'
	import AccountsSection from '$lib/components/AccountsSection.svelte'
	import ApprovalsSection from '$lib/components/ApprovalsSection.svelte'
	import JobsSection from '$lib/components/JobsSection.svelte'
	import RightRail from '$lib/components/RightRail.svelte'
	import SystemSection from '$lib/components/SystemSection.svelte'
	import WorkflowSection from '$lib/components/WorkflowSection.svelte'
	import { executeTruth, getAccountSummary, loadOperatorShell } from '$lib/api'
	import { navSections, type AccountWorkspaceSummary, type ApprovalListItem, type OperatorDashboard, type OpportunityListItem, type OrganizationListItem, type Section, type SystemProfile, type TruthExecutionInputs, type TruthExecutionSession, type TruthListItem, type WorkflowCaseListItem } from '$lib/types'

	let activeSection = $state<Section>('jobs')
	let dashboard = $state<OperatorDashboard | null>(null)
	let truths = $state<TruthListItem[]>([])
	let organizations = $state<OrganizationListItem[]>([])
	let opportunities = $state<OpportunityListItem[]>([])
	let workflows = $state<WorkflowCaseListItem[]>([])
	let approvals = $state<ApprovalListItem[]>([])
	let account = $state<AccountWorkspaceSummary | null>(null)
	let profile = $state<SystemProfile | null>(null)
	let selectedOrganizationId = $state<string | null>(null)
	let latestExecution = $state<TruthExecutionSession | null>(null)
	let loading = $state(true)
	let running = $state(false)
	let error = $state('')

	async function loadShell() {
		loading = true
		error = ''
		try {
			const shell = await loadOperatorShell()

			dashboard = shell.dashboard
			truths = shell.truths
			organizations = shell.organizations
			opportunities = shell.opportunities
			workflows = shell.workflows
			approvals = shell.approvals
			profile = shell.profile

			if (!selectedOrganizationId && organizations.length > 0) {
				selectedOrganizationId = organizations[0].id
			}
			if (selectedOrganizationId) {
				account = await getAccountSummary(selectedOrganizationId)
			}
		} catch (cause) {
			error = String(cause)
		} finally {
			loading = false
		}
	}

	async function selectOrganization(orgId: string) {
		selectedOrganizationId = orgId
		account = await getAccountSummary(orgId)
		activeSection = 'accounts'
	}

	function sampleTruthInputs(requireManualReview: boolean): TruthExecutionInputs {
		return {
			organization_name: requireManualReview ? 'Helios Freight' : 'Praxis Systems',
			inbound_summary: requireManualReview
				? 'Buyer asked for exception handling, legal review, and staged rollout.'
				: 'Inbound buyer wants a governed CRM substrate with operator workflows.',
			contact_name: requireManualReview ? 'Jordan Vale' : 'Riley Park',
			contact_title: requireManualReview ? 'Finance Director' : 'COO',
			owner_user_id: requireManualReview ? 'commercial-review' : 'kenneth',
			next_step: requireManualReview
				? 'Open approval path and validate non-standard commercials.'
				: 'Schedule qualification follow-up and share architecture note.',
			opportunity_value_minor: requireManualReview ? '45000000' : '18000000',
			require_manual_review: requireManualReview ? 'true' : 'false',
			manual_review_reason: 'Commercial terms fall outside the standard path.'
		}
	}

	async function runSampleTruth(requireManualReview: boolean) {
		running = true
		error = ''
		try {
			latestExecution = await executeTruth('qualify-inbound-lead', sampleTruthInputs(requireManualReview))
			activeSection = requireManualReview ? 'approvals' : 'jobs'
			await loadShell()
		} catch (cause) {
			error = String(cause)
		} finally {
			running = false
		}
	}

	onMount(() => {
		loadShell()
	})
</script>

<div class="page">
	<header class="hero">
		<div>
			<p class="eyebrow">Prio CRM Operator Cockpit</p>
			<h1>Jobs first. Exceptions visible. Records close at hand.</h1>
		</div>
		<div class="button-row">
			<a class="button secondary button-link" href="/revenue">Revenue View</a>
			<button class="button" onclick={() => runSampleTruth(false)} disabled={running}>
				Run Happy Path
			</button>
			<button class="button secondary" onclick={() => runSampleTruth(true)} disabled={running}>
				Run Blocked Path
			</button>
		</div>
	</header>

	{#if error}
		<section class="panel danger">
			<strong>Desktop shell error</strong>
			<p>{error}</p>
		</section>
	{/if}

	{#if loading}
		<section class="panel">
			<p>Loading operator shell...</p>
		</section>
	{:else}
		<div class="cockpit">
			<nav class="panel sidebar">
				<div class="sidebar-header">
					<h2>Workspace</h2>
					<p>Week 2 cleanup in progress on top of `crm-app`.</p>
				</div>

				<div class="nav-list">
					{#each navSections as item}
						<button
							class:active={activeSection === item.id}
							class="nav-button"
							onclick={() => (activeSection = item.id)}
						>
							<span>{item.label}</span>
						</button>
					{/each}
				</div>

				<section class="mini-panel">
					<div class="section-title">Accounts</div>
					<div class="mini-list">
						{#each organizations as organization}
							<div class:selected={selectedOrganizationId === organization.id} class="account-row">
								<button class="account-select" onclick={() => selectOrganization(organization.id)}>
									<strong>{organization.name}</strong>
									<span>{organization.lifecycle} · {organization.open_opportunity_count} open opps</span>
								</button>
								<a class="detail-link" href={`/accounts/${organization.id}`}>Open</a>
							</div>
						{/each}
					</div>
				</section>
			</nav>

			<main class="panel content">
				{#if activeSection === 'jobs'}
					<JobsSection {truths} {latestExecution} />
				{:else if activeSection === 'accounts'}
					<AccountsSection {account} />
				{:else if activeSection === 'workflows'}
					<WorkflowSection {workflows} />
				{:else if activeSection === 'approvals'}
					<ApprovalsSection {approvals} />
				{:else if activeSection === 'system'}
					<SystemSection {profile} />
				{/if}
			</main>

			<RightRail {dashboard} />
		</div>
	{/if}
</div>
