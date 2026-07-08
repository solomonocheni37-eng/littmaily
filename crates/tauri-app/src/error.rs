use serde::Serialize;
use specta::Type;
use thiserror::Error;

/// The `#[serde(tag = "kind", content = "message")]` attribute ensures the error is
/// serialized as a discriminated union in TypeScript, allowing the frontend to
/// pattern-match on `error.kind` rather than parsing string messages.
#[derive(Debug, Error, Serialize, Type)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Authentication failed: {0}")]
    Auth(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("System/OS error: {0}")]
    System(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::System(err.to_string())
    }
}

impl From<email_core::oauth::TokenError> for AppError {
    fn from(err: email_core::oauth::TokenError) -> Self {
        AppError::Auth(err.to_string())
    }
}

impl From<email_core::mime_parser::MimeError> for AppError {
    fn from(err: email_core::mime_parser::MimeError) -> Self {
        AppError::Internal(err.to_string())
    }
}

// Catch-all for reqwest and other network-related string errors
impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Network(err.to_string())
    }
}
