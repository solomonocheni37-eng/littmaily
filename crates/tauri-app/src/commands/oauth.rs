use crate::error::AppError;
use crate::state::AppState;
use email_core::oauth::{FileStore, TokenManager};
use email_core::oauth_flow::OAuth2Config;
use tauri::State;

/// Initiates the OAuth2 PKCE flow, generating a local TCP listener or using a custom URI scheme.
#[tauri::command]
#[specta::specta]
pub async fn start_oauth2_login(
    state: State<'_, AppState>,
    client_id: String,
    client_secret: Option<String>,
    auth_url: String,
    token_url: String,
    scopes: Vec<String>,
    extra_auth_params: Vec<(String, String)>,
    redirect_uri: Option<String>,
) -> Result<String, AppError> {
    // Enforce a single active OAuth flow to prevent state collisions and verifier mismatches.
    // Recover from mutex poisoning to avoid permanently locking the user out of adding accounts.
    {
        let flow_lock = match state.pending_flow.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::error!("OAuth2 pending flow mutex poisoned! Recovering...");
                poisoned.into_inner()
            }
        };
        if flow_lock.is_some() {
            return Err(AppError::BadRequest(
                "An authentication flow is already in progress.".into(),
            ));
        }
    }

    let config = OAuth2Config {
        client_id,
        client_secret,
        auth_url,
        token_url,
        scopes,
        extra_auth_params,
    };
    let (auth_url_str, flow) = config.start_flow(redirect_uri).await?;

    {
        let mut flow_lock = match state.pending_flow.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::error!("OAuth2 pending flow mutex poisoned! Recovering...");
                poisoned.into_inner()
            }
        };
        *flow_lock = Some(flow);
    }

    open::that(&auth_url_str)?;
    Ok("System browser opened. Please complete the login in your browser.".into())
}

/// Completes the OAuth2 flow by exchanging the authorization code for tokens.
///
/// Accepts either a deep-link callback (from the OS custom URI scheme) or falls back
/// to waiting on the local TCP listener (for standard localhost redirects).
#[tauri::command]
#[specta::specta]
pub async fn complete_oauth2_login(
    state: State<'_, AppState>,
    email: String,
    client_id: String,
    client_secret: String,
    token_url: String,
    deep_link_code: Option<String>,
    deep_link_state: Option<String>,
) -> Result<String, AppError> {
    let flow = {
        let mut flow_lock = match state.pending_flow.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::error!("OAuth2 pending flow mutex poisoned! Recovering...");
                poisoned.into_inner()
            }
        };
        flow_lock
            .take()
            .ok_or_else(|| AppError::BadRequest("No pending OAuth2 flow found.".into()))?
    };

    // If the OS intercepted the redirect via deep-link, use those params directly.
    // Otherwise, block and wait for the local TCP listener to catch the browser redirect.
    let tokens = if let (Some(code), Some(state_str)) = (deep_link_code, deep_link_state) {
        flow.complete_with_code(code, state_str).await?
    } else {
        flow.complete().await?
    };

    let store = FileStore::new(&email).map_err(|e| AppError::System(e.to_string()))?;
    let manager = TokenManager::new(store, client_id, client_secret, token_url);
    manager.set_tokens(&tokens).await?;

    Ok("Success".into())
}
