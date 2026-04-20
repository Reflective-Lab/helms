//! Example: PDF text extraction (no OCR — for text-native PDFs)
//!
//! Extracts text content, metadata, and chunks from a PDF document.
//! For scanned documents, use extract-ocr instead.
//!
//! Usage:
//!   cargo run -p example-extract-pdf -- document.pdf

use std::path::PathBuf;

use organism_intelligence::pdf::PdfIngester;

fn main() {
    let file_path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: extract-pdf <file.pdf>");
        std::process::exit(1);
    });

    let path = PathBuf::from(&file_path);
    if !path.exists() {
        eprintln!("File not found: {file_path}");
        std::process::exit(1);
    }

    let ingester = PdfIngester::new();

    match ingester.ingest_file(&path) {
        Ok(doc) => {
            println!("--- PDF Document ---");
            println!("Path:     {}", doc.path.display());
            println!("Title:    {}", doc.title.as_deref().unwrap_or("(none)"));
            println!("Author:   {}", doc.author.as_deref().unwrap_or("(none)"));
            println!("Pages:    {}", doc.page_count);
            println!("Chunks:   {}", doc.chunks.len());

            if !doc.metadata.is_empty() {
                println!("\n--- Metadata ---");
                for (k, v) in &doc.metadata {
                    println!("  {k}: {v}");
                }
            }

            println!("\n--- Content Preview (first 3 chunks) ---\n");
            for (i, chunk) in doc.chunks.iter().take(3).enumerate() {
                let preview = &chunk.content[..chunk.content.len().min(200)];
                println!("  [Chunk {i} | page {}] {preview}", chunk.page_number);
                println!();
            }

            let total_chars: usize = doc.chunks.iter().map(|c| c.content.len()).sum();
            println!("--- Total: {total_chars} chars across {} chunks ---", doc.chunks.len());
        }
        Err(e) => {
            eprintln!("PDF Error: {e}");
            std::process::exit(1);
        }
    }
}
