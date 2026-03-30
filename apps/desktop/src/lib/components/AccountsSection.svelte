<script lang="ts">
	import { formatMoney, formatTime } from '$lib/format'
	import type { AccountWorkspaceSummary } from '$lib/types'

	type Props = {
		account: AccountWorkspaceSummary | null
	}

	let { account }: Props = $props()
</script>

<section class="content-section">
	<div class="section-title">Account Workspace</div>
	{#if account}
		<div class="grid two">
			<div class="card">
				<strong>{account.organization.name}</strong>
				<div class="meta">
					{account.organization.lifecycle}
					{#if account.organization.industry}
						· {account.organization.industry}
					{/if}
				</div>
				<p>{account.organization.website ?? 'No website captured yet.'}</p>
			</div>
			<div class="card">
				<strong>People</strong>
				<div class="meta">{account.people.length} contacts</div>
				<p>
					{account.people[0]
						? `${account.people[0].full_name}${account.people[0].title ? ` · ${account.people[0].title}` : ''}`
						: 'No contacts yet.'}
				</p>
			</div>
			<div class="card">
				<strong>Opportunities</strong>
				<div class="meta">{account.opportunities.length} active records</div>
				<p>
					{account.opportunities[0]
						? `${account.opportunities[0].name} · ${formatMoney(account.opportunities[0].value_minor, account.opportunities[0].currency_code)}`
						: 'No opportunities projected yet.'}
				</p>
			</div>
			<div class="card">
				<strong>Entitlements</strong>
				<div class="meta">{account.entitlements.length} projected</div>
				<p>{account.entitlements[0]?.value_summary ?? 'No entitlements on this account yet.'}</p>
			</div>
		</div>

		<div class="content-section">
			<div class="section-title">Recent Timeline</div>
			<div class="list compact">
				{#each account.recent_timeline as event}
					<div class="list-item">
						<div class="row-between">
							<strong>{event.summary}</strong>
							<div class="meta">{formatTime(event.timestamp)}</div>
						</div>
						<div class="meta">{event.kind} · {event.actor}</div>
					</div>
				{/each}
			</div>
		</div>
	{:else}
		<p class="empty">Select an account from the left rail.</p>
	{/if}
</section>
