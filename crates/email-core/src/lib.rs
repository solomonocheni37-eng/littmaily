pub mod discovery;
pub mod mime_parser;
pub mod oauth;
pub mod oauth_flow;
pub mod smtp;
pub mod sync_worker;
pub mod threading;

pub const SNIPPET_MAX_LENGTH: usize = 150;

use async_imap::Client;
use async_imap::error::Result as ImapResult;
use async_imap::imap_proto::BodyStructure;
use async_imap::types::{Flag, Name, NameAttribute};
use base64::Engine;
use futures::{StreamExt, TryStreamExt};
use oauth::{Credentials, TokenManager};
use rustls::pki_types::ServerName;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{self, AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

// ==========================================
// Threading Helpers
// ==========================================

/// Strips `<`, `>`, whitespace, and normalizes case/suffixes for robust threading.
/// Real-world gateways (Gmail, Outlook) frequently mutate Message-IDs by changing
/// case or appending routing suffixes like `@mail.gmail.com`.
pub fn clean_message_id(id: &str) -> String {
    let s = id.trim().trim_start_matches('<').trim_end_matches('>').trim();
    let mut lower = s.to_lowercase();

    // Strip known noisy gateway suffixes if they appear after the original domain
    let noisy_suffixes = ["@mail.gmail.com", "@prod.outlook.com", "@outlook.com"];
    for suffix in noisy_suffixes {
        if lower.ends_with(suffix) && lower.matches('@').count() > 1 {
            lower = lower.trim_end_matches(suffix).to_string();
        }
    }
    lower.trim_end_matches('.').to_string()
}

/// Strips localized reply/forward prefixes (e.g., "Re:", "Aw:", "Sv:") to group threads by root subject.
pub fn normalize_subject(subject: &str) -> String {
    let mut s = subject.trim().to_string();
    let prefixes = ["re:", "fwd:", "fw:", "aw:", "sv:", "vs:", "ref:", "ods:"];
    loop {
        let lower = s.to_lowercase();
        let mut matched = false;
        for p in prefixes {
            if lower.starts_with(p) {
                s = s[p.len()..].trim().to_string();
                matched = true;
                break;
            }
        }
        if !matched { break; }
    }
    s
}

/// Parses the raw `References` header bytes into a list of clean Message-IDs.
/// Handles RFC 5322 header folding (continuation lines starting with whitespace).
fn parse_references_header(raw: &[u8]) -> Vec<String> {
    let s = String::from_utf8_lossy(raw);
    let mut refs = Vec::new();
    let mut in_refs = false;
    let mut current = String::new();
    let mut in_bracket = false;

    for line in s.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("references:") {
            in_refs = true;
            let val = &line["references:".len()..];
            for c in val.chars() {
                if c == '<' { in_bracket = true; continue; }
                if c == '>' {
                    if in_bracket && !current.is_empty() {
                        refs.push(current.trim().to_string());
                        current.clear();
                    }
                    in_bracket = false;
                    continue;
                }
                if in_bracket { current.push(c); }
            }
        } else if in_refs {
            if line.starts_with(' ') || line.starts_with('\t') {
                for c in line.chars() {
                    if c == '<' { in_bracket = true; continue; }
                    if c == '>' {
                        if in_bracket && !current.is_empty() {
                            refs.push(current.trim().to_string());
                            current.clear();
                        }
                        in_bracket = false;
                        continue;
                    }
                    if in_bracket { current.push(c); }
                }
            } else {
                break; // Next header started
            }
        }
    }
    refs
}

// ==========================================
// IMAP Stream & Connection Logic
// ==========================================

/// Newtype wrapper around a TLS stream to implement `AsyncRead`/`AsyncWrite`
/// required by `async-imap`, which expects a generic stream type.
pub struct ImapStream {
    inner: tokio_rustls::client::TlsStream<TcpStream>,
}

impl fmt::Debug for ImapStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImapStream").finish()
    }
}

impl AsyncRead for ImapStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for ImapStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

struct Xoauth2Authenticator {
    response: String,
}

impl async_imap::Authenticator for Xoauth2Authenticator {
    type Response = String;
    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        self.response.clone()
    }
}

pub async fn connect_imap(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
) -> ImapResult<async_imap::Session<ImapStream>> {
    let tcp = TcpStream::connect((host, port)).await?;
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));
    let tls_stream = connector.connect(server_name, tcp).await?;
    let stream = ImapStream { inner: tls_stream };

    let client = Client::new(stream);
    let session = client.login(username, password).await.map_err(|(e, _)| e)?;
    Ok(session)
}

pub async fn connect_imap_oauth2<S>(
    host: &str,
    port: u16,
    email: &str,
    token_manager: &TokenManager<S>,
) -> ImapResult<async_imap::Session<ImapStream>>
where
    S: oauth::TokenStore + 'static,
{
    let access_token = token_manager
        .get_access_token()
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    let tcp = TcpStream::connect((host, port)).await?;
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));
    let tls_stream = connector.connect(server_name, tcp).await?;
    let stream = ImapStream { inner: tls_stream };

    let client = Client::new(stream);

    // XOAUTH2 SASL format requires null-byte separated fields: user={email}\x01auth=Bearer {token}\x01\x01
    let auth_string = format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token);
    let encoded = base64::engine::general_purpose::STANDARD.encode(auth_string.as_bytes());
    let authenticator = Xoauth2Authenticator { response: encoded };

    let session = client
        .authenticate("XOAUTH2", authenticator)
        .await
        .map_err(|(e, _)| e)?;
    Ok(session)
}

pub async fn connect_account<S>(
    host: &str,
    port: u16,
    credentials: &Credentials<S>,
) -> ImapResult<async_imap::Session<ImapStream>>
where
    S: oauth::TokenStore + 'static,
{
    match credentials {
        Credentials::Password {
            email, password, ..
        } => connect_imap(host, port, email, password.as_str()).await,
        Credentials::OAuth2 {
            email,
            token_manager,
        } => connect_imap_oauth2(host, port, email, token_manager).await,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OwnedFlag {
    Seen,
    Answered,
    Flagged,
    Deleted,
    Draft,
    Recent,
    MayCreate,
    Custom(String),
}

impl From<&Flag<'_>> for OwnedFlag {
    fn from(flag: &Flag<'_>) -> Self {
        match flag {
            Flag::Seen => OwnedFlag::Seen,
            Flag::Answered => OwnedFlag::Answered,
            Flag::Flagged => OwnedFlag::Flagged,
            Flag::Deleted => OwnedFlag::Deleted,
            Flag::Draft => OwnedFlag::Draft,
            Flag::Recent => OwnedFlag::Recent,
            Flag::MayCreate => OwnedFlag::MayCreate,
            Flag::Custom(cow) => OwnedFlag::Custom(cow.as_ref().into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnedNameAttribute {
    NoSelect,
    NoInferiors,
    Marked,
    Unmarked,
    Custom(String),
}

impl From<&NameAttribute<'_>> for OwnedNameAttribute {
    fn from(attr: &NameAttribute<'_>) -> Self {
        match attr {
            NameAttribute::NoSelect => OwnedNameAttribute::NoSelect,
            NameAttribute::NoInferiors => OwnedNameAttribute::NoInferiors,
            NameAttribute::Marked => OwnedNameAttribute::Marked,
            NameAttribute::Unmarked => OwnedNameAttribute::Unmarked,
            _ => OwnedNameAttribute::Custom(format!("{:?}", attr)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MailboxInfo {
    pub name: String,
    pub delimiter: Option<String>,
    pub attributes: Vec<OwnedNameAttribute>,
}

#[derive(Debug, Clone)]
pub struct MessageHeader {
    pub uid: u32,
    pub subject: String,
    pub from: String,
    pub date: Option<String>,
    pub flags: Vec<OwnedFlag>,
    pub size: u32,
    pub attachment_names: Option<String>,
    pub snippet: Option<String>,
    // NEW: Threading Fields
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub thread_id: String,
    pub thread_subject: String,
}

/// Decodes RFC 2047 encoded MIME headers (e.g., `=?UTF-8?Q?...?=`).
/// Strips whitespace between adjacent encoded words as required by the RFC.
pub fn decode_mime_header(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let mut result = String::new();
    let mut chars = s.char_indices().peekable();
    let mut last_end = 0;
    let mut prev_was_encoded = false;

    while let Some((i, c)) = chars.next() {
        if c == '=' && chars.peek().map(|(_, c)| *c) == Some('?') {
            let start_idx = i;
            chars.next(); // consume '?'
            while let Some((_, c)) = chars.next() {
                if c == '?' { break; }
            }
            let mut encoding = String::new();
            while let Some((_, c)) = chars.next() {
                if c == '?' { break; }
                encoding.push(c);
            }
            let mut text = String::new();
            let mut end_idx = start_idx;
            let mut valid_end = false;
            while let Some((idx, c)) = chars.next() {
                if c == '?' && chars.peek().map(|(_, c)| *c) == Some('=') {
                    chars.next(); // consume '='
                    end_idx = idx + 2;
                    valid_end = true;
                    break;
                }
                text.push(c);
            }
            if valid_end {
                let between = &s[last_end..start_idx];
                // RFC 2047: whitespace between adjacent encoded words is ignored
                if !(prev_was_encoded && between.trim().is_empty()) {
                    result.push_str(between);
                }

                let enc_upper = encoding.to_uppercase();
                let decoded_bytes = if enc_upper == "B" {
                    base64::engine::general_purpose::STANDARD.decode(&text).unwrap_or_default()
                } else if enc_upper == "Q" {
                    let mut bytes = Vec::new();
                    let mut t_chars = text.bytes();
                    while let Some(b) = t_chars.next() {
                        if b == b'_' {
                            bytes.push(b' ');
                        } else if b == b'=' {
                            let h1 = t_chars.next().unwrap_or(b'0');
                            let h2 = t_chars.next().unwrap_or(b'0');
                            let hex_str = format!("{}{}", h1 as char, h2 as char);
                            if let Ok(byte) = u8::from_str_radix(&hex_str, 16) {
                                bytes.push(byte);
                            } else {
                                bytes.push(b'=');
                                bytes.push(h1);
                                bytes.push(h2);
                            }
                        } else {
                            bytes.push(b);
                        }
                    }
                    bytes
                } else {
                    text.into_bytes()
                };
                result.push_str(&String::from_utf8_lossy(&decoded_bytes));
                last_end = end_idx;
                prev_was_encoded = true;
            }
        }
    }
    result.push_str(&s[last_end..]);
    result
}

pub async fn list_mailboxes(
    session: &mut async_imap::Session<ImapStream>,
) -> ImapResult<Vec<MailboxInfo>> {
    let stream = session.list(None, Some("*")).await?;
    let names: Vec<Name> = stream.try_collect().await?;
    let infos = names
        .iter()
        .map(|n| MailboxInfo {
            name: n.name().to_string(),
            delimiter: n.delimiter().map(|d| d.to_string()),
            attributes: n.attributes().iter().map(|a| a.into()).collect(),
        })
        .collect();
    Ok(infos)
}

pub async fn fetch_headers(
    session: &mut async_imap::Session<ImapStream>,
    mailbox: &str,
    start_uid: u32,
    end_uid: u32,
) -> ImapResult<Vec<MessageHeader>> {
    session.select(mailbox).await?;
    let range = match (start_uid, end_uid) {
        (0, 0) => "1:*".to_string(),
        (s, 0) => format!("{}:*", s),
        (s, e) => format!("{}:{}", s, e),
    };

    // CRITICAL: Use BODY.PEEK to avoid implicitly setting the \Seen flag on the server.
    // Added BODY.PEEK[HEADER.FIELDS (REFERENCES)] to grab the thread chain without downloading the full body.
    let mut stream = session
        .uid_fetch(
            range,
            "(UID ENVELOPE FLAGS RFC822.SIZE BODYSTRUCTURE BODY.PEEK[HEADER.FIELDS (REFERENCES)])",
        )
        .await?;

    let mut headers = Vec::new();
    while let Some(fetch_result) = stream.next().await {
        let fetch = fetch_result?;
        if let Some(uid) = fetch.uid {
            let envelope = fetch.envelope();
            let subject = envelope
                .and_then(|e| e.subject.as_ref())
                .map(|cow| decode_mime_header(cow))
                .unwrap_or_default();
            let from = envelope
                .and_then(|e| e.from.as_ref())
                .and_then(|v| v.first())
                .map(|addr| {
                    let name = addr.name.as_ref().map(|cow| decode_mime_header(cow));
                    let mailbox = addr
                        .mailbox
                        .as_ref()
                        .map(|cow| decode_mime_header(cow))
                        .unwrap_or_default();
                    match name {
                        Some(n) => format!("{} <{}>", n, mailbox),
                        None => mailbox,
                    }
                })
                .unwrap_or_default();
            let date = envelope
                .and_then(|e| e.date.as_ref())
                .map(|cow| decode_mime_header(cow));

            let flags: Vec<OwnedFlag> = fetch.flags().map(|f| (&f).into()).collect();
            let size = fetch.size.unwrap_or(0);
            let attachment_names = extract_attachment_names(&fetch);
            let snippet = extract_snippet(&fetch);

            // --- O(1) Threading Extraction ---
            let threading = crate::threading::ThreadingFields::from_imap_fetch(&fetch, &subject, uid);

            headers.push(MessageHeader {
                uid,
                subject,
                from,
                date,
                flags,
                size,
                attachment_names,
                snippet,
                message_id: threading.message_id,
                in_reply_to: threading.in_reply_to,
                references: threading.references,
                thread_id: threading.thread_id,
                thread_subject: threading.thread_subject,
            });
        }
    }
    Ok(headers)
}

/// Extracts attachment filenames by recursively traversing the IMAP `BodyStructure` enum.
/// Checks both `Content-Disposition: filename` and `Content-Type: name` parameters.
pub fn extract_attachment_names(fetch: &async_imap::types::Fetch) -> Option<String> {
    let bs = fetch.bodystructure()?;
    let names = get_filenames_recursive(bs);
    if names.is_empty() {
        None
    } else {
        Some(names.join(","))
    }
}

fn get_filenames_recursive(bs: &BodyStructure) -> Vec<String> {
    let mut names = Vec::new();
    match bs {
        BodyStructure::Multipart { bodies, .. } => {
            for b in bodies {
                names.extend(get_filenames_recursive(b));
            }
        }
        BodyStructure::Message { body, .. } => {
            names.extend(get_filenames_recursive(body));
        }
        BodyStructure::Basic { common, .. } | BodyStructure::Text { common, .. } => {
            if let Some(disp) = &common.disposition {
                if let Some(params) = &disp.params {
                    for (k, v) in params {
                        if k.eq_ignore_ascii_case("filename") {
                            names.push(v.to_string());
                        }
                    }
                }
            }
            if let Some(params) = &common.ty.params {
                for (k, v) in params {
                    if k.eq_ignore_ascii_case("name") {
                        let name = v.to_string();
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
            }
        }
    }
    names
}

/// Extracts a plain-text snippet from a partial body fetch for inbox previews.
/// Falls back to stripping HTML tags if only an HTML body is available in the partial fetch.
pub fn extract_snippet(fetch: &async_imap::types::Fetch) -> Option<String> {
    if let Some(body) = fetch.body() {
        if let Some(msg) = mail_parser::MessageParser::default().parse(body) {
            if let Some(text) = msg.body_text(0) {
                let clean: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !clean.is_empty() { return Some(clean.chars().take(SNIPPET_MAX_LENGTH).collect()); }
            }
            if let Some(html) = msg.body_html(0) {
                let text = html.replace('<', " <").replace('>', "> ");
                let clean: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
                return Some(clean.chars().take(SNIPPET_MAX_LENGTH).collect());
            }
        }
        let text = String::from_utf8_lossy(body);
        let clean: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        return Some(clean.chars().take(SNIPPET_MAX_LENGTH).collect());
    }
    None
}

pub async fn fetch_full_message(
    session: &mut async_imap::Session<ImapStream>,
    mailbox: &str,
    uid: u32,
) -> ImapResult<Vec<u8>> {
    tracing::info!("[BACKEND] fetch_full_message: Selecting mailbox '{}'", mailbox);
    session.select(mailbox).await?;

    // CRITICAL FIX: Use BODY[] instead of RFC822.
    // RFC822 is obsolete and often returns empty bodies on modern/strict IMAP servers (e.g., Gmail).
    tracing::info!("[BACKEND] fetch_full_message: Fetching UID {} with BODY[]", uid);
    let mut stream = session.uid_fetch(uid.to_string(), "(BODY[])").await?;

    if let Some(fetch_result) = stream.next().await {
        let fetch = fetch_result?;
        tracing::info!("[BACKEND] fetch_full_message: Received fetch result for UID {}", uid);
        if let Some(body) = fetch.body() {
            tracing::info!("[BACKEND] fetch_full_message: Successfully extracted body ({} bytes)", body.len());
            return Ok(body.to_vec());
        } else {
            tracing::error!("[BACKEND] fetch_full_message: fetch.body() returned None for UID {}", uid);
        }
    } else {
        tracing::error!("[BACKEND] fetch_full_message: stream.next() returned None (no fetch result) for UID {}", uid);
    }
    Err(async_imap::error::Error::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Message body not found",
    )))
}

pub async fn set_message_flag(
    session: &mut async_imap::Session<ImapStream>,
    mailbox: &str,
    uid: u32,
    flag: &str,
    add: bool,
) -> ImapResult<()> {
    session.select(mailbox).await?;
    let op = if add { "+" } else { "-" };
    let query = format!("{}FLAGS ({})", op, flag);
    session
        .uid_store(uid.to_string(), query)
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    Ok(())
}

pub async fn delete_message(
    session: &mut async_imap::Session<ImapStream>,
    mailbox: &str,
    uid: u32,
) -> ImapResult<()> {
    session.select(mailbox).await?;
    session
        .uid_store(uid.to_string(), "+FLAGS (\\Deleted)")
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    session.expunge().await?.try_collect::<Vec<_>>().await?;
    Ok(())
}

pub async fn move_message(
    session: &mut async_imap::Session<ImapStream>,
    src_mailbox: &str,
    dest_mailbox: &str,
    uid: u32,
) -> ImapResult<()> {
    session.select(src_mailbox).await?;
    session.uid_copy(uid.to_string(), dest_mailbox).await?;
    session
        .uid_store(uid.to_string(), "+FLAGS (\\Deleted)")
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    session.expunge().await?.try_collect::<Vec<_>>().await?;
    Ok(())
}

pub async fn append_message(
    session: &mut async_imap::Session<ImapStream>,
    mailbox: &str,
    raw_mime: &[u8],
    flags: &[&str],
) -> ImapResult<()> {
    let _ = flags;
    session.append(mailbox, raw_mime).await?;
    Ok(())
}

pub async fn create_mailbox(
    session: &mut async_imap::Session<ImapStream>,
    name: &str,
) -> ImapResult<()> {
    session.create(name).await?;
    let _ = session.subscribe(name).await;
    Ok(())
}

pub async fn delete_mailbox(
    session: &mut async_imap::Session<ImapStream>,
    name: &str,
) -> ImapResult<()> {
    session.delete(name).await?;
    Ok(())
}

pub async fn rename_mailbox(
    session: &mut async_imap::Session<ImapStream>,
    old_name: &str,
    new_name: &str,
) -> ImapResult<()> {
    session.rename(old_name, new_name).await?;
    Ok(())
}

/// Checks if the IMAP server advertises the THREAD=REFERENCES capability (RFC 5256).
pub async fn supports_threading(
    session: &mut async_imap::Session<ImapStream>,
) -> ImapResult<bool> {
    let caps = session.capabilities().await?;
    for cap in caps.iter() {
        // async-imap's Capability enum does not implement `Display`, so `.to_string()` fails.
        // Checking the Debug string is a robust workaround to detect the `THREAD=REFERENCES` extension.
        if format!("{:?}", cap).to_uppercase().contains("THREAD=REFERENCES") {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_imap::types::Flag;
    use std::borrow::Cow;

    #[test]
    fn convert_flag_to_owned() {
        assert_eq!(OwnedFlag::from(&Flag::Seen), OwnedFlag::Seen);
        assert_eq!(OwnedFlag::from(&Flag::Answered), OwnedFlag::Answered);
        assert_eq!(
            OwnedFlag::from(&Flag::Custom(Cow::Borrowed("myflag"))),
            OwnedFlag::Custom("myflag".into())
        );
    }

    #[test]
    fn convert_name_attribute_to_owned() {
        use async_imap::types::NameAttribute;
        assert_eq!(
            OwnedNameAttribute::from(&NameAttribute::NoSelect),
            OwnedNameAttribute::NoSelect
        );
    }

    #[test]
    fn test_message_id_gateway_normalization() {
        // Standard cleaning
        assert_eq!(clean_message_id("<ABC@def.com>"), "abc@def.com");
        assert_eq!(clean_message_id("  <abc@def.com>  "), "abc@def.com");

        // Gmail mutation: appends routing domain to the end
        assert_eq!(
            clean_message_id("<original-id@company.com@mail.gmail.com>"),
            "original-id@company.com"
        );

        // Outlook mutation
        assert_eq!(
            clean_message_id("<original-id@company.com@prod.outlook.com>"),
            "original-id@company.com"
        );

        // Trailing dot cleanup
        assert_eq!(clean_message_id("<abc@def.com.>"), "abc@def.com");
    }
}
