use crate::error::AppError;
use crate::services::imap::get_imap_session;
use crate::state::{AppState, IpcAttachment, IpcParsedEmail, QueueEmailPayload, SaveDraftPayload};
use crate::util::mime::{replace_cid_with_data_uri, sanitize_html};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use storage::models::{Mailbox, Message, OutboxMessage};
use storage::repository::{
    AccountRepository, MailboxRepository, MessageRepository, OutboxRepository,
    PendingActionRepository,
};
use tauri::{Emitter, State};
use tauri_plugin_dialog::DialogExt;
use futures::StreamExt;

// Hard limit to prevent Out-Of-Memory panics in the Rust backend or IPC serialization failures
// when the frontend attempts to render/parse massively bloated emails (e.g., 100MB base64 attachments).
const EMAIL_BODY_MAX_BYTES: i32 = 50 * 1024 * 1024;

#[tauri::command]
#[specta::specta]
pub async fn get_mailboxes(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<Mailbox>, AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    Ok(MailboxRepository::new(pool).list_for_account(&account_id).await?)
}

/// Fetches a paginated list of threaded emails.
///
/// `before_id` is typed as `f64` in the IPC boundary because Tauri Specta maps `i64` to
/// strings or requires BigInt in TypeScript. Using `f64` keeps them as standard JS numbers,
/// which is perfectly safe since DB row IDs will never exceed JS's $2^{53}$ precision limit.
#[tauri::command]
#[specta::specta]
pub async fn get_emails_paginated(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_name: String,
    before_id: Option<f64>,
    page_size: i32,
) -> Result<Vec<Message>, AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let before_id_i64 = before_id.map(|t| t as i64);
    Ok(MessageRepository::new(pool)
        .list_threads_cursor(&account_id, &mailbox_name, before_id_i64, page_size as i64)
        .await?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_thread_messages(
    state: State<'_, AppState>,
    thread_id: String,
) -> Result<Vec<Message>, AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    Ok(MessageRepository::new(pool).get_thread_messages(&thread_id).await?)
}

/// Fetches the full raw MIME body from the IMAP server, parses it, and caches it locally.
#[tauri::command]
#[specta::specta]
pub async fn fetch_email_body(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_name: String,
    uid: u32,
) -> Result<IpcParsedEmail, AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let blob_store = state.blob_store.get().ok_or_else(|| AppError::System("Blob store not ready".into()))?;
    let msg_repo = MessageRepository::new(pool);

    if let Some(msg) = msg_repo.get_by_uid(&account_id, &mailbox_name, uid as i32).await? {
        if msg.size > EMAIL_BODY_MAX_BYTES {
            return Err(AppError::BadRequest("This email is larger than 50MB.".into()));
        }
    }

    let acc_repo = AccountRepository::new(pool);
    let account = acc_repo.get_by_id(&account_id).await?.ok_or_else(|| AppError::NotFound("Account".into()))?;
    let mut session = get_imap_session(&account).await?;

    let raw_mime = email_core::fetch_full_message(&mut session, &mailbox_name, uid)
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;
    let parsed = email_core::mime_parser::parse_mime(&raw_mime)?;

    // Cache the raw MIME in the content-addressed blob store for instant offline retrieval later
    let mime_hash = blob_store.save(&raw_mime).await?;
    let _ = msg_repo.update_blob_hash(&account_id, &mailbox_name, uid as i32, &mime_hash).await;

    let mut ipc_attachments = Vec::new();
    for att in &parsed.attachments {
        let att_hash = blob_store.save(&att.content).await?;
        ipc_attachments.push(IpcAttachment {
            filename: att.filename.clone(),
            mime_type: att.mime_type.clone(),
            size: att.size,
            blob_hash: att_hash,
        });
    }

    // Sanitize HTML at the IPC boundary. The core parser intentionally returns raw HTML
    // to remain pure and fast; XSS prevention and privacy rewriting happen here.
    let safe_html = parsed.html_body.map(|h| {
        let sanitized = sanitize_html(&h);
        replace_cid_with_data_uri(&sanitized, &parsed.attachments)
    });

    Ok(IpcParsedEmail {
        subject: parsed.subject,
        from: parsed.from,
        text_body: parsed.text_body,
        html_body: safe_html,
        attachments: ipc_attachments,
    })
}

/// Attempts to load the email body from the local encrypted blob cache before hitting the network.
#[tauri::command]
#[specta::specta]
pub async fn get_cached_email_body(
    state: State<'_, AppState>,
    account_id: String,
    mailbox_name: String,
    uid: u32,
) -> Result<Option<IpcParsedEmail>, AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let blob_store = state.blob_store.get().ok_or_else(|| AppError::System("Blob store not ready".into()))?;
    let msg_repo = MessageRepository::new(pool);

    if let Some(m) = msg_repo.get_by_uid(&account_id, &mailbox_name, uid as i32).await? {
        if m.size > EMAIL_BODY_MAX_BYTES {
            return Err(AppError::BadRequest("This email is larger than 50MB.".into()));
        }
        if let Some(hash) = m.blob_hash {
            let raw_mime = blob_store.load(&hash).await?;
            let parsed = email_core::mime_parser::parse_mime(&raw_mime)?;
            let mut ipc_attachments = Vec::new();
            for att in &parsed.attachments {
                let att_hash = blob_store.save(&att.content).await?;
                ipc_attachments.push(IpcAttachment {
                    filename: att.filename.clone(),
                    mime_type: att.mime_type.clone(),
                    size: att.size,
                    blob_hash: att_hash,
                });
            }
            let safe_html = parsed.html_body.map(|h| {
                let sanitized = sanitize_html(&h);
                replace_cid_with_data_uri(&sanitized, &parsed.attachments)
            });
            return Ok(Some(IpcParsedEmail {
                subject: parsed.subject,
                from: parsed.from,
                text_body: parsed.text_body,
                html_body: safe_html,
                attachments: ipc_attachments,
            }));
        }
    }
    Ok(None)
}

/// Applies an action to an email using an "Optimistic UI + Offline Queue" pattern.
///
/// The local database is updated immediately for UI responsiveness. The IMAP command is then
/// attempted; if it fails (e.g., due to being offline), the action is queued in `pending_actions`
/// to be retried by the background sync worker when connectivity is restored.
#[tauri::command]
#[specta::specta]
pub async fn update_email_state(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    account_id: String,
    mailbox_name: String,
    uid: u32,
    action: String,
    dest_mailbox: Option<String>,
) -> Result<(), AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let msg_repo = MessageRepository::new(pool);
    let pending_repo = PendingActionRepository::new(pool);

    // 1. Optimistic Local Update
    match action.as_str() {
        "read" | "unread" => {
            let is_read = action == "read";
            if let Some(m) = msg_repo.get_by_uid(&account_id, &mailbox_name, uid as i32).await? {
                let mut flags: Vec<String> = serde_json::from_str(&m.flags).unwrap_or_default();
                if is_read { if !flags.contains(&"Seen".to_string()) { flags.push("Seen".to_string()); } }
                else { flags.retain(|f| f != "Seen"); }
                msg_repo.update_flags(&account_id, &mailbox_name, uid as i32, &flags).await?;
            }
        }
        "star" | "unstar" => {
            let is_starred = action == "star";
            if let Some(m) = msg_repo.get_by_uid(&account_id, &mailbox_name, uid as i32).await? {
                let mut flags: Vec<String> = serde_json::from_str(&m.flags).unwrap_or_default();
                if is_starred { if !flags.contains(&"Flagged".to_string()) { flags.push("Flagged".to_string()); } }
                else { flags.retain(|f| f != "Flagged"); }
                msg_repo.update_flags(&account_id, &mailbox_name, uid as i32, &flags).await?;
            }
        }
        "delete" => { msg_repo.delete_by_uid(&account_id, &mailbox_name, uid as i32).await?; }
        "move" => { if let Some(dest) = &dest_mailbox { msg_repo.move_to_mailbox(&account_id, &mailbox_name, dest, uid as i32).await?; } }
        "archive" => { msg_repo.move_to_mailbox(&account_id, &mailbox_name, "Archive", uid as i32).await?; }
        _ => return Err(AppError::BadRequest(format!("Unknown action: {}", action))),
    }

    // 2. IMAP Sync & Offline Queueing
    let acc_repo = AccountRepository::new(pool);
    let account = acc_repo.get_by_id(&account_id).await?.ok_or_else(|| AppError::NotFound("Account".into()))?;
    match get_imap_session(&account).await {
        Ok(mut session) => {
            let imap_result = match action.as_str() {
                "read" | "unread" => email_core::set_message_flag(&mut session, &mailbox_name, uid, "\\Seen", action == "read").await,
                "star" | "unstar" => email_core::set_message_flag(&mut session, &mailbox_name, uid, "\\Flagged", action == "star").await,
                "delete" => email_core::delete_message(&mut session, &mailbox_name, uid).await,
                "move" => { if let Some(dest) = &dest_mailbox { email_core::move_message(&mut session, &mailbox_name, dest, uid).await } else { Ok(()) } }
                "archive" => email_core::move_message(&mut session, &mailbox_name, "Archive", uid).await,
                _ => Ok(()),
            };
            if let Err(e) = imap_result {
                tracing::warn!(error = %e, "IMAP sync failed, queueing action for later");
                pending_repo.queue_action(&account_id, &mailbox_name, uid as i32, &action, dest_mailbox.as_deref()).await?;
            }
        }
        Err(_) => {
            pending_repo.queue_action(&account_id, &mailbox_name, uid as i32, &action, dest_mailbox.as_deref()).await?;
            let _ = app_handle.emit("sync:offline-action-queued", ());
        }
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn queue_email(state: State<'_, AppState>, payload: QueueEmailPayload) -> Result<f64, AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let raw_mime = STANDARD.decode(&payload.raw_mime_base64).map_err(|e| AppError::BadRequest(format!("Invalid base64 MIME: {}", e)))?;
    let repo = OutboxRepository::new(pool);
    let account_repo = AccountRepository::new(pool);
    let account = account_repo.get_by_id(&payload.account_id).await?.ok_or_else(|| AppError::NotFound("Account not found".into()))?;

    let mut all_recipients = payload.to.clone();
    all_recipients.extend(payload.cc);
    all_recipients.extend(payload.bcc);

    let msg = repo.enqueue(&account.id, &raw_mime, &account.email, &all_recipients, Some(&payload.subject), None, payload.scheduled_for).await?;
    Ok(msg.id as f64)
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_scheduled_email(state: State<'_, AppState>, id: f64) -> Result<(), AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    Ok(OutboxRepository::new(pool).cancel_scheduled(id as i64).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_draft(state: State<'_, AppState>, payload: SaveDraftPayload) -> Result<f64, AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let raw_mime = STANDARD.decode(&payload.raw_mime_base64).unwrap_or_default();
    let repo = OutboxRepository::new(pool);
    let account_repo = AccountRepository::new(pool);
    let account = account_repo.get_by_id(&payload.account_id).await?.ok_or_else(|| AppError::NotFound("Account not found".into()))?;

    let mut all_recipients = payload.to.clone();
    all_recipients.extend(payload.cc);
    all_recipients.extend(payload.bcc);

    let msg = repo.save_draft(&account.id, &raw_mime, &account.email, &all_recipients, Some(&payload.subject), Some(&payload.body), payload.draft_id).await?;
    Ok(msg.id as f64)
}

#[tauri::command]
#[specta::specta]
pub async fn get_drafts(state: State<'_, AppState>, account_id: String) -> Result<Vec<OutboxMessage>, AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    Ok(OutboxRepository::new(pool).get_drafts(&account_id).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_draft(state: State<'_, AppState>, draft_id: f64) -> Result<(), AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    Ok(OutboxRepository::new(pool).delete_draft(draft_id as i64).await?)
}

/// Decrypts an attachment and writes it to a temporary cache directory for external apps.
#[tauri::command]
#[specta::specta]
pub async fn get_attachment_path(state: State<'_, AppState>, blob_hash: String) -> Result<String, AppError> {
    let blob_store = state.blob_store.get().ok_or_else(|| AppError::System("Blob store not ready".into()))?;
    let cache_dir = blob_store.base_dir().parent().unwrap().join("cache");
    let cache_path = cache_dir.join(&blob_hash);

    // Only decrypt and write to disk if the cached plaintext version doesn't already exist
    if !cache_path.exists() {
        let decrypted_bytes = blob_store.load(&blob_hash).await.map_err(|_| AppError::NotFound("Attachment not found".into()))?;
        tokio::fs::create_dir_all(&cache_dir).await.map_err(|e| AppError::System(e.to_string()))?;
        tokio::fs::write(&cache_path, &decrypted_bytes).await.map_err(|e| AppError::System(e.to_string()))?;
    }
    Ok(cache_path.to_string_lossy().into_owned())
}

/// Prompts the user with a native OS save dialog and writes the decrypted attachment to their chosen path.
#[tauri::command]
#[specta::specta]
pub async fn save_attachment_dialog(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    blob_hash: String,
    filename: String,
) -> Result<bool, AppError> {
    let blob_store = state.blob_store.get().ok_or_else(|| AppError::System("Blob store not ready".into()))?;
    let decrypted_bytes = blob_store.load(&blob_hash).await.map_err(|_| AppError::NotFound("Attachment not found".into()))?;

    let file_path_opt = app_handle.dialog().file().set_file_name(&filename).blocking_save_file();
    if let Some(file_path) = file_path_opt {
        let path_buf = match file_path {
            tauri_plugin_dialog::FilePath::Path(p) => p,
            tauri_plugin_dialog::FilePath::Url(u) => u.to_file_path().map_err(|_| AppError::System("Invalid file URL".into()))?,
        };
        tokio::fs::write(path_buf, &decrypted_bytes).await.map_err(|e| AppError::System(format!("Failed to write file: {}", e)))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_attachment_base64(state: State<'_, AppState>, blob_hash: String) -> Result<String, AppError> {
    let blob_store = state.blob_store.get().ok_or_else(|| AppError::System("Blob store not ready".into()))?;
    let decrypted_bytes = blob_store.load(&blob_hash).await.map_err(|_| AppError::NotFound("Attachment not found".into()))?;
    Ok(STANDARD.encode(&decrypted_bytes))
}

/// Manually triggers an IMAP sync for the current mailbox.
///
/// If an FTS5 corruption error is detected during upsert, it intentionally skips auto-healing
/// to prevent an infinite retry loop that would lock up the UI thread.
#[tauri::command]
#[specta::specta]
pub async fn check_for_new_emails(state: State<'_, AppState>, account_id: String, mailbox_name: String) -> Result<u32, AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let acc_repo = AccountRepository::new(pool);
    let msg_repo = MessageRepository::new(pool);
    let mb_repo = MailboxRepository::new(pool);
    let account = acc_repo.get_by_id(&account_id).await?.ok_or_else(|| AppError::NotFound("Account".into()))?;
    let _ = mb_repo.upsert(&account_id, &mailbox_name, Some("/"), &[]).await;

    let latest_msgs = msg_repo.list_threads_cursor(&account_id, &mailbox_name, None, 1).await?;
    let start_uid = if let Some(m) = latest_msgs.first() { m.uid as u32 + 1 } else { 1 };

    let mut session = get_imap_session(&account).await?;
    let headers = email_core::fetch_headers(&mut session, &mailbox_name, start_uid, 0).await.map_err(|e| AppError::Network(e.to_string()))?;
    let count = headers.len() as u32;

    for h in headers {
        let flags: Vec<String> = h.flags.iter().map(|f| format!("{:?}", f)).collect();
        let timestamp = h.date.as_deref().and_then(|d| chrono::DateTime::parse_from_rfc2822(d).ok()).map(|dt| dt.timestamp());
        let references_json = serde_json::to_string(&h.references).unwrap_or_else(|_| "[]".to_string());

        let res = msg_repo.upsert(
            &account_id, &mailbox_name, h.uid as i32, Some(&h.subject), Some(&h.from), h.date.as_deref(), timestamp, &flags, h.size as i32, h.attachment_names.is_some(),
            h.snippet.as_deref(), None, h.attachment_names.as_deref(), h.message_id.as_deref(), h.in_reply_to.as_deref(), Some(&references_json), Some(&h.thread_id), Some(&h.thread_subject),
        ).await;

        if let Err(e) = res {
            let err_str = e.to_string().to_lowercase();
            // FTS5 syntax errors or disk image corruption can cause cascading failures.
            // We log and skip rather than auto-healing, which could trigger infinite loops.
            if err_str.contains("fts") || err_str.contains("trigger") || err_str.contains("malformed") || err_str.contains("disk image") {
                tracing::error!("FTS corruption detected. Skipping auto-heal to prevent infinite loop. Error: {}", e);
            } else {
                tracing::error!("Failed to upsert message: {}", e);
            }
        }
    }
    Ok(count)
}

#[tauri::command]
#[specta::specta]
pub async fn create_folder(state: State<'_, AppState>, account_id: String, name: String) -> Result<(), AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let account = AccountRepository::new(pool).get_by_id(&account_id).await?.ok_or_else(|| AppError::NotFound("Account".into()))?;
    let mut session = get_imap_session(&account).await?;
    email_core::create_mailbox(&mut session, &name).await.map_err(|e| AppError::Network(e.to_string()))?;
    MailboxRepository::new(pool).upsert(&account_id, &name, Some("/"), &[]).await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_folder(state: State<'_, AppState>, account_id: String, name: String) -> Result<(), AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let account = AccountRepository::new(pool).get_by_id(&account_id).await?.ok_or_else(|| AppError::NotFound("Account".into()))?;
    let mut session = get_imap_session(&account).await?;
    email_core::delete_mailbox(&mut session, &name).await.map_err(|e| AppError::Network(e.to_string()))?;
    MailboxRepository::new(pool).delete_by_name(&account_id, &name).await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn rename_folder(state: State<'_, AppState>, account_id: String, old_name: String, new_name: String) -> Result<(), AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let account = AccountRepository::new(pool).get_by_id(&account_id).await?.ok_or_else(|| AppError::NotFound("Account".into()))?;
    let mut session = get_imap_session(&account).await?;
    email_core::rename_mailbox(&mut session, &old_name, &new_name).await.map_err(|e| AppError::Network(e.to_string()))?;
    MailboxRepository::new(pool).rename(&account_id, &old_name, &new_name).await?;
    Ok(())
}

/// Fetches partial body payloads for a specific set of UIDs currently visible in the UI viewport.
///
/// Uses IMAP partial fetch (`<0.2000`) to grab only the first 2KB of the body,
/// drastically reducing bandwidth compared to downloading full headers for snippet generation.
#[tauri::command]
#[specta::specta]
pub async fn fetch_viewport_snippets(state: State<'_, AppState>, account_id: String, mailbox_name: String, uids: Vec<u32>) -> Result<std::collections::HashMap<u32, String>, AppError> {
    if uids.is_empty() { return Ok(std::collections::HashMap::new()); }
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let msg_repo = MessageRepository::new(pool);
    let acc_repo = AccountRepository::new(pool);
    let account = acc_repo.get_by_id(&account_id).await?.ok_or_else(|| AppError::NotFound("Account".into()))?;
    let mut session = get_imap_session(&account).await?;
    session.select(&mailbox_name).await.map_err(|e| AppError::Network(e.to_string()))?;

    let uid_str = uids.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(",");
    let query = "(UID BODY.PEEK[]<0.2000>)";
    let mut stream = session.uid_fetch(&uid_str, query).await.map_err(|e| AppError::Network(e.to_string()))?;

    let mut map = std::collections::HashMap::new();
    while let Some(fetch_result) = stream.next().await {
        if let Ok(fetch) = fetch_result {
            if let Some(uid) = fetch.uid {
                if let Some(snippet) = email_core::extract_snippet(&fetch) {
                    let _ = msg_repo.update_snippet(&account_id, &mailbox_name, uid as i32, &snippet).await;
                    map.insert(uid, snippet);
                }
            }
        }
    }
    Ok(map)
}

/// Backfills older emails by fetching a specific UID range from the IMAP server.
#[tauri::command]
#[specta::specta]
pub async fn backfill_older_emails(state: State<'_, AppState>, account_id: String, mailbox_name: String, before_uid: u32, limit: u32) -> Result<Vec<Message>, AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let acc_repo = AccountRepository::new(pool);
    let msg_repo = MessageRepository::new(pool);
    let mb_repo = MailboxRepository::new(pool);
    let account = acc_repo.get_by_id(&account_id).await?.ok_or_else(|| AppError::NotFound("Account".into()))?;
    let _ = mb_repo.upsert(&account_id, &mailbox_name, Some("/"), &[]).await;

    let mut session = get_imap_session(&account).await?;
    let start_uid = if before_uid > limit { before_uid.saturating_sub(limit) } else { 1 };
    let end_uid = before_uid.saturating_sub(1);
    if start_uid > end_uid { return Ok(vec![]); }

    let headers = email_core::fetch_headers(&mut session, &mailbox_name, start_uid, end_uid).await.map_err(|e| AppError::Network(e.to_string()))?;

    let mut tx = Some(pool.begin().await?);
    for h in headers {
        let flags: Vec<String> = h.flags.iter().map(|f| format!("{:?}", f)).collect();
        let flags_json = serde_json::to_string(&flags).unwrap_or_else(|_| "[]".to_string());
        let timestamp = h.date.as_deref().and_then(|d| chrono::DateTime::parse_from_rfc2822(d).ok()).map(|dt| dt.timestamp());
        let references_json = serde_json::to_string(&h.references).unwrap_or_else(|_| "[]".to_string());

        let res = sqlx::query("INSERT OR IGNORE INTO messages (account_id, mailbox_name, uid, subject, sender, date, date_timestamp, flags, size, has_attachments, snippet, blob_hash, attachment_names, message_id, in_reply_to, references_json, thread_id, thread_subject) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&account_id).bind(&mailbox_name).bind(h.uid as i32).bind(&h.subject).bind(&h.from).bind(h.date.as_deref()).bind(timestamp).bind(&flags_json).bind(h.size as i32).bind(h.attachment_names.is_some()).bind(h.snippet.as_deref()).bind::<Option<String>>(None).bind(h.attachment_names.as_deref())
            .bind(h.message_id.as_deref()).bind(h.in_reply_to.as_deref()).bind(&references_json).bind(&h.thread_id).bind(&h.thread_subject)
            .execute(&mut *tx.as_deref_mut().unwrap()).await;

        if let Err(e) = res {
            let err_str = e.to_string().to_lowercase();
            if err_str.contains("fts") || err_str.contains("trigger") || err_str.contains("malformed") || err_str.contains("disk image") {
                tracing::error!("FTS corruption detected. Skipping auto-heal to prevent infinite loop. Error: {}", e);
            } else {
                tracing::error!("Failed to upsert message: {}", e);
            }
        }
    }
    if let Some(current_tx) = tx.take() { let _ = current_tx.commit().await; }

    msg_repo.list_threads_cursor(&account_id, &mailbox_name, None, limit as i64).await.map_err(Into::into)
}
