use crate::{clean_message_id, decode_mime_header, normalize_subject, parse_references_header};
use sha2::{Digest, Sha256};

/// Extracted fields used to construct the email threading graph.
/// `thread_id` is resolved by taking the first available ID in this order:
/// 1. The oldest message in the `References` header.
/// 2. The `In-Reply-To` header.
/// 3. The message's own `Message-ID` (if it's the root of a new thread).
#[derive(Debug, Clone, Default)]
pub struct ThreadingFields {
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub thread_id: String,
    pub thread_subject: String,
}

impl ThreadingFields {
    /// Extracts threading fields from an IMAP Fetch result (Envelope + Header bytes)
    pub fn from_imap_fetch(fetch: &async_imap::types::Fetch, subject: &str, uid: u32) -> Self {
        let envelope = fetch.envelope();
        let message_id = envelope
            .and_then(|e| e.message_id.as_ref())
            .map(|cow| clean_message_id(&decode_mime_header(cow)))
            .filter(|s| !s.is_empty());

        let in_reply_to_raw = envelope
            .and_then(|e| e.in_reply_to.as_ref())
            .map(|cow| decode_mime_header(cow))
            .unwrap_or_default();
        let in_reply_to = in_reply_to_raw
            .split_whitespace()
            .map(|s| clean_message_id(s))
            .find(|s| !s.is_empty());

        let references = fetch.header()
            .map(|h| parse_references_header(h))
            .unwrap_or_default();

        let thread_id = references.first()
            .or(in_reply_to.as_ref())
            .or(message_id.as_ref())
            .cloned()
            .unwrap_or_else(|| format!("fallback-{}", uid));

        let thread_subject = normalize_subject(subject);

        Self {
            message_id,
            in_reply_to,
            references,
            thread_id,
            thread_subject,
        }
    }

    /// Extracts threading fields from a parsed mail_parser::Message
    pub fn from_mime_message(message: &mail_parser::Message<'_>, subject: Option<&str>, raw_mime: &[u8]) -> Self {
        let message_id = message.message_id().map(|s| clean_message_id(s));

        let in_reply_to_raw = match message.in_reply_to() {
            mail_parser::HeaderValue::Text(t) => t.to_string(),
            mail_parser::HeaderValue::TextList(list) => {
                list.iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(" ")
            }
            _ => String::new(),
        };
        let in_reply_to = in_reply_to_raw
            .split_whitespace()
            .map(|id| clean_message_id(id))
            .find(|id| !id.is_empty());

        let references_raw = match message.references() {
            mail_parser::HeaderValue::Text(t) => t.to_string(),
            mail_parser::HeaderValue::TextList(list) => {
                list.iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(" ")
            }
            _ => String::new(),
        };
        let references = references_raw
            .split_whitespace()
            .map(|id| clean_message_id(id))
            .filter(|id| !id.is_empty())
            .collect::<Vec<_>>();

        let thread_id = references.first()
            .or(in_reply_to.as_ref())
            .or(message_id.as_ref())
            .cloned()
            .unwrap_or_else(|| {
                let mut hasher = Sha256::new();
                hasher.update(raw_mime);
                format!("fallback-{:x}", hasher.finalize())
            });

        let thread_subject = subject.map(normalize_subject).unwrap_or_default();

        Self {
            message_id,
            in_reply_to,
            references,
            thread_id,
            thread_subject,
        }
    }
}
