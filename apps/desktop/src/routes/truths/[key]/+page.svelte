<script lang="ts">
	import { page } from '$app/state'

	import { executeTruth, getTruthCatalogItem } from '$lib/api'
	import type {
		TruthExecutionInputs,
		TruthExecutionSession,
		TruthInputSchema,
		TruthListItem
	} from '$lib/types'
	import { truthInputSchemas } from '$lib/types'

	let truth = $state<TruthListItem | null>(null)
	let execution = $state<TruthExecutionSession | null>(null)
	let inputs = $state<TruthExecutionInputs>({})
	let loading = $state(true)
	let executing = $state(false)
	let error = $state('')
	let lastLoadedKey = $state('')

	const truthKey = $derived(page.params.key ?? '')
	const schema = $derived<TruthInputSchema | null>(
		truthKey ? (truthInputSchemas[truthKey] ?? null) : null
	)

	function normalizeFieldValue(value: string | number | boolean | undefined) {
		if (typeof value === 'boolean') return value ? 'true' : 'false'
		if (value === undefined) return ''
		return String(value)
	}

	function schemaDefaults(nextSchema: TruthInputSchema | null) {
		if (!nextSchema) return {}

		return Object.fromEntries(
			nextSchema.fields.map((field) => [field.key, normalizeFieldValue(field.defaultValue)])
		)
	}

	function cleanedInputs() {
		return Object.fromEntries(
			Object.entries(inputs).filter(([, value]) => value.trim() !== '')
		)
	}

	function applyPreset(values: TruthExecutionInputs) {
		inputs = {
			...schemaDefaults(schema),
			...values
		}
	}

	async function loadTruth(key: string) {
		loading = true
		error = ''
		execution = null

		try {
			truth = await getTruthCatalogItem(key)
			if (!truth) {
				error = `Truth not found: ${key}`
				inputs = {}
				return
			}

			inputs = schemaDefaults(schema)
		} catch (cause) {
			error = String(cause)
		} finally {
			loading = false
		}
	}

	async function runTruth() {
		if (!truth?.executable) return

		executing = true
		error = ''

		try {
			execution = await executeTruth(truth.key, cleanedInputs())
		} catch (cause) {
			error = String(cause)
		} finally {
			executing = false
		}
	}

	$effect(() => {
		if (truthKey && truthKey !== lastLoadedKey) {
			lastLoadedKey = truthKey
			void loadTruth(truthKey)
		}
	})
</script>

<div class="page route-page">
	<header class="hero">
		<div>
			<p class="eyebrow">Truth Detail</p>
			<h1>{truth?.display_name ?? truthKey}</h1>
			<p>
				Inspect catalog metadata, run the supported truth path, and inspect convergence output
				without leaving the operator shell.
			</p>
		</div>
		<div class="button-row">
			<a class="button secondary button-link" href="/">Back To Cockpit</a>
		</div>
	</header>

	{#if error}
		<section class="panel danger">
			<strong>Truth route error</strong>
			<p>{error}</p>
		</section>
	{/if}

	{#if loading}
		<section class="panel">
			<p>Loading truth detail...</p>
		</section>
	{:else if truth}
		<div class="route-stack">
			<section class="panel">
				<div class="row-between">
					<div>
						<div class="section-title">Metadata</div>
						<h2>{truth.display_name}</h2>
					</div>
					<span class:muted={!truth.executable} class="badge">
						{truth.executable ? 'executable' : 'catalog only'}
					</span>
				</div>

				<div class="detail-grid">
					<div class="card">
						<strong>Kind</strong>
						<div class="meta">{truth.kind}</div>
					</div>
					<div class="card">
						<strong>Packs</strong>
						<div class="pill-list">
							{#each truth.packs as pack}
								<span class="pill">{pack}</span>
							{/each}
						</div>
					</div>
				</div>

				<div class="content-section">
					<div class="section-title">Summary</div>
					<p>{truth.summary}</p>
				</div>
			</section>

			<section class="panel">
				<div class="row-between">
					<div class="section-title">Execution</div>
					{#if !truth.executable}
						<span class="badge muted">not yet executable in crm-app</span>
					{/if}
				</div>

				{#if truth.executable && schema}
					<div class="toolbar">
						{#each schema.presets ?? [] as preset}
							<button class="button secondary" onclick={() => applyPreset(preset.values)}>
								{preset.label}
							</button>
						{/each}
					</div>

					<div class="list compact">
						{#each schema.presets ?? [] as preset}
							<div class="list-item">
								<strong>{preset.label}</strong>
								<div class="meta">{preset.description}</div>
							</div>
						{/each}
					</div>

					<form class="form-grid" onsubmit={(event) => event.preventDefault()}>
						{#each schema.fields as field}
							<label class="field">
								<span>
									{field.label}
									{#if field.required}
										<span class="required">*</span>
									{/if}
								</span>

								{#if field.type === 'textarea'}
									<textarea
										rows="4"
										placeholder={field.placeholder}
										value={inputs[field.key] ?? ''}
										oninput={(event) => (inputs[field.key] = event.currentTarget.value)}
									></textarea>
								{:else if field.type === 'boolean'}
									<span class="checkbox-row">
										<input
											type="checkbox"
											checked={inputs[field.key] === 'true'}
											onchange={(event) =>
												(inputs[field.key] = event.currentTarget.checked ? 'true' : 'false')}
										/>
										<span>{field.description ?? 'Toggle this execution path.'}</span>
									</span>
								{:else}
									<input
										type={field.type}
										placeholder={field.placeholder}
										value={inputs[field.key] ?? ''}
										oninput={(event) => (inputs[field.key] = event.currentTarget.value)}
									/>
								{/if}

								{#if field.description && field.type !== 'boolean'}
									<span class="field-help">{field.description}</span>
								{/if}
							</label>
						{/each}
					</form>

					<div class="button-row left">
						<button class="button" onclick={runTruth} disabled={executing}>
							{executing ? 'Executing…' : 'Execute Truth'}
						</button>
					</div>
				{:else if truth.executable}
					<p class="empty">This truth is executable but does not have a route input schema yet.</p>
				{:else}
					<p class="empty">
						This truth is visible in the catalog, but the shared `crm-app` layer does not support
						executing it yet.
					</p>
				{/if}
			</section>

			{#if execution}
				<section class="panel">
					<div class="section-title">Execution Result</div>
					<div class="detail-grid">
						<div class="card">
							<strong>State</strong>
							<div class="meta">{execution.state}</div>
						</div>
						<div class="card">
							<strong>Cycles</strong>
							<div class="meta">{execution.result?.cycles ?? 0}</div>
						</div>
						<div class="card">
							<strong>Stop Reason</strong>
							<div class="meta">{execution.result?.stop_reason ?? execution.error ?? 'n/a'}</div>
						</div>
					</div>

					<div class="content-section">
						<div class="section-title">Criteria Outcomes</div>
						<div class="list">
							{#each execution.criteria_outcomes as outcome}
								<div class="list-item">
									<div class="row-between">
										<strong>{outcome.description}</strong>
										<div class="meta">{outcome.status}</div>
									</div>
									{#if outcome.detail}
										<div>{outcome.detail}</div>
									{/if}
									{#if outcome.approval_ref}
										<div class="meta">Approval ref: {outcome.approval_ref}</div>
									{/if}
								</div>
							{/each}
						</div>
					</div>

					<div class="content-section">
						<div class="section-title">Projection Summary</div>
						<div class="list compact">
							<div class="list-item">
								<strong>Organization</strong>
								<div class="meta">{execution.projection?.organization_id ?? 'none'}</div>
							</div>
							<div class="list-item">
								<strong>Opportunity</strong>
								<div class="meta">{execution.projection?.opportunity_id ?? 'none'}</div>
							</div>
							<div class="list-item">
								<strong>Workflow Cases</strong>
								<div class="meta">
									{execution.projection?.workflow_case_ids.length
										? execution.projection.workflow_case_ids.join(', ')
										: 'none'}
								</div>
							</div>
							<div class="list-item">
								<strong>Approvals</strong>
								<div class="meta">
									{execution.projection?.approval_ids.length
										? execution.projection.approval_ids.join(', ')
										: 'none'}
								</div>
							</div>
						</div>
					</div>
				</section>
			{/if}
		</div>
	{/if}
</div>
