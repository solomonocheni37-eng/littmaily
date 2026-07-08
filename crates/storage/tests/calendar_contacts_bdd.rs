// FILE: ./crates/storage/tests/calendar_contacts_bdd.rs
use storage::db::init_test_pool;
use storage::repository::{AccountRepository, CalendarRepository, ContactRepository};

#[tokio::test]
async fn given_calendar_when_sync_token_updates_then_upsert_preserves_events() {
    let (pool, _temp_dir) = init_test_pool().await.unwrap();
    let acc_repo = AccountRepository::new(&pool);
    let cal_repo = CalendarRepository::new(&pool);
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
    let cal = cal_repo
        .upsert_calendar(
            &acc.id,
            "http://cal",
            "My Cal",
            Some("ctag1"),
            Some("token1"),
        )
        .await
        .unwrap();
    cal_repo
        .upsert_event(
            cal.id,
            "uid1",
            "etag1",
            "http://cal/1",
            "BEGIN:VCALENDAR
SUMMARY:Test
END:VCALENDAR",
            None,
        )
        .await
        .unwrap();
    // Action: Update sync token on next poll
    cal_repo
        .upsert_calendar(
            &acc.id,
            "http://cal",
            "My Cal",
            Some("ctag2"),
            Some("token2"),
        )
        .await
        .unwrap();
    let cals = cal_repo.get_calendars_for_account(&acc.id).await.unwrap();
    assert_eq!(cals[0].sync_token, Some("token2".to_string()));
    // Ensures that updating the calendar collection's metadata (like sync_token or ctag)
    // via upsert does not inadvertently delete or overwrite the individual events
    // belonging to that calendar.
    let events = cal_repo.get_events_for_calendar(cal.id).await.unwrap();
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn given_contact_when_searched_via_fts_then_matches_vcard_data() {
    let (pool, _temp_dir) = init_test_pool().await.unwrap();
    let acc_repo = AccountRepository::new(&pool);
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
    let book = contact_repo
        .upsert_address_book(&acc.id, "http://book", "Contacts", None, None)
        .await
        .unwrap();
    let vcard =
        "BEGIN:VCARD
VERSION:3.0
FN:Alice Wonderland
EMAIL:alice@wonderland.com
END:VCARD";
    contact_repo
        .upsert_contact(
            book.id,
            "alice_uid",
            "etag1",
            "http://book/alice",
            vcard,
            None,
        )
        .await
        .unwrap();
    // Action: Search FTS
    let results = contact_repo
        .search_contacts("Wonderland", 10)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    // Verifies that the CardDAV FTS5 trigger correctly indexes the raw vCard text,
    // allowing full-text search to match against fields like FN or EMAIL
    // even though they aren't explicitly parsed into separate columns.
    assert!(results[0].vcard_data.contains("Alice Wonderland"));
}
