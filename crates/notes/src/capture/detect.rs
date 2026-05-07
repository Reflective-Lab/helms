//! URL detection — what kind of content does this URL point to?

pub enum UrlKind {
    Social(SocialPlatform),
    Web,
    Pdf,
}

pub enum SocialPlatform {
    LinkedIn,
    X,
    Instagram,
    Facebook,
}

pub fn detect_url(url: &str) -> UrlKind {
    let lower = url.to_lowercase();

    if lower.ends_with(".pdf") || lower.contains(".pdf?") {
        return UrlKind::Pdf;
    }

    if lower.contains("linkedin.com/in/") || lower.contains("linkedin.com/company/") {
        return UrlKind::Social(SocialPlatform::LinkedIn);
    }
    if lower.contains("x.com/") || lower.contains("twitter.com/") {
        return UrlKind::Social(SocialPlatform::X);
    }
    if lower.contains("instagram.com/") {
        return UrlKind::Social(SocialPlatform::Instagram);
    }
    if lower.contains("facebook.com/") || lower.contains("fb.com/") {
        return UrlKind::Social(SocialPlatform::Facebook);
    }

    UrlKind::Web
}
