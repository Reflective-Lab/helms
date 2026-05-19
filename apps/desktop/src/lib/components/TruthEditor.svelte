<script lang="ts">
	type Props = {
		source: string
		path?: string
	}

	type Severity = 'error' | 'warning' | 'info'

	type EditorDiagnostic = {
		line: number
		severity: Severity
		message: string
	}

	type OutlineItem = {
		line: number
		kind: 'truth' | 'governance' | 'scenario' | 'background' | 'rule' | 'examples'
		label: string
	}

	type Analysis = {
		diagnostics: EditorDiagnostic[]
		outline: OutlineItem[]
		tags: string[]
		scenarioCount: number
		stepCount: number
		governanceCount: number
	}

	type HighlightRange = {
		start: number
		end: number
		className: string
	}

	let { source, path = '' }: Props = $props()

	let draft = $state('')
	let mode = $state<'edit' | 'preview'>('edit')
	let lastSource = $state('')

	const analysis = $derived(analyzeTruthSource(draft))
	const displayLines = $derived(draft.split('\n'))

	$effect(() => {
		if (source !== lastSource) {
			draft = source
			lastSource = source
		}
	})

	function resetDraft() {
		draft = source
	}

	function formatDraft() {
		draft = draft
			.split('\n')
			.map((line) => formatTruthLine(line))
			.join('\n')
	}

	function analyzeTruthSource(value: string): Analysis {
		const lines = value.split('\n')
		const diagnostics: EditorDiagnostic[] = []
		const outline: OutlineItem[] = []
		const tags = new Set<string>()
		const governanceSeen = new Set<string>()
		let scenarioCount = 0
		let stepCount = 0
		let governanceCount = 0
		let currentGovernance: string | null = null
		let hasTitle = false

		lines.forEach((line, index) => {
			const lineNumber = index + 1
			const trimmed = line.trim()

			if (!trimmed) return

			for (const tag of trimmed.matchAll(/@[A-Za-z0-9_:-]+/g)) {
				tags.add(tag[0])
			}

			if (/^(Truth|Feature):/.test(trimmed)) {
				hasTitle = true
				currentGovernance = null
				outline.push({
					line: lineNumber,
					kind: 'truth',
					label: trimmed
				})
				return
			}

			const governance = trimmed.match(/^(Intent|Authority|Constraint|Evidence|Exception):$/)
			if (governance) {
				const block = governance[1]
				if (governanceSeen.has(block)) {
					diagnostics.push({
						line: lineNumber,
						severity: 'error',
						message: `Duplicate ${block} block`
					})
				} else {
					governanceSeen.add(block)
					governanceCount += 1
				}
				currentGovernance = block
				outline.push({
					line: lineNumber,
					kind: 'governance',
					label: trimmed
				})
				return
			}

			if (/^(Scenario|Scenario Outline):/.test(trimmed)) {
				scenarioCount += 1
				currentGovernance = null
				outline.push({
					line: lineNumber,
					kind: 'scenario',
					label: trimmed
				})
				return
			}

			if (/^Background:/.test(trimmed)) {
				currentGovernance = null
				outline.push({ line: lineNumber, kind: 'background', label: trimmed })
				return
			}

			if (/^Rule:/.test(trimmed)) {
				currentGovernance = null
				outline.push({ line: lineNumber, kind: 'rule', label: trimmed })
				return
			}

			if (/^Examples?:/.test(trimmed)) {
				currentGovernance = null
				outline.push({ line: lineNumber, kind: 'examples', label: trimmed })
				return
			}

			if (/^(Given|When|Then|And|But)\b/.test(trimmed)) {
				stepCount += 1
				currentGovernance = null
				return
			}

			if (currentGovernance && /^[A-Za-z][A-Za-z ]*:/.test(trimmed)) {
				validateGovernanceField(currentGovernance, trimmed, lineNumber, diagnostics)
			}
		})

		if (!hasTitle) {
			diagnostics.unshift({
				line: 1,
				severity: 'error',
				message: 'Missing Truth or Feature header'
			})
		}

		if (hasTitle && scenarioCount === 0) {
			diagnostics.push({
				line: 1,
				severity: 'warning',
				message: 'No scenario declared'
			})
		}

		return {
			diagnostics,
			outline,
			tags: Array.from(tags).sort(),
			scenarioCount,
			stepCount,
			governanceCount
		}
	}

	function validateGovernanceField(
		block: string,
		line: string,
		lineNumber: number,
		diagnostics: EditorDiagnostic[]
	) {
		const [field, ...rest] = line.split(':')
		const value = rest.join(':').trim()
		const allowedFields: Record<string, string[]> = {
			Intent: ['Outcome', 'Goal'],
			Authority: ['Actor', 'May', 'Must Not', 'Requires Approval', 'Expires'],
			Constraint: ['Budget', 'Cost Limit', 'Must Not'],
			Evidence: ['Requires', 'Provenance', 'Audit'],
			Exception: ['Escalates To', 'Requires']
		}

		if (!allowedFields[block]?.includes(field.trim())) {
			diagnostics.push({
				line: lineNumber,
				severity: 'error',
				message: `Unknown ${block} field: ${field.trim()}`
			})
		}

		if (!value) {
			diagnostics.push({
				line: lineNumber,
				severity: 'warning',
				message: `${field.trim()} has no value`
			})
		}
	}

	function formatTruthLine(line: string) {
		const trimmed = line.trim()
		if (!trimmed) return ''
		if (/^(Truth|Feature):/.test(trimmed)) return trimmed
		if (/^#/.test(trimmed)) return trimmed.length > 1 && trimmed[1] !== ' ' ? `# ${trimmed.slice(1)}` : trimmed
		if (/^@/.test(trimmed)) return `  ${trimmed}`
		if (/^(Intent|Authority|Constraint|Evidence|Exception):$/.test(trimmed)) return `  ${trimmed}`
		if (/^(Rule|Background|Scenario|Scenario Outline|Examples|Example):/.test(trimmed)) return `  ${trimmed}`
		if (/^(Given|When|Then|And|But)\b/.test(trimmed)) return `    ${trimmed}`
		if (/^[A-Za-z][A-Za-z ]*:/.test(trimmed)) return `    ${trimmed}`
		if (/^\|/.test(trimmed)) return `    ${trimmed}`
		return line
	}

	function highlightedLine(line: string) {
		const ranges: HighlightRange[] = []
		const trimmedStart = line.search(/\S/)
		const start = trimmedStart === -1 ? 0 : trimmedStart
		const trimmed = line.slice(start)
		const commentIndex = line.indexOf('#')
		const commentStart = commentIndex >= 0 ? commentIndex : line.length

		addKeywordRange(line, start, trimmed, ranges)

		for (const match of line.matchAll(/@[A-Za-z0-9_:-]+/g)) {
			ranges.push({
				start: match.index ?? 0,
				end: (match.index ?? 0) + match[0].length,
				className: 'tok-tag'
			})
		}

		const quoteRe = /"[^"]*"/g
		for (const match of line.slice(0, commentStart).matchAll(quoteRe)) {
			ranges.push({
				start: match.index ?? 0,
				end: (match.index ?? 0) + match[0].length,
				className: 'tok-string'
			})
		}

		if (commentIndex >= 0) {
			ranges.push({ start: commentIndex, end: line.length, className: 'tok-comment' })
		}

		return renderHighlightedLine(line, ranges)
	}

	function addKeywordRange(line: string, start: number, trimmed: string, ranges: HighlightRange[]) {
		const keyword =
			trimmed.match(/^(Truth|Feature|Scenario Outline|Scenario|Rule|Background|Examples|Example):/)?.[0] ??
			trimmed.match(/^(Intent|Authority|Constraint|Evidence|Exception):/)?.[0] ??
			trimmed.match(/^(Given|When|Then|And|But)\b/)?.[0] ??
			trimmed.match(/^(Outcome|Goal|Actor|May|Must Not|Requires Approval|Expires|Budget|Cost Limit|Requires|Provenance|Audit|Escalates To):/)?.[0]

		if (!keyword) return

		ranges.push({
			start,
			end: Math.min(start + keyword.length, line.length),
			className: /^(Given|When|Then|And|But)/.test(keyword)
				? 'tok-step'
				: /^(Intent|Authority|Constraint|Evidence|Exception|Outcome|Goal|Actor|May|Must Not|Requires Approval|Expires|Budget|Cost Limit|Requires|Provenance|Audit|Escalates To)/.test(keyword)
					? 'tok-governance'
					: 'tok-keyword'
		})
	}

	function renderHighlightedLine(line: string, ranges: HighlightRange[]) {
		let cursor = 0
		let output = ''
		const sorted = ranges
			.filter((range) => range.end > range.start)
			.sort((left, right) => left.start - right.start || right.end - left.end)

		for (const range of sorted) {
			if (range.start < cursor) continue
			output += escapeHtml(line.slice(cursor, range.start))
			output += `<span class="${range.className}">${escapeHtml(line.slice(range.start, range.end))}</span>`
			cursor = range.end
		}

		output += escapeHtml(line.slice(cursor))
		return output || '&nbsp;'
	}

	function escapeHtml(value: string) {
		return value
			.replaceAll('&', '&amp;')
			.replaceAll('<', '&lt;')
			.replaceAll('>', '&gt;')
			.replaceAll('"', '&quot;')
			.replaceAll("'", '&#039;')
	}
</script>

<section class="panel truth-editor">
	<div class="editor-head">
		<div>
			<div class="section-title">Truth Source</div>
			<h2>{path || 'Catalog Source'}</h2>
		</div>
		<div class="editor-actions">
			<div class="mode-tabs" aria-label="Editor mode">
				<button class:active={mode === 'edit'} type="button" onclick={() => (mode = 'edit')}>
					Edit
				</button>
				<button class:active={mode === 'preview'} type="button" onclick={() => (mode = 'preview')}>
					Preview
				</button>
			</div>
			<button class="button secondary compact-button" type="button" onclick={formatDraft}>Format</button>
			<button class="button secondary compact-button" type="button" onclick={resetDraft}>Reset</button>
		</div>
	</div>

	<div class="editor-metrics">
		<span>{analysis.scenarioCount} scenarios</span>
		<span>{analysis.stepCount} steps</span>
		<span>{analysis.governanceCount} governance blocks</span>
		<span>{analysis.diagnostics.length} diagnostics</span>
	</div>

	<div class="editor-grid">
		<div class="source-pane">
			{#if mode === 'edit'}
				<textarea
					aria-label="Truth source"
					bind:value={draft}
					spellcheck="false"
					autocapitalize="off"
					autocomplete="off"
				></textarea>
			{:else}
				<pre aria-label="Highlighted truth source">{#each displayLines as line, index}<span class="code-line"><span class="line-number">{index + 1}</span><code>{@html highlightedLine(line)}</code></span>{/each}</pre>
			{/if}
		</div>

		<aside class="analysis-pane">
			<div class="analysis-section">
				<div class="section-title">Diagnostics</div>
				{#if analysis.diagnostics.length}
					<div class="analysis-list">
						{#each analysis.diagnostics as diagnostic}
							<div class:error={diagnostic.severity === 'error'} class="analysis-item">
								<span class="line-chip">L{diagnostic.line}</span>
								<span>{diagnostic.message}</span>
							</div>
						{/each}
					</div>
				{:else}
					<p class="empty">No local diagnostics.</p>
				{/if}
			</div>

			<div class="analysis-section">
				<div class="section-title">Outline</div>
				{#if analysis.outline.length}
					<div class="analysis-list">
						{#each analysis.outline as item}
							<div class="analysis-item">
								<span class="line-chip">L{item.line}</span>
								<span>{item.label}</span>
							</div>
						{/each}
					</div>
				{:else}
					<p class="empty">No outline.</p>
				{/if}
			</div>

			<div class="analysis-section">
				<div class="section-title">Tags</div>
				{#if analysis.tags.length}
					<div class="editor-tags">
						{#each analysis.tags as tag}
							<span>{tag}</span>
						{/each}
					</div>
				{:else}
					<p class="empty">No tags.</p>
				{/if}
			</div>
		</aside>
	</div>
</section>

<style>
	.truth-editor {
		display: grid;
		gap: 16px;
	}

	.editor-head,
	.editor-actions,
	.editor-metrics,
	.editor-grid,
	.mode-tabs,
	.analysis-item,
	.editor-tags {
		display: flex;
	}

	.editor-head {
		justify-content: space-between;
		align-items: start;
		gap: 16px;
	}

	.editor-head h2 {
		margin: 4px 0 0;
		overflow-wrap: anywhere;
	}

	.editor-actions {
		flex-wrap: wrap;
		justify-content: end;
		gap: 8px;
	}

	.mode-tabs {
		border: 1px solid var(--line);
		border-radius: 14px;
		background: rgba(255, 255, 255, 0.56);
		padding: 3px;
	}

	.mode-tabs button {
		appearance: none;
		border: 0;
		border-radius: 10px;
		background: transparent;
		color: var(--ink-muted);
		cursor: pointer;
		font: inherit;
		padding: 8px 11px;
	}

	.mode-tabs button.active {
		background: var(--paper-strong);
		color: var(--ink);
	}

	.compact-button {
		min-width: 86px;
		padding: 10px 12px;
	}

	.editor-metrics {
		flex-wrap: wrap;
		gap: 8px;
	}

	.editor-metrics span,
	.editor-tags span,
	.line-chip {
		border: 1px solid var(--line);
		border-radius: 999px;
		background: rgba(255, 255, 255, 0.58);
		color: var(--ink-soft);
		font-size: 0.78rem;
		padding: 6px 9px;
	}

	.editor-grid {
		align-items: stretch;
		display: grid;
		grid-template-columns: minmax(0, 1fr) 320px;
		gap: 14px;
	}

	.source-pane,
	.analysis-pane {
		border: 1px solid var(--line);
		border-radius: 18px;
		background: rgba(255, 255, 255, 0.62);
		min-width: 0;
		overflow: hidden;
	}

	.source-pane textarea,
	.source-pane pre {
		width: 100%;
		min-height: 520px;
		margin: 0;
		border: 0;
		background: #18202a;
		color: #edf4f8;
		font: 0.86rem/1.55 "SFMono-Regular", "Cascadia Code", "Liberation Mono", monospace;
		tab-size: 2;
	}

	.source-pane textarea {
		display: block;
		resize: vertical;
		padding: 16px;
	}

	.source-pane textarea:focus {
		outline: 2px solid rgba(17, 94, 89, 0.34);
		outline-offset: -2px;
	}

	.source-pane pre {
		overflow: auto;
		padding: 12px 0;
	}

	.code-line {
		display: grid;
		grid-template-columns: 52px minmax(0, 1fr);
		min-height: 1.55em;
		padding-right: 16px;
	}

	.line-number {
		color: rgba(237, 244, 248, 0.36);
		text-align: right;
		user-select: none;
		padding-right: 12px;
	}

	code {
		white-space: pre;
	}

	:global(.tok-keyword) {
		color: #8bd3ff;
		font-weight: 700;
	}

	:global(.tok-governance) {
		color: #a7f3d0;
		font-weight: 700;
	}

	:global(.tok-step) {
		color: #fbbf24;
		font-weight: 700;
	}

	:global(.tok-tag) {
		color: #f0abfc;
	}

	:global(.tok-string) {
		color: #fda4af;
	}

	:global(.tok-comment) {
		color: #94a3b8;
	}

	.analysis-pane {
		display: grid;
		align-content: start;
		gap: 16px;
		padding: 14px;
	}

	.analysis-section,
	.analysis-list {
		display: grid;
		gap: 10px;
	}

	.analysis-item {
		align-items: start;
		gap: 8px;
		color: var(--ink-muted);
		font-size: 0.9rem;
		line-height: 1.35;
	}

	.analysis-item.error {
		color: var(--warn);
	}

	.line-chip {
		flex: 0 0 auto;
		font-size: 0.72rem;
		padding: 4px 7px;
	}

	.editor-tags {
		flex-wrap: wrap;
		gap: 8px;
	}

	@media (max-width: 980px) {
		.editor-head,
		.editor-actions {
			display: grid;
			justify-content: stretch;
		}

		.editor-grid {
			grid-template-columns: 1fr;
		}

		.source-pane textarea,
		.source-pane pre {
			min-height: 420px;
		}
	}
</style>
