<script lang="ts">
	import type { TimelineEntry } from '$lib/types'
	import { formatTime } from '$lib/format'

	type Props = {
		entries: TimelineEntry[]
	}

	let { entries }: Props = $props()
</script>

<div class="section">
	<h3 class="section-title">Timeline</h3>
	{#if entries.length === 0}
		<p class="empty">No activity yet.</p>
	{:else}
		<div class="timeline-list">
			{#each entries as entry}
				<div class="timeline-item">
					<span class="timeline-summary">{entry.summary}</span>
					<div class="timeline-meta">
						<span>{entry.actor}</span>
						<span>{formatTime(entry.timestamp)}</span>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
