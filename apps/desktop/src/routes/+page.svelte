<script lang="ts">
	const activeJobs = [
		{
			title: 'Convert website intent into account signals',
			status: 'Listening to analyticsIngest and analyticsAggregates',
			note: 'Anonymous sessions will later correlate into people and organizations.'
		},
		{
			title: 'Govern LinkedIn research before promotion',
			status: 'Runtime module: linkedin-scan',
			note: 'Evidence stays proposed until approval or policy promotion.'
		},
		{
			title: 'Route follow-up work through optimization',
			status: 'Module planned: lead-routing',
			note: 'Priority is derived from CRM state and first-party usage, not inbox order.'
		}
	]

	const approvals = [
		'Promote “high-intent pricing revisit” from usage signal to account fact',
		'Approve LinkedIn-derived stakeholder graph for Northwind',
		'Confirm pipeline stage move before quote generation'
	]

	const timeline = [
		{
			kicker: 'FACT PROMOTION',
			headline: 'Pricing-page revisit elevated to account signal',
			body: 'Three sessions from the same company domain crossed the JTBD threshold.'
		},
		{
			kicker: 'WORKFLOW CASE',
			headline: 'Security review opened for enterprise opportunity',
			body: 'Waiting on product constraints, evidence pack, and human approval.'
		},
		{
			kicker: 'COMMUNICATION',
			headline: 'Discovery call captured against opportunity',
			body: 'Champion confirmed need for governed agent workflows and audit trail.'
		}
	]

	const surfaces = ['crm.prio.ai', 'analytics.prio.ai', 'hr.prio.ai', 'plan.prio.ai']
</script>

<div class="page">
	<section class="hero">
		<p class="eyebrow">Prio CRM Shell</p>
		<h1>Jobs first. Records second.</h1>
		<p>
			This desktop surface is intentionally thin. Converge owns interaction and orchestration;
			the CRM kernel owns durable business truth, auditability, metadata, and workflow state.
		</p>
		<div class="pill-row">
			{#each surfaces as surface}
				<div class="pill">{surface}</div>
			{/each}
		</div>
	</section>

	<div class="layout">
		<div class="stack">
			<section class="panel">
				<h2>Active Jobs</h2>
				<p>The main UI is a workspace for intent, execution, and promotion gates.</p>

				<div class="list">
					{#each activeJobs as job}
						<div class="list-item">
							<strong>{job.title}</strong>
							<div class="meta">{job.status}</div>
							<div>{job.note}</div>
						</div>
					{/each}
				</div>
			</section>

			<section class="panel">
				<h2>Account Summary</h2>
				<div class="grid two">
					<div class="card">
						<strong>Northwind</strong>
						<div class="meta">Active opportunity · CTO champion · enterprise evaluation</div>
					</div>
					<div class="card">
						<strong>Behavioral context</strong>
						<div class="meta">Pricing revisits, release-readiness traffic, podcast affinity</div>
					</div>
					<div class="card">
						<strong>Open workflow cases</strong>
						<div class="meta">Security review, procurement path, approval sequencing</div>
					</div>
					<div class="card">
						<strong>Metadata layer</strong>
						<div class="meta">Custom object: usage_event · Saved view: high-intent visits</div>
					</div>
				</div>
			</section>

			<section class="panel">
				<h2>Timeline</h2>
				<div class="list">
					{#each timeline as item}
						<div class="list-item">
							<div class="kicker">{item.kicker}</div>
							<strong>{item.headline}</strong>
							<div>{item.body}</div>
						</div>
					{/each}
				</div>
			</section>
		</div>

		<div class="stack">
			<section class="panel">
				<h3>Approvals</h3>
				<div class="list">
					{#each approvals as approval}
						<div class="list-item">
							<div class="kicker">HUMAN REQUIRED</div>
							<strong>{approval}</strong>
						</div>
					{/each}
				</div>
			</section>

			<section class="panel">
				<h3>Runtime Hooks</h3>
				<div class="list">
					<div class="list-item">
						<div class="kicker">ANALYTICS</div>
						<strong>First-party site events only</strong>
						<div>Use Firebase functions and your own event model. Plausible stays off.</div>
					</div>
					<div class="list-item">
						<div class="kicker">OPTIMIZATION</div>
						<strong>Lead routing and queue pressure</strong>
						<div>Use converge-optimization as soon as signal quality is credible.</div>
					</div>
					<div class="list-item">
						<div class="kicker">LINKEDIN</div>
						<strong>Research module needs runtime work</strong>
						<div>The provider seam exists in Converge, but the production integration is not there yet.</div>
					</div>
				</div>
			</section>

			<section class="panel warning">
				<h3>Exceptions</h3>
				<div class="list">
					<div class="list-item">
						<div class="kicker">WARNING</div>
						<strong>Do not drift toward a classic CRM UI</strong>
						<div>Humans should navigate jobs, summaries, approvals, and exceptions, not raw tables all day.</div>
					</div>
					<div class="list-item">
						<div class="kicker">WARNING</div>
						<strong>Keep metadata and workflow first-class</strong>
						<div>Standard objects are not enough if custom signals and domain growth matter.</div>
					</div>
				</div>
			</section>
		</div>
	</div>
</div>

