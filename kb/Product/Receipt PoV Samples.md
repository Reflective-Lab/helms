# Receipt PoV Samples

This proof-of-value currently uses two real sample files from `data/receipts/`.

## 1. Online Receipt

- File: `data/receipts/receipt-anthropic-pbc-max-plan-2026-04-11.pdf`
- Reference: `data/receipts/receipt-anthropic-pbc-max-plan-2026-04-11.reference.yaml`
- Intake path: email-style receipt ingestion
- Current extracted fields shown in the expense UI:
- `merchant = Anthropic, PBC`
- `receipt_number = 2664-9489-6888`
- `invoice_number = WVRUCII3-0010`
- `paid_at = 2026-04-11`
- `total = 112.50`
- `currency = EUR`
- `vat = 22.50`
- `vat_rate = 25%`
- `country = Sweden`
- Expected PoV outcome: should land as a clean digital receipt without manual review

## 2. Scanned Receipt

- File: `data/receipts/invoice-la-matade-scan-march-2026.pdf`
- Reference: `data/receipts/invoice-la-matade-scan-march-2026.reference.yaml`
- Intake path: scanned / scan-like receipt flow
- Current extracted fields shown in the expense UI:
- `merchant_guess = La Matade`
- `service_date = 2026-03-31`
- `total = 275.88`
- `currency = EUR`
- `source_kind = scan-like-pdf`
- `verification_note = body text is not fully extracted yet; verify invoice number, VAT, and line items`
- Expected PoV outcome: should land in review with manual OCR verification flags

## Where To Inspect

- Desktop seed/demo view: `/expenses`
- Backend intake test path: `POST /v1/integrations/expenses/email-receipts`

If any extracted value is wrong, update the field mapping first and keep the original receipt files unchanged so the PoV stays reproducible.
