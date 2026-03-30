<script lang="ts">
	import { formatTime } from '$lib/format'
	import type { WorkflowCaseListItem } from '$lib/types'

	type Props = {
		workflows: WorkflowCaseListItem[]
	}

	let { workflows }: Props = $props()
</script>

<section class="content-section">
	<div class="row-between">
		<div class="section-title">Workflow Cases</div>
		<a class="detail-link" href="/workflow">Open full queue</a>
	</div>
	<div class="list">
		{#each workflows as workflow}
			<div class="list-item">
				<div class="row-between">
					<strong>{workflow.title}</strong>
					<div class="meta">{workflow.state}</div>
				</div>
				<div>{workflow.definition_key}</div>
				<div class="meta">{workflow.priority} · opened {formatTime(workflow.created_at)}</div>
			</div>
		{/each}
	</div>
</section>
