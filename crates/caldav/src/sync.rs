// FILE: ./crates/caldav/src/sync.rs
use crate::client::CalDavClient;
use crate::error::CalDavError;
use crate::models::{Calendar, CalendarObject};

/// Handles calendar discovery and incremental synchronization using RFC 6578.
pub struct SyncEngine {
    client: CalDavClient,
}

impl SyncEngine {
    pub fn new(client: CalDavClient) -> Self {
        Self { client }
    }

    /// Discovers all calendar collections within a given home set URL.
    ///
    /// Parses the PROPFIND response to extract URLs, display names, and sync tokens.
    /// Note that server responses may include non-calendar resources (like address books
    /// if the home set is shared), so we strictly filter by the `calendar` resourcetype.
    pub async fn discover_calendars(
        &self,
        home_set_url: &str,
    ) -> Result<Vec<Calendar>, CalDavError> {
        let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<propfind xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav" xmlns:CS="http://calendarserver.org/ns/">
<prop>
<displayname />
<resourcetype />
<CS:getctag />
<sync-token />
</prop>
</propfind>"#;
        let xml_resp = self.client.propfind(home_set_url, "1", body).await?;
        let doc = roxmltree::Document::parse(&xml_resp)
            .map_err(|e| CalDavError::XmlParseError(e.to_string()))?;

        let mut calendars = Vec::new();
        for response in doc.descendants().filter(|n| n.has_tag_name("response")) {
            let href = response
                .descendants()
                .find(|n| n.has_tag_name("href"))
                .and_then(|n| n.text())
                .unwrap_or("")
                .trim()
                .to_string();

            let mut is_calendar = false;
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
                            if rt.descendants().any(|n| n.has_tag_name("calendar")) {
                                is_calendar = true;
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

            if is_calendar && !href.is_empty() {
                calendars.push(Calendar {
                    url: href,
                    display_name,
                    ctag,
                    sync_token,
                });
            }
        }
        Ok(calendars)
    }

    /// Performs an incremental sync using the RFC 6578 `sync-collection` report.
    ///
    /// Returns a tuple of:
    /// - Changed or newly added calendar objects.
    /// - URLs of objects that have been deleted on the server.
    /// - The new sync-token to use for the next incremental sync.
    ///
    /// If the server rejects the provided `old_sync_token` (e.g., it expired),
    /// the caller should catch the error and retry with `None` to force a full sync.
    pub async fn sync_collection(
        &self,
        calendar: &Calendar,
        old_sync_token: Option<&str>,
    ) -> Result<(Vec<CalendarObject>, Vec<String>, String), CalDavError> {
        let token = old_sync_token.unwrap_or("");
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8" ?>
<sync-collection xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
<sync-token>{}</sync-token>
<sync-level>1</sync-level>
<prop>
<getetag/>
</prop>
</sync-collection>"#,
            token
        );
        let xml_resp = self.client.report(&calendar.url, &body).await?;
        let doc = roxmltree::Document::parse(&xml_resp)
            .map_err(|e| CalDavError::XmlParseError(e.to_string()))?;

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

            if let Some(status) = response
                .descendants()
                .find(|n| n.has_tag_name("status"))
                .and_then(|n| n.text())
            {
                // A 404 status within a 207 Multi-Status response indicates the resource was deleted
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
                // sync-collection only returns etags/hrefs; we must fetch the actual iCal data individually
                let ical_data = self.client.get_ical(&href).await.unwrap_or_default();
                let uid = extract_uid_from_ical(&ical_data).unwrap_or_default();
                changed_objects.push(CalendarObject {
                    uid,
                    etag,
                    url: href,
                    ical_data,
                    last_modified: None,
                });
            }
        }
        Ok((changed_objects, deleted_hrefs, new_sync_token))
    }

    /// Performs the full RFC 6764 discovery chain to find calendar collections.
    ///
    /// Tries multiple strategies in order of preference:
    /// 1. Direct home-set discovery (some servers expose it on the base URL).
    /// 2. Standard chain: Base URL -> Principal URL -> Home Set URL.
    /// 3. Fallback: Assumes the provided `base_url` is already the home set (legacy behavior).
    pub async fn discover_full_chain(&self, base_url: &str) -> Result<Vec<Calendar>, CalDavError> {
        // 1. Try to find calendar-home-set directly (some servers expose it on the base URL)
        if let Ok(home_set) = self.discover_calendar_home_set(base_url).await {
            if let Ok(cals) = self.discover_calendars(&home_set).await {
                if !cals.is_empty() {
                    return Ok(cals);
                }
            }
        }

        // 2. Standard chain: Base -> Principal -> Home Set
        if let Ok(principal) = self.discover_principal(base_url).await {
            if let Ok(home_set) = self.discover_calendar_home_set(&principal).await {
                return self.discover_calendars(&home_set).await;
            }
        }

        // 3. Fallback: Assume base_url is already the home set (legacy behavior)
        self.discover_calendars(base_url).await
    }

    /// Extracts the current user's principal URL from the server root.
    async fn discover_principal(&self, url: &str) -> Result<String, CalDavError> {
        let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:">
<D:prop><D:current-user-principal/></D:prop>
</D:propfind>"#;
        let xml_resp = self.client.propfind(url, "0", body).await?;
        let doc = roxmltree::Document::parse(&xml_resp)
            .map_err(|e| CalDavError::XmlParseError(e.to_string()))?;

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
        Err(CalDavError::DiscoveryFailed(
            "current-user-principal not found".into(),
        ))
    }

    /// Extracts the calendar home set URL from a principal URL.
    async fn discover_calendar_home_set(&self, principal_url: &str) -> Result<String, CalDavError> {
        let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
<D:prop><C:calendar-home-set/></D:prop>
</D:propfind>"#;
        let xml_resp = self.client.propfind(principal_url, "0", body).await?;
        let doc = roxmltree::Document::parse(&xml_resp)
            .map_err(|e| CalDavError::XmlParseError(e.to_string()))?;

        for response in doc.descendants().filter(|n| n.has_tag_name("response")) {
            if let Some(prop) = response.descendants().find(|n| n.has_tag_name("prop")) {
                if let Some(chs) = prop
                    .descendants()
                    .find(|n| n.has_tag_name("calendar-home-set"))
                {
                    if let Some(href) = chs
                        .descendants()
                        .find(|n| n.has_tag_name("href"))
                        .and_then(|n| n.text())
                    {
                        return Ok(href.trim().to_string());
                    }
                }
            }
        }
        Err(CalDavError::DiscoveryFailed(
            "calendar-home-set not found".into(),
        ))
    }
}

/// Unfolds iCalendar/vCard lines according to RFC 5545 / RFC 6350.
///
/// Long lines are folded by inserting a CRLF (or LF) followed by a single whitespace character.
/// This must be done before parsing properties to ensure multi-line values (like long UIDs)
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

/// Robust UID extraction from raw iCalendar text (RFC 5545).
///
/// Handles both standard `UID:` and parameterized `UID;PARAM=value:` syntax.
/// Returns `None` if the UID property is missing or empty.
fn extract_uid_from_ical(ical: &str) -> Option<String> {
    let unfolded = unfold_lines(ical);
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
