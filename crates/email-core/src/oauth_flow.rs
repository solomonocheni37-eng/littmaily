use crate::oauth::{TokenError, Tokens};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Duration, sleep};
use url::Url;

const OAUTH_FLOW_TIMEOUT_SECS: u64 = 300;

pub struct OAuth2Config {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub auth_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
    pub extra_auth_params: Vec<(String, String)>,
}

/// Manages the state of an in-progress OAuth2 authorization code flow.
/// Spins up a local TCP listener to intercept the browser's redirect callback,
/// or accepts the code/state manually if a custom URI scheme (deep link) is used.
pub struct PendingOAuth2Flow {
    listener: Option<TcpListener>,
    verifier: String,
    state: String,
    redirect_uri: String,
    config: OAuth2Config,
}

impl OAuth2Config {
    pub async fn start_flow(self, redirect_uri_override: Option<String>) -> Result<(String, PendingOAuth2Flow), TokenError> {
        let (verifier, challenge) = generate_pkce();
        let mut state_bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut state_bytes);
        let state = URL_SAFE_NO_PAD.encode(state_bytes);

        let (listener, redirect_uri) = if let Some(uri) = redirect_uri_override {
            (None, uri)
        } else {
            let l = TcpListener::bind("127.0.0.1:0").await.map_err(|e| TokenError::StorageError(format!("Failed to bind local port: {}", e)))?;
            let port = l.local_addr().unwrap().port();
            (Some(l), format!("http://127.0.0.1:{}", port))
        };

        let auth_url = build_auth_url(&self, &redirect_uri, &challenge, &state);
        Ok((auth_url, PendingOAuth2Flow { listener, verifier, state, redirect_uri, config: self }))
    }
}

impl PendingOAuth2Flow {
    pub async fn complete(self) -> Result<Tokens, TokenError> {
        let listener = self.listener.ok_or_else(|| TokenError::RefreshFailed("Cannot wait on TCP listener for custom URI scheme.".into()))?;
        let mut code = None;
        let timeout_duration = Duration::from_secs(OAUTH_FLOW_TIMEOUT_SECS);

        // Loop until we receive a valid callback or hit the timeout.
        // The local TCP listener accepts the raw HTTP GET request from the browser redirect.
        while code.is_none() {
            tokio::select! {
                _ = sleep(timeout_duration) => return Err(TokenError::RefreshFailed("OAuth2 flow timed out".into())),
                accept_result = listener.accept() => {
                    let (mut socket, _) = accept_result.map_err(|e| TokenError::RefreshFailed(e.to_string()))?;
                    let mut buf = vec![0; 4096];
                    let n = socket.read(&mut buf).await.map_err(|e| TokenError::RefreshFailed(e.to_string()))?;
                    let request = String::from_utf8_lossy(&buf[..n]);

                    if let Some((c, returned_state)) = extract_params_from_request(&request) {
                        if returned_state != self.state { return Err(TokenError::RefreshFailed("State mismatch".into())); }
                        code = Some(c);
                        let _ = socket.write_all(b"HTTP/1.1 200 OK\r\n\r\nSuccess").await;
                    }
                }
            }
        }
        exchange_code_for_tokens(&self.config, &code.unwrap(), &self.verifier, &self.redirect_uri).await
    }

    pub async fn complete_with_code(self, code: String, returned_state: String) -> Result<Tokens, TokenError> {
        if returned_state != self.state { return Err(TokenError::RefreshFailed("State mismatch".into())); }
        exchange_code_for_tokens(&self.config, &code, &self.verifier, &self.redirect_uri).await
    }
}

fn generate_pkce() -> (String, String) {
    let mut verifier_bytes = vec![0u8; 64];
    rand::thread_rng().fill_bytes(&mut verifier_bytes);
    let verifier_str = URL_SAFE_NO_PAD.encode(&verifier_bytes);
    let mut hasher = Sha256::new();
    hasher.update(verifier_str.as_bytes());
    let challenge_hash = hasher.finalize();
    let challenge = URL_SAFE_NO_PAD.encode(challenge_hash);
    (verifier_str, challenge)
}

fn build_auth_url(
    config: &OAuth2Config,
    redirect_uri: &str,
    challenge: &str,
    state: &str,
) -> String {
    let mut url = Url::parse(&config.auth_url).unwrap();
    {
        let mut pairs = url.query_pairs_mut();
        pairs
            .append_pair("client_id", &config.client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", &config.scopes.join(" "))
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", state);
        for (k, v) in &config.extra_auth_params {
            pairs.append_pair(k, v);
        }
    }
    url.to_string()
}

fn extract_params_from_request(request: &str) -> Option<(String, String)> {
    let first_line = request.lines().next()?;
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let path = parts[1];
    let url = Url::parse(&format!("http://localhost{}", path)).ok()?;
    let mut code = None;
    let mut state = None;
    for (k, v) in url.query_pairs() {
        if k == "code" {
            code = Some(v.to_string());
        }
        if k == "state" {
            state = Some(v.to_string());
        }
    }
    match (code, state) {
        (Some(c), Some(s)) => Some((c, s)),
        _ => None,
    }
}

async fn exchange_code_for_tokens(
    config: &OAuth2Config,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<Tokens, TokenError> {
    let client = reqwest::Client::new();
    let mut params = vec![
        ("code", code.to_string()),
        ("client_id", config.client_id.clone()),
        ("redirect_uri", redirect_uri.to_string()),
        ("grant_type", "authorization_code".to_string()),
        ("code_verifier", verifier.to_string()),
    ];
    if let Some(secret) = &config.client_secret {
        params.push(("client_secret", secret.clone()));
    }
    let resp = client
        .post(&config.token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| TokenError::RefreshFailed(e.to_string()))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(TokenError::RefreshFailed(format!(
            "Token exchange failed: {}",
            text
        )));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| TokenError::RefreshFailed(e.to_string()))?;

    let access_token = data["access_token"].as_str().unwrap_or("").to_string();
    let refresh_token = data["refresh_token"].as_str().unwrap_or("").to_string();
    let expires_in = data["expires_in"].as_i64().unwrap_or(3600);
    let expiry = chrono::Utc::now() + chrono::Duration::seconds(expires_in);

    Ok(Tokens {
        access_token,
        refresh_token,
        expiry,
    })
}
