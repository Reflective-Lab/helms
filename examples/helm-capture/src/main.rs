//! helm capture — smart capture from CLI
//!
//! Auto-detects content type and writes a rich note to the vault.
//!
//! Usage:
//!   cargo run -p helm-capture -- "https://linkedin.com/in/someone"
//!   cargo run -p helm-capture -- "https://example.com/article"
//!   cargo run -p helm-capture -- ./receipt.jpg
//!   cargo run -p helm-capture -- ./document.pdf
//!
//! The vault root defaults to ~/Notes. Override with NOTES_VAULT_ROOT.

use std::path::PathBuf;

use helm_notes::capture::{CaptureRequest, capture};
use organism_notes::vault::ObsidianVault;

fn main() {
    let input = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: helm-capture <url-or-file>");
        eprintln!();
        eprintln!("  URLs:   auto-detects social (LinkedIn, X, Instagram, Facebook) vs web");
        eprintln!("  Files:  auto-detects PDF vs image (JPG, PNG, HEIC)");
        eprintln!();
        eprintln!("  Set NOTES_VAULT_ROOT to change vault location (default: ~/Notes)");
        eprintln!("  Set MISTRAL_API_KEY for cloud OCR");
        std::process::exit(1);
    });

    let vault_root = std::env::var("NOTES_VAULT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs_next::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Notes")
        });

    let vault = ObsidianVault::from_root(&vault_root);

    let request = if input.starts_with("http://") || input.starts_with("https://") {
        CaptureRequest::Url(input)
    } else {
        let path = PathBuf::from(&input);
        if !path.exists() {
            eprintln!("File not found: {input}");
            std::process::exit(1);
        }
        CaptureRequest::File(path)
    };

    match capture(&vault, &request) {
        Ok(report) => {
            println!("Captured: {}", report.title);
            println!("  Kind:       {:?}", report.kind);
            println!("  Vault path: {}", report.vault_path);
            println!("  Fields:     {}", report.extracted_fields);
            println!("  Provenance: {}", report.provenance);
        }
        Err(e) => {
            eprintln!("Capture failed: {e}");
            std::process::exit(1);
        }
    }
}
