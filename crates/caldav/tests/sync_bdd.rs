#[tokio::test]
async fn given_folded_uid_when_syncing_then_extracts_full_uid() {
    let mock_server = MockServer::start().await;
    let report_xml = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:multistatus xmlns:D="DAV:">
    <D:response>
        <D:href>/calendars/user/personal/event_folded.ics</D:href>
        <D:propstat><D:prop><D:getetag>"etag_folded"</D:getetag></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
    </D:response>
    <D:sync-token>token_folded</D:sync-token>
</D:multistatus>"#;

    // Notice the CRLF + Space folding the UID across two lines
    let ical_payload = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:very-long-uid-that-is-fo\r\n lded-across-lines\r\nSUMMARY:Folded Event\r\nEND:VEVENT\r\nEND:VCALENDAR";

    Mock::given(method("REPORT"))
        .and(path("/calendars/user/personal/"))
        .respond_with(ResponseTemplate::new(207).set_body_string(report_xml))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/calendars/user/personal/event_folded.ics"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ical_payload))
        .mount(&mock_server)
        .await;

    let client = caldav::CalDavClient::new(&mock_server.uri(), "user", "pass").unwrap();
    let engine = caldav::SyncEngine::new(client);
    let calendar = caldav::Calendar {
        url: "/calendars/user/personal/".to_string(),
        display_name: "Personal".to_string(),
        ctag: None,
        sync_token: None,
    };

    let (changed, _, _) = engine.sync_collection(&calendar, None).await.unwrap();

    assert_eq!(changed.len(), 1);
    // Assert that the space was removed and the UID was successfully stitched back together
    assert_eq!(changed[0].uid, "very-long-uid-that-is-folded-across-lines");
}
