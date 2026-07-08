use email_core::mime_parser::parse_mime;

#[test]
fn given_email_with_attachment_when_parsed_then_extracts_attachment_metadata_and_content() {
    let raw_mime = b"From: sender@example.com\r\n\
Subject: Attachment Test\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"boundary123\"\r\n\
\r\n\
--boundary123\r\n\
Content-Type: text/plain\r\n\
\r\n\
Here is the file.\r\n\
--boundary123\r\n\
Content-Type: text/plain; name=\"test.txt\"\r\n\
Content-Disposition: attachment; filename=\"test.txt\"\r\n\
\r\n\
File content here\r\n\
--boundary123--\r\n";

    let parsed = parse_mime(raw_mime).unwrap();
    assert_eq!(parsed.subject, Some("Attachment Test".to_string()));
    assert_eq!(parsed.attachments.len(), 1);

    let attachment = &parsed.attachments[0];
    assert_eq!(attachment.filename, Some("test.txt".to_string()));
    assert_eq!(attachment.mime_type, "text/plain");

    // mail-parser automatically strips trailing CRLF from text-based attachments,
    // which is why we assert exactly 17 bytes ("File content here") instead of 19.
    assert_eq!(attachment.content, b"File content here");
    assert_eq!(attachment.size, 17);
}

#[test]
fn given_html_email_when_parsed_then_extracts_raw_html_body() {
    // The parser must faithfully extract the raw payload *without* sanitizing it.
    // Sanitization (XSS prevention) is deliberately deferred to the Tauri IPC boundary
    // (using `ammonia`) to keep the core parsing layer pure, fast, and reusable.
    let raw_mime = b"Subject: HTML Test\r\n\
Content-Type: text/html\r\n\
\r\n\
<html><body><script>document.location='http://evil.com'</script><img src=x onerror=alert(1)><p>Safe content</p></body></html>";

    let parsed = parse_mime(raw_mime).unwrap();
    let html = parsed.html_body.unwrap();

    // The parser must faithfully extract the raw payload
    assert!(html.contains("<script>"));
    assert!(html.contains("onerror"));
    assert!(html.contains("<p>Safe content</p>"));
}
