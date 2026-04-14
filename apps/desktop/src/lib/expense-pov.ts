import type { ExpenseItem, ExpenseReport } from '$lib/types'

const onlineReportId = '66666666-6666-4666-8666-666666666666'
const scannedReportId = '77777777-7777-4777-8777-777777777777'

export const expensePovReports: ExpenseReport[] = [
	{
		id: onlineReportId,
		title: 'Receipt PoV · Online receipt',
		employee_name: 'Kenneth Pernyer',
		employee_email: 'kenneth@prio.ai',
		status: 'submitted',
		currency_code: 'EUR',
		total_minor: 11_250,
		description: 'Proof of value using the Anthropic PDF receipt sample.',
		submitted_at: '2026-04-11T10:10:00Z',
		created_at: '2026-04-11T09:55:00Z',
		updated_at: '2026-04-11T10:10:00Z'
	},
	{
		id: scannedReportId,
		title: 'Receipt PoV · Scanned receipt',
		employee_name: 'Kenneth Pernyer',
		employee_email: 'kenneth@prio.ai',
		status: 'in-review',
		currency_code: 'EUR',
		total_minor: 27_588,
		description: 'Proof of value using the hard-to-read La Matade PDF sample.',
		created_at: '2026-04-11T10:20:00Z',
		updated_at: '2026-04-11T10:25:00Z'
	}
]

export const expensePovItems: ExpenseItem[] = [
	{
		id: '88888888-8888-4888-8888-888888888888',
		report_id: onlineReportId,
		merchant: 'Anthropic, PBC',
		amount: {
			currency_code: 'EUR',
			amount_minor: 11_250
		},
		category: 'software',
		occurred_at: '2026-04-11T00:00:00Z',
		description: 'Max plan receipt from the provider billing portal.',
		capture_source: 'email',
		receipt_document_id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
		ocr_status: 'scanned',
		ocr_engine: 'portal-pdf-pov',
		extracted_summary:
			'Receipt shows Anthropic, PBC, Max plan 5x, total EUR 112.50, VAT EUR 22.50.',
		ocr_fields: {
			source_file: 'data/receipts/receipt-anthropic-pbc-max-plan-2026-04-11.pdf',
			merchant: 'Anthropic, PBC',
			receipt_number: '2664-9489-6888',
			invoice_number: 'WVRUCII3-0010',
			paid_at: '2026-04-11',
			total: '112.50',
			currency: 'EUR',
			vat: '22.50',
			vat_rate: '25%',
			country: 'Sweden'
		},
		policy_flags: [],
		created_at: '2026-04-11T10:00:00Z',
		updated_at: '2026-04-11T10:00:00Z'
	},
	{
		id: '99999999-9999-4999-8999-999999999999',
		report_id: scannedReportId,
		merchant: 'La Matade',
		amount: {
			currency_code: 'EUR',
			amount_minor: 27_588
		},
		category: 'other',
		occurred_at: '2026-03-31T00:00:00Z',
		description: 'Scan-like supplier invoice sample with partial extraction.',
		capture_source: 'desktop-upload',
		receipt_document_id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
		ocr_status: 'needs-review',
		ocr_engine: 'pov-scan-review',
		extracted_summary:
			'Hard scan-like invoice. Current PoV confidently captures merchant guess, service date 2026-03-31, and total EUR 275.88, but line items and VAT still need review.',
		ocr_fields: {
			source_file: 'data/receipts/invoice-la-matade-scan-march-2026.pdf',
			merchant_guess: 'La Matade',
			service_date: '2026-03-31',
			total: '275.88',
			currency: 'EUR',
			source_kind: 'scan-like-pdf',
			verification_note:
				'body text is not fully extracted yet; verify invoice number, VAT, and line items'
		},
		policy_flags: [
			'verify-vendor',
			'verify-vat',
			'verify-line-items',
			'manual-review-required'
		],
		created_at: '2026-04-11T10:20:00Z',
		updated_at: '2026-04-11T10:20:00Z'
	}
]
