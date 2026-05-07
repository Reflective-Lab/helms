//! Note formatting — structured data → Markdown note with YAML frontmatter.

use chrono::Utc;

pub fn social_note(
    platform: &str,
    handle: Option<&str>,
    display_name: Option<&str>,
    headline: Option<&str>,
    description: Option<&str>,
    url: &str,
    links: &[String],
) -> String {
    let now = Utc::now().format("%Y-%m-%d");
    let title = display_name.unwrap_or(handle.unwrap_or("Unknown"));

    let mut note =
        format!("---\nsource: helm-capture\nplatform: {platform}\ncaptured: {now}\nurl: {url}\n");
    if let Some(h) = handle {
        note.push_str(&format!("handle: {h}\n"));
    }
    note.push_str("---\n\n");
    note.push_str(&format!("# {title}\n\n"));

    if let Some(h) = headline {
        note.push_str(&format!("**{h}**\n\n"));
    }
    if let Some(d) = description {
        note.push_str(&format!("{d}\n\n"));
    }

    if !links.is_empty() {
        note.push_str("## Links\n\n");
        for link in links {
            note.push_str(&format!("- {link}\n"));
        }
        note.push('\n');
    }

    note.push_str(&format!(
        "---\n*Captured from [{platform}]({url}) on {now}*\n"
    ));
    note
}

pub fn web_note(
    title: Option<&str>,
    description: Option<&str>,
    url: &str,
    body_preview: &str,
) -> String {
    let now = Utc::now().format("%Y-%m-%d");
    let heading = title.unwrap_or(url);

    let mut note =
        format!("---\nsource: helm-capture\ntype: web\ncaptured: {now}\nurl: {url}\n---\n\n");
    note.push_str(&format!("# {heading}\n\n"));

    if let Some(d) = description {
        note.push_str(&format!("> {d}\n\n"));
    }

    let preview = if body_preview.len() > 2000 {
        &body_preview[..2000]
    } else {
        body_preview
    };
    note.push_str(preview);
    note.push_str("\n\n---\n");
    note.push_str(&format!("*Captured from [{url}]({url}) on {now}*\n"));
    note
}

pub fn ocr_note(
    source_filename: &str,
    extracted_text: &str,
    provider: &str,
    pages: usize,
) -> String {
    let now = Utc::now().format("%Y-%m-%d");

    let mut note = format!(
        "---\nsource: helm-capture\ntype: ocr\ncaptured: {now}\noriginal: {source_filename}\nprovider: {provider}\npages: {pages}\n---\n\n"
    );
    note.push_str(&format!("# OCR: {source_filename}\n\n"));
    note.push_str(extracted_text);
    note.push_str("\n\n---\n");
    note.push_str(&format!(
        "*Extracted from {source_filename} via {provider} on {now}*\n"
    ));
    note
}

pub fn pdf_note(
    source_filename: &str,
    title: Option<&str>,
    author: Option<&str>,
    page_count: usize,
    content_preview: &str,
) -> String {
    let now = Utc::now().format("%Y-%m-%d");
    let heading = title.unwrap_or(source_filename);

    let mut note = format!(
        "---\nsource: helm-capture\ntype: pdf\ncaptured: {now}\noriginal: {source_filename}\npages: {page_count}\n"
    );
    if let Some(a) = author {
        note.push_str(&format!("author: {a}\n"));
    }
    note.push_str("---\n\n");
    note.push_str(&format!("# {heading}\n\n"));
    note.push_str(content_preview);
    note.push_str("\n\n---\n");
    note.push_str(&format!("*Extracted from {source_filename} on {now}*\n"));
    note
}
