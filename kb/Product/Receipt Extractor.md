# Receipt Extractor

This extractor is meant to compare multiple local OCR approaches against the editable ground-truth
sidecars in `data/receipts/*.reference.yaml`.

The OCR execution path now imports directly from `../organism/crates/intelligence/src/ocr.rs`.
That keeps the receipt heuristics local to `saas-killer`, while pushing the OCR implementation and
PDF extraction ownership into `organism`.

## Current Engines

1. `canonical-name`
   - Fastest baseline.
   - Extracts only from the canonical filename.
   - Useful as a routing and sanity-check layer.

2. `tesseract-cli`
   - Classical OCR path.
   - Uses the local `tesseract` binary if installed.
   - Runs through Organism's receipt OCR backend, which rasterizes scanned PDFs with `sips` on macOS
     and preserves OCR provenance in extractor metadata.

3. `ollama-glm-ocr`
   - Smart local OCR path.
   - Sends either extracted text or a rendered image to a local Ollama model and expects JSON back.
   - Visual OCR uses the same Organism-owned OCR contract as Tesseract.
   - Intended for GLM-OCR or another document-capable local vision model.

For `digital-pdf` fixtures, the extractor now tries Organism's direct PDF text
ingester first and only falls back to OCR when the extracted text is not
meaningful.

4. `reference`
   - Not a real OCR engine.
   - Returns the YAML sidecar fields directly.
   - Useful as the gold reference output during benchmarking.

## CLI

List fixtures:

```bash
cargo run -p prio-expenses --bin receipt-extractor -- list
```

Extract one sample:

```bash
cargo run -p prio-expenses --bin receipt-extractor -- extract canonical-name receipt-anthropic-pbc-max-plan-2026-04-11
```

Benchmark one engine against all sidecars:

```bash
cargo run -p prio-expenses --bin receipt-extractor -- benchmark canonical-name
```

Benchmark a single sample:

```bash
cargo run -p prio-expenses --bin receipt-extractor -- benchmark canonical-name invoice-la-matade-scan-march-2026
```

## Ollama Configuration

Set these before running the Ollama engine:

```bash
export EXPENSES_OCR_OLLAMA_BASE_URL=http://127.0.0.1:11434
export EXPENSES_OCR_OLLAMA_MODEL=<your-local-model-tag>
```

The model tag is intentionally not hardcoded beyond a default placeholder because local Ollama
model names are unstable across installs and pulls.

## Tesseract Configuration

Optional language override:

```bash
export EXPENSES_OCR_TESSERACT_LANG=eng+fra+swe
```

## Intended Routing

- Start with `canonical-name` for cheap routing hints.
- Use `tesseract-cli` for lightweight offline OCR.
- Escalate low-confidence scans and photos to `ollama-glm-ocr`.
- Compare all outputs against the YAML sidecars before changing extraction rules.

## Migration Note

- `saas-killer` now takes the shared OCR contract, receipt OCR backends, and PDF
  text extraction directly from `../organism`.
- Migrating this repo to the latest `../converge` remains a separate larger job; see
  [[../Architecture/OCR Organism Converge Migration]].
