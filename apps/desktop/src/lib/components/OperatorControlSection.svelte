<script lang="ts">
	import type {
		EvidenceReadinessStatus,
		OperatorControlPreview,
		OperatorLedgerEntry,
		ReceiptFamily
	} from '$lib/types'

	type Props = {
		previews: OperatorControlPreview[]
	}

	let { previews }: Props = $props()
	let selectedPacketId = $state('')
	const preview = $derived.by(
		() =>
			previews.find((candidate) => candidate.packet.packet_id === selectedPacketId) ??
			previews[0] ??
			null
	)

	function formatToken(value: string) {
		return value.replaceAll('_', ' ').replaceAll('-', ' ')
	}

	function shortId(value: string) {
		if (value.length <= 26) {
			return value
		}
		return `${value.slice(0, 16)}...${value.slice(-8)}`
	}

	function evidenceTone(status: EvidenceReadinessStatus) {
		if (status === 'present') {
			return 'ready'
		}
		if (status === 'missing' || status === 'blocked') {
			return 'blocked'
		}
		return 'attention'
	}

	function familyTitle(family: ReceiptFamily) {
		return formatToken(family)
	}

	function ledgerTitle(entry: OperatorLedgerEntry) {
		return `${formatToken(entry.record_kind)} #${entry.sequence}`
	}

	function formatBasisPoints(value: number) {
		const whole = Math.floor(value / 100)
		const fraction = value % 100
		if (fraction === 0) {
			return `${whole}%`
		}
		return `${whole}.${String(fraction).padStart(2, '0').replace(/0+$/, '')}%`
	}

	function packetButtonLabel(candidate: OperatorControlPreview) {
		return `${candidate.packet.job_key} - ${candidate.packet.domain_hint}`
	}
</script>

<section class="content-section operator-control">
	<div class="section-head">
		<div>
			<p class="eyebrow">Operator Control</p>
			<h2>Readiness Packet</h2>
		</div>
		{#if preview}
			<span class="badge muted">{preview.packet.domain_hint}</span>
		{/if}
	</div>

	{#if preview}
		{#if previews.length > 1}
			<div class="packet-selector" aria-label="Operator control packets">
				{#each previews as candidate}
					<button
						type="button"
						class:selected={candidate.packet.packet_id === preview.packet.packet_id}
						aria-label={packetButtonLabel(candidate)}
						onclick={() => (selectedPacketId = candidate.packet.packet_id)}
					>
						<strong>{candidate.packet.job_key}</strong>
						<span>{candidate.packet.domain_hint}</span>
					</button>
				{/each}
			</div>
		{:else}
			<span class="badge muted">{previews.length} readiness packet</span>
		{/if}

		<div class="operator-grid">
			<article class="card packet-card">
				<div class="row-between">
					<div>
						<div class="section-title">Job</div>
						<strong>{preview.packet.job_key}</strong>
					</div>
					<span class={`badge ${preview.packet.verdict ?? 'muted'}`}>
						{formatToken(preview.packet.verdict ?? 'pending')}
					</span>
				</div>
				<div class="kv-grid compact">
					<div class="kv-row">
						<span>Package</span>
						<strong>{preview.packet.package_id}</strong>
					</div>
					<div class="kv-row">
						<span>Subject</span>
						<strong>{preview.packet.subject_ref}</strong>
					</div>
					<div class="kv-row">
						<span>Adapter</span>
						<strong>{preview.packet.adapter_receipt_id}</strong>
					</div>
					<div class="kv-row">
						<span>Packet</span>
						<strong>{shortId(preview.packet.packet_id)}</strong>
					</div>
				</div>
				<div class="boundary-row">
					<span class="badge">authority effect: none</span>
					<span class="badge muted">
						domain action: {preview.packet.authorizes_domain_action ? 'yes' : 'no'}
					</span>
				</div>
			</article>

			<article class="card">
				<div class="section-title">Operator Actions</div>
				<div class="list compact">
					{#each preview.packet.operator_actions as action}
						<div class="list-item">
							<strong>{action}</strong>
						</div>
					{/each}
				</div>
			</article>
		</div>

		<section class="operator-block">
			<div class="section-title">Evidence Readiness</div>
			<div class="readiness-list">
				{#each preview.packet.evidence_status as evidence}
					<div class={`readiness-row ${evidenceTone(evidence.status)}`}>
						<div>
							<strong>{evidence.label}</strong>
							<div class="meta">{evidence.clause_key}</div>
						</div>
						<div class="readiness-meta">
							<span class="badge">{formatToken(evidence.status)}</span>
							{#if evidence.fact_ids.length}
								<span>{evidence.fact_ids.length} facts</span>
							{/if}
							{#if evidence.concern_record_ids.length}
								<span>{evidence.concern_record_ids.length} concerns</span>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		</section>

		{#if preview.packet.fuzzy_trace}
			<section class="operator-grid">
				<article class="card fuzzy-card">
					<div class="row-between">
						<div>
							<div class="section-title">Fuzzy Trace</div>
							<strong>{formatToken(preview.packet.fuzzy_trace.variable_key)}</strong>
						</div>
						<span class="badge muted">
							{formatBasisPoints(preview.packet.fuzzy_trace.observed_value_basis_points)}
						</span>
					</div>
					<div class="score-list">
						{#if preview.packet.fuzzy_trace.defuzzified_score}
							<div class="score-row">
								<span>{formatToken(preview.packet.fuzzy_trace.defuzzified_score.method)} score</span>
								<strong>
									{formatBasisPoints(
										preview.packet.fuzzy_trace.defuzzified_score.score_basis_points
									)}
								</strong>
							</div>
						{/if}
						{#each preview.packet.fuzzy_trace.memberships as membership}
							<div class="score-row">
								<span>{formatToken(membership.label)}</span>
								<strong>{formatBasisPoints(membership.score_basis_points)}</strong>
							</div>
						{/each}
					</div>
				</article>

				<article class="card">
					<div class="section-title">Activated Rules</div>
					<div class="list compact">
						{#each preview.packet.fuzzy_trace.activated_rules as rule}
							<div class="list-item">
								<strong>{rule.rule_id}</strong>
								<div class="meta">{rule.conclusion}</div>
								<span class="badge muted">{formatBasisPoints(rule.strength_basis_points)}</span>
							</div>
						{:else}
							<p class="empty">No fuzzy rules activated.</p>
						{/each}
					</div>
				</article>
			</section>
		{/if}

		<section class="operator-grid">
			<article class="card">
				<div class="section-title">Ledger Entries</div>
				<div class="list compact">
					{#each preview.ledger_entries as entry}
						<div class="list-item">
							<strong>{ledgerTitle(entry)}</strong>
							<div class="meta">{entry.summary}</div>
							<div class="boundary-row">
								<span class="badge muted">{formatToken(entry.receipt_family)}</span>
								<span class="badge muted">{formatToken(entry.authority_effect)}</span>
							</div>
							<div class="mono-line">{shortId(entry.payload_hash)}</div>
						</div>
					{/each}
				</div>
			</article>

			<article class="card">
				<div class="section-title">Receipt Families</div>
				<div class="family-list">
					{#each preview.receipt_families as family}
						<div class="family-row">
							<strong>{familyTitle(family.family)}</strong>
							<span>{family.record_kinds.length} record kinds</span>
						</div>
					{/each}
				</div>
			</article>
		</section>

		<section class="operator-block">
			<div class="section-title">Forbidden Actions</div>
			<div class="pill-list">
				{#each preview.packet.verifier_forbidden_actions as action}
					<span class="pill warning">{action}</span>
				{/each}
			</div>
		</section>
	{:else}
		<p class="empty">No operator-control preview loaded.</p>
	{/if}
</section>
