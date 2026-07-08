// Verifies that the client correctly reconstructs UIDs that were folded across multiple lines
// by the server, which is common for long URN-based UUIDs in vCard 3.0/4.0.
#[tokio::test]
async fn given_folded_uid_when_syncing_then_extracts_full_uid() {
    let mock_server = MockServer::start().await;
    let report_xml = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:multistatus xmlns:D="DAV:">
<D:response>
<D:href>/addressbooks/user/contacts/contact_folded.vcf</D:href>
<D:propstat><D:prop><D:getetag>"etag_folded"</D:getetag></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
</D:response>
<D:sync-token>token_folded</D:sync-token>
</D:multistatus>"#;

    // Notice the CRLF + Tab folding the UID
    let vcard_payload = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:urn:uuid:12345678-1234-1234-12\r\n\t34-123456789012\r\nFN:Alice Wonderland\r\nEND:VCARD";

    Mock::given(method("REPORT"))
        .and(path("/addressbooks/user/contacts/"))
        .respond_with(ResponseTemplate::new(207).set_body_string(report_xml))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/addressbooks/user/contacts/contact_folded.vcf"))
        .respond_with(ResponseTemplate::new(200).set_body_string(vcard_payload))
        .mount(&mock_server)
        .await;

    let client = carddav::CardDavClient::new(&mock_server.uri(), "user", "pass").unwrap();
    let engine = carddav::SyncEngine::new(client);
    let book = carddav::AddressBook {
        url: "/addressbooks/user/contacts/".to_string(),
        display_name: "Contacts".to_string(),
        ctag: None,
        sync_token: None,
    };

    let (changed, _, _) = engine.sync_collection(&book, None).await.unwrap();
    assert_eq!(changed.len(), 1);

    // Assert that the tab was removed and the UUID was successfully stitched back together
    assert_eq!(
        changed[0].uid,
        "urn:uuid:12345678-1234-1234-1234-123456789012"
    );
}
