use thiserror::Error;

/// Errors that can occur during CalDAV operations.
#[derive(Error, Debug)]
pub enum CalDavError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("XML parsing error: {0}")]
    XmlParseError(String),
    #[error("URL parse error: {0}")]
    UrlError(#[from] url::ParseError),
    /// The server returned an unexpected HTTP status code.
    /// Note that 207 Multi-Status is treated as a success for WebDAV methods.
    #[error("CalDAV server returned error status: {0}")]
    ServerError(u16),
    #[error("Discovery failed: {0}")]
    DiscoveryFailed(String),
}
