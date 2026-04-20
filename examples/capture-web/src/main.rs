//! Example: Web page capture and metadata extraction
//!
//! Usage:
//!   cargo run -p example-capture-web -- "https://example.com"

use organism_intelligence::provenance::CallContext;
use organism_intelligence::web::{
    HttpWebCaptureProvider, WebCaptureMode, WebCaptureProvider, WebCaptureRequest,
};

fn main() {
    let url = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: capture-web <url>");
        std::process::exit(1);
    });

    let provider = HttpWebCaptureProvider::new().expect("failed to create web provider");
    let request = WebCaptureRequest {
        url: url.clone(),
        mode: WebCaptureMode::Http,
        user_agent: None,
    };
    let ctx = CallContext {
        correlation_id: Some("capture-web-cli".into()),
        metadata: Default::default(),
    };

    match provider.capture(&request, &ctx) {
        Ok(response) => {
            let doc = &response.capture.content;
            println!("--- Web Capture ---");
            println!("URL:         {}", doc.final_url);
            println!("Title:       {}", doc.title.as_deref().unwrap_or("(none)"));
            println!("Description: {}", doc.description.as_deref().unwrap_or("(none)"));
            println!("Site:        {}", doc.site_name.as_deref().unwrap_or("(none)"));
            println!("Status:      {}", doc.status_code);
            println!("Body size:   {} bytes", doc.body.len());
            println!("Links:       {} found", doc.links.len());

            if !doc.links.is_empty() {
                println!("\n--- Top 10 Links ---");
                for link in doc.links.iter().take(10) {
                    println!("  {} {}", link.href, link.text.as_deref().unwrap_or(""));
                }
            }

            println!("\n--- Provenance ---");
            println!("Vendor:  {}", response.capture.vendor);
            println!("Latency: {}ms", response.capture.latency_ms);
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
