<script lang="ts">
	import { formatTime } from '$lib/format'
	import type { OperatorDashboard } from '$lib/types'

	type Props = {
		dashboard: OperatorDashboard | null
	}

	let { dashboard }: Props = $props()
</script>

<aside class="panel aside">
	<section class="content-section">
		<div class="section-title">Approvals</div>
		<div class="list compact">
			{#if dashboard?.approvals.length}
				{#each dashboard.approvals as approval}
					<div class="list-item">
						<strong>{approval.reason}</strong>
						<div class="meta">{approval.truth_key}</div>
					</div>
				{/each}
			{:else}
				<p class="empty">No pending approvals.</p>
			{/if}
		</div>
	</section>

	<section class="content-section">
		<div class="section-title">Exceptions</div>
		<div class="list compact">
			{#if dashboard?.exceptions.length}
				{#each dashboard.exceptions as exception}
					<div class="list-item">
						<strong>{exception.title}</strong>
						<div class="meta">{exception.state}</div>
					</div>
				{/each}
			{:else}
				<p class="empty">No blocked workflow cases.</p>
			{/if}
		</div>
	</section>

	<section class="content-section">
		<div class="section-title">Recent Timeline</div>
		<div class="list compact">
			{#if dashboard?.recent_timeline.length}
				{#each dashboard.recent_timeline as event}
					<div class="list-item">
						<strong>{event.summary}</strong>
						<div class="meta">{event.kind} · {formatTime(event.timestamp)}</div>
					</div>
				{/each}
			{:else}
				<p class="empty">No timeline events yet.</p>
			{/if}
		</div>
	</section>
</aside>
