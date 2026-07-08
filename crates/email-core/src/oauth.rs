use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use tokio::sync::Mutex;
use std::fmt;
use zeroize::Zeroizing;
use crypto::{decrypt_blob, encrypt_blob, MasterKeyManager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expiry: DateTime<Utc>,
}

#[derive(Error, Debug)]
pub enum TokenError {
    #[error("no tokens available")]
    NoTokens,
    #[error("token refresh failed: {0}")]
    RefreshFailed(String),
    #[error("storage error: {0}")]
    StorageError(String),
}

#[async_trait::async_trait]
pub trait TokenStore: Send + Sync + std::fmt::Debug {
    async fn get_tokens(&self) -> Result<Tokens, TokenError>;
    async fn set_tokens(&self, tokens: &Tokens) -> Result<(), TokenError>;
    async fn clear(&self) -> Result<(), TokenError>;
}

#[derive(Debug)]
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub fn new(account_id: &str) -> Result<Self, TokenError> {
        let mut dir = dirs::data_local_dir().ok_or_else(|| {
            TokenError::StorageError("Could not resolve local data directory".into())
        })?;
        dir.push("littmaily");
        std::fs::create_dir_all(&dir)
            .map_err(|e| TokenError::StorageError(format!("Failed to create data dir: {}", e)))?;

        let safe_id = account_id.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        dir.push(format!("tokens_{}.json", safe_id));
        Ok(Self { path: dir })
    }

    /// Migrates the token file from a temporary email-based filename to a permanent UUID-based filename.
    /// Used during account creation to stabilize the file path before the account ID is fully generated.
    pub fn rename(old_key: &str, new_key: &str) -> Result<(), TokenError> {
        let mut dir = dirs::data_local_dir().ok_or_else(|| {
            TokenError::StorageError("Could not resolve local data directory".into())
        })?;
        dir.push("littmaily");
        let safe_old = old_key.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let safe_new = new_key.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let old_path = dir.join(format!("tokens_{}.json", safe_old));
        let new_path = dir.join(format!("tokens_{}.json", safe_new));
        if old_path.exists() {
            std::fs::rename(old_path, new_path)
                .map_err(|e| TokenError::StorageError(e.to_string()))?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl TokenStore for FileStore {
    async fn get_tokens(&self) -> Result<Tokens, TokenError> {
        let encrypted = fs::read(&self.path)
            .await
            .map_err(|e| TokenError::StorageError(e.to_string()))?;
        let key = MasterKeyManager::get_or_create_key()
            .map_err(|e| TokenError::StorageError(e.to_string()))?;
        let decrypted = decrypt_blob(&key, &encrypted)
            .map_err(|e| TokenError::StorageError(format!("Decryption failed: {}", e)))?;
        let json = String::from_utf8(decrypted)
            .map_err(|e| TokenError::StorageError(e.to_string()))?;
        serde_json::from_str(&json).map_err(|e| TokenError::StorageError(e.to_string()))
    }

    async fn set_tokens(&self, tokens: &Tokens) -> Result<(), TokenError> {
        let json = serde_json::to_string_pretty(tokens)
            .map_err(|e| TokenError::StorageError(e.to_string()))?;
        let key = MasterKeyManager::get_or_create_key()
            .map_err(|e| TokenError::StorageError(e.to_string()))?;
        let encrypted = encrypt_blob(&key, json.as_bytes())
            .map_err(|e| TokenError::StorageError(format!("Encryption failed: {}", e)))?;
        fs::write(&self.path, encrypted)
            .await
            .map_err(|e| TokenError::StorageError(e.to_string()))
    }

    async fn clear(&self) -> Result<(), TokenError> {
        if self.path.exists() {
            fs::remove_file(&self.path)
                .await
                .map_err(|e| TokenError::StorageError(e.to_string()))?;
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct MemoryStore {
    tokens: Mutex<Option<Tokens>>,
}

#[async_trait::async_trait]
impl TokenStore for MemoryStore {
    async fn get_tokens(&self) -> Result<Tokens, TokenError> {
        self.tokens
            .lock()
            .await
            .as_ref()
            .cloned()
            .ok_or(TokenError::NoTokens)
    }

    async fn set_tokens(&self, tokens: &Tokens) -> Result<(), TokenError> {
        *self.tokens.lock().await = Some(tokens.clone());
        Ok(())
    }

    async fn clear(&self) -> Result<(), TokenError> {
        *self.tokens.lock().await = None;
        Ok(())
    }
}

#[derive(Debug)]
pub struct TokenManager<S: TokenStore> {
    store: Arc<S>,
    client_id: String,
    client_secret: String,
    token_url: String,
    refresh_before: chrono::Duration,
}

impl<S: TokenStore> Clone for TokenManager<S> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            token_url: self.token_url.clone(),
            refresh_before: self.refresh_before,
        }
    }
}

impl<S: TokenStore + 'static> TokenManager<S> {
    pub fn new(store: S, client_id: String, client_secret: String, token_url: String) -> Self {
        Self {
            store: Arc::new(store),
            client_id,
            client_secret,
            token_url,
            refresh_before: chrono::Duration::minutes(5),
        }
    }

    pub async fn get_access_token(&self) -> Result<String, TokenError> {
        let tokens = self.store.get_tokens().await?;
        let now = Utc::now();

        // Proactively refresh 5 minutes before expiry to prevent requests from failing
        // with an expired token during network latency or clock skew.
        if tokens.expiry - self.refresh_before > now {
            return Ok(tokens.access_token);
        }

        let new_tokens = self.refresh(&tokens.refresh_token).await?;
        self.store.set_tokens(&new_tokens).await?;
        Ok(new_tokens.access_token)
    }

    async fn refresh(&self, refresh_token: &str) -> Result<Tokens, TokenError> {
        let client = reqwest::Client::new();
        let params = [
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];
        let resp = client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| TokenError::RefreshFailed(e.to_string()))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(TokenError::RefreshFailed(text));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| TokenError::RefreshFailed(e.to_string()))?;

        let access_token = data["access_token"]
            .as_str()
            .ok_or(TokenError::RefreshFailed("missing access_token".into()))?
            .to_string();
        let expires_in = data["expires_in"].as_i64().unwrap_or(3600);
        let expiry = Utc::now() + chrono::Duration::seconds(expires_in);

        // Some providers rotate the refresh token on every access token refresh
        let new_refresh_token = data["refresh_token"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| refresh_token.to_string());

        Ok(Tokens {
            access_token,
            refresh_token: new_refresh_token,
            expiry,
        })
    }

    pub async fn set_tokens(&self, tokens: &Tokens) -> Result<(), TokenError> {
        self.store.set_tokens(tokens).await
    }
}

/// Represents the authentication state for an email account.
/// `Password` holds the raw credentials for basic IMAP/SMTP auth.
/// `OAuth2` holds a `TokenManager` that handles transparent token refresh.
#[derive(Clone)]
pub enum Credentials<S: TokenStore> {
    Password {
        full_name: String,
        email: String,
        password: Zeroizing<String>,
    },
    OAuth2 {
        email: String,
        token_manager: TokenManager<S>,
    },
}

impl<S: TokenStore> fmt::Debug for Credentials<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Password { full_name, email, .. } => f
                .debug_struct("Credentials::Password")
                .field("full_name", full_name)
                .field("email", email)
                .field("password", &"[REDACTED]")
                .finish(),
            Self::OAuth2 { email, .. } => f
                .debug_struct("Credentials::OAuth2")
                .field("email", email)
                .finish(),
        }
    }
}

impl Credentials<FileStore> {
    pub fn oauth2(
        email: String,
        account_id: String,
        client_id: String,
        client_secret: String,
        token_url: String,
    ) -> Result<Self, TokenError> {
        let store = FileStore::new(&account_id)?;
        let manager = TokenManager::new(store, client_id, client_secret, token_url);
        Ok(Credentials::OAuth2 {
            email,
            token_manager: manager,
        })
    }
}
