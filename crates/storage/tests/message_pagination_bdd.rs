// FILE: ./crates/storage/tests/message_pagination_bdd.rs
use storage::db::init_test_pool;
use storage::repository::{AccountRepository, MailboxRepository, MessageRepository};

#[tokio::test]
async fn given_multiple_messages_when_paginating_without_cursor_then_returns_latest() {
    let (pool, _temp_dir) = init_test_pool().await.unwrap();
    let acc_repo = AccountRepository::new(&pool);
    let mb_repo = MailboxRepository::new(&pool);
    let msg_repo = MessageRepository::new(&pool);
    let acc = acc_repo
        .create(
            "user@test.com",
            "test",
            "imap",
            993,
            "smtp",
            587,
            "password",
            None,
            None,
            None,
        )
        .await
        .unwrap();
    mb_repo
        .upsert(&acc.id, "INBOX", Some("/"), &[])
        .await
        .unwrap();
    for i in 1..=10 {
        msg_repo
            .upsert(
                &acc.id,
                "INBOX",
                i,
                Some(&format!("Msg {}", i)),
                None,
                None,
                None, // date_timestamp
                &[],
                100,
                false,
                None,
                None,
                None,
                None, // message_id
                None, // in_reply_to
                None, // references_json
                None, // thread_id
                None, // thread_subject
            )
            .await
            .unwrap();
    }
    let page = msg_repo
        .list_cursor(&acc.id, "INBOX", None, 5)
        .await
        .unwrap();
    assert_eq!(page.len(), 5);
    // Verifies the default descending UID ordering for the initial page load.
    assert_eq!(page[0].uid, 10);
    assert_eq!(page[4].uid, 6);
}

#[tokio::test]
async fn given_cursor_when_paginating_then_returns_older_messages() {
    let (pool, _temp_dir) = init_test_pool().await.unwrap();
    let acc_repo = AccountRepository::new(&pool);
    let mb_repo = MailboxRepository::new(&pool);
    let msg_repo = MessageRepository::new(&pool);
    let acc = acc_repo
        .create(
            "user@test.com",
            "test",
            "imap",
            993,
            "smtp",
            587,
            "password",
            None,
            None,
            None,
        )
        .await
        .unwrap();
    mb_repo
        .upsert(&acc.id, "INBOX", Some("/"), &[])
        .await
        .unwrap();
    for i in 1..=10 {
        msg_repo
            .upsert(
                &acc.id,
                "INBOX",
                i,
                Some(&format!("Msg {}", i)),
                None,
                None,
                None, // date_timestamp
                &[],
                100,
                false,
                None,
                None,
                None,
                None, // message_id
                None, // in_reply_to
                None, // references_json
                None, // thread_id
                None, // thread_subject
            )
            .await
            .unwrap();
    }
    // Action: Cursor at UID 6 (meaning we want messages < 6)
    let page = msg_repo
        .list_cursor(&acc.id, "INBOX", Some(6), 3)
        .await
        .unwrap();
    assert_eq!(page.len(), 3);
    // Validates keyset pagination: passing a cursor (UID) must return
    // strictly older messages (UID < cursor) to prevent skipping or duplicating items
    // when new messages arrive during pagination.
    assert_eq!(page[0].uid, 5);
    assert_eq!(page[2].uid, 3);
}
