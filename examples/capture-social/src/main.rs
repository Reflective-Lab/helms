//! Example: Social profile capture (LinkedIn, X, Instagram, Facebook)
//!
//! Demonstrates using organism-intelligence's social extraction to capture
//! a public profile from a URL and produce structured data.
//!
//! Usage:
//!   cargo run -p example-capture-social -- "https://linkedin.com/in/someone"
//!   cargo run -p example-capture-social -- "https://x.com/someone"
//!   cargo run -p example-capture-social -- "https://instagram.com/someone"

use organism_intelligence::provenance::CallContext;
use organism_intelligence::social::{
    SocialExtractProvider, SocialExtractRequest, WebCaptureSocialExtractProvider,
};
use organism_intelligence::web::HttpWebCaptureProvider;

fn main() {
    let url = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: capture-social <url>");
        eprintln!("  Supported: LinkedIn, X/Twitter, Instagram, Facebook public profiles");
        std::process::exit(1);
    });

    let web_provider = HttpWebCaptureProvider::new().expect("failed to create web provider");
    let social_provider = WebCaptureSocialExtractProvider::new(Box::new(web_provider));

    let request = SocialExtractRequest::new(&url);
    let ctx = CallContext {
        correlation_id: Some("capture-social-cli".into()),
        metadata: Default::default(),
    };

    match social_provider.extract(&request, &ctx) {
        Ok(response) => {
            let profile = &response.profile.content;
            println!("--- Social Profile ---");
            println!("Platform:    {:?}", profile.platform);
            println!("Handle:      {}", profile.handle.as_deref().unwrap_or("?"));
            println!("Name:        {}", profile.display_name.as_deref().unwrap_or("?"));
            println!("Headline:    {}", profile.headline.as_deref().unwrap_or("?"));
            println!("URL:         {}", profile.canonical_url);
            if let Some(desc) = &profile.description {
                println!("Description: {}", &desc[..desc.len().min(200)]);
            }
            if !profile.outbound_links.is_empty() {
                println!("Links:       {} found", profile.outbound_links.len());
            }
            println!("\n--- Provenance ---");
            println!("Vendor:  {}", response.profile.vendor);
            println!("Latency: {}ms", response.profile.latency_ms);
            println!("\n--- Raw JSON ---");
            println!("{}", serde_json::to_string_pretty(profile).unwrap_or_default());
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
