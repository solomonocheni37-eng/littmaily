use email_core::mime_parser::parse_mime;

#[test]
fn given_mime_with_no_subject_when_parsed_then_subject_is_none() {
    let raw_mime = b"From: sender@example.com\r\n\
To: receiver@example.com\r\n\
\r\n\
Hello world";

    let parsed = parse_mime(raw_mime).unwrap();
    assert!(parsed.subject.is_none());
    assert_eq!(parsed.from, "sender@example.com");
    assert_eq!(parsed.text_body, Some("Hello world".to_string()));
}

#[test]
fn given_empty_bytes_when_parsed_then_returns_parse_error() {
    let raw_mime = b"";
    let result = parse_mime(raw_mime);
    // Ensures the parser fails gracefully rather than panicking or returning a dummy message
    // when given completely invalid or empty input.
    assert!(result.is_err(), "Empty bytes should result in a ParseError");
}

#[test]
fn given_multipart_mime_with_inline_image_when_parsed_then_extracts_correctly() {
    // `multipart/related` is the standard structure for HTML emails with embedded inline images.
    // We verify that the parser doesn't panic when encountering `Content-ID` and that the
    // HTML body correctly retains the `cid:` reference for later resolution.
    let raw_mime = b"From: a@b.com\r\n\
Content-Type: multipart/related; boundary=\"boundary123\"\r\n\
\r\n\
--boundary123\r\n\
Content-Type: text/html\r\n\
\r\n\
<html><body><img src=\"cid:image1\"></body></html>\r\n\
--boundary123\r\n\
Content-Type: image/png\r\n\
Content-ID: <image1>\r\n\
Content-Disposition: inline\r\n\
\r\n\
FAKEPNGDATA\r\n\
--boundary123--\r\n";

    let parsed = parse_mime(raw_mime).unwrap();
    assert!(parsed.html_body.is_some());

    // Inline images are often treated as attachments by mail-parser unless specifically filtered.
    // We just verify it doesn't panic and parses the HTML body successfully.
    assert!(
        parsed
            .html_body
            .unwrap()
            .contains("<img src=\"cid:image1\">")
    );
}
