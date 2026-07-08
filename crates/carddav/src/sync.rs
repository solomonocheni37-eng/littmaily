// FILE: ./crates/carddav/src/sync.rs
use crate::client::CardDavClient;
use crate::error::CardDavError;
use crate::models::{AddressBook, VCardObject};

/// Handles address book discovery and incremental contact synchronization.
pub struct SyncEngine {
    client: CardDavClient,
}

impl SyncEngine {
    pub fn new(client: CardDavClient) -> Self {
        Self { client }
    }

    /// Discovers all address book collections within a given home set URL.
    ///
    /// Filters responses by the `addressbook` resourcetype to exclude non-contact
    /// resources that might share the same home set.
    pub async fn discover_address_books(
        &self,
        home_set_url: &str,
    ) -> Result<Vec<AddressBook>, CardDavError> {
        let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<propfind xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav" xmlns:CS="http://calendarserver.org/ns/">
<prop>
<displayname />
<resourcetype />
<CS:getctag />
<sync-token />
</prop>
</propfind>"#;
        let xml_resp = self.client.propfind(home_set_url, "1", body).await?;
        let doc = roxmltree::Document::parse(&xml_resp)
            .map_err(|e| CardDavError::XmlParseError(e.to_string()))?;

        let mut address_books = Vec::new();
        for response in doc.descendants().filter(|n| n.has_tag_name("response")) {
            let href = response
                .descendants()
                .find(|n| n.has_tag_name("href"))
                .and_then(|n| n.text())
                .unwrap_or("")
                .trim()
                .to_string();

            let mut is_address_book = false;
            let mut display_name = String::new();
            let mut ctag = None;
            let mut sync_token = None;

            for propstat in response
                .descendants()
                .filter(|n| n.has_tag_name("propstat"))
            {
                let status = propstat
                    .descendants()
                    .find(|n| n.has_tag_name("status"))
                    .and_then(|n| n.text())
                    .unwrap_or("");

                if status.contains("200 OK") {
                    if let Some(prop) = propstat.descendants().find(|n| n.has_tag_name("prop")) {
                        if let Some(rt) =
                            prop.descendants().find(|n| n.has_tag_name("resourcetype"))
                        {
                            // Strictly check for the CardDAV addressbook tag to avoid false positives
                            if rt
                                .descendants()
                                .any(|n| n.tag_name().name() == "addressbook")
                            {
                                is_address_book = true;
                            }
                        }
                        if let Some(name) = prop
                            .descendants()
                            .find(|n| n.has_tag_name("displayname"))
                            .and_then(|n| n.text())
                        {
                            display_name = name.to_string();
                        }
                        if let Some(ctag_node) = prop
                            .descendants()
                            .find(|n| n.tag_name().name() == "getctag")
                        {
                            ctag = ctag_node.text().map(String::from);
                        }
                        if let Some(token_node) =
                            prop.descendants().find(|n| n.has_tag_name("sync-token"))
                        {
                            sync_token = token_node.text().map(String::from);
                        }
                    }
                }
            }

            if is_address_book && !href.is_empty() {
                address_books.push(AddressBook {
                    url: href,
                    display_name,
                    ctag,
                    sync_token,
                });
            }
        }
        Ok(address_books)
    }

    /// Performs an incremental sync using the RFC 6578 `sync-collection` report.
    ///
    /// Returns a tuple of:
    /// - Changed or newly added vCard objects.
    /// - URLs of objects deleted on the server (indicated by a 404 status inside the 207 response).
    /// - The new sync-token for the next incremental sync.
    ///
    /// If the server rejects the provided `old_sync_token` (e.g., it expired or was invalidated),
    /// the caller must catch the error and retry with `None` to force a full collection sync.
    pub async fn sync_collection(
        &self,
        address_book: &AddressBook,
        old_sync_token: Option<&str>,
    ) -> Result<(Vec<VCardObject>, Vec<String>, String), CardDavError> {
        let token = old_sync_token.unwrap_or("");
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8" ?>
<sync-collection xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav">
<sync-token>{}</sync-token>
<sync-level>1</sync-level>
<prop>
<getetag/>
</prop>
</sync-collection>"#,
            token
        );
        let xml_resp = self.client.report(&address_book.url, &body).await?;
        let doc = roxmltree::Document::parse(&xml_resp)
            .map_err(|e| CardDavError::XmlParseError(e.to_string()))?;

        let mut changed_objects = Vec::new();
        let mut deleted_hrefs = Vec::new();
        let mut new_sync_token = String::new();

        if let Some(token_node) = doc.descendants().find(|n| n.has_tag_name("sync-token")) {
            if let Some(t) = token_node.text() {
                new_sync_token = t.to_string();
            }
        }

        for response in doc.descendants().filter(|n| n.has_tag_name("response")) {
            let href = response
                .descendants()
                .find(|n| n.has_tag_name("href"))
                .and_then(|n| n.text())
                .unwrap_or("")
                .trim()
                .to_string();

            let mut etag = String::new();
            let mut is_deleted = false;

            // A 404 status within a 207 Multi-Status response indicates the resource was deleted
            if let Some(status) = response
                .descendants()
                .find(|n| n.has_tag_name("status"))
                .and_then(|n| n.text())
            {
                if status.contains("404") {
                    is_deleted = true;
                }
            }

            for propstat in response
                .descendants()
                .filter(|n| n.has_tag_name("propstat"))
            {
                let status = propstat
                    .descendants()
                    .find(|n| n.has_tag_name("status"))
                    .and_then(|n| n.text())
                    .unwrap_or("");

                if status.contains("200 OK") {
                    if let Some(e) = propstat
                        .descendants()
                        .find(|n| n.has_tag_name("getetag"))
                        .and_then(|n| n.text())
                    {
                        // Etags are typically returned wrapped in quotes by the server
                        etag = e.trim_matches('"').to_string();
                    }
                }
            }

            if is_deleted {
                deleted_hrefs.push(href);
            } else if !href.ends_with('/') && !etag.is_empty() {
                // sync-collection only returns etags/hrefs; we must fetch the actual vCard data individually
                let vcard_data = self.client.get_vcard(&href).await.unwrap_or_default();
                let uid = extract_uid_from_vcard(&vcard_data).unwrap_or_default();
                changed_objects.push(VCardObject {
                    uid,
                    etag,
                    url: href,
                    vcard_data,
                    last_modified: None,
                });
            }
        }
        Ok((changed_objects, deleted_hrefs, new_sync_token))
    }

    /// Performs the full RFC 6764 discovery chain to find address book collections.
    ///
    /// Tries multiple strategies in order of preference:
    /// 1. Direct home-set discovery (some servers expose it on the base URL).
    /// 2. Standard chain: Base URL -> Principal URL -> Home Set URL.
    /// 3. Fallback: Assumes the provided `base_url` is already the home set.
    pub async fn discover_full_chain(
        &self,
        base_url: &str,
    ) -> Result<Vec<AddressBook>, CardDavError> {
        // 1. Try to find addressbook-home-set directly
        if let Ok(home_set) = self.discover_address_book_home_set(base_url).await {
            if let Ok(books) = self.discover_address_books(&home_set).await {
                if !books.is_empty() {
                    return Ok(books);
                }
            }
        }

        // 2. Standard chain: Base -> Principal -> Home Set
        if let Ok(principal) = self.discover_principal(base_url).await {
            if let Ok(home_set) = self.discover_address_book_home_set(&principal).await {
                return self.discover_address_books(&home_set).await;
            }
        }

        // 3. Fallback: Assume base_url is already the home set
        self.discover_address_books(base_url).await
    }

    /// Extracts the current user's principal URL from the server root.
    async fn discover_principal(&self, url: &str) -> Result<String, CardDavError> {
        let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:">
<D:prop><D:current-user-principal/></D:prop>
</D:propfind>"#;
        let xml_resp = self.client.propfind(url, "0", body).await?;
        let doc = roxmltree::Document::parse(&xml_resp)
            .map_err(|e| CardDavError::XmlParseError(e.to_string()))?;

        for response in doc.descendants().filter(|n| n.has_tag_name("response")) {
            if let Some(prop) = response.descendants().find(|n| n.has_tag_name("prop")) {
                if let Some(cup) = prop
                    .descendants()
                    .find(|n| n.has_tag_name("current-user-principal"))
                {
                    if let Some(href) = cup
                        .descendants()
                        .find(|n| n.has_tag_name("href"))
                        .and_then(|n| n.text())
                    {
                        return Ok(href.trim().to_string());
                    }
                }
            }
        }
        Err(CardDavError::DiscoveryFailed(
            "current-user-principal not found".into(),
        ))
    }

    /// Extracts the address book home set URL from a principal URL.
    async fn discover_address_book_home_set(
        &self,
        principal_url: &str,
    ) -> Result<String, CardDavError> {
        let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav">
<D:prop><C:addressbook-home-set/></D:prop>
</D:propfind>"#;
        let xml_resp = self.client.propfind(principal_url, "0", body).await?;
        let doc = roxmltree::Document::parse(&xml_resp)
            .map_err(|e| CardDavError::XmlParseError(e.to_string()))?;

        for response in doc.descendants().filter(|n| n.has_tag_name("response")) {
            if let Some(prop) = response.descendants().find(|n| n.has_tag_name("prop")) {
                if let Some(ahs) = prop
                    .descendants()
                    .find(|n| n.has_tag_name("addressbook-home-set"))
                {
                    if let Some(href) = ahs
                        .descendants()
                        .find(|n| n.has_tag_name("href"))
                        .and_then(|n| n.text())
                    {
                        return Ok(href.trim().to_string());
                    }
                }
            }
        }
        Err(CardDavError::DiscoveryFailed(
            "addressbook-home-set not found".into(),
        ))
    }
}

/// Unfolds vCard lines according to RFC 6350.
///
/// Long lines are folded by inserting a CRLF (or LF) followed by a single whitespace character.
/// This must be reversed before parsing properties to ensure multi-line values (like long UIDs)
/// are correctly reconstructed.
fn unfold_lines(text: &str) -> String {
    let mut unfolded = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\r' && chars.peek() == Some(&'\n') {
            chars.next(); // consume '\n'
            if matches!(chars.peek(), Some(&' ') | Some(&'\t')) {
                chars.next(); // consume folding whitespace
            } else {
                unfolded.push_str("\r\n");
            }
        } else if c == '\n' {
            if matches!(chars.peek(), Some(&' ') | Some(&'\t')) {
                chars.next(); // consume folding whitespace
            } else {
                unfolded.push('\n');
            }
        } else {
            unfolded.push(c);
        }
    }
    unfolded
}

/// Robust UID extraction from raw vCard text (RFC 6350).
///
/// Handles both standard `UID:` and parameterized `UID;PARAM=value:` syntax.
/// Returns `None` if the UID property is missing or empty.
fn extract_uid_from_vcard(vcard: &str) -> Option<String> {
    let unfolded = unfold_lines(vcard);
    for line in unfolded.lines() {
        let upper_line = line.to_uppercase();
        if upper_line.starts_with("UID:") || upper_line.starts_with("UID;") {
            if let Some(idx) = line.find(':') {
                return Some(line[idx + 1..].trim().to_string());
            }
        }
    }
    None
}
