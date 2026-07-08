// FILE: ./crates/tauri-app/src/state.rs
use email_core::oauth_flow::PendingOAuth2Flow;
use email_core::sync_worker::SyncCommand;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use storage::blob::BlobStore;
use tokio::sync::mpsc;

pub struct WorkerHandle {
    pub cmd_tx: mpsc::Sender<SyncCommand>,
    pub task_handle: tokio::task::JoinHandle<()>,
}

pub struct AppState {
    // Wrapped in Arc so the async initialization task and the Tauri state share
    // the EXACT SAME OnceLock instance. OnceLock is required because SQLCipher
    // decryption cannot be done synchronously in the Tauri setup closure.
    pub pool: Arc<OnceLock<SqlitePool>>,
    pub blob_store: Arc<OnceLock<BlobStore>>,
    pub pending_flow: Mutex<Option<PendingOAuth2Flow>>,
    pub sync_workers: Arc<Mutex<HashMap<String, WorkerHandle>>>,
}

// ==========================================
// IPC Payloads & Response Structs
// ==========================================

#[derive(Serialize, Clone, Type)]
pub struct IpcAttachment {
    pub filename: Option<String>,
    pub mime_type: String,
    // Cast to i32 for TypeScript interop. JS numbers lose precision above 2^53,
    // but file sizes will never exceed this limit in practice.
    #[specta(type = i32)]
    pub size: usize,
    pub blob_hash: String,
}

#[derive(Serialize, Clone, Type)]
pub struct IpcParsedEmail {
    pub subject: Option<String>,
    pub from: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub attachments: Vec<IpcAttachment>,
}

#[derive(Serialize, Clone, Deserialize, Debug, Type)]
pub struct SyncNotification {
    pub account_id: String,
    pub mailbox: String,
    pub new_uids: Vec<u32>,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AddAccountPayload {
    pub email: String,
    pub provider: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub password: Option<String>,
    pub auth_method: String,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret: Option<String>,
    pub oauth_token_url: Option<String>,
    pub sync_window: Option<String>,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QueueEmailPayload {
    pub account_id: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub raw_mime_base64: String,
    #[specta(type = Option<f64>)]
    pub scheduled_for: Option<i64>,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveDraftPayload {
    pub account_id: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub raw_mime_base64: String,
    #[specta(type = Option<i32>)]
    pub draft_id: Option<i64>,
}
