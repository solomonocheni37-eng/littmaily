use crate::error::CardDavError;
use reqwest::{
    Client,
    header::{self, HeaderMap, HeaderValue},
};
use url::Url;

/// Low-level HTTP client for WebDAV/CardDAV operations.
///
/// Handles Basic Auth, URL resolution for relative WebDAV hrefs, and the specific
/// HTTP status codes expected by WebDAV extensions.
pub struct CardDavClient {
    http_client: Client,
    base_url: Url,
    username: String,
    password: String,
}

impl CardDavClient {
    /// Creates a client configured with Basic Auth and a custom User-Agent,
    /// resolving relative WebDAV hrefs against the provided base.
    pub fn new(base_url: &str, username: &str, password: &str) -> Result<Self, CardDavError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static("Littmaily-CardDAV/1.0"),
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
    /// CardDAV servers frequently return absolute paths or relative paths in `<href>`
    /// elements. This ensures we always have a fully qualified URL for subsequent requests.
    fn resolve_url(&self, url: &str) -> Result<Url, CardDavError> {
        if url.starts_with("http://") || url.starts_with("https://") {
            Ok(Url::parse(url)?)
        } else {
            Ok(self.base_url.join(url)?)
        }
    }

    /// Executes a WebDAV PROPFIND request.
    ///
    /// Treats `207 Multi-Status` as a success code, which is standard for WebDAV
    /// but atypical for standard HTTP clients.
    pub async fn propfind(
        &self,
        url: &str,
        depth: &str,
        body: &str,
    ) -> Result<String, CardDavError> {
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

        if !status.is_success() && status.as_u16() != 207 {
            return Err(CardDavError::ServerError(status.as_u16()));
        }
        Ok(text)
    }

    /// Executes a WebDAV REPORT request, primarily used for RFC 6578 `sync-collection`.
    ///
    /// Like PROPFIND, REPORT returns `207 Multi-Status` on success.
    pub async fn report(&self, url: &str, body: &str) -> Result<String, CardDavError> {
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
            return Err(CardDavError::ServerError(status.as_u16()));
        }
        Ok(text)
    }

    /// Fetches the raw vCard data for a specific resource.
    pub async fn get_vcard(&self, url: &str) -> Result<String, CardDavError> {
        let full_url = self.resolve_url(url)?;
        let resp = self
            .http_client
            .get(full_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Accept", "text/vcard")
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(CardDavError::ServerError(status.as_u16()));
        }
        Ok(text)
    }

    /// Creates or updates a vCard resource.
    ///
    /// Uses `If-Match` for optimistic concurrency on updates, and `If-None-Match: *`
    /// to prevent accidentally overwriting existing resources during creation.
    /// Treats both 201 Created and 204 No Content as success.
    pub async fn put(&self, url: &str, body: &str, etag: Option<&str>) -> Result<(), CardDavError> {
        let full_url = self.resolve_url(url)?;
        let mut req = self
            .http_client
            .put(full_url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "text/vcard; charset=utf-8")
            .body(body.to_string());

        if let Some(etag) = etag {
            req = req.header("If-Match", etag);
        } else {
            req = req.header("If-None-Match", "*");
        }

        let resp = req.send().await?;
        let status = resp.status();

        if !status.is_success() && status.as_u16() != 201 && status.as_u16() != 204 {
            return Err(CardDavError::ServerError(status.as_u16()));
        }
        Ok(())
    }

    /// Deletes a vCard resource from the server.
    ///
    /// Treats 204 No Content as the standard success response.
    pub async fn delete(&self, url: &str, etag: Option<&str>) -> Result<(), CardDavError> {
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

        if !status.is_success() && status.as_u16() != 204 {
            return Err(CardDavError::ServerError(status.as_u16()));
        }
        Ok(())
    }

    /// Moves a vCard resource using the WebDAV MOVE method.
    ///
    /// The `Destination` header must be an absolute URI or a properly resolved relative URI.
    pub async fn move_resource(&self, src_url: &str, dest_url: &str) -> Result<(), CardDavError> {
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

        if !status.is_success() && status.as_u16() != 201 && status.as_u16() != 204 {
            return Err(CardDavError::ServerError(status.as_u16()));
        }
        Ok(())
    }
}
