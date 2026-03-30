<script lang="ts">
	import type { TruthExecutionSession, TruthListItem } from '$lib/types'

	type Props = {
		truths: TruthListItem[]
		latestExecution: TruthExecutionSession | null
	}

	let { truths, latestExecution }: Props = $props()
</script>

<section class="content-section">
	<div class="section-title">Truth Catalog</div>
	<div class="list">
		{#each truths as truth}
			<div class="list-item">
				<div class="row-between">
					<div>
						<strong>{truth.display_name}</strong>
						<div class="meta">{truth.kind}</div>
					</div>
					<span class:muted={!truth.executable} class="badge">
						{truth.executable ? 'executable' : 'catalog only'}
					</span>
				</div>
				<div>{truth.summary}</div>
				{#if truth.packs.length}
					<div class="meta">Packs: {truth.packs.join(', ')}</div>
				{/if}
				<div>
					<a class="detail-link" href={`/truths/${truth.key}`}>Open truth detail</a>
				</div>
			</div>
		{/each}
	</div>
</section>

<section class="content-section">
	<div class="section-title">Latest Execution</div>
	{#if latestExecution}
		<div class="card">
			<div class="row-between">
				<strong>{latestExecution.truth_key}</strong>
				<span class="badge">{latestExecution.state}</span>
			</div>
			<p>{latestExecution.result?.stop_reason ?? latestExecution.error ?? 'No result yet.'}</p>
			<div class="list compact">
				{#each latestExecution.criteria_outcomes as outcome}
					<div class="list-item">
						<strong>{outcome.description}</strong>
						<div class="meta">{outcome.status}</div>
						{#if outcome.detail}
							<div>{outcome.detail}</div>
						{/if}
					</div>
				{/each}
			</div>
		</div>
	{:else}
		<p class="empty">Run one of the sample actions to exercise the shared app layer.</p>
	{/if}
</section>
