# OCR, Organism, and Converge

## What Changed

`prio-expenses` now imports OCR directly from `organism-intelligence`.

The field heuristics and fixture benchmarking stay in
`crates/prio-expenses/src/receipt_extractor.rs`, but the working receipt OCR
implementations now live in `../organism/crates/intelligence/src/ocr/receipt.rs`.

Generic PDF text extraction has also been moved into
`../organism/crates/intelligence/src/pdf.rs`, and the receipt extractor now uses
that shared capability for `digital-pdf` samples before falling back to OCR.

## Why A Local Bridge Exists

The contract dependency path into `../organism` is now ready, but the full
backend move is not complete yet.

Verification on April 13, 2026:

```bash
cargo check -p organism-intelligence --features ocr
```

Result:

- passed
- shared OCR request/result types compile in `../organism`
- receipt-specific provider implementations now live in
  `../organism/crates/intelligence/src/ocr/receipt.rs`
- direct PDF text extraction now lives in `../organism/crates/intelligence/src/pdf.rs`
- the remaining generic local photo/screenshot OCR backends in `../organism`
  still need to be finished
- `TesseractOcrProvider` in `../organism` is still placeholder-only

So the safe move for `saas-killer` is:

1. take the OCR contract from Organism now
2. import Organism OCR/PDF directly from call sites
3. continue moving the remaining photo/screenshot local backends and notes-facing
   integration onto the same Organism-owned surface

## Recommended Merge Sequence

1. Finish and stabilize the receipt benchmark set in `data/receipts/*.reference.yaml`.
2. Compare `tesseract-cli` and `ollama-glm-ocr` against those references.
3. Port the working receipt provider implementations into `../organism`.
4. Port text-native PDF extraction into `../organism`.
5. Finish the remaining photo/screenshot local OCR backend move.

## Latest Converge Migration

This is a separate major workstream from OCR.

Current local indicators:

- `crates/crm-storage/src/lib.rs` already documents a temporary workaround because the local
  `converge_core::Context` is serialize-only in the checked out Converge snapshot.
- Earlier `application-server` verification was blocked by Converge-side breakage around
  `AgentEffect::with_fact` in `../converge/crates/analytics`.

That means the right order is:

1. keep OCR merge work isolated from the Converge upgrade
2. plan a dedicated Converge upgrade branch
3. fix runtime and storage integration against the new `Context` and agent APIs
4. re-run truth-runtime verification after that upgrade

## Practical Boundary

For now:

- shared OCR contract ownership: `organism-intelligence`
- working receipt OCR implementations: `organism-intelligence`
- working PDF text extraction: `organism-intelligence`
- Converge runtime integration: unchanged
- Converge upgrade: intentionally deferred into a separate migration job
