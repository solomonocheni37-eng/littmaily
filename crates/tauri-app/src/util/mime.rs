use base64::{engine::general_purpose::STANDARD, Engine as _};
use regex::Regex;

/// Strictly sanitizes HTML to prevent XSS and rewrites DOM attributes to protect user privacy.
///
/// Uses a 4-step pipeline:
/// 1. Ammonia: Strips dangerous tags/attributes (XSS prevention).
/// 2. lol_html: Safely traverses the DOM to rewrite remote image/background URLs to data-src.
/// 3. Regex CSS: Strips entire `@import` rules and rewrites remaining `url()` patterns.
/// 4. Regex CSS Danger: Strips `position: fixed` and `z-index` to prevent UI redressing/clickjacking.
pub fn sanitize_html(html: &str) -> String {
    let fallback_pixel =
        "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";

    // 1. Security Sanitization (Ammonia)
    let clean = ammonia::Builder::new()
        .rm_clean_content_tags(&["style"])
        .add_tags(&["style"])
        .rm_tags(&[
            "iframe", "frame", "object", "embed", "applet", "script", "noscript",
        ])
        .add_generic_attributes(&[
            "style", "class", "align", "valign", "bgcolor", "background",
            "width", "height", "data-src", "data-background", "type",
        ])
        .add_tag_attributes("img", &["src", "alt", "title", "width", "height", "data-src"])
        .add_tag_attributes("a", &["href", "title", "target"])
        .add_tag_attributes("td", &["background", "data-background"])
        .add_tag_attributes("table", &["background", "data-background"])
        .url_schemes(std::collections::HashSet::from([
            "http", "https", "mailto", "cid", "data",
        ]))
        .clean(html)
        .to_string();

    // 2. DOM Rewriting for Privacy (lol_html)
    let rewritten = match lol_html::rewrite_str(
        &clean,
        lol_html::RewriteStrSettings {
            element_content_handlers: vec![
                lol_html::element!("img", |el| {
                    if let Some(src) = el.get_attribute("src") {
                        if src.starts_with("http://") || src.starts_with("https://") {
                            el.set_attribute("data-src", &src)?;
                            el.set_attribute("src", fallback_pixel)?;
                        }
                    }
                    Ok(())
                }),
                lol_html::element!("*", |el| {
                    let tag = el.tag_name();
                    // Strip VML / XML namespaces (e.g., <v:shape>, <o:p>) often used in Outlook tracking
                    if tag.contains(':') {
                        el.remove_and_keep_content();
                        return Ok(());
                    }
                    if let Some(bg) = el.get_attribute("background") {
                        if bg.starts_with("http://") || bg.starts_with("https://") {
                            el.set_attribute("data-background", &bg)?;
                            el.set_attribute("background", "transparent")?;
                        }
                    }
                    Ok(())
                }),
            ],
            ..lol_html::RewriteStrSettings::default()
        },
    ) {
        Ok(res) => res,
        Err(_) => clean.clone(),
    };

    // 3. CSS @import Stripping & URL Rewriting (Regex)
    //
    // STEP 3a: Strip the ENTIRE @import rule — keyword, URL/string, optional
    // media queries, and the trailing semicolon.
    //
    // The old code replaced only `@import ` → `/* @import */ `, leaving a
    // dangling `url('http://…')`. The URL rewriter below then turned it into
    // `url('data:image/gif;base64,…')`. WebKit's CSS parser saw the bare `url()`
    // after the comment closed and still attempted a stylesheet load on the
    // data URI, producing:
    //   "Did not parse stylesheet at 'data:image/gif;…' because non CSS MIME
    //    types are not allowed in strict mode."
    //
    // By matching the full rule and replacing it with an inert comment, no
    // `url()` survives for the rewriter to touch.
    //
    // `\s*` (not `\s+`) handles minified CSS like `@import'url(…)';`.
    // `[^;}{]+` consumes the URL, quotes, and any media queries up to the
    // semicolon or the next block boundary.
    let re_import = match Regex::new(r#"(?i)@import\s*[^;}{]+;?"#) {
        Ok(re) => re,
        Err(_) => return rewritten,
    };
    let after_import_strip = re_import.replace_all(&rewritten, "/* import rule removed */");

    // STEP 3b: Safety net — if any @import keyword survived (e.g. malformed
    // CSS with no URL), neutralise it so the URL rewriter below can never
    // produce a `url()` that WebKit would try to parse as a stylesheet.
    // The replacement deliberately avoids the literal "@import" so this
    // pass cannot match inside the comment from step 3a.
    let re_import_keyword = match Regex::new(r#"(?i)@import\b"#) {
        Ok(re) => re,
        Err(_) => return after_import_strip.into_owned(),
    };
    let neutralized_imports = re_import_keyword.replace_all(&after_import_strip, "/* removed */");

    // STEP 3c: Rewrite any remaining legitimate url() references in CSS
    // declarations (background-image, list-style-image, etc.).
    // At this point NO @import url() survives, so every url() we touch here
    // is a property value — the browser loads it as an image, never as a
    // stylesheet.
    let re_css_url = match Regex::new(r#"(?i)url\(\s*['"]?(https?://[^'")]+|cid:[^'")]+)['"]?\s*\)"#) {
        Ok(re) => re,
        Err(_) => return neutralized_imports.into_owned(),
    };
    let html_after_url_rewrite = re_css_url.replace_all(
        &neutralized_imports,
        format!("url('{}')", fallback_pixel).as_str(),
    );

    // 4. CSS Property Sanitization (Prevent UI Redressing / Clickjacking inside iframe)
    let re_css_danger = match Regex::new(r#"(?i)(position\s*:\s*fixed|z-index\s*:\s*[^;}"']+)"#) {
        Ok(re) => re,
        Err(_) => return html_after_url_rewrite.into_owned(),
    };
    let final_html = re_css_danger.replace_all(&html_after_url_rewrite, "").into_owned();

    final_html
}

/// Replaces `cid:` references in HTML with base64 data URIs from extracted attachments.
///
/// Uses magic byte sniffing to determine the true MIME type, as email clients often lie
/// about Content-Type (e.g., sending a PNG as `application/octet-stream`).
/// Falls back to a transparent pixel for any unresolved CIDs to prevent broken image icons.
pub fn replace_cid_with_data_uri(
    html: &str,
    attachments: &[email_core::mime_parser::ExtractedAttachment],
) -> String {
    let mut result = html.to_string();

    for att in attachments {
        if let Some(cid) = &att.content_id {
            let clean_cid = cid.trim_matches(|c: char| {
                c == '<' || c == '>' || c == '"' || c == '\'' || c.is_whitespace()
            });
            if clean_cid.is_empty() {
                continue;
            }

            let mime_type = if att.content.starts_with(&[0xFF, 0xD8, 0xFF]) {
                "image/jpeg"
            } else if att.content.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                "image/png"
            } else if att.content.starts_with(&[0x47, 0x49, 0x46, 0x38]) {
                "image/gif"
            } else if att.content.get(8..12) == Some(b"WEBP".as_slice())
                && att.content.starts_with(&[0x52, 0x49, 0x46, 0x46])
            {
                "image/webp"
            } else if att.content.starts_with(b"<?xml")
                || att.content.starts_with(b"<svg")
                || att.content.windows(4).any(|w| w == b"<svg")
            {
                "image/svg+xml"
            } else if att.mime_type.starts_with("image/") {
                &att.mime_type
            } else {
                "image/png"
            };

            let b64 = STANDARD.encode(&att.content);
            let data_uri = format!("data:{};base64,{}", mime_type, b64);

            let escaped_cid = regex::escape(clean_cid);
            let encoded_cid = regex::escape(&clean_cid.replace(" ", "%20"));
            let pattern = format!(r#"(?i)cid:(?:<)?(?:{}|{})(?:>)?"#, escaped_cid, encoded_cid);

            if let Ok(re) = Regex::new(&pattern) {
                result = re.replace_all(&result, data_uri.as_str()).into_owned();
            }
        }
    }

    let fallback_pixel =
        "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";

    // Fallback replacements for any unresolved CIDs to prevent broken UI elements
    if let Ok(re_dq) = Regex::new(r#"(?i)((?:src|background)\s*=\s*")cid:[^"]*""#) {
        result = re_dq.replace_all(&result, format!(r#"$1{}""#, fallback_pixel).as_str()).into_owned();
    }
    if let Ok(re_sq) = Regex::new(r#"(?i)((?:src|background)\s*=\s*')cid:[^']*'"#) {
        result = re_sq.replace_all(&result, format!(r#"$1{}'"#, fallback_pixel).as_str()).into_owned();
    }
    if let Ok(re_css_dq) = Regex::new(r#"(?i)url\(\s*"cid:[^"]*"\s*\)"#) {
        result = re_css_dq.replace_all(&result, format!("url(\"{}\")", fallback_pixel).as_str()).into_owned();
    }
    if let Ok(re_css_sq) = Regex::new(r#"(?i)url\(\s*'cid:[^']*'\s*\)"#) {
        result = re_css_sq.replace_all(&result, format!("url('{}')", fallback_pixel).as_str()).into_owned();
    }
    if let Ok(re_css_nq) = Regex::new(r#"(?i)url\(\s*cid:[^)]*\s*\)"#) {
        result = re_css_nq.replace_all(&result, format!("url({})", fallback_pixel).as_str()).into_owned();
    }
    if let Ok(re_nq) = Regex::new(r#"(?i)((?:src|background)\s*=\s*)cid:[^\s>]+"#) {
        result = re_nq.replace_all(&result, format!(r#"$1"{}""#, fallback_pixel).as_str()).into_owned();
    }

    result
}
