<script lang="ts">
	import { formatTime } from '$lib/format'
	import type { ApprovalListItem } from '$lib/types'

	type Props = {
		approvals: ApprovalListItem[]
	}

	let { approvals }: Props = $props()
</script>

<section class="content-section">
	<div class="section-title">Approval Queue</div>
	<div class="list">
		{#each approvals as approval}
			<div class="list-item">
				<div class="row-between">
					<strong>{approval.reason}</strong>
					<div class="meta">{approval.status}</div>
				</div>
				<div>{approval.truth_key}</div>
				<div class="meta">Created {formatTime(approval.created_at)}</div>
			</div>
		{/each}
	</div>
</section>
