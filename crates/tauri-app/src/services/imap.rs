use crate::error::AppError;
use crate::services::auth;
use email_core::oauth::Credentials;
use storage::models::Account;

pub async fn get_imap_session(
    account: &Account,
) -> Result<async_imap::Session<email_core::ImapStream>, AppError> {
    if account.auth_method == "oauth2" {
        let client_id = account.oauth_client_id.clone().unwrap_or_default();
        let client_secret = account.oauth_client_secret.clone().unwrap_or_default();
        let token_url = account.oauth_token_url.clone().unwrap_or_default();
        let creds = Credentials::oauth2(
            account.email.clone(),
            account.id.clone(),
            client_id,
            client_secret,
            token_url,
        )
        .map_err(|e| AppError::Auth(e.to_string()))?;
        email_core::connect_account(&account.imap_host, account.imap_port as u16, &creds)
            .await
            .map_err(|e| AppError::Network(e.to_string()))
    } else {
        let password = auth::get_password(&account.id, "imap")?;
        email_core::connect_imap(
            &account.imap_host,
            account.imap_port as u16,
            &account.email,
            &password,
        )
        .await
        .map_err(|e| AppError::Network(e.to_string()))
    }
}

pub async fn get_smtp_password(account: &Account) -> Result<String, AppError> {
    if account.auth_method == "oauth2" {
        let client_id = account.oauth_client_id.clone().unwrap_or_default();
        let client_secret = account.oauth_client_secret.clone().unwrap_or_default();
        let token_url = account.oauth_token_url.clone().unwrap_or_default();
        let creds = Credentials::oauth2(
            account.email.clone(),
            account.id.clone(),
            client_id,
            client_secret,
            token_url,
        )
        .map_err(|e| AppError::Auth(e.to_string()))?;
        if let Credentials::OAuth2 { token_manager, .. } = creds {
            token_manager
                .get_access_token()
                .await
                .map_err(|e| AppError::Auth(e.to_string()))
        } else {
            Err(AppError::Auth("Invalid credentials type".into()))
        }
    } else {
        auth::get_password(&account.id, "smtp")
    }
}

/// Discovers the true CalDAV/CardDAV endpoint using RFC 6764 (.well-known) and redirects.
/// This is necessary because many providers don't advertise their DAV endpoints via DNS SRV,
/// and the base URL often redirects to the actual collection path.
pub async fn discover_dav_endpoint(domain: &str, service: &str) -> Option<String> {
    let client = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return None,
    };

    // 1. RFC 6764: Try .well-known
    let well_known = format!("https://{}/.well-known/{}", domain, service);
    if let Ok(resp) = client.get(&well_known).send().await {
        let final_url = resp.url().to_string();
        let status = resp.status().as_u16();
        // If the URL changed (redirect) or we got a valid auth/method response, we found it.
        if final_url != well_known || status == 200 || status == 401 || status == 405 {
            return Some(final_url);
        }
    }

    // 2. Fallback: Try common subdomains
    let fallbacks = [
        format!("https://{}.{}", service, domain),
        format!("https://dav.{}", domain),
    ];
    for url in fallbacks {
        if let Ok(resp) = client.get(&url).send().await {
            let status = resp.status().as_u16();
            if status == 200 || status == 401 || status == 405 {
                return Some(url);
            }
        }
    }
    None
}
