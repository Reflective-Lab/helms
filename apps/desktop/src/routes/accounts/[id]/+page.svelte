<script lang="ts">
	import { page } from '$app/state'

	import { getAccountSummary } from '$lib/api'
	import { formatMoney, formatTime } from '$lib/format'
	import type { AccountWorkspaceSummary } from '$lib/types'

	let account = $state<AccountWorkspaceSummary | null>(null)
	let loading = $state(true)
	let error = $state('')
	let lastLoadedId = $state('')

	const accountId = $derived(page.params.id ?? '')

	async function loadAccount(id: string) {
		loading = true
		error = ''

		try {
			account = await getAccountSummary(id)
		} catch (cause) {
			error = String(cause)
		} finally {
			loading = false
		}
	}

	$effect(() => {
		if (accountId && accountId !== lastLoadedId) {
			lastLoadedId = accountId
			void loadAccount(accountId)
		}
	})
</script>

<div class="page route-page">
	<header class="hero">
		<div>
			<p class="eyebrow">Account Detail</p>
			<h1>{account?.organization.name ?? 'Account Workspace'}</h1>
			<p>
				Organization context, commercial records, entitlements, and recent timeline in one
				operator view.
			</p>
		</div>
		<div class="button-row">
			<a class="button secondary button-link" href="/">Back To Cockpit</a>
		</div>
	</header>

	{#if error}
		<section class="panel danger">
			<strong>Account route error</strong>
			<p>{error}</p>
		</section>
	{/if}

	{#if loading}
		<section class="panel">
			<p>Loading account detail...</p>
		</section>
	{:else if account}
		<div class="route-stack">
			<section class="panel">
				<div class="section-title">Organization</div>
				<div class="detail-grid">
					<div class="card">
						<strong>Name</strong>
						<div class="meta">{account.organization.name}</div>
					</div>
					<div class="card">
						<strong>Lifecycle</strong>
						<div class="meta">{account.organization.lifecycle}</div>
					</div>
					<div class="card">
						<strong>Industry</strong>
						<div class="meta">{account.organization.industry ?? 'Not captured'}</div>
					</div>
					<div class="card">
						<strong>Owner</strong>
						<div class="meta">{account.organization.owner_user_id ?? 'Unassigned'}</div>
					</div>
				</div>
				{#if account.organization.website}
					<div class="content-section">
						<div class="section-title">Website</div>
						<p>{account.organization.website}</p>
					</div>
				{/if}
			</section>

			<section class="panel">
				<div class="section-title">People</div>
				<div class="list">
					{#each account.people as person}
						<div class="list-item">
							<div class="row-between">
								<strong>{person.full_name}</strong>
								<div class="meta">{person.title ?? 'No title'}</div>
							</div>
							<div class="meta">{person.email ?? 'No email captured'}</div>
						</div>
					{/each}
				</div>
			</section>

			<section class="panel">
				<div class="section-title">Opportunities</div>
				<div class="list">
					{#each account.opportunities as opportunity}
						<div class="list-item">
							<div class="row-between">
								<strong>{opportunity.name}</strong>
								<div class="meta">{opportunity.stage}</div>
							</div>
							<div>{formatMoney(opportunity.value_minor, opportunity.currency_code)}</div>
							<div class="meta">
								Confidence {opportunity.confidence_bps / 100}%{#if opportunity.next_step}
									· {opportunity.next_step}
								{/if}
							</div>
						</div>
					{/each}
				</div>
			</section>

			<div class="detail-grid">
				<section class="panel">
					<div class="section-title">Subscriptions</div>
					<div class="list compact">
						{#if account.subscriptions.length}
							{#each account.subscriptions as subscription}
								<div class="list-item">
									<strong>{subscription.status}</strong>
									<div class="meta">
										{formatMoney(subscription.value_minor, subscription.currency_code)} · started
										{formatTime(subscription.started_at)}
									</div>
								</div>
							{/each}
						{:else}
							<p class="empty">No subscriptions projected on this account.</p>
						{/if}
					</div>
				</section>

				<section class="panel">
					<div class="section-title">Entitlements</div>
					<div class="list compact">
						{#if account.entitlements.length}
							{#each account.entitlements as entitlement}
								<div class="list-item">
									<strong>{entitlement.key}</strong>
									<div class="meta">{entitlement.value_summary}</div>
								</div>
							{/each}
						{:else}
							<p class="empty">No entitlements projected on this account.</p>
						{/if}
					</div>
				</section>
			</div>

			<section class="panel">
				<div class="section-title">Recent Timeline</div>
				<div class="list">
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
			</section>
		</div>
	{/if}
</div>
