//! Example: OCR extraction from image or PDF
//!
//! Usage:
//!   cargo run -p example-extract-ocr -- receipt.jpg
//!   cargo run -p example-extract-ocr -- document.pdf
//!
//! Environment:
//!   MISTRAL_API_KEY — for Mistral OCR (cloud)

use std::path::PathBuf;

use organism_intelligence::ocr::cloud::MistralOcrProvider;
use organism_intelligence::ocr::{OcrInput, OcrOutputFormat, OcrProvider, OcrRequest};

fn main() {
    let file_path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: extract-ocr <file>");
        eprintln!("  Supports: .jpg, .png, .pdf");
        eprintln!("  Set MISTRAL_API_KEY for cloud OCR");
        std::process::exit(1);
    });

    let path = PathBuf::from(&file_path);
    if !path.exists() {
        eprintln!("File not found: {file_path}");
        std::process::exit(1);
    }

    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        eprintln!("Failed to read file: {e}");
        std::process::exit(1);
    });

    let input = if file_path.ends_with(".pdf") {
        OcrInput::PdfBytes(bytes)
    } else {
        OcrInput::ImageBytes(bytes)
    };

    let request = OcrRequest {
        input,
        output_format: OcrOutputFormat::Markdown,
        languages: vec![],
        extract_tables: true,
        extract_images: false,
        page_range: None,
    };

    let provider = MistralOcrProvider::from_env().unwrap_or_else(|e| {
        eprintln!("Failed to initialize Mistral OCR: {e:?}");
        eprintln!("Ensure MISTRAL_API_KEY is set.");
        std::process::exit(1);
    });

    match provider.extract(&request) {
        Ok(result) => {
            println!("--- OCR Result ---");
            println!("Pages:    {}", result.pages);
            println!("Provider: {}", result.provenance.provider);
            println!("Model:    {}", result.provenance.version);
            if let Some(ms) = result.processing_time_ms {
                println!("Time:     {ms}ms");
            }
            println!("\n--- Extracted Text ---\n{}", result.text);
        }
        Err(e) => {
            eprintln!("OCR Error: {e:?}");
            std::process::exit(1);
        }
    }
}
