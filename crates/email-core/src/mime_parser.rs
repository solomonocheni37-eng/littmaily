use mail_parser::{Address, MessageParser, MimeHeaders};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MimeError {
    #[error("Failed to parse MIME message")]
    ParseError,
}

#[derive(Debug, Clone)]
pub struct ExtractedAttachment {
    pub filename: Option<String>,
    pub mime_type: String,
    pub size: usize,
    pub content: Vec<u8>,
    pub content_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedEmail {
    pub subject: Option<String>,
    pub from: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub attachments: Vec<ExtractedAttachment>,
    // Threading Fields
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub thread_id: String,
    pub thread_subject: String,
}

pub fn parse_mime(raw_mime: &[u8]) -> Result<ParsedEmail, MimeError> {
    let message = MessageParser::default()
        .parse(raw_mime)
        .ok_or(MimeError::ParseError)?;

    let subject = message.subject().map(|s| s.to_string());
    let from = message
        .from()
        .map(|addr| match addr {
            Address::List(list) => {
                if let Some(first) = list.first() {
                    let name = first.name.as_deref();
                    let email = first.address.as_deref().unwrap_or("");
                    if let Some(n) = name { format!("{} <{}>", n, email) } else { email.to_string() }
                } else { String::new() }
            }
            Address::Group(groups) => {
                if let Some(first_group) = groups.first() { first_group.name.as_deref().unwrap_or("Group").to_string() } else { String::new() }
            }
        })
        .unwrap_or_default();

    let text_body = message.body_text(0).map(|s| s.to_string());
    let html_body = message.body_html(0).map(|s| s.to_string());
    let threading = crate::threading::ThreadingFields::from_mime_message(&message, subject.as_deref(), raw_mime);

    let mut attachments = Vec::new();
    let mut seen_cids = HashSet::new();
    let text_body_ids = message.text_body.clone();
    let html_body_ids = message.html_body.clone();

    for (part_id, part) in message.parts.iter().enumerate() {
        if part.is_multipart() || part.is_message() { continue; }

        let content_id = part.content_id().map(|s| {
            let s_str = s.to_string();
            s_str.trim_matches(|c: char| c == '<' || c == '>' || c == '"' || c == '\'' || c.is_whitespace()).to_string()
        });

        // Deduplicate inline images: if an image is referenced multiple times via CID,
        // only extract it once to avoid bloating the attachment list.
        if let Some(cid) = &content_id {
            if !cid.is_empty() && !seen_cids.insert(cid.clone()) { continue; }
        }

        let filename = part.attachment_name().map(|s| s.to_string());
        let is_main_body = text_body_ids.contains(&part_id) || html_body_ids.contains(&part_id);
        if is_main_body && content_id.is_none() && filename.is_none() { continue; }

        if filename.is_some() || content_id.is_some() {
            let mime_type = part.content_type()
                .map(|ct| format!("{}/{}", ct.ctype(), ct.subtype().unwrap_or("octet-stream")))
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let content = part.contents().to_vec();
            let size = content.len();
            if size > 0 {
                // Push directly without hash deduplication
                attachments.push(ExtractedAttachment { filename, mime_type, size, content, content_id });
            }
        }
    }

    Ok(ParsedEmail {
        subject,
        from,
        text_body,
        html_body,
        attachments,
        message_id: threading.message_id,
        in_reply_to: threading.in_reply_to,
        references: threading.references,
        thread_id: threading.thread_id,
        thread_subject: threading.thread_subject,
    })
}
