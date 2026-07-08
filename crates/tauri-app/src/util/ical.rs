/// Unfolds iCalendar/vCard lines according to RFC 5545 / RFC 6350.
///
/// Long lines are broken by inserting a CRLF (or LF) followed by a single whitespace character.
/// If we don't unfold them, property parsing will break on long UIDs or descriptions.
pub fn unfold_lines(text: &str) -> String {
    let mut unfolded = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' && chars.peek() == Some(&'\n') {
            chars.next(); // consume '\n'
            if matches!(chars.peek(), Some(&' ') | Some(&'\t')) {
                chars.next(); // consume folding whitespace
            } else {
                unfolded.push_str("\r\n");
            }
        } else if c == '\n' {
            if matches!(chars.peek(), Some(&' ') | Some(&'\t')) {
                chars.next(); // consume folding whitespace
            } else {
                unfolded.push('\n');
            }
        } else {
            unfolded.push(c);
        }
    }
    unfolded
}

/// Extracts a specific field from raw iCalendar text.
///
/// Handles both standard `FIELD:` and parameterized `FIELD;PARAM=value:` syntax.
/// We must split on the *first* colon, as parameters can appear before the value delimiter.
pub fn parse_ical_field(ical: &str, field: &str) -> Option<String> {
    let unfolded = unfold_lines(ical);
    let field_upper = field.to_uppercase();
    for line in unfolded.lines() {
        let upper_line = line.to_uppercase();
        if upper_line.starts_with(&field_upper)
            || upper_line.starts_with(&format!("{};", field_upper))
        {
            if let Some(idx) = line.find(':') {
                return Some(line[idx + 1..].trim().to_string());
            }
        }
    }
    None
}
