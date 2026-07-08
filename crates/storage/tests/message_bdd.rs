// FILE: ./crates/storage/tests/message_bdd.rs
use storage::blob::BlobStore;
use storage::db::init_test_pool;
use storage::repository::{AccountRepository, MailboxRepository, MessageRepository};
use tempfile::tempdir;

#[tokio::test]
async fn given_message_when_saved_then_can_be_retrieved_and_searched() {
    let (pool, _temp_dir) = init_test_pool().await.unwrap();
    let acc_repo = AccountRepository::new(&pool);
    let mb_repo = MailboxRepository::new(&pool);
    let msg_repo = MessageRepository::new(&pool);
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
    mb_repo
        .upsert(&account.id, "INBOX", Some("/"), &[])
        .await
        .unwrap();
    let msg = msg_repo
        .upsert(
            &account.id,
            "INBOX",
            1,
            Some("Hello World"),
            Some("Alice"),
            Some("2023-10-01"),
            None, // date_timestamp
            &["\\Seen".to_string()],
            1024,
            false,
            Some("This is a test email body snippet for searching"),
            None, // blob_hash
            None, // attachment_names
            None, // message_id
            None, // in_reply_to
            None, // references_json
            None, // thread_id
            None, // thread_subject
        )
        .await
        .unwrap();
    let retrieved = msg_repo
        .search_with_highlight("test email", 10)
        .await
        .unwrap();
    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].id, msg.id);
    assert_eq!(retrieved[0].subject.as_deref(), Some("Hello World"));
}

#[tokio::test]
async fn given_blob_when_saved_then_can_be_loaded_and_deduplicated() {
    let dir = tempdir().unwrap();
    let blob_store = BlobStore::new(dir.path().to_path_buf(), [0u8; 32]);
    blob_store.init().await.unwrap();
    let data = b"raw mime data content";
    let hash1 = blob_store.save(data).await.unwrap();
    let hash2 = blob_store.save(data).await.unwrap();
    assert_eq!(hash1, hash2);
    let loaded = blob_store.load(&hash1).await.unwrap();
    assert_eq!(loaded, data);
}
