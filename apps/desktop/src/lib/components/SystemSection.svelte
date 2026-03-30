<script lang="ts">
	import type { SystemProfile } from '$lib/types'

	type Props = {
		profile: SystemProfile | null
	}

	let { profile }: Props = $props()
</script>

<section class="content-section">
	<div class="section-title">System Profile</div>
	{#if profile}
		<div class="grid two">
			<div class="card">
				<strong>Feature Toggles</strong>
				<div class="list compact">
					<div class="list-item">
						<strong>Analytics</strong>
						<div class="meta">{profile.feature_toggles.analytics_enabled ? 'enabled' : 'disabled'}</div>
					</div>
					<div class="list-item">
						<strong>Optimization</strong>
						<div class="meta">{profile.feature_toggles.optimization_enabled ? 'enabled' : 'disabled'}</div>
					</div>
					<div class="list-item">
						<strong>LLM</strong>
						<div class="meta">{profile.feature_toggles.llm_enabled ? 'enabled' : 'disabled'}</div>
					</div>
				</div>
			</div>

			<div class="card">
				<strong>Supported Truths</strong>
				<div class="list compact">
					{#each profile.feature_toggles.supported_truth_keys as key}
						<div class="list-item">{key}</div>
					{/each}
				</div>
			</div>
		</div>

		<div class="content-section">
			<div class="section-title">Modules</div>
			<div class="list compact">
				{#each profile.modules as module}
					<div class="list-item">
						<div class="row-between">
							<strong>{module.display_name}</strong>
							<div class="meta">{module.suite}</div>
						</div>
						<div>{module.key}</div>
					</div>
				{/each}
			</div>
		</div>
	{/if}
</section>
