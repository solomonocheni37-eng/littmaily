pub mod ical;
pub mod mime;
pub mod vcard;

/// Sanitizes and formats user input for SQLite FTS5 queries.
///
/// FTS5 syntax is fragile; unquoted special characters (like `*`, `-`, or unmatched `"`)
/// will cause the SQL query to throw a syntax error. This function strictly filters allowed
/// characters and wraps each token in double quotes to guarantee a valid query.
pub fn build_smart_fts_query(input: &str) -> String {
    let sanitized: String = input
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || ".-@_".contains(*c))
        .collect();
    let tokens: Vec<&str> = sanitized.split_whitespace().collect();
    if tokens.is_empty() {
        return String::new();
    }
    tokens
        .iter()
        .map(|t| format!("\"{}\"", t))
        .collect::<Vec<String>>()
        .join(" OR ")
}
