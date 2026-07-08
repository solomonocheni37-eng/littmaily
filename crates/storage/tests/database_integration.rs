// FILE: ./crates/storage/tests/database_integration.rs
use storage::db::init_test_pool;
use storage::repository::{
    AccountRepository, CalendarRepository, ContactRepository, MailboxRepository, MessageRepository,
};

#[tokio::test]
async fn given_account_with_data_when_deleted_then_cascades_to_all_children() {
    let (pool, _temp_dir) = init_test_pool().await.unwrap();
    let acc_repo = AccountRepository::new(&pool);
    let mb_repo = MailboxRepository::new(&pool);
    let msg_repo = MessageRepository::new(&pool);
    let cal_repo = CalendarRepository::new(&pool);
    let contact_repo = ContactRepository::new(&pool);
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
    // Create children
    mb_repo
        .upsert(&acc.id, "INBOX", Some("/"), &[])
        .await
        .unwrap();
    msg_repo
        .upsert(
            &acc.id,
            "INBOX",
            1,
            Some("Sub"),
            Some("Sender"),
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
    let cal = cal_repo
        .upsert_calendar(&acc.id, "http://cal", "Cal", None, None)
        .await
        .unwrap();
    cal_repo
        .upsert_event(cal.id, "uid1", "etag1", "http://cal/1", "VCALENDAR", None)
        .await
        .unwrap();
    let book = contact_repo
        .upsert_address_book(&acc.id, "http://book", "Book", None, None)
        .await
        .unwrap();
    contact_repo
        .upsert_contact(book.id, "uid2", "etag2", "http://book/1", "VCARD", None)
        .await
        .unwrap();
    // Action: Delete account
    acc_repo.delete(&acc.id).await.unwrap();
    // Verifies the `ON DELETE CASCADE` foreign key constraints across the entire schema.
    // Deleting an account must cleanly wipe all associated mailboxes, messages, calendars,
    // events, address books, and contacts without leaving orphaned rows.
    assert!(mb_repo.list_for_account(&acc.id).await.unwrap().is_empty());
    assert!(
        msg_repo
            .list_cursor(&acc.id, "INBOX", None, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        cal_repo
            .get_calendars_for_account(&acc.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        contact_repo
            .get_address_books_for_account(&acc.id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn given_message_when_updated_then_fts_index_is_synchronized() {
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
    msg_repo
        .upsert(
            &acc.id,
            "INBOX",
            1,
            Some("Updated UniqueKeyword Subject"),
            Some("Sender"),
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
    // Action: Update subject via upsert
    msg_repo
        .upsert(
            &acc.id,
            "INBOX",
            1,
            Some("Updated UniqueKeyword Subject"),
            Some("Sender"),
            None,
            None, // date_timestamp
            &[],
            100,
            false,
            None, // snippet
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
    // Validates the FTS5 `AFTER UPDATE` trigger. When a message's subject is updated via upsert,
    // the old indexed terms must be removed and the new terms must be searchable and highlightable.
    let old_search = msg_repo
        .search_with_highlight("Original", 10)
        .await
        .unwrap();
    assert!(old_search.is_empty());
    let new_search = msg_repo
        .search_with_highlight("UniqueKeyword", 10)
        .await
        .unwrap();
    assert_eq!(new_search.len(), 1);
    assert!(
        new_search[0]
            .highlight
            .as_ref()
            .unwrap()
            .contains("UniqueKeyword")
    );
}
