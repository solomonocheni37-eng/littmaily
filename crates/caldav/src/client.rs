use crate::error::CalDavError;
use reqwest::{
    Client,
    header::{self, HeaderMap, HeaderValue},
};
use url::Url;

/// A low-level HTTP client for WebDAV/CalDAV operations.
///
/// Handles authentication, URL resolution, and the specific HTTP status codes
/// expected by WebDAV extensions (e.g., treating 207 Multi-Status as success).
pub struct CalDavClient {
    http_client: Client,
    base_url: Url,
    username: String,
    password: String,
}

impl CalDavClient {
    /// Creates a new client configured with Basic Auth.
    ///
    /// The `base_url` is used to resolve relative URLs returned by the server
    /// in `<href>` elements during PROPFIND/REPORT responses.
    pub fn new(base_url: &str, username: &str, password: &str) -> Result<Self, CalDavError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static("Littmaily-CalDAV/1.0"),
        );
        let http_client = Client::builder().default_headers(headers).build()?;
        Ok(Self {
            http_client,
            base_url: Url::parse(base_url)?,
            username: username.to_string(),
            password: password.to_string(),
        })
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Resolves a potentially relative URL against the client's base URL.
    ///
    /// CalDAV servers frequently return absolute paths or relative paths in `<href>`
    /// elements. This ensures we always have a fully qualified URL for subsequent requests.
    fn resolve_url(&self, url: &str) -> Result<Url, CalDavError> {
        if url.starts_with("http://") || url.starts_with("https://") {
            Ok(Url::parse(url)?)
        } else {
            Ok(self.base_url.join(url)?)
        }
    }

    /// Executes a WebDAV PROPFIND request to retrieve properties of a resource.
    ///
    /// Unlike standard HTTP, PROPFIND considers `207 Multi-Status` a success code
    /// because the response body contains the status of each individual property requested.
    pub async fn propfind(
        &self,
        url: &str,
        depth: &str,
        body: &str,
    ) -> Result<String, CalDavError> {
        let full_url = self.resolve_url(url)?;
        let resp = self
            .http_client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), full_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", depth)
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body.to_string())
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        // 207 Multi-Status is the standard success response for PROPFIND
        if !status.is_success() && status.as_u16() != 207 {
            return Err(CalDavError::ServerError(status.as_u16()));
        }
        Ok(text)
    }

    /// Executes a WebDAV REPORT request, typically used for `sync-collection` (RFC 6578).
    ///
    /// Like PROPFIND, REPORT returns `207 Multi-Status` on success.
    pub async fn report(&self, url: &str, body: &str) -> Result<String, CalDavError> {
        let full_url = self.resolve_url(url)?;
        let resp = self
            .http_client
            .request(reqwest::Method::from_bytes(b"REPORT").unwrap(), full_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", "1")
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body.to_string())
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() && status.as_u16() != 207 {
            return Err(CalDavError::ServerError(status.as_u16()));
        }
        Ok(text)
    }

    /// Fetches the raw iCalendar data for a specific resource.
    pub async fn get_ical(&self, url: &str) -> Result<String, CalDavError> {
        let full_url = self.resolve_url(url)?;
        let resp = self
            .http_client
            .get(full_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Accept", "text/calendar")
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(CalDavError::ServerError(status.as_u16()));
        }
        Ok(text)
    }

    /// Creates or updates an iCalendar resource on the server.
    ///
    /// Uses `If-Match` for optimistic concurrency control when updating existing resources,
    /// and `If-None-Match: *` to prevent accidentally overwriting existing resources when creating new ones.
    pub async fn put(&self, url: &str, body: &str, etag: Option<&str>) -> Result<(), CalDavError> {
        let full_url = self.resolve_url(url)?;
        let mut req = self
            .http_client
            .put(full_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "text/calendar; charset=utf-8")
            .body(body.to_string());

        // If-Match prevents overwriting concurrent changes.
        // If-None-Match: * ensures we only create new resources.
        if let Some(etag) = etag {
            req = req.header("If-Match", etag);
        } else {
            req = req.header("If-None-Match", "*");
        }

        let resp = req.send().await?;
        let status = resp.status();

        // 201 Created or 204 No Content are both valid success states for PUT
        if !status.is_success() && status.as_u16() != 201 && status.as_u16() != 204 {
            return Err(CalDavError::ServerError(status.as_u16()));
        }
        Ok(())
    }

    /// Deletes an iCalendar resource from the server.
    pub async fn delete(&self, url: &str, etag: Option<&str>) -> Result<(), CalDavError> {
        let full_url = self.resolve_url(url)?;
        let mut req = self
            .http_client
            .delete(full_url)
            .basic_auth(&self.username, Some(&self.password));

        if let Some(etag) = etag {
            req = req.header("If-Match", etag);
        }

        let resp = req.send().await?;
        let status = resp.status();

        // 204 No Content is the standard success response for DELETE
        if !status.is_success() && status.as_u16() != 204 {
            return Err(CalDavError::ServerError(status.as_u16()));
        }
        Ok(())
    }

    /// Moves a resource from one URL to another using the WebDAV MOVE method.
    pub async fn move_resource(&self, src_url: &str, dest_url: &str) -> Result<(), CalDavError> {
        let full_src_url = self.resolve_url(src_url)?;
        let full_dest_url = self.resolve_url(dest_url)?;
        let resp = self
            .http_client
            .request(reqwest::Method::from_bytes(b"MOVE").unwrap(), full_src_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Destination", full_dest_url.as_str())
            .send()
            .await?;

        let status = resp.status();

        // MOVE returns 201 Created if the destination is new, or 204 No Content if it overwrites an existing resource
        if !status.is_success() && status.as_u16() != 201 && status.as_u16() != 204 {
            return Err(CalDavError::ServerError(status.as_u16()));
        }
        Ok(())
    }
}
