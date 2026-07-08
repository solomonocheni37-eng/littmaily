use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Account {
    pub id: String,
    pub email: String,
    pub provider: String,
    pub imap_host: String,
    pub imap_port: i32,
    pub smtp_host: String,
    pub smtp_port: i32,
    pub auth_method: String,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret: Option<String>,
    pub oauth_token_url: Option<String>,
    pub sync_window: String,
    // Cast to f64 for TypeScript interop. Tauri's default JSON serialization maps i64 to strings
    // or requires BigInt on the frontend; f64 keeps them as standard JS numbers (safe up to 2^53).
    #[specta(type = f64)]
    pub created_at: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Mailbox {
    #[specta(type = f64)]
    pub id: i64,
    pub account_id: String,
    pub name: String,
    pub delimiter: Option<String>,
    pub attributes: String,
    #[specta(type = Option<f64>)]
    pub uid_validity: Option<i64>,
    #[specta(type = Option<f64>)]
    pub uid_next: Option<i64>,
    #[specta(type = Option<f64>)]
    pub highest_modseq: Option<i64>,
    #[specta(type = f64)]
    pub unread_count: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Message {
    #[specta(type = f64)]
    pub id: i64,
    pub account_id: String,
    pub mailbox_name: String,
    pub uid: i32,
    pub subject: Option<String>,
    pub sender: Option<String>,
    pub date: Option<String>,
    #[specta(type = Option<f64>)]
    pub date_timestamp: Option<i64>,
    pub flags: String,
    pub size: i32,
    pub has_attachments: bool,
    pub snippet: Option<String>,
    pub blob_hash: Option<String>,
    pub attachment_names: Option<String>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references_json: Option<String>,
    pub thread_id: Option<String>,
    pub thread_subject: Option<String>,
    pub thread_count: Option<i32>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize, specta::Type)]
pub struct MessageSearchRow {
    #[specta(type = f64)]
    pub id: i64,
    pub account_id: String,
    pub mailbox_name: String,
    pub uid: i32,
    pub subject: Option<String>,
    pub sender: Option<String>,
    pub date: Option<String>,
    #[specta(type = Option<f64>)]
    pub date_timestamp: Option<i64>,
    pub flags: String,
    pub size: i32,
    pub has_attachments: bool,
    pub snippet: Option<String>,
    pub blob_hash: Option<String>,
    pub attachment_names: Option<String>,
    pub highlight: Option<String>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references_json: Option<String>,
    pub thread_id: Option<String>,
    pub thread_subject: Option<String>,
    pub thread_count: Option<i32>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct OutboxMessage {
    #[specta(type = f64)]
    pub id: i64,
    pub account_id: String,
    pub status: String,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub raw_mime: Vec<u8>,
    pub envelope_from: String,
    pub envelope_to: String,
    pub subject: Option<String>,
    pub body: Option<String>,
    #[specta(type = f64)]
    pub created_at: i64,
    #[specta(type = Option<f64>)]
    pub sent_at: Option<i64>,
    #[specta(type = Option<f64>)]
    pub scheduled_for: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct CalendarRecord {
    #[specta(type = f64)]
    pub id: i64,
    pub account_id: String,
    pub url: String,
    pub display_name: String,
    pub ctag: Option<String>,
    pub sync_token: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct CalendarEventRecord {
    #[specta(type = f64)]
    pub id: i64,
    #[specta(type = f64)]
    pub calendar_id: i64,
    pub uid: String,
    pub etag: String,
    pub url: String,
    pub ical_data: String,
    #[specta(type = Option<f64>)]
    pub last_modified: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct AddressBookRecord {
    #[specta(type = f64)]
    pub id: i64,
    pub account_id: String,
    pub url: String,
    pub display_name: String,
    pub ctag: Option<String>,
    pub sync_token: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ContactRecord {
    #[specta(type = f64)]
    pub id: i64,
    #[specta(type = f64)]
    pub address_book_id: i64,
    pub uid: String,
    pub etag: String,
    pub url: String,
    pub vcard_data: String,
    #[specta(type = Option<f64>)]
    pub last_modified: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct PendingAction {
    #[specta(type = f64)]
    pub id: i64,
    pub account_id: String,
    pub mailbox_name: String,
    pub uid: i32,
    pub action: String,
    pub dest_mailbox: Option<String>,
    #[specta(type = f64)]
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct UnifiedSearchItem {
    pub item_type: String,
    pub relevance: f64,
    #[specta(type = f64)]
    pub timestamp: i64,
    pub title: String,
    pub subtitle: String,
    pub data: UnifiedSearchData,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", content = "record")]
pub enum UnifiedSearchData {
    Email(Message),
    CalendarEvent(CalendarEventRecord),
    Contact(ContactRecord),
}
