use async_imap::types::Name;
use chrono::Utc;
use email_core::connect_account;
use email_core::oauth::{Credentials, MemoryStore, TokenManager, Tokens};
use futures::TryStreamExt;

// Integration test requiring live IMAP server credentials.
// Skips gracefully in CI environments where these env vars are not injected.
#[tokio::test]
async fn test_oauth2_connect_and_list_mailboxes() {
    let refresh = std::env::var("REFRESH_TOKEN").ok();
    let cid = std::env::var("CLIENT_ID").ok();
    let csecret = std::env::var("CLIENT_SECRET").ok();

    let (refresh, cid, csecret) = match (refresh, cid, csecret) {
        (Some(r), Some(c), Some(s)) => (r, c, s),
        _ => {
            eprintln!("Skipping test: REFRESH_TOKEN, CLIENT_ID, or CLIENT_SECRET not set");
            return;
        }
    };

    // MemoryStore is used instead of FileStore to avoid disk I/O and file cleanup overhead in tests.
    let store = MemoryStore::default();
    let manager = TokenManager::new(
        store,
        cid.clone(),
        csecret.clone(),
        "https://oauth2.googleapis.com/token".to_string(),
    );

    // Setting expiry to `Utc::now()` forces the TokenManager to immediately refresh the token
    // on the first API call, validating the refresh flow during the connection phase.
    let dummy_tokens = Tokens {
        access_token: "initial".to_string(),
        refresh_token: refresh,
        expiry: Utc::now(),
    };
    manager.set_tokens(&dummy_tokens).await.expect("set_tokens");

    let credentials = Credentials::<MemoryStore>::OAuth2 {
        // The email address is required to construct the XOAUTH2 SASL string.
        // In a real scenario, this would be the actual user's email, but for this
        // generic test, it's hardcoded. The server validates the token, not the email format.
        email: "your.email@gmail.com".to_string(),
        token_manager: manager,
    };

    let mut session = connect_account("imap.gmail.com", 993, &credentials)
        .await
        .expect("connect_account failed");

    let mailboxes: Vec<Name> = session
        .list(None, Some("*"))
        .await
        .expect("LIST failed")
        .try_collect()
        .await
        .expect("collect failed");

    let names: Vec<String> = mailboxes
        .into_iter()
        .map(|m| m.name().to_string())
        .collect();

    eprintln!("Mailboxes: {:?}", names);
    assert!(!names.is_empty());
}
