<script lang="ts">
	import { onMount, onDestroy } from 'svelte'
	import type { JobState, TimelineEntry } from '$lib/types'
	import { getTruths, getTimeline, executeTruth, healthCheck } from '$lib/api'
	import { createPoller } from '$lib/poll'
	import ConnectionStatus from './components/ConnectionStatus.svelte'
	import JobStateView from './components/JobStateView.svelte'
	import FactTimeline from './components/FactTimeline.svelte'

	let connected = $state(false)
	let secondsAgo = $state(0)
	let jobs = $state<JobState[]>([])
	let timeline = $state<TimelineEntry[]>([])
	let error = $state<string | null>(null)

	// Demo: track executed truths as "active jobs"
	// In production this would come from a job-state API
	let executedJobs = $state<JobState[]>([])

	async function refresh() {
		try {
			connected = await healthCheck()
			if (!connected) return

			const [timelineData] = await Promise.all([
				getTimeline(10)
			])
			timeline = timelineData

			// Merge persisted jobs with any new timeline data
			jobs = executedJobs
			error = null
		} catch (e) {
			error = e instanceof Error ? e.message : 'Unknown error'
			connected = false
		}
	}

	const poller = createPoller(async () => {
		await refresh()
		secondsAgo = 0
	}, 3000)

	// Update the seconds counter every second
	let secondsTimer: ReturnType<typeof setInterval>

	onMount(() => {
		poller.start()
		secondsTimer = setInterval(() => {
			secondsAgo = poller.secondsSinceUpdate()
		}, 1000)
	})

	onDestroy(() => {
		poller.stop()
		clearInterval(secondsTimer)
	})

	async function handleExecuteDemo() {
		try {
			const result = await executeTruth('qualify-inbound-lead', {
				organization_name: 'Acme Corp',
				inbound_summary: 'Enterprise customer onboarding — wants governed CRM with audit trail and AI-driven workflows.',
				contact_name: 'Sarah Chen',
				contact_title: 'VP Operations',
				contact_email: 'sarah@acme.example',
				website: 'https://acme.example',
				owner_user_id: 'karl',
				next_step: 'Schedule qualification review',
				opportunity_value_minor: '5040000',
				require_manual_review: 'true',
				manual_review_reason: '$50,400 ARR exceeds auto-approve threshold.'
			})

			const job: JobState = {
				truth: result.truth,
				execution: result.execution,
				projection: result.projection,
				executed_at: new Date().toISOString()
			}
			executedJobs = [job, ...executedJobs]
			await refresh()
		} catch (e) {
			error = e instanceof Error ? e.message : 'Execution failed'
		}
	}

	async function handleApprove(truthKey: string, _approvalRef: string) {
		// For the demo: re-execute the truth which will show the approval flow
		// In production, this would call a dedicated approval endpoint
		try {
			await executeTruth(truthKey, { approve: 'true' })
			await refresh()
		} catch (e) {
			error = e instanceof Error ? e.message : 'Approval failed'
		}
	}
</script>

<div class="panel-root">
	<div class="panel-header">
		<div>
			<p class="eyebrow">Converge</p>
			<h1>Job State</h1>
		</div>
		<ConnectionStatus {connected} {secondsAgo} />
	</div>

	{#if error}
		<div class="blocked-callout">
			<strong>Error</strong>
			<p>{error}</p>
		</div>
	{/if}

	{#if !connected}
		<div class="card">
			<strong>Waiting for backend</strong>
			<div class="meta">Start the server with <code>just server</code> on port 8081.</div>
		</div>
	{:else}
		<div class="section">
			<div style="display: flex; justify-content: space-between; align-items: center;">
				<h3 class="section-title">Active Jobs</h3>
				<button class="btn primary" onclick={handleExecuteDemo}>
					Run Demo
				</button>
			</div>

			{#if jobs.length === 0}
				<div class="card">
					<strong>No active jobs</strong>
					<div class="meta">Click "Run Demo" to execute the onboarding scenario.</div>
				</div>
			{:else}
				{#each jobs as job}
					<JobStateView {job} onapprove={handleApprove} />
				{/each}
			{/if}
		</div>

		<FactTimeline entries={timeline} />
	{/if}
</div>
