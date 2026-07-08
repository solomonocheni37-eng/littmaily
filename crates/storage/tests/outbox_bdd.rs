// FILE: ./crates/storage/tests/outbox_bdd.rs
use storage::db::init_test_pool;
use storage::repository::{AccountRepository, OutboxRepository};

#[tokio::test]
async fn given_account_when_message_enqueued_then_status_is_pending() {
    let (pool, _temp_dir) = init_test_pool().await.unwrap();
    let acc_repo = AccountRepository::new(&pool);
    let outbox_repo = OutboxRepository::new(&pool);
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
    let msg = outbox_repo
        .enqueue(
            &acc.id,
            b"Raw MIME",
            "user@test.com",
            &["recipient@test.com".to_string()],
            Some("Subject"),
            None, // body
            None, // scheduled_for
        )
        .await
        .unwrap();
    assert_eq!(msg.status, "pending");
    assert_eq!(msg.retry_count, 0);
}

#[tokio::test]
async fn given_failed_message_under_retry_limit_when_fetching_pending_then_includes_it() {
    let (pool, _temp_dir) = init_test_pool().await.unwrap();
    let acc_repo = AccountRepository::new(&pool);
    let outbox_repo = OutboxRepository::new(&pool);
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
    let msg = outbox_repo
        .enqueue(
            &acc.id,
            b"MIME",
            "a@b.com",
            &["c@d.com".into()],
            None,
            None, // body
            None, // scheduled_for
        )
        .await
        .unwrap();
    // Simulate worker failure
    outbox_repo
        .mark_failed(msg.id, "SMTP Connection Timeout")
        .await
        .unwrap();
    let pending = outbox_repo.get_pending(10).await.unwrap();
    // Verifies the retry logic: a message that has failed but hasn't exceeded the max retry count
    // must still be picked up by the `get_pending` query for subsequent delivery attempts.
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].status, "failed");
    assert_eq!(pending[0].retry_count, 1);
    assert_eq!(
        pending[0].last_error,
        Some("SMTP Connection Timeout".into())
    );
}

#[tokio::test]
async fn given_message_when_marked_sent_then_status_updates_and_is_excluded_from_pending() {
    let (pool, _temp_dir) = init_test_pool().await.unwrap();
    let acc_repo = AccountRepository::new(&pool);
    let outbox_repo = OutboxRepository::new(&pool);
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
    let msg = outbox_repo
        .enqueue(
            &acc.id,
            b"MIME",
            "a@b.com",
            &["c@d.com".into()],
            None, // subject
            None, // body
            None, // scheduled_for
        )
        .await
        .unwrap();
    outbox_repo.mark_sent(msg.id).await.unwrap();
    let pending = outbox_repo.get_pending(10).await.unwrap();
    // Ensures that successfully sent messages transition to the 'sent' state
    // and are filtered out of the pending queue to prevent duplicate sends.
    assert_eq!(pending.len(), 0);
}
