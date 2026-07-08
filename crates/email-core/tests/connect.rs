use async_imap::types::Name;
use email_core::connect_imap;
use futures::TryStreamExt;

// Integration test requiring live IMAP server credentials.
// Skips gracefully in CI environments where these env vars are not injected.
#[tokio::test]
async fn test_connect_and_list_mailboxes() {
    let host = std::env::var("IMAP_HOST").ok();
    let user = std::env::var("IMAP_USER").ok();
    let pass = std::env::var("IMAP_PASS").ok();

    let (host, user, pass) = match (host, user, pass) {
        (Some(h), Some(u), Some(p)) => (h, u, p),
        _ => {
            eprintln!("Skipping test: IMAP_HOST, IMAP_USER, or IMAP_PASS not set");
            return;
        }
    };

    let mut session = connect_imap(&host, 993, &user, &pass)
        .await
        .expect("Failed to connect and authenticate");

    let mailboxes: Vec<Name> = session
        .list(None, Some("*"))
        .await
        .expect("LIST command failed")
        .try_collect()
        .await
        .expect("Failed to collect mailbox list");

    let mailbox_names: Vec<String> = mailboxes
        .into_iter()
        .map(|m: Name| m.name().to_string())
        .collect();

    eprintln!("Mailboxes: {:?}", mailbox_names);
    assert!(!mailbox_names.is_empty(), "Expected at least one mailbox");

    // Asserting INBOX exists is a standard IMAP compliance check;
    // a server without an INBOX is fundamentally broken or not a standard IMAP server.
    assert!(
        mailbox_names.contains(&"INBOX".to_string()),
        "INBOX must exist"
    );
}
