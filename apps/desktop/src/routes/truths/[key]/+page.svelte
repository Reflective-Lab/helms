<script lang="ts">
	import { page } from '$app/state'

	import TruthEditor from '$lib/components/TruthEditor.svelte'
	import { executeTruth, getTruthDetail } from '$lib/api'
	import type {
		TruthDetailItem,
		TruthExecutionInputs,
		TruthExecutionSession,
		TruthInputSchema,
	} from '$lib/types'
	import { truthInputSchemas } from '$lib/types'

	let truth = $state<TruthDetailItem | null>(null)
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
			truth = await getTruthDetail(key)
			inputs = schemaDefaults(schema)
		} catch (cause) {
			error = String(cause)
			truth = null
			inputs = {}
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
			<a class="button secondary button-link" href="/">Back To Workbench</a>
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

				<div class="detail-grid">
					<div class="card">
						<strong>Feature</strong>
						<div class="meta">{truth.feature_path}</div>
					</div>
					<div class="card">
						<strong>Actors</strong>
						<div class="pill-list">
							{#each truth.actor_roles as actor}
								<span class="pill">{actor}</span>
							{/each}
						</div>
					</div>
				</div>

				<div class="content-section">
					<div class="section-title">Desired Outcomes</div>
					<div class="list compact">
						{#each truth.desired_outcomes as outcome}
							<div class="list-item">{outcome}</div>
						{/each}
					</div>
				</div>

				<div class="content-section">
					<div class="section-title">Guardrails</div>
					<div class="list compact">
						{#each truth.guardrails as guardrail}
							<div class="list-item">{guardrail}</div>
						{/each}
					</div>
				</div>

				<div class="content-section">
					<div class="section-title">Modules</div>
					<div class="list compact">
						{#each truth.modules as module}
							<div class="list-item">
								<div class="row-between">
									<strong>{module.module_key}</strong>
								</div>
								<div class="meta">{module.responsibility}</div>
							</div>
						{/each}
					</div>
				</div>
			</section>

			<TruthEditor source={truth.gherkin} path={truth.feature_path} />

			{#if truth.organism_resolution}
				<section class="panel">
					<div class="row-between">
						<div>
							<div class="section-title">Organism Resolution</div>
							<h2>{truth.organism_resolution.truth_key}</h2>
						</div>
						<span class:muted={!truth.organism_resolution.readiness.ready} class="badge">
							{truth.organism_resolution.readiness.ready ? 'ready' : 'has readiness gaps'}
						</span>
					</div>

					<div class="detail-grid">
						<div class="card">
							<strong>Blueprint</strong>
							<div class="meta">{truth.organism_resolution.blueprint ?? 'none'}</div>
						</div>
						<div class="card">
							<strong>Completeness</strong>
							<div class="meta">{truth.organism_resolution.completeness_confidence_bps} bps</div>
						</div>
					</div>

					<div class="content-section">
						<div class="section-title">Resolution Levels</div>
						<div class="meta">
							Attempted: {truth.organism_resolution.levels_attempted.join(', ') || 'none'}
						</div>
						<div class="meta">
							Contributed: {truth.organism_resolution.levels_contributed.join(', ') || 'none'}
						</div>
					</div>

					<div class="content-section">
						<div class="section-title">Packs</div>
						<div class="list compact">
							{#each truth.organism_resolution.packs as pack}
								<div class="list-item">
									<div class="row-between">
										<strong>{pack.pack_name}</strong>
										<div class="meta">{pack.confidence_bps} bps · {pack.source}</div>
									</div>
									<div>{pack.reason}</div>
								</div>
							{/each}
						</div>
					</div>

					<div class="content-section">
						<div class="section-title">Capabilities</div>
						{#if truth.organism_resolution.capabilities.length}
							<div class="list compact">
								{#each truth.organism_resolution.capabilities as capability}
									<div class="list-item">
										<div class="row-between">
											<strong>{capability.capability}</strong>
											<div class="meta">{capability.confidence_bps} bps · {capability.source}</div>
										</div>
										<div>{capability.reason}</div>
									</div>
								{/each}
							</div>
						{:else}
							<p class="empty">No extra capability requirements declared.</p>
						{/if}
					</div>

					<div class="content-section">
						<div class="section-title">Invariants</div>
						{#if truth.organism_resolution.invariants.length}
							<div class="pill-list">
								{#each truth.organism_resolution.invariants as invariant}
									<span class="pill">{invariant}</span>
								{/each}
							</div>
						{:else}
							<p class="empty">No extra invariants declared beyond pack defaults.</p>
						{/if}
					</div>

					<div class="content-section">
						<div class="section-title">Readiness Confirmed</div>
						{#if truth.organism_resolution.readiness.confirmed.length}
							<div class="list compact">
								{#each truth.organism_resolution.readiness.confirmed as item}
									<div class="list-item">
										<div class="row-between">
											<strong>{item.resource}</strong>
											<div class="meta">{item.kind}</div>
										</div>
										<div class="meta">{item.detail}</div>
									</div>
								{/each}
							</div>
						{:else}
							<p class="empty">No readiness confirmations recorded.</p>
						{/if}
					</div>

					<div class="content-section">
						<div class="section-title">Readiness Gaps</div>
						{#if truth.organism_resolution.readiness.gaps.length}
							<div class="list compact">
								{#each truth.organism_resolution.readiness.gaps as gap}
									<div class="list-item">
										<div class="row-between">
											<strong>{gap.resource}</strong>
											<div class="meta">{gap.kind} · {gap.severity}</div>
										</div>
										<div>{gap.reason}</div>
										{#if gap.suggestion}
											<div class="meta">{gap.suggestion}</div>
										{/if}
									</div>
								{/each}
							</div>
						{:else}
							<p class="empty">No readiness gaps.</p>
						{/if}
					</div>
				</section>
			{/if}

			{#if truth.converge_resolution}
				<section class="panel">
					<div class="section-title">Converge Resolution</div>

					<div class="detail-grid">
						<div class="card">
							<strong>Runtime</strong>
							<div class="meta">{truth.converge_resolution.runtime}</div>
						</div>
						<div class="card">
							<strong>Intent Kind</strong>
							<div class="meta">{truth.converge_resolution.intent_kind}</div>
						</div>
					</div>

					<div class="content-section">
						<div class="section-title">Request</div>
						<p>{truth.converge_resolution.request}</p>
					</div>

					<div class="content-section">
						<div class="section-title">Pack IDs</div>
						<div class="pill-list">
							{#each truth.converge_resolution.pack_ids as packId}
								<span class="pill">{packId}</span>
							{/each}
						</div>
					</div>

					<div class="content-section">
						<div class="section-title">Required Success Criteria</div>
						<div class="list compact">
							{#each truth.converge_resolution.required_success_criteria as criterion}
								<div class="list-item">{criterion}</div>
							{/each}
						</div>
					</div>

					<div class="content-section">
						<div class="section-title">Hard Constraints</div>
						<div class="list compact">
							{#each truth.converge_resolution.hard_constraints as constraint}
								<div class="list-item">{constraint}</div>
							{/each}
						</div>
					</div>
				</section>
			{/if}

			<section class="panel">
				<div class="row-between">
					<div class="section-title">Execution</div>
					{#if !truth.executable}
						<span class="badge muted">not yet executable in the workbench backend</span>
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
						This truth is visible in the catalog, but the shared workbench backend does not support
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
