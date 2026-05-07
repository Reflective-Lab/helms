//! Social profile capture → vault note.

use organism_intelligence::provenance::CallContext;
use organism_intelligence::social::{
    SocialExtractProvider, SocialExtractRequest, WebCaptureSocialExtractProvider,
};
use organism_intelligence::web::HttpWebCaptureProvider;
use organism_notes::vault::ObsidianVault;

use super::format;
use super::{CaptureKind, CaptureReport};

pub fn capture_social(vault: &ObsidianVault, url: &str) -> Result<CaptureReport, String> {
    let web = HttpWebCaptureProvider::new().map_err(|e| format!("web provider: {e}"))?;
    let provider = WebCaptureSocialExtractProvider::new(Box::new(web));

    let request = SocialExtractRequest::new(url);
    let ctx = CallContext {
        correlation_id: Some("helm-capture".into()),
        metadata: Default::default(),
    };

    let response = provider.extract(&request, &ctx)?;
    let profile = &response.profile.content;

    let platform = format!("{:?}", profile.platform).to_lowercase();
    let note_body = format::social_note(
        &platform,
        profile.handle.as_deref(),
        profile.display_name.as_deref(),
        profile.headline.as_deref(),
        profile.description.as_deref(),
        url,
        &profile.outbound_links,
    );

    let filename = sanitize_filename(
        profile
            .display_name
            .as_deref()
            .or(profile.handle.as_deref())
            .unwrap_or("profile"),
    );
    let vault_path = format!("Inbox/Social/{filename}.md");

    vault
        .save_note(&vault_path, &note_body)
        .map_err(|e| format!("vault write: {e}"))?;

    Ok(CaptureReport {
        kind: CaptureKind::SocialProfile,
        title: profile
            .display_name
            .clone()
            .or(profile.handle.clone())
            .unwrap_or_else(|| url.to_string()),
        vault_path,
        extracted_fields: count_fields(profile),
        provenance: format!("{} via {}", platform, response.profile.vendor),
    })
}

fn count_fields(profile: &organism_intelligence::social::SocialProfile) -> usize {
    let mut count = 1; // platform always present
    if profile.handle.is_some() {
        count += 1;
    }
    if profile.display_name.is_some() {
        count += 1;
    }
    if profile.headline.is_some() {
        count += 1;
    }
    if profile.description.is_some() {
        count += 1;
    }
    count += profile.outbound_links.len().min(1);
    count
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}
