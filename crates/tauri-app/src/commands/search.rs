use crate::error::AppError;
use crate::state::AppState;
use crate::util::ical::parse_ical_field;
use crate::util::vcard::parse_vcard_field;
use crate::util::build_smart_fts_query;
use storage::models::{Message, UnifiedSearchData, UnifiedSearchItem};
use storage::repository::{CalendarRepository, ContactRepository, MessageRepository};
use tauri::State;

/// Executes a unified full-text search across emails, calendar events, and contacts.
///
/// Uses `tokio::join!` to query all three FTS5 virtual tables concurrently.
/// Relevance is determined purely by the FTS5 `rank` order (index `i`), not a custom scoring algorithm.
#[tauri::command]
#[specta::specta]
pub async fn unified_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i32>,
) -> Result<Vec<UnifiedSearchItem>, AppError> {
    let pool = state
        .pool
        .get()
        .ok_or_else(|| AppError::System("Database is still initializing...".into()))?;
    let limit = limit.unwrap_or(100) as i64;

    // Sanitize the raw user input into safe FTS5 syntax to prevent SQL syntax errors
    // from unmatched quotes or special operators.
    let fts_query = build_smart_fts_query(&query);
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }

    let msg_repo = MessageRepository::new(pool);
    let cal_repo = CalendarRepository::new(pool);
    let contact_repo = ContactRepository::new(pool);

    let (emails_res, events_res, contacts_res) = tokio::join!(
        msg_repo.search_with_highlight(&fts_query, limit),
        cal_repo.search_events(&fts_query, limit),
        contact_repo.search_contacts(&fts_query, limit)
    );

    let mut results = Vec::new();
    if let Ok(emails) = emails_res {
        for (i, row) in emails.into_iter().enumerate() {
            let timestamp = row.date.as_deref()
                .and_then(|d| chrono::DateTime::parse_from_rfc2822(d).ok())
                .map(|dt| dt.timestamp())
                .unwrap_or(0);
            let msg = Message {
                id: row.id, account_id: row.account_id, mailbox_name: row.mailbox_name,
                uid: row.uid, subject: row.subject.clone(), sender: row.sender.clone(),
                date: row.date, date_timestamp: row.date_timestamp, flags: row.flags,
                size: row.size, has_attachments: row.has_attachments, snippet: row.snippet,
                blob_hash: row.blob_hash, attachment_names: row.attachment_names,
                message_id: row.message_id, in_reply_to: row.in_reply_to,
                references_json: row.references_json, thread_id: row.thread_id,
                thread_subject: row.thread_subject, thread_count: row.thread_count,
            };
            results.push(UnifiedSearchItem {
                item_type: "email".into(),
                relevance: i as f64,
                timestamp,
                title: msg.subject.clone().unwrap_or_else(|| "(No Subject)".into()),
                subtitle: row.highlight.unwrap_or_else(|| msg.sender.clone().unwrap_or_default()),
                data: UnifiedSearchData::Email(msg),
            });
        }
    }
    if let Ok(events) = events_res {
        for (i, evt) in events.into_iter().enumerate() {
            results.push(UnifiedSearchItem {
                item_type: "event".into(),
                relevance: i as f64,
                timestamp: evt.last_modified.unwrap_or(0),
                title: parse_ical_field(&evt.ical_data, "SUMMARY").unwrap_or_else(|| "Untitled Event".into()),
                subtitle: "Calendar Event".into(),
                data: UnifiedSearchData::CalendarEvent(evt),
            });
        }
    }
    if let Ok(contacts) = contacts_res {
        for (i, c) in contacts.into_iter().enumerate() {
            results.push(UnifiedSearchItem {
                item_type: "contact".into(),
                relevance: i as f64,
                timestamp: c.last_modified.unwrap_or(0),
                title: parse_vcard_field(&c.vcard_data, "FN").unwrap_or_else(|| "Unknown Contact".into()),
                subtitle: parse_vcard_field(&c.vcard_data, "EMAIL").unwrap_or_default(),
                data: UnifiedSearchData::Contact(c),
            });
        }
    }
    Ok(results)
}
