# Receipt Fixtures

Canonical sample filenames in this folder use this pattern:

- `<kind>-<merchant>-<date-or-reference>.<ext>`
- `<same-basename>.reference.yaml`

Each `.reference.yaml` file is meant to be edited by hand after review. Treat it as the
ground-truth reference for extraction tests and OCR benchmarking.

Current sidecar fields:

- `document_type`: `receipt`, `invoice`, `statement`, or `email`
- `capture_type`: `digital-pdf`, `scanned-pdf`, `photo`, or `rich-text-email`
- `expense_candidate`: whether the file should enter the expense flow
- `expected`: manually verified fields for comparison
- `notes`: anything extraction commonly gets wrong or still needs review

The original source filename is preserved inside each sidecar under `original_file_name`.
