// FILE: ./crates/storage/tests/account_bdd.rs
use storage::db::init_test_pool;
use storage::repository::{AccountRepository, MailboxRepository};

#[tokio::test]
async fn given_new_account_when_saved_then_can_be_retrieved() {
    let (pool, _temp_dir) = init_test_pool().await.unwrap();
    let repo = AccountRepository::new(&pool);
    let created = repo
        .create(
            "user@gmail.com",
            "gmail",
            "imap.gmail.com",
            993,
            "smtp.gmail.com",
            465,
            "password",
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let retrieved = repo.get_by_email("user@gmail.com").await.unwrap().unwrap();
    assert_eq!(created.email, retrieved.email);
    assert_eq!(created.provider, "gmail");
    assert_eq!(created.smtp_port, 465);
}

#[tokio::test]
async fn given_multiple_accounts_when_listed_then_returns_all() {
    let (pool, _temp_dir) = init_test_pool().await.unwrap();
    let repo = AccountRepository::new(&pool);
    repo.create(
        "user1@gmail.com",
        "gmail",
        "imap.gmail.com",
        993,
        "smtp.gmail.com",
        465,
        "password",
        None,
        None,
        None,
    )
    .await
    .unwrap();
    repo.create(
        "user2@outlook.com",
        "outlook",
        "outlook.office365.com",
        993,
        "smtp.office365.com",
        587,
        "password",
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let accounts = repo.list_all().await.unwrap();
    assert_eq!(accounts.len(), 2);
}

#[tokio::test]
async fn given_mailbox_when_upserted_then_updates_attributes() {
    let (pool, _temp_dir) = init_test_pool().await.unwrap();
    let acc_repo = AccountRepository::new(&pool);
    let mb_repo = MailboxRepository::new(&pool);
    let account = acc_repo
        .create(
            "user@gmail.com",
            "gmail",
            "imap.gmail.com",
            993,
            "smtp.gmail.com",
            465,
            "password",
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let attrs = vec!["\\HasNoChildren".to_string()];
    let mb1 = mb_repo
        .upsert(&account.id, "INBOX", Some("/"), &attrs)
        .await
        .unwrap();
    let new_attrs = vec!["\\HasNoChildren".to_string(), "\\Marked".to_string()];
    let mb2 = mb_repo
        .upsert(&account.id, "INBOX", Some("/"), &new_attrs)
        .await
        .unwrap();
    assert_eq!(mb1.id, mb2.id);
    // Attributes are stored as a JSON array string. This verifies that upserting
    // completely overwrites the JSON representation rather than appending to it.
    assert_eq!(mb2.attributes, r#"["\\HasNoChildren","\\Marked"]"#);
}
