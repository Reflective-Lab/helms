<script lang="ts">
	import { onMount } from 'svelte'

	import { getWorkflowCases } from '$lib/api'
	import { formatTime } from '$lib/format'
	import type { WorkflowCaseListItem } from '$lib/types'

	let workflows = $state<WorkflowCaseListItem[]>([])
	let loading = $state(true)
	let error = $state('')

	async function loadWorkflows() {
		loading = true
		error = ''

		try {
			workflows = await getWorkflowCases()
		} catch (cause) {
			error = String(cause)
		} finally {
			loading = false
		}
	}

	onMount(() => {
		void loadWorkflows()
	})
</script>

<div class="page route-page">
	<header class="hero">
		<div>
			<p class="eyebrow">Workflow View</p>
			<h1>Exceptions, approvals, and manual review cases.</h1>
			<p>
				Operator work queue containing cases that require human intervention before business state can advance.
			</p>
		</div>
		<div class="button-row">
			<a class="button secondary button-link" href="/">Back To Cockpit</a>
		</div>
	</header>

	{#if error}
		<section class="panel danger">
			<strong>Workflow route error</strong>
			<p>{error}</p>
		</section>
	{/if}

	{#if loading}
		<section class="panel">
			<p>Loading workflow cases...</p>
		</section>
	{:else}
		<div class="route-stack">
			<section class="panel">
				<div class="section-title">Workflow Queue</div>
				<div class="list">
					{#if workflows.length}
						{#each workflows as workflow}
							<div class="list-item">
								<div class="row-between">
									<strong>{workflow.title}</strong>
									<span class="badge">{workflow.state}</span>
								</div>
								<div>{workflow.definition_key}</div>
								<div class="meta">
									Priority: {workflow.priority} · Opened {formatTime(workflow.created_at)}
								</div>
							</div>
						{/each}
					{:else}
						<p class="empty">No active workflow cases in the queue.</p>
					{/if}
				</div>
			</section>
		</div>
	{/if}
</div>
