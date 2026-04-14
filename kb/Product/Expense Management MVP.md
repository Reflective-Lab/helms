# Expense Management MVP

## Product Direction

Build the first release around one narrow job:

- employees get receipts into the system fast
- finance sees OCR confidence and policy exceptions immediately
- finished reports are easy to re-enter into the current booking system

Do not block on ERP integration for the first version. Treat export as a manual-but-clean handoff.

## Capture Channels

Support these channels in this order:

1. Email forwarding to a shared alias such as `receipts@expenses.prio.local`
2. Mobile camera upload for paper receipts
3. Desktop upload for PDFs from airlines, hotels, and card portals
4. Corporate card ingestion later, once the core review flow is stable

The important design rule is convergence into one review queue and one report model, regardless of capture path.

## Implemented Backend Slice

The repository now includes:

- an `expenses` capability module
- kernel records for `expense_report` and `expense_item`
- Tauri commands to list reports and items
- HTTP endpoints:
  - `GET /v1/expenses/reports`
  - `GET /v1/expenses/items`
  - `POST /v1/integrations/expenses/email-receipts`
- a responsive `/expenses` route in the Svelte app

The email intake endpoint is the handoff target for a mailbox processor. It creates or reuses a report, stores the receipt document, creates an expense item, and opens review workflow when OCR or policy flags require it.

## OCR Strategy

This is an engineering inference, not a benchmark claim:

- keep the current Converge OCR path for digital PDFs and already-clean receipts
- add an Ollama-served OCR worker for hard photo receipts and skewed scans
- use a stronger OCR model such as the Z.ai OCR model only on low-confidence cases
- if extraction is ambiguous, mark the item `needs-review` instead of inventing fields

Recommended pipeline:

1. normalize image orientation and contrast
2. run OCR
3. extract merchant, date, total, currency, tax, and category hints
4. score confidence
5. attach source evidence and create policy flags when confidence is low

## Review And Export

Keep the report states simple:

- `draft`
- `submitted`
- `in-review`
- `approved`
- `export-pending`
- `exported`

For now, `export-pending` is the state that means “ready for somebody in finance to copy into the booking system.”

## Next Steps

- add authenticated employee submission instead of a shared demo actor
- add receipt image upload from the responsive web route
- persist OCR confidence breakdown and raw extraction payloads
- add policy rules for mileage, meal attendees, and per diem ceilings
- add CSV and bookkeeping-export templates once the manual flow settles
