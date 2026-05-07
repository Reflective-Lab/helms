//! Web page capture → vault note.

use organism_intelligence::provenance::CallContext;
use organism_intelligence::web::{
    HttpWebCaptureProvider, WebCaptureMode, WebCaptureProvider, WebCaptureRequest,
};
use organism_notes::vault::ObsidianVault;

use super::format;
use super::{CaptureKind, CaptureReport};

pub fn capture_web(vault: &ObsidianVault, url: &str) -> Result<CaptureReport, String> {
    let provider = HttpWebCaptureProvider::new().map_err(|e| format!("web provider: {e}"))?;
    let request = WebCaptureRequest {
        url: url.to_string(),
        mode: WebCaptureMode::Http,
        user_agent: None,
    };
    let ctx = CallContext {
        correlation_id: Some("helm-capture".into()),
        metadata: Default::default(),
    };

    let response = provider.capture(&request, &ctx)?;
    let doc = &response.capture.content;

    let note_body = format::web_note(
        doc.title.as_deref(),
        doc.description.as_deref(),
        &doc.final_url,
        &strip_html_rough(&doc.body),
    );

    let filename = sanitize_filename(
        doc.title
            .as_deref()
            .unwrap_or_else(|| url.split('/').last().unwrap_or("page")),
    );
    let vault_path = format!("Inbox/Web/{filename}.md");

    vault
        .save_note(&vault_path, &note_body)
        .map_err(|e| format!("vault write: {e}"))?;

    Ok(CaptureReport {
        kind: CaptureKind::WebPage,
        title: doc.title.clone().unwrap_or_else(|| url.to_string()),
        vault_path,
        extracted_fields: 3 + doc.links.len().min(1),
        provenance: format!("http via {}", response.capture.vendor),
    })
}

fn strip_html_rough(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut last_was_space = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                if !last_was_space {
                    result.push(' ');
                    last_was_space = true;
                }
            }
            _ if !in_tag => {
                if ch.is_whitespace() {
                    if !last_was_space {
                        result.push(' ');
                        last_was_space = true;
                    }
                } else {
                    result.push(ch);
                    last_was_space = false;
                }
            }
            _ => {}
        }
    }

    result.truncate(4000);
    result
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .take(80)
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
