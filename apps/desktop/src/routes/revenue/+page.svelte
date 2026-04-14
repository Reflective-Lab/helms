<script lang="ts">
	import { onMount } from 'svelte'

	import { getCatalogItems, getSubscriptions } from '$lib/api'
	import { formatMoney, formatTime } from '$lib/format'
	import type { CatalogItemListItem, SubscriptionListItem } from '$lib/types'

	let subscriptions = $state<SubscriptionListItem[]>([])
	let catalogItems = $state<CatalogItemListItem[]>([])
	let loading = $state(true)
	let error = $state('')

	async function loadRevenue() {
		loading = true
		error = ''

		try {
			const [nextSubscriptions, nextCatalogItems] = await Promise.all([
				getSubscriptions(),
				getCatalogItems()
			])

			subscriptions = nextSubscriptions
			catalogItems = nextCatalogItems
		} catch (cause) {
			error = String(cause)
		} finally {
			loading = false
		}
	}

	onMount(() => {
		void loadRevenue()
	})
</script>

<div class="page route-page">
	<header class="hero">
		<div>
			<p class="eyebrow">Revenue View</p>
			<h1>Catalog and subscription state in one operator pass.</h1>
			<p>
				Use this route to inspect current revenue objects and copy the IDs needed for truth
				execution detail.
			</p>
		</div>
		<div class="button-row">
			<a class="button secondary button-link" href="/truths/activate-subscription">
				Activate Subscription
			</a>
			<a class="button secondary button-link" href="/truths/refill-prepaid-ai-credits">
				Refill Credits
			</a>
			<a class="button secondary button-link" href="/">Back To Workbench</a>
		</div>
	</header>

	{#if error}
		<section class="panel danger">
			<strong>Revenue route error</strong>
			<p>{error}</p>
		</section>
	{/if}

	{#if loading}
		<section class="panel">
			<p>Loading revenue view...</p>
		</section>
	{:else}
		<div class="route-stack">
			<section class="panel">
				<div class="section-title">Overview</div>
				<div class="detail-grid">
					<div class="card">
						<strong>Subscriptions</strong>
						<div class="meta">{subscriptions.length} records</div>
					</div>
					<div class="card">
						<strong>Catalog Items</strong>
						<div class="meta">{catalogItems.length} plans</div>
					</div>
				</div>
			</section>

			<div class="detail-grid">
				<section class="panel">
					<div class="row-between">
						<div class="section-title">Subscriptions</div>
						<a class="detail-link" href="/truths/activate-subscription">Run activation truth</a>
					</div>
					<div class="list">
						{#if subscriptions.length}
							{#each subscriptions as subscription}
								<div class="list-item">
									<div class="row-between">
										<div>
											<strong>{subscription.catalog_item_name ?? 'Unresolved plan'}</strong>
											<div class="meta">
												<a class="detail-link" href={`/accounts/${subscription.organization_id}`}>
													{subscription.organization_name}
												</a>
											</div>
										</div>
										<span class="badge">{subscription.status}</span>
									</div>
									<div>{formatMoney(subscription.value_minor, subscription.currency_code)}</div>
									<div class="meta">
										Subscription ID: {subscription.id}
										{#if subscription.catalog_item_id}
											· Catalog ID: {subscription.catalog_item_id}
										{/if}
									</div>
									<div class="meta">
										Started {formatTime(subscription.started_at)}
										{#if subscription.activated_at}
											· Activated {formatTime(subscription.activated_at)}
										{/if}
									</div>
								</div>
							{/each}
						{:else}
							<p class="empty">No subscriptions are available in the current desktop workspace.</p>
						{/if}
					</div>
				</section>

				<section class="panel">
					<div class="row-between">
						<div class="section-title">Catalog</div>
						<a class="detail-link" href="/truths/refill-prepaid-ai-credits">Run refill truth</a>
					</div>
					<div class="list">
						{#if catalogItems.length}
							{#each catalogItems as item}
								<div class="list-item">
									<div class="row-between">
										<div>
											<strong>{item.name}</strong>
											<div class="meta">{item.sku}</div>
										</div>
										<span class:muted={!item.active} class="badge">
											{item.active ? item.plan_kind : 'inactive'}
										</span>
									</div>
									{#if item.description}
										<div>{item.description}</div>
									{/if}
									<div class="meta">
										Catalog ID: {item.id}
										{#if item.billing_period}
											· {item.billing_period}
										{/if}
										{#if item.price_minor !== undefined && item.currency_code}
											· {formatMoney(item.price_minor, item.currency_code)}
										{/if}
									</div>
									{#if item.entitlements_summary.length}
										<div class="pill-list">
											{#each item.entitlements_summary as entitlement}
												<span class="pill">{entitlement}</span>
											{/each}
										</div>
									{/if}
								</div>
							{/each}
						{:else}
							<p class="empty">No catalog items are available in the current desktop workspace.</p>
						{/if}
					</div>
				</section>
			</div>
		</div>
	{/if}
</div>
