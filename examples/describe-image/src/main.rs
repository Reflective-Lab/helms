//! Example: Image description and object detection via vision models
//!
//! Usage:
//!   cargo run -p example-describe-image -- photo.jpg
//!   cargo run -p example-describe-image -- --prompt "What products are shown?" shelf.jpg
//!
//! Environment:
//!   ANTHROPIC_API_KEY — for Claude vision (default)

use std::path::PathBuf;

use organism_intelligence::vision::{AnthropicVision, VisionDescriber, VisionInput, VisionRequest};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (prompt, file_path) = parse_args(&args);

    let path = PathBuf::from(&file_path);
    if !path.exists() {
        eprintln!("File not found: {file_path}");
        std::process::exit(1);
    }

    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        eprintln!("Failed to read file: {e}");
        std::process::exit(1);
    });

    let request = VisionRequest {
        input: VisionInput::Bytes(bytes),
        prompt: Some(prompt),
        max_tokens: 1024,
        extract_objects: true,
        detect_text: true,
    };

    let provider = AnthropicVision::from_env().unwrap_or_else(|e| {
        eprintln!("Failed to initialize vision: {e:?}");
        eprintln!("Ensure ANTHROPIC_API_KEY is set.");
        std::process::exit(1);
    });

    match provider.describe(&request) {
        Ok(description) => {
            println!("--- Vision Description ---");
            println!("{}", description.scene);

            if !description.objects.is_empty() {
                println!("\n--- Objects ({}) ---", description.objects.len());
                for obj in &description.objects {
                    println!("  {} (confidence: {:.0}%)", obj.label, obj.confidence * 100.0);
                }
            }

            if let Some(text) = &description.text.first().map(|t| t.text.clone()) {
                println!("\n--- Extracted Text ---\n{text}");
            }
        }
        Err(e) => {
            eprintln!("Vision Error: {e:?}");
            std::process::exit(1);
        }
    }
}

fn parse_args(args: &[String]) -> (String, String) {
    let mut prompt = "Describe this image in detail. What objects, text, and notable features do you see?".to_string();
    let mut file = String::new();

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--prompt" && i + 1 < args.len() {
            prompt = args[i + 1].clone();
            i += 2;
        } else {
            file = args[i].clone();
            i += 1;
        }
    }

    if file.is_empty() {
        eprintln!("Usage: describe-image [--prompt \"...\"] <image>");
        std::process::exit(1);
    }

    (prompt, file)
}
