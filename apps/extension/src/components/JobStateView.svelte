<script lang="ts">
	import type { JobState } from '$lib/types'
	import { formatTime } from '$lib/format'

	type Props = {
		job: JobState
		onapprove?: (truthKey: string, approvalRef: string) => void
	}

	let { job, onapprove }: Props = $props()

	let metCount = $derived(job.execution.criteria.filter((c) => c.status === 'met').length)
	let totalCount = $derived(job.execution.criteria.length)
	let progressPct = $derived(totalCount > 0 ? (metCount / totalCount) * 100 : 0)
	let blockedCriteria = $derived(job.execution.criteria.filter((c) => c.status === 'blocked'))
	let hasBlocked = $derived(blockedCriteria.length > 0)

	function statusIcon(status: string): string {
		switch (status) {
			case 'met':
				return '✓'
			case 'blocked':
				return '⧖'
			case 'unmet':
				return '○'
			case 'indeterminate':
				return '?'
			default:
				return '·'
		}
	}
</script>

<div class="card">
	<div style="display: flex; justify-content: space-between; align-items: start;">
		<strong>{job.truth.display_name}</strong>
		<span class="badge" class:converged={job.execution.converged} class:running={!job.execution.converged && !hasBlocked} class:blocked={hasBlocked}>
			{#if job.execution.converged}
				converged
			{:else if hasBlocked}
				blocked
			{:else}
				running
			{/if}
		</span>
	</div>

	<div class="meta">{job.truth.summary}</div>

	<div class="progress-bar">
		<div class="progress-fill" style="width: {progressPct}%"></div>
	</div>
	<div class="meta">Convergence: {metCount}/{totalCount} criteria met</div>

	<div class="criteria-list">
		{#each job.execution.criteria as criterion}
			<div class="criterion">
				<span class="criterion-icon" class:met={criterion.status === 'met'} class:blocked={criterion.status === 'blocked'}>
					{statusIcon(criterion.status)}
				</span>
				<div class="criterion-body">
					<span>{criterion.description}</span>
					{#if criterion.detail}
						<span class="criterion-detail">{criterion.detail}</span>
					{/if}
					{#if criterion.status === 'blocked' && criterion.approval_ref && onapprove}
						<div class="actions" style="margin-top: 4px;">
							<button class="btn primary" onclick={() => onapprove?.(job.truth.key, criterion.approval_ref!)}>
								Approve
							</button>
							<button class="btn">Escalate</button>
						</div>
					{/if}
				</div>
			</div>
		{/each}
	</div>

	<div class="meta" style="margin-top: 4px;">
		{formatTime(job.executed_at)} · {job.execution.cycles} cycle{job.execution.cycles !== 1 ? 's' : ''}
	</div>
</div>
