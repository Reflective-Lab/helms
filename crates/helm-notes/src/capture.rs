//! Smart capture: URL/file → detect → extract → format → vault note.

use std::path::Path;

use organism_notes::vault::ObsidianVault;
use serde::Serialize;

mod detect;
mod format;
#[cfg(feature = "web")]
mod web;
#[cfg(feature = "social")]
mod social;
#[cfg(feature = "ocr")]
mod ocr;
#[cfg(feature = "pdf")]
mod pdf;

// ── Public Types ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum CaptureRequest {
    Url(String),
    File(std::path::PathBuf),
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureReport {
    pub kind: CaptureKind,
    pub title: String,
    pub vault_path: String,
    pub extracted_fields: usize,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureKind {
    WebPage,
    SocialProfile,
    Pdf,
    Image,
    Unknown,
}

// ── Main Entry Point ────────────────────────────────────────────────

pub fn capture(vault: &ObsidianVault, request: &CaptureRequest) -> Result<CaptureReport, String> {
    match request {
        CaptureRequest::Url(url) => capture_url(vault, url),
        CaptureRequest::File(path) => capture_file(vault, path),
    }
}

fn capture_url(vault: &ObsidianVault, url: &str) -> Result<CaptureReport, String> {
    let detected = detect::detect_url(url);

    match detected {
        #[cfg(feature = "social")]
        detect::UrlKind::Social(_platform) => social::capture_social(vault, url),
        #[cfg(not(feature = "social"))]
        detect::UrlKind::Social(_) => web::capture_web(vault, url),
        #[cfg(feature = "web")]
        detect::UrlKind::Web => web::capture_web(vault, url),
        #[cfg(not(feature = "web"))]
        detect::UrlKind::Web => Err("web capture not enabled".into()),
        detect::UrlKind::Pdf => {
            // PDF URL: fetch then extract
            #[cfg(feature = "web")]
            {
                web::capture_web(vault, url)
            }
            #[cfg(not(feature = "web"))]
            {
                Err("web capture not enabled for PDF URLs".into())
            }
        }
    }
}

fn capture_file(vault: &ObsidianVault, path: &Path) -> Result<CaptureReport, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        #[cfg(feature = "pdf")]
        "pdf" => pdf::capture_pdf(vault, path),
        #[cfg(feature = "ocr")]
        "jpg" | "jpeg" | "png" | "heic" | "webp" => ocr::capture_image(vault, path),
        _ => Err(format!("unsupported file type: .{ext}")),
    }
}
