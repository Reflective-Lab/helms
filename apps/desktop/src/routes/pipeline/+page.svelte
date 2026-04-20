<script lang="ts">
	import { onMount, onDestroy } from 'svelte'

	const apiBase = import.meta.env.PUBLIC_CRM_API_BASE_URL || 'http://127.0.0.1:8081'

	interface PipelineEvent {
		type: string
		[key: string]: unknown
	}

	interface PendingApproval {
		approval_ref: string
		truth_key: string
		step: number
		reason: string
		created_at: string
	}

	let events: PipelineEvent[] = $state([])
	let status: string = $state('idle')
	let runId: string | null = $state(null)
	let pendingApprovals: PendingApproval[] = $state([])
	let prospectId: string = $state('prospect-001')
	let eventSource: EventSource | null = null

	const steps = [
		{ key: 'score-inbound-fit', label: 'Score Fit', icon: '1' },
		{ key: 'qualify-inbound-lead', label: 'Qualify Lead', icon: '2' },
		{ key: 'schedule-strategic-meetings', label: 'Schedule Meeting', icon: '3' }
	]

	let stepStates: Record<string, string> = $state({})

	function connectSSE() {
		if (eventSource) eventSource.close()
		eventSource = new EventSource(`${apiBase}/v1/pipeline/showcase/stream`)
		eventSource.onmessage = (e) => {
			const event: PipelineEvent = JSON.parse(e.data)
			events = [...events, event]

			switch (event.type) {
				case 'pipeline-started':
					status = 'running'
					stepStates = {}
					break
				case 'step-started':
					stepStates[event['truth_key'] as string] = 'running'
					break
				case 'step-completed':
					stepStates[event['truth_key'] as string] = 'completed'
					break
				case 'step-blocked':
					stepStates[event['truth_key'] as string] = 'blocked'
					fetchApprovals()
					break
				case 'step-failed':
					stepStates[event['truth_key'] as string] = 'failed'
					break
				case 'pipeline-completed':
					status = event['status'] as string
					break
			}
		}
	}

	async function startPipeline() {
		events = []
		status = 'starting'
		stepStates = {}
		pendingApprovals = []

		const res = await fetch(`${apiBase}/v1/pipeline/showcase/run`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ prospect_id: prospectId })
		})
		const data = await res.json()
		runId = data.run_id
		status = 'running'
	}

	async function resetPipeline() {
		await fetch(`${apiBase}/v1/pipeline/showcase/reset`, { method: 'POST' })
		events = []
		status = 'idle'
		runId = null
		stepStates = {}
		pendingApprovals = []
	}

	async function fetchApprovals() {
		const res = await fetch(`${apiBase}/v1/approvals/pending`)
		pendingApprovals = await res.json()
	}

	async function approve(ref: string) {
		await fetch(`${apiBase}/v1/approvals/${ref}/approve`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ reason: 'approved by operator' })
		})
		await fetchApprovals()
	}

	async function reject(ref: string) {
		const reason = prompt('Rejection reason:')
		if (!reason) return
		await fetch(`${apiBase}/v1/approvals/${ref}/reject`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ reason })
		})
		await fetchApprovals()
	}

	onMount(() => {
		connectSSE()
	})

	onDestroy(() => {
		if (eventSource) eventSource.close()
	})

	function stepColor(key: string): string {
		switch (stepStates[key]) {
			case 'running': return 'text-blue-400 animate-pulse'
			case 'completed': return 'text-green-400'
			case 'blocked': return 'text-yellow-400'
			case 'failed': return 'text-red-400'
			default: return 'text-gray-500'
		}
	}
</script>

<div class="p-6 max-w-4xl mx-auto">
	<div class="flex items-center justify-between mb-8">
		<h1 class="text-2xl font-bold">Pipeline Showcase</h1>
		<div class="flex gap-2">
			<select bind:value={prospectId} class="px-3 py-1 rounded bg-gray-800 border border-gray-600 text-sm">
				<option value="prospect-001">HighIntent (NovaTech)</option>
				<option value="prospect-002">DeepTechnical (DataForge)</option>
				<option value="prospect-003">TireKicker (BuzzMetrics)</option>
				<option value="prospect-004">Enterprise (GlobalScale)</option>
				<option value="prospect-005">QuickWin (SwiftOps)</option>
			</select>
			<button onclick={startPipeline} disabled={status === 'running'}
				class="px-4 py-1 bg-blue-600 rounded text-sm disabled:opacity-50">
				Run Pipeline
			</button>
			<button onclick={resetPipeline} class="px-4 py-1 bg-gray-700 rounded text-sm">
				Reset
			</button>
		</div>
	</div>

	<!-- Step Progress -->
	<div class="flex items-center gap-4 mb-8 p-4 bg-gray-900 rounded-lg">
		{#each steps as step, i}
			<div class="flex items-center gap-2 {stepColor(step.key)}">
				<div class="w-8 h-8 rounded-full border-2 flex items-center justify-center text-sm font-bold
					{stepStates[step.key] === 'completed' ? 'border-green-400 bg-green-900' : ''}
					{stepStates[step.key] === 'running' ? 'border-blue-400 bg-blue-900' : ''}
					{stepStates[step.key] === 'blocked' ? 'border-yellow-400 bg-yellow-900' : ''}
					{stepStates[step.key] === 'failed' ? 'border-red-400 bg-red-900' : ''}
					{!stepStates[step.key] ? 'border-gray-600' : ''}">
					{step.icon}
				</div>
				<span class="text-sm font-medium">{step.label}</span>
			</div>
			{#if i < steps.length - 1}
				<div class="flex-1 h-0.5 bg-gray-700"></div>
			{/if}
		{/each}
	</div>

	<!-- Pending Approvals -->
	{#if pendingApprovals.length > 0}
		<div class="mb-6 p-4 bg-yellow-900/30 border border-yellow-700 rounded-lg">
			<h2 class="text-lg font-semibold text-yellow-400 mb-3">Awaiting Approval</h2>
			{#each pendingApprovals as approval}
				<div class="flex items-center justify-between p-3 bg-gray-900 rounded mb-2">
					<div>
						<p class="font-medium">{approval.truth_key}</p>
						<p class="text-sm text-gray-400">{approval.reason}</p>
					</div>
					<div class="flex gap-2">
						<button onclick={() => approve(approval.approval_ref)}
							class="px-3 py-1 bg-green-700 rounded text-sm">Approve</button>
						<button onclick={() => reject(approval.approval_ref)}
							class="px-3 py-1 bg-red-700 rounded text-sm">Reject</button>
					</div>
				</div>
			{/each}
		</div>
	{/if}

	<!-- Event Timeline -->
	<div class="bg-gray-900 rounded-lg p-4">
		<h2 class="text-lg font-semibold mb-3">Convergence Timeline</h2>
		<div class="space-y-1 max-h-96 overflow-y-auto font-mono text-xs">
			{#if events.length === 0}
				<p class="text-gray-500">No events yet. Run the pipeline to see live convergence.</p>
			{/if}
			{#each events as event, i}
				<div class="flex gap-2 py-0.5
					{event.type === 'step-completed' ? 'text-green-400' : ''}
					{event.type === 'step-blocked' ? 'text-yellow-400' : ''}
					{event.type === 'step-failed' ? 'text-red-400' : ''}
					{event.type === 'pipeline-completed' ? 'text-blue-400 font-bold' : ''}
					{event.type === 'step-started' ? 'text-blue-300' : ''}
					{event.type === 'fact-proposed' ? 'text-gray-400' : ''}">
					<span class="text-gray-600 w-6">{i + 1}</span>
					<span>{event.type}</span>
					{#if event.truth_key}<span class="text-gray-500">({event.truth_key})</span>{/if}
					{#if event.fact_count}<span class="text-gray-500">{event.fact_count} facts</span>{/if}
					{#if event.reason}<span class="text-yellow-600">- {event.reason}</span>{/if}
				</div>
			{/each}
		</div>
	</div>

	<!-- Status Badge -->
	<div class="mt-4 text-sm text-gray-500">
		Status: <span class="font-medium
			{status === 'completed' ? 'text-green-400' : ''}
			{status === 'running' ? 'text-blue-400' : ''}
			{status === 'blocked' ? 'text-yellow-400' : ''}
			{status === 'failed' ? 'text-red-400' : ''}
			{status === 'idle' ? 'text-gray-400' : ''}">{status}</span>
		{#if runId}<span class="ml-2 text-gray-600">run: {runId.slice(0, 8)}</span>{/if}
	</div>
</div>
