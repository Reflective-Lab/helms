//! PDF text extraction → vault note.

use std::path::Path;

use organism_intelligence::pdf::PdfIngester;
use organism_notes::vault::ObsidianVault;

use super::format;
use super::{CaptureKind, CaptureReport};

pub fn capture_pdf(vault: &ObsidianVault, path: &Path) -> Result<CaptureReport, String> {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document.pdf");

    let ingester = PdfIngester::new();
    let doc = ingester
        .ingest_file(path)
        .map_err(|e| format!("PDF extraction: {e}"))?;

    let content_preview: String = doc
        .chunks
        .iter()
        .take(10)
        .map(|c| c.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    let note_body = format::pdf_note(
        filename,
        doc.title.as_deref(),
        doc.author.as_deref(),
        doc.page_count,
        &content_preview,
    );

    let stem = filename.strip_suffix(".pdf").unwrap_or(filename);
    let vault_path = format!("Inbox/PDF/{stem}.md");

    vault
        .save_note(&vault_path, &note_body)
        .map_err(|e| format!("vault write: {e}"))?;

    Ok(CaptureReport {
        kind: CaptureKind::Pdf,
        title: doc.title.unwrap_or_else(|| filename.to_string()),
        vault_path,
        extracted_fields: doc.chunks.len(),
        provenance: "pdf-extract (local)".into(),
    })
}
