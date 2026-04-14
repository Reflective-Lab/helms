<script lang="ts">
	import { onMount } from 'svelte'

	import { compareReceiptOcr, getExpenseItems, getExpenseReports, getReceiptSamples } from '$lib/api'
	import { expensePovItems, expensePovReports } from '$lib/expense-pov'
	import { formatMoney, formatTime } from '$lib/format'
	import type {
		ExpenseItem,
		ExpenseOcrRun,
		ExpenseReceiptSample,
		ExpenseReport
	} from '$lib/types'

	const inboxAddress = 'receipts@expenses.local'

	const captureChannels = [
		{
			title: 'Email Forwarding',
			detail: 'Forward receipts and PDFs to a shared inbox alias for automatic report capture.'
		},
		{
			title: 'Mobile Camera',
			detail: 'Snap paper receipts on the phone and push them into the same review queue.'
		},
		{
			title: 'Desktop Upload',
			detail: 'Drag PDFs from card portals or booking confirmations directly into a draft report.'
		},
		{
			title: 'Card Feed Later',
			detail: 'Reserve a later connector for card statements without blocking the first release.'
		}
	]

	let reports = $state<ExpenseReport[]>([])
	let items = $state<ExpenseItem[]>([])
	let receiptSamples = $state<ExpenseReceiptSample[]>([])
	let ocrRuns = $state<ExpenseOcrRun[]>([])
	let selectedReportId = $state<string | null>(null)
	let selectedSampleId = $state<string | null>(null)
	let loading = $state(true)
	let ocrLoading = $state(false)
	let error = $state('')
	let fallbackNotice = $state('')
	let ocrNotice = $state('')
	let ocrError = $state('')

	const selectedReport = $derived(reports.find((report) => report.id === selectedReportId) ?? null)
	const selectedItems = $derived(
		selectedReportId ? items.filter((item) => item.report_id === selectedReportId) : []
	)
	const selectedSample = $derived(
		receiptSamples.find((sample) => sample.sample_id === selectedSampleId) ?? null
	)
	const bestRun = $derived.by(() => {
		const candidates = ocrRuns.filter((run) => run.status === 'completed' && run.benchmark)
		return (
			[...candidates].sort(
				(left, right) =>
					runScore(right) - runScore(left) ||
					right.benchmark!.matched_fields - left.benchmark!.matched_fields
			)[0] ?? null
		)
	})
	const totals = $derived.by(() => ({
		reports: reports.length,
		review: reports.filter((report) => report.status === 'in-review').length,
		exportPending: reports.filter((report) => report.status === 'export-pending').length,
		flaggedItems: items.filter((item) => item.policy_flags.length > 0).length
	}))

	function itemCount(reportId: string) {
		return items.filter((item) => item.report_id === reportId).length
	}

	function flaggedCount(reportId: string) {
		return items.filter((item) => item.report_id === reportId && item.policy_flags.length > 0).length
	}

	function sortedFields(fields: Record<string, string>) {
		return Object.entries(fields).sort(([left], [right]) => left.localeCompare(right))
	}

	function runScore(run: ExpenseOcrRun) {
		if (!run.benchmark || run.benchmark.compared_fields === 0) return -1
		return run.benchmark.matched_fields / run.benchmark.compared_fields
	}

	function benchmarkLabel(run: ExpenseOcrRun) {
		if (!run.benchmark) return 'no benchmark'
		return `${run.benchmark.matched_fields}/${run.benchmark.compared_fields} fields matched`
	}

	function selectReport(reportId: string) {
		selectedReportId = reportId
	}

	async function runReceiptComparison(sampleId: string) {
		ocrLoading = true
		ocrError = ''
		try {
			ocrRuns = await compareReceiptOcr(sampleId)
		} catch (cause) {
			ocrRuns = []
			ocrError = String(cause)
		} finally {
			ocrLoading = false
		}
	}

	async function selectSample(sampleId: string) {
		selectedSampleId = sampleId
		await runReceiptComparison(sampleId)
	}

	async function loadExpenses() {
		loading = true
		error = ''
		fallbackNotice = ''
		ocrNotice = ''
		ocrError = ''

		try {
			const [reportResult, itemResult, sampleResult] = await Promise.allSettled([
				getExpenseReports(),
				getExpenseItems(),
				getReceiptSamples()
			])

			if (reportResult.status === 'fulfilled' && itemResult.status === 'fulfilled') {
				reports = reportResult.value
				items = itemResult.value
				fallbackNotice = ''

				if (!selectedReportId && reports.length > 0) {
					selectedReportId =
						reports.find((report) => report.status === 'in-review')?.id ?? reports[0].id
				}
			} else {
				reports = expensePovReports
				items = expensePovItems
				fallbackNotice =
					'Live expense data is unavailable. Showing the local two-receipt PoV fixture instead.'

				if (!selectedReportId && reports.length > 0) {
					selectedReportId =
						reports.find((report) => report.status === 'in-review')?.id ?? reports[0].id
				}
			}

			if (sampleResult.status === 'fulfilled') {
				receiptSamples = sampleResult.value
				if (!selectedSampleId && receiptSamples.length > 0) {
					selectedSampleId = receiptSamples[0].sample_id
				}
				if (selectedSampleId) {
					await runReceiptComparison(selectedSampleId)
				} else {
					ocrNotice =
						'Receipt OCR comparison is available in the desktop runtime when local fixtures exist.'
				}
			} else {
				receiptSamples = []
				ocrRuns = []
				ocrNotice =
					'Receipt OCR comparison is only active in the desktop runtime with local fixture access.'
			}
		} catch (cause) {
			console.error(cause)
			error = String(cause)
		} finally {
			loading = false
		}
	}

	onMount(() => {
		void loadExpenses()
	})
</script>

<div class="page route-page">
	<header class="hero">
		<div>
			<p class="eyebrow">Expense Desk</p>
			<h1>Receipt intake, OCR review, submission, and manual export.</h1>
			<p>
				Keep the business process visible end to end: receipts arrive through one queue, OCR
				engines compete against the reference set, reviewers resolve ambiguity, and the final
				report is ready to copy into the current booking system.
			</p>
		</div>
		<div class="button-row">
			<a class="button secondary button-link" href="/">Back To Workbench</a>
			<a class="button secondary button-link" href="/truths/submit-expense-report">
				Open Submit Truth
			</a>
			<button class="button" onclick={() => void loadExpenses()} disabled={loading || ocrLoading}>
				{loading ? 'Refreshing…' : 'Refresh'}
			</button>
		</div>
	</header>

	{#if error}
		<section class="panel danger">
			<strong>Expense route error</strong>
			<p>{error}</p>
		</section>
	{/if}

	{#if fallbackNotice}
		<section class="panel">
			<div class="row-between">
				<strong>Receipt PoV Fixture</strong>
				<span class="badge">offline fallback</span>
			</div>
			<p>{fallbackNotice}</p>
		</section>
	{/if}

	{#if loading}
		<section class="panel">
			<p>Loading expense workspace...</p>
		</section>
	{:else}
		<div class="route-stack">
			<section class="panel">
				<div class="row-between">
					<div>
						<div class="section-title">Inbox</div>
						<h2>{inboxAddress}</h2>
					</div>
					<span class="badge">email-first intake</span>
				</div>
				<p>
					Everything still converges on one queue. Email is the default, mobile and desktop uploads
					feed the same desk, and card feeds can join later without changing the review model.
				</p>
				<div class="stat-grid">
					<div class="card">
						<strong>{totals.reports}</strong>
						<div class="meta">reports in workspace</div>
					</div>
					<div class="card">
						<strong>{totals.review}</strong>
						<div class="meta">reports waiting on review</div>
					</div>
					<div class="card">
						<strong>{totals.exportPending}</strong>
						<div class="meta">ready for booking export</div>
					</div>
					<div class="card">
						<strong>{totals.flaggedItems}</strong>
						<div class="meta">items with policy or OCR flags</div>
					</div>
				</div>
			</section>

			<section class="panel">
				<div class="section-title">Process</div>
				<div class="detail-grid">
					{#each captureChannels as channel}
						<div class="card">
							<strong>{channel.title}</strong>
							<div>{channel.detail}</div>
						</div>
					{/each}
				</div>
			</section>

			<div class="expense-layout">
				<section class="panel expense-sidebar">
					<div class="row-between">
						<div class="section-title">Reports</div>
						<span class="meta">{reports.length} total</span>
					</div>
					<div class="list compact">
						{#if reports.length}
							{#each reports as report}
								<button
									class:selected={selectedReportId === report.id}
									class="expense-report-button"
									onclick={() => selectReport(report.id)}
								>
									<div class="row-between">
										<strong>{report.title}</strong>
										<span class="badge">{report.status}</span>
									</div>
									<div class="meta">{report.employee_name} · {report.employee_email}</div>
									<div class="meta">
										{itemCount(report.id)} items
										{#if flaggedCount(report.id)}
											· {flaggedCount(report.id)} flagged
										{/if}
									</div>
									<div>{formatMoney(report.total_minor, report.currency_code)}</div>
								</button>
							{/each}
						{:else}
							<p class="empty">No expense reports exist yet.</p>
						{/if}
					</div>
				</section>

				<section class="panel expense-detail">
					{#if selectedReport}
						<div class="row-between">
							<div>
								<div class="section-title">Selected Report</div>
								<h2>{selectedReport.title}</h2>
								<p>{selectedReport.employee_name} · {selectedReport.employee_email}</p>
							</div>
							<div class="expense-total">
								<span class="badge">{selectedReport.status}</span>
								<strong>{formatMoney(selectedReport.total_minor, selectedReport.currency_code)}</strong>
							</div>
						</div>

						<div class="detail-grid">
							<div class="card">
								<strong>Submission</strong>
								<div class="meta">
									Created {formatTime(selectedReport.created_at)}
									{#if selectedReport.submitted_at}
										· Submitted {formatTime(selectedReport.submitted_at)}
									{/if}
								</div>
								{#if selectedReport.description}
									<div>{selectedReport.description}</div>
								{/if}
							</div>
							<div class="card">
								<strong>Booking Export</strong>
								<div class="meta">
									{selectedReport.booking_export_reference ?? 'Manual export queue'}
								</div>
								<div>
									Submission is only one stage. The desk still has to prove receipt quality, make
									approval state explicit, and leave finance with a clean handoff.
								</div>
							</div>
						</div>

						<section class="content-section">
							<div class="row-between">
								<div class="section-title">Report Items</div>
								<span class="meta">{selectedItems.length} line items</span>
							</div>
							<div class="list">
								{#if selectedItems.length}
									{#each selectedItems as item}
										<div class="list-item">
											<div class="row-between">
												<div>
													<strong>{item.merchant}</strong>
													<div class="meta">
														{item.category} · {item.capture_source} · {formatTime(item.occurred_at)}
													</div>
												</div>
												<div class="expense-item-amount">
													<div>{formatMoney(item.amount.amount_minor, item.amount.currency_code)}</div>
													<span class="badge">{item.ocr_status}</span>
												</div>
											</div>
											{#if item.description}
												<div>{item.description}</div>
											{/if}
											{#if item.extracted_summary}
												<div class="meta">
													OCR: {item.ocr_engine ?? 'not set'} · {item.extracted_summary}
												</div>
											{/if}
											{#if Object.keys(item.ocr_fields).length}
												<div class="kv-grid">
													{#each sortedFields(item.ocr_fields) as [key, value]}
														<div class="kv-row">
															<span class="meta">{key}</span>
															<strong>{value}</strong>
														</div>
													{/each}
												</div>
											{/if}
											{#if item.policy_flags.length}
												<div class="pill-list">
													{#each item.policy_flags as flag}
														<span class="pill">{flag}</span>
													{/each}
												</div>
											{/if}
										</div>
									{/each}
								{:else}
									<p class="empty">No items are attached to this report yet.</p>
								{/if}
							</div>
						</section>
					{:else}
						<p class="empty">Select a report to inspect line items and export readiness.</p>
					{/if}
				</section>
			</div>

			<section class="panel">
				<div class="row-between">
					<div>
						<div class="section-title">Receipt Lab</div>
						<h2>Reference truth vs. OCR engines</h2>
					</div>
					<div class="button-row left">
						{#if selectedSample}
							<button
								class="button"
								onclick={() => void runReceiptComparison(selectedSample.sample_id)}
								disabled={ocrLoading}
							>
								{ocrLoading ? 'Running OCR…' : 'Run OCR Comparison'}
							</button>
						{/if}
					</div>
				</div>
				<p>
					The expense app should not guess. Each sample has an editable YAML reference, and each
					engine is scored against that reference before it earns the right to automate more of the
					process.
				</p>

				{#if ocrNotice}
					<p class="meta">{ocrNotice}</p>
				{/if}

				<div class="receipt-lab">
					<section class="expense-sidebar">
						<div class="row-between">
							<div class="section-title">Samples</div>
							<span class="meta">{receiptSamples.length} total</span>
						</div>
						<div class="list compact">
							{#if receiptSamples.length}
								{#each receiptSamples as sample}
									<button
										class:selected={selectedSampleId === sample.sample_id}
										class="expense-report-button"
										onclick={() => void selectSample(sample.sample_id)}
									>
										<div class="row-between">
											<strong>{sample.sample_id}</strong>
											<span class="badge">{sample.capture_type}</span>
										</div>
										<div class="meta">{sample.document_file}</div>
										<div class="meta">
											{Object.keys(sample.expected_fields).length} reference fields
											{#if !sample.expense_candidate}
												· non-expense
											{/if}
										</div>
									</button>
								{/each}
							{:else}
								<p class="empty">No receipt fixtures are available in this runtime.</p>
							{/if}
						</div>
					</section>

					<section class="expense-detail">
						{#if selectedSample}
							<div class="row-between">
								<div>
									<div class="section-title">Selected Sample</div>
									<h2>{selectedSample.sample_id}</h2>
									<p>{selectedSample.original_file_name}</p>
								</div>
								<div class="expense-total">
									<span class="badge">{selectedSample.reference_status}</span>
									<strong>{selectedSample.document_type}</strong>
								</div>
							</div>

							<div class="detail-grid">
								<div class="card">
									<strong>Fixture</strong>
									<div class="meta">{selectedSample.document_file}</div>
									<div>{selectedSample.document_path}</div>
								</div>
								<div class="card">
									<strong>Reference</strong>
									<div class="meta">{selectedSample.reference_path}</div>
									<div>
										{Object.keys(selectedSample.expected_fields).length} expected fields
										{#if selectedSample.report_id}
											· linked to expense report
										{/if}
									</div>
								</div>
							</div>

							{#if selectedSample.notes.length}
								<section class="content-section">
									<div class="section-title">Reference Notes</div>
									<div class="pill-list">
										{#each selectedSample.notes as note}
											<span class="pill">{note}</span>
										{/each}
									</div>
								</section>
							{/if}

							<section class="content-section">
								<div class="row-between">
									<div class="section-title">Reference Fields</div>
									<span class="meta">{Object.keys(selectedSample.expected_fields).length} fields</span>
								</div>
								<div class="kv-grid">
									{#each sortedFields(selectedSample.expected_fields) as [key, value]}
										<div class="kv-row">
											<span class="meta">{key}</span>
											<strong>{value}</strong>
										</div>
									{/each}
								</div>
							</section>

							<section class="content-section">
								<div class="row-between">
									<div class="section-title">Engine Comparison</div>
									{#if bestRun}
										<span class="meta">
											Best current match: {bestRun.engine} · {benchmarkLabel(bestRun)}
										</span>
									{/if}
								</div>

								{#if ocrError}
									<div class="card">
										<strong>OCR comparison failed</strong>
										<div>{ocrError}</div>
									</div>
								{:else if ocrLoading}
									<p class="empty">Running OCR engines against the selected sample...</p>
								{:else if ocrRuns.length}
									<div class="ocr-run-grid">
										{#each ocrRuns as run}
											<div class="card">
												<div class="row-between">
													<div>
														<strong>{run.engine}</strong>
														<div class="meta">{run.implementation ?? 'implementation unavailable'}</div>
													</div>
													<span class="badge">{run.status}</span>
												</div>

												{#if run.error}
													<div>{run.error}</div>
												{:else}
													<div class="meta">{benchmarkLabel(run)}</div>

													{#if run.warnings.length}
														<div class="pill-list">
															{#each run.warnings as warning}
																<span class="pill">{warning}</span>
															{/each}
														</div>
													{/if}

													{#if Object.keys(run.fields).length}
														<div class="kv-grid">
															{#each sortedFields(run.fields) as [key, value]}
																<div class="kv-row">
																	<span class="meta">{key}</span>
																	<strong>{value}</strong>
																</div>
															{/each}
														</div>
													{/if}

													{#if run.benchmark && (run.benchmark.missing_fields.length || run.benchmark.mismatched_fields.length)}
														<div class="diff-grid">
															<div class="list compact">
																<div class="section-title">Missing</div>
																{#if run.benchmark.missing_fields.length}
																	{#each run.benchmark.missing_fields as field}
																		<div class="list-item">
																			<strong>{field.field}</strong>
																			<div class="meta">expected {field.expected}</div>
																		</div>
																	{/each}
																{:else}
																	<p class="empty">No missing fields.</p>
																{/if}
															</div>
															<div class="list compact">
																<div class="section-title">Mismatched</div>
																{#if run.benchmark.mismatched_fields.length}
																	{#each run.benchmark.mismatched_fields as field}
																		<div class="list-item">
																			<strong>{field.field}</strong>
																			<div class="meta">expected {field.expected}</div>
																			<div class="meta">actual {field.actual}</div>
																		</div>
																	{/each}
																{:else}
																	<p class="empty">No mismatched fields.</p>
																{/if}
															</div>
														</div>
													{/if}
												{/if}
											</div>
										{/each}
									</div>
								{:else}
									<p class="empty">Run the comparison to benchmark OCR engines for this sample.</p>
								{/if}
							</section>
						{:else}
							<p class="empty">Select a sample to compare OCR engines against the reference sidecar.</p>
						{/if}
					</section>
				</div>
			</section>
		</div>
	{/if}
</div>
