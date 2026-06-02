//! Image OCR capture → vault note.

use std::path::Path;

use organism_intelligence::ocr::cloud::MistralOcrProvider;
use organism_intelligence::ocr::{OcrInput, OcrOutputFormat, OcrProvider, OcrRequest};
use organism_notes::vault::ObsidianVault;

use super::format;
use super::{CaptureKind, CaptureProvenance, CaptureReport};

pub fn capture_image(vault: &ObsidianVault, path: &Path) -> Result<CaptureReport, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read file: {e}"))?;
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("image");

    let request = OcrRequest {
        input: OcrInput::ImageBytes(bytes),
        output_format: OcrOutputFormat::Markdown,
        languages: vec![],
        extract_tables: true,
        extract_images: false,
        page_range: None,
    };

    let provider = MistralOcrProvider::from_env()
        .map_err(|e| format!("OCR provider: {e:?} — set MISTRAL_API_KEY"))?;

    let result = provider
        .extract(&request)
        .map_err(|e| format!("OCR: {e:?}"))?;

    let note_body = format::ocr_note(
        filename,
        &result.text,
        &result.provenance.provider,
        result.pages,
    );
    let vault_path = format!("Inbox/OCR/{}.md", filename.replace('.', "_"));

    vault
        .save_note(&vault_path, &note_body)
        .map_err(|e| format!("vault write: {e}"))?;

    Ok(CaptureReport {
        kind: CaptureKind::Image,
        title: format!("OCR: {filename}"),
        vault_path,
        extracted_fields: 1,
        provenance: CaptureProvenance::Ocr {
            provider: result.provenance.provider,
            version: result.provenance.version,
        },
    })
}
