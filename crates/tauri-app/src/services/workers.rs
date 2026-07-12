// FILE: ./crates/tauri-app/src/services/workers.rs
use crate::services::{auth, imap};
use crate::state::{SyncNotification, WorkerHandle};
use email_core::oauth::{Credentials, FileStore};
use email_core::sync_worker::{SyncCommand, SyncEvent, SyncWorker};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use storage::blob::BlobStore;
use storage::repository::{
    AccountRepository, CalendarRepository, ContactRepository, MailboxRepository,
    MessageRepository, PendingActionRepository,
};
use tauri::{Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{sleep, Duration};

// ==========================================
// Outbox Worker Integration
// ==========================================

pub struct TauriMessageSender {
    pub app_handle: tauri::AppHandle,
}

#[async_trait::async_trait]
impl crate::outbox_worker::MessageSender for TauriMessageSender {
    async fn send(
        &self,
        account: &storage::models::Account,
        msg: &storage::models::OutboxMessage,
    ) -> Result<(), String> {
        let to_addresses: Vec<String> = serde_json::from_str(&msg.envelope_to).unwrap_or_default();
        if account.auth_method == "oauth2" {
            let access_token = imap::get_smtp_password(account)
                .await
                .map_err(|e| e.to_string())?;
            email_core::smtp::send_raw_mime_xoauth2(
                &account.smtp_host,
                account.smtp_port as u16,
                &account.email,
                &access_token,
                &msg.envelope_from,
                &to_addresses,
                &msg.raw_mime,
            )
            .await
        } else {
            let password = imap::get_smtp_password(account)
                .await
                .map_err(|e| e.to_string())?;
            // Infer encryption type from port since we don't store it explicitly in the DB.
            // 587 is universally STARTTLS, everything else is assumed to be Implicit TLS (465).
            let encryption = if account.smtp_port == 587 {
                email_core::discovery::Encryption::StartTls
            } else {
                email_core::discovery::Encryption::Tls
            };
            let config = email_core::smtp::SmtpConfig {
                host: account.smtp_host.clone(),
                port: account.smtp_port as u16,
                username: account.email.clone(),
                password,
                encryption,
            };
            email_core::smtp::send_raw_mime(
                &config,
                &msg.envelope_from,
                &to_addresses,
                &msg.raw_mime,
            )
            .await
        }
    }

    async fn on_success(
        &self,
        account: &storage::models::Account,
        msg: &storage::models::OutboxMessage,
    ) -> Result<(), String> {
        // Append the sent message to the IMAP "Sent" folder so it appears in the UI
        // and syncs across other devices.
        if let Ok(mut session) = imap::get_imap_session(account).await {
            let _ =
                email_core::append_message(&mut session, "Sent", &msg.raw_mime, &["\\Seen"]).await;
        }
        let _ = self.app_handle.emit("outbox:sent", msg.id);
        Ok(())
    }
}

// ==========================================
// Notification Helper
// ==========================================

async fn send_new_email_notification(
    app_handle: &tauri::AppHandle,
    count: usize,
    sender_name: Option<&str>,
) {
    let title = if count == 1 {
        "New Email".to_string()
    } else {
        format!("{} New Emails", count)
    };
    let body = if count == 1 {
        if let Some(sender) = sender_name {
            format!("From: {}", sender)
        } else {
            "You have a new message".to_string()
        }
    } else {
        format!("You have {} new messages", count)
    };
    let _ = app_handle
        .notification()
        .builder()
        .title(&title)
        .body(&body)
        .show();

    update_app_badge(app_handle).await;
}

async fn update_app_badge(app_handle: &tauri::AppHandle) {
    // Tauri 2 lacks a unified cross-platform API for taskbar badges.
    // We reset the title here to clear any previous counts when new emails arrive.
    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_title("Littmaily");
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_title("Littmaily");
        }
    }
}

// ==========================================
// Orchestrator
// ==========================================

pub fn spawn_all_workers(
    app_handle: tauri::AppHandle,
    pool: SqlitePool,
    blob_store: BlobStore,
    sync_workers: Arc<Mutex<HashMap<String, WorkerHandle>>>,
) {
    // Staggered startup delays prevent a thundering herd of network requests and DB queries
    // immediately after app launch, which could spike CPU/bandwidth usage and cause UI jank.
    let pool_sync = pool.clone();
    let h2 = app_handle.clone();
    let sync_mgr_workers = sync_workers.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        run_sync_manager(pool_sync, h2, sync_mgr_workers).await;
    });

    let h1 = app_handle.clone();
    let pool_outbox = pool.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let sender = TauriMessageSender { app_handle: h1 };
        let worker = crate::outbox_worker::OutboxWorker::new(pool_outbox, sender);
        worker.run().await;
    });

    let pool_action = pool.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        run_action_sync_worker(pool_action).await;
    });

    let pool_caldav = pool.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(15)).await;
        run_caldav_sync_worker(pool_caldav).await;
    });

    let pool_carddav = pool.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        run_carddav_sync_worker(pool_carddav).await;
    });

    let pool_migrations = pool.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            tokio::time::sleep(Duration::from_secs(30)).await;
            if let Err(e) = storage::db::run_background_migrations(&pool_migrations).await {
                tracing::error!(error = %e, "Background migration failed");
            }
        });
    });

    let pool_gc = pool.clone();
    let blob_gc = blob_store.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        run_blob_gc_worker(pool_gc, blob_gc).await;
    });

    let pool_maint = pool.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(120)).await;
        run_db_maintenance_worker(pool_maint).await;
    });

    let pool_pruner = pool.clone();
    let blob_pruner = blob_store.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        run_sync_window_pruner(pool_pruner, blob_pruner).await;
    });
}

// ==========================================
// Background Loops
// ==========================================

/// Retries offline actions (read/star/delete/move) that were queued when the user
/// had no network connectivity.
pub async fn run_action_sync_worker(pool: SqlitePool) {
    loop {
        let acc_repo = AccountRepository::new(&pool);
        let pending_repo = PendingActionRepository::new(&pool);
        if let Ok(accounts) = acc_repo.list_all().await {
            for account in accounts {
                if let Ok(actions) = pending_repo.get_pending_actions(&account.id).await {
                    if let Ok(mut session) = imap::get_imap_session(&account).await {
                        for action in actions {
                            let res = match action.action.as_str() {
                                "read" | "unread" => {
                                    email_core::set_message_flag(
                                        &mut session,
                                        &action.mailbox_name,
                                        action.uid as u32,
                                        "\\Seen",
                                        action.action == "read",
                                    )
                                    .await
                                }
                                "star" | "unstar" => {
                                    email_core::set_message_flag(
                                        &mut session,
                                        &action.mailbox_name,
                                        action.uid as u32,
                                        "\\Flagged",
                                        action.action == "star",
                                    )
                                    .await
                                }
                                "delete" => {
                                    email_core::delete_message(
                                        &mut session,
                                        &action.mailbox_name,
                                        action.uid as u32,
                                    )
                                    .await
                                }
                                "move" => {
                                    if let Some(dest) = &action.dest_mailbox {
                                        email_core::move_message(
                                            &mut session,
                                            &action.mailbox_name,
                                            dest,
                                            action.uid as u32,
                                        )
                                        .await
                                    } else {
                                        Ok(())
                                    }
                                }
                                "archive" => {
                                    let dest = action.dest_mailbox.as_deref().unwrap_or("Archive");
                                    email_core::move_message(
                                        &mut session,
                                        &action.mailbox_name,
                                        dest,
                                        action.uid as u32,
                                    )
                                    .await
                                }
                                _ => Ok(()),
                            };
                            if res.is_ok() {
                                let _ = pending_repo.delete_action(action.id).await;
                            } else {
                                // If one action fails (e.g. network drop), break and retry the rest later.
                                break;
                            }
                        }
                    }
                }
            }
        }
        sleep(Duration::from_secs(60)).await;
    }
}

pub async fn run_sync_manager(
    pool: SqlitePool,
    app_handle: tauri::AppHandle,
    workers: Arc<Mutex<HashMap<String, WorkerHandle>>>,
) {
    loop {
        let acc_repo = AccountRepository::new(&pool);
        if let Ok(accounts) = acc_repo.list_all().await {
            let db_ids: std::collections::HashSet<String> =
                accounts.iter().map(|a| a.id.clone()).collect();
            {
                let mut workers_lock = match workers.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => {
                        tracing::error!(
                            "Sync workers mutex was poisoned during cleanup! Recovering guard..."
                        );
                        poisoned.into_inner()
                    }
                };
                // Terminate workers for accounts that have been deleted from the DB.
                workers_lock.retain(|id, handle| {
                    if !db_ids.contains(id) {
                        let _ = handle.cmd_tx.try_send(SyncCommand::Shutdown);
                        handle.task_handle.abort();
                        false
                    } else {
                        true
                    }
                });
            }

            for account in accounts {
                let has_worker = match workers.lock() {
                    Ok(guard) => guard.contains_key(&account.id),
                    Err(poisoned) => {
                        tracing::error!("Sync workers mutex poisoned during check! Recovering...");
                        poisoned.into_inner().contains_key(&account.id)
                    }
                };

                if !has_worker {
                    let creds: Credentials<FileStore> = if account.auth_method == "oauth2" {
                        let client_id = account.oauth_client_id.clone().unwrap_or_default();
                        let client_secret = account.oauth_client_secret.clone().unwrap_or_default();
                        let token_url = account.oauth_token_url.clone().unwrap_or_default();
                        match Credentials::oauth2(
                            account.email.clone(),
                            account.id.clone(),
                            client_id,
                            client_secret,
                            token_url,
                        ) {
                            Ok(c) => c,
                            Err(e) => {
                                tracing::error!("OAuth2 init error for {}: {}", account.email, e);
                                continue;
                            }
                        }
                    } else {
                        let password = match auth::get_password(&account.id, "imap") {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        Credentials::Password {
                            full_name: "User".into(),
                            email: account.email.clone(),
                            password: zeroize::Zeroizing::new(password),
                        }
                    };

                    // Discover the true INBOX name, as some providers use localized names
                    // or custom attributes instead of the literal string "INBOX".
                    let inbox_name = match imap::get_imap_session(&account).await {
                        Ok(mut session) => {
                            let mailboxes = email_core::list_mailboxes(&mut session)
                                .await
                                .unwrap_or_default();
                            mailboxes
                                .iter()
                                .find(|mb| {
                                    mb.attributes.contains(
                                        &email_core::OwnedNameAttribute::Custom(
                                            "\\Inbox".to_string(),
                                        ),
                                    ) || mb.name.eq_ignore_ascii_case("INBOX")
                                })
                                .map(|mb| mb.name.clone())
                                .unwrap_or_else(|| "INBOX".to_string())
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to discover mailboxes for {}, defaulting to INBOX: {}",
                                account.email,
                                e
                            );
                            "INBOX".to_string()
                        }
                    };

                    let mb_repo = MailboxRepository::new(&pool);
                    let _ = mb_repo
                        .upsert(&account.id, &inbox_name, Some("/"), &[])
                        .await;

                    let (cmd_tx, cmd_rx) = mpsc::channel(10);
                    let (event_tx, _) = broadcast::channel(100);
                    let mut event_rx = event_tx.subscribe();

                    let worker = SyncWorker::new(
                        creds,
                        account.imap_host.clone(),
                        account.imap_port as u16,
                        inbox_name.clone(),
                        cmd_rx,
                        event_tx,
                        account.sync_window.clone(),
                    );

                    let pool_clone = pool.clone();
                    let acc_id = account.id.clone();
                    let handle_clone = app_handle.clone();
                    let inbox_name_clone = inbox_name.clone();

                    // Spawn a dedicated task to process IMAP IDLE events for this account.
                    tokio::spawn(async move {
                        while let Ok(event) = event_rx.recv().await {
                            match event {
                                SyncEvent::NewMessages(headers) => {
                                    tracing::info!(
                                        account_id = acc_id,
                                        count = headers.len(),
                                        "SyncWorker fetched new messages"
                                    );
                                    let mb_repo = MailboxRepository::new(&pool_clone);
                                    let _ = mb_repo
                                        .upsert(&acc_id, &inbox_name_clone, Some("/"), &[])
                                        .await;

                                    let mut new_uids = Vec::new();
                                    let mut batch_count = 0;
                                    let mut first_sender: Option<String> = None;

                                    let mut tx = match pool_clone.begin().await {
                                        Ok(tx) => Some(tx),
                                        Err(e) => {
                                            tracing::error!(
                                                "Failed to begin sync transaction: {}",
                                                e
                                            );
                                            continue;
                                        }
                                    };

                                    for h in headers {
                                        if first_sender.is_none() {
                                            first_sender = Some(h.from.clone());
                                        }
                                        let flags: Vec<String> =
                                            h.flags.iter().map(|f| format!("{:?}", f)).collect();
                                        let flags_json = serde_json::to_string(&flags)
                                            .unwrap_or_else(|_| "[]".to_string());
                                        let timestamp = h
                                            .date
                                            .as_deref()
                                            .and_then(|d| {
                                                chrono::DateTime::parse_from_rfc2822(d).ok()
                                            })
                                            .map(|dt| dt.timestamp());
                                        let references_json = serde_json::to_string(&h.references)
                                            .unwrap_or_else(|_| "[]".to_string());

                                        let result = sqlx::query(
                                            "INSERT OR IGNORE INTO messages (account_id, mailbox_name, uid, subject, sender, date, date_timestamp, flags, size, has_attachments, snippet, blob_hash, attachment_names, message_id, in_reply_to, references_json, thread_id, thread_subject)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                                        )
                                        .bind(&acc_id)
                                        .bind(&inbox_name_clone)
                                        .bind(h.uid as i32)
                                        .bind(&h.subject)
                                        .bind(&h.from)
                                        .bind(h.date.as_deref())
                                        .bind(timestamp)
                                        .bind(&flags_json)
                                        .bind(h.size as i32)
                                        .bind(h.attachment_names.is_some())
                                        .bind(h.snippet.as_deref())
                                        .bind::<Option<String>>(None)
                                        .bind(h.attachment_names.as_deref())
                                        .bind(h.message_id.as_deref())
                                        .bind(h.in_reply_to.as_deref())
                                        .bind(&references_json)
                                        .bind(&h.thread_id)
                                        .bind(&h.thread_subject)
                                        .execute(tx.as_deref_mut().unwrap())
                                        .await;

                                        match result {
                                            Ok(res) => {
                                                if res.rows_affected() > 0 {
                                                    new_uids.push(h.uid);
                                                    batch_count += 1;
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Failed to insert message UID {}: {}",
                                                    h.uid,
                                                    e
                                                );
                                                let err_str = e.to_string().to_lowercase();
                                                // FTS5 corruption recovery: If the virtual table triggers fail,
                                                // we drop and rebuild it. This is destructive but necessary to
                                                // recover from SQLite disk corruption without manual intervention.
                                                if err_str.contains("fts")
                                                    || err_str.contains("trigger")
                                                    || err_str.contains("malformed")
                                                    || err_str.contains("disk image")
                                                {
                                                    tracing::warn!(
                                                        "FTS corruption detected. Safely dropping and recreating FTS table..."
                                                    );
                                                    let _ = tx.take();
                                                    let _ = sqlx::query(
                                                        "DROP TABLE IF EXISTS email_fts;",
                                                    )
                                                    .execute(&pool_clone)
                                                    .await;
                                                    let _ = sqlx::query("CREATE VIRTUAL TABLE email_fts USING fts5(subject, sender, snippet, attachment_names, content='messages', content_rowid='id');").execute(&pool_clone).await;
                                                    let _ = sqlx::query("INSERT INTO email_fts(email_fts) VALUES('rebuild');").execute(&pool_clone).await;
                                                    break;
                                                }
                                                continue;
                                            }
                                        }

                                        // Commit in batches of 100 to prevent holding a massive transaction lock
                                        // that could block UI reads or cause WAL checkpoint starvation.
                                        if batch_count > 0 && batch_count % 100 == 0 {
                                            if let Some(current_tx) = tx.take() {
                                                if let Err(e) = current_tx.commit().await {
                                                    tracing::error!(
                                                        "Failed to commit sync batch: {}",
                                                        e
                                                    );
                                                    break;
                                                }
                                            }
                                            tx = match pool_clone.begin().await {
                                                Ok(new_tx) => Some(new_tx),
                                                Err(e) => {
                                                    tracing::error!(
                                                        "Failed to begin new batch transaction: {}",
                                                        e
                                                    );
                                                    break;
                                                }
                                            };
                                        }
                                    }

                                    if let Some(current_tx) = tx.take() {
                                        if let Err(e) = current_tx.commit().await {
                                            tracing::error!(
                                                "Failed to commit final sync transaction: {}",
                                                e
                                            );
                                        }
                                    }

                                    if !new_uids.is_empty() {
                                        send_new_email_notification(
                                            &handle_clone,
                                            new_uids.len(),
                                            first_sender.as_deref(),
                                        )
                                        .await;
                                        let _ = handle_clone.emit(
                                            "sync:new-email",
                                            SyncNotification {
                                                account_id: acc_id.clone(),
                                                mailbox: inbox_name_clone.clone(),
                                                new_uids,
                                            },
                                        );
                                    }
                                }
                                SyncEvent::StateSync(updates) => {
                                    tracing::info!(
                                        account_id = acc_id,
                                        count = updates.len(),
                                        "SyncWorker state sync received"
                                    );
                                    let local_msgs: Vec<(i32, String)> = sqlx::query_as(
                                        "SELECT uid, flags FROM messages WHERE account_id = ? AND mailbox_name = ?",
                                    )
                                    .bind(&acc_id)
                                    .bind(&inbox_name_clone)
                                    .fetch_all(&pool_clone)
                                    .await
                                    .unwrap_or_default();

                                    let mut server_uids = std::collections::HashSet::new();
                                    let mut has_changes = false;
                                    let mut tx = match pool_clone.begin().await {
                                        Ok(tx) => Some(tx),
                                        Err(e) => {
                                            tracing::error!("Failed to begin state sync tx: {}", e);
                                            continue;
                                        }
                                    };

                                    for (uid, flags) in &updates {
                                        server_uids.insert(*uid);
                                        let flags_json = serde_json::to_string(&flags)
                                            .unwrap_or_else(|_| "[]".to_string());
                                        if let Some((_, local_flags_json)) =
                                            local_msgs.iter().find(|(u, _)| *u as u32 == *uid)
                                        {
                                            if local_flags_json != &flags_json {
                                                let _ = sqlx::query("UPDATE messages SET flags = ? WHERE account_id = ? AND mailbox_name = ? AND uid = ?")
                                                    .bind(&flags_json)
                                                    .bind(&acc_id)
                                                    .bind(&inbox_name_clone)
                                                    .bind(*uid as i32)
                                                    .execute(&mut *tx.as_deref_mut().unwrap())
                                                    .await;
                                                has_changes = true;
                                            }
                                        }
                                    }

                                    // Delete local messages that no longer exist on the server.
                                    for (local_uid, _) in &local_msgs {
                                        if !server_uids.contains(&(*local_uid as u32)) {
                                            let _ = sqlx::query("DELETE FROM messages WHERE account_id = ? AND mailbox_name = ? AND uid = ?")
                                                .bind(&acc_id)
                                                .bind(&inbox_name_clone)
                                                .bind(*local_uid)
                                                .execute(&mut *tx.as_deref_mut().unwrap())
                                                .await;
                                            has_changes = true;
                                        }
                                    }

                                    if let Some(current_tx) = tx.take() {
                                        if let Err(e) = current_tx.commit().await {
                                            tracing::error!("Failed to commit state sync: {}", e);
                                        }
                                    }

                                    if has_changes {
                                        let _ = handle_clone.emit(
                                            "sync:state-updated",
                                            serde_json::json!({
                                                "account_id": acc_id,
                                                "mailbox": inbox_name_clone
                                            }),
                                        );
                                    }
                                }
                                SyncEvent::Error(e) => {
                                    tracing::error!(account_id = acc_id, "SyncWorker Error: {}", e);
                                    let _ = handle_clone
                                        .emit("sync:error", format!("Sync failed: {}", e));
                                }
                                SyncEvent::Disconnected(e) => {
                                    tracing::warn!(
                                        account_id = acc_id,
                                        "SyncWorker Disconnected: {}",
                                        e
                                    );
                                }
                                SyncEvent::Connected => {
                                    tracing::info!(
                                        account_id = acc_id,
                                        "SyncWorker Connected to IMAP"
                                    );
                                }
                                SyncEvent::SyncComplete => {
                                    tracing::debug!(
                                        account_id = acc_id,
                                        "SyncWorker IDLE cycle complete"
                                    );
                                }
                            }
                        }
                    });

                    let task_handle = tokio::spawn(async move {
                        worker.run().await;
                    });

                    let mut workers_lock = match workers.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => {
                            tracing::error!(
                                "Sync workers mutex poisoned during insert! Recovering..."
                            );
                            poisoned.into_inner()
                        }
                    };
                    workers_lock.insert(
                        account.id.clone(),
                        WorkerHandle {
                            cmd_tx,
                            task_handle,
                        },
                    );
                }
            }
        }
        sleep(Duration::from_secs(60)).await;
    }
}

pub async fn run_blob_gc_worker(pool: SqlitePool, blob_store: BlobStore) {
    sleep(Duration::from_secs(5 * 60)).await;
    loop {
        let msg_repo = MessageRepository::new(&pool);
        if let Ok(active_hashes) = msg_repo.list_all_blob_hashes().await {
            if let Ok(deleted) = blob_store.garbage_collect(&active_hashes).await {
                if deleted > 0 {
                    tracing::info!(deleted_count = deleted, "Blob GC completed");
                }
            }
        }
        // Run once every 24 hours.
        sleep(Duration::from_secs(24 * 60 * 60)).await;
    }
}

pub async fn run_db_maintenance_worker(pool: SqlitePool) {
    sleep(Duration::from_secs(120)).await;
    loop {
        tracing::info!("Starting daily database maintenance...");
        // Passive WAL checkpoint moves frames from the WAL file into the main DB
        // without blocking readers. This prevents the WAL file from growing indefinitely.
        if let Err(e) = sqlx::query("PRAGMA wal_checkpoint(PASSIVE);")
            .execute(&pool)
            .await
        {
            tracing::error!(error = %e, "WAL checkpoint failed");
        } else {
            tracing::info!("WAL checkpoint successful.");
        }
        sleep(Duration::from_secs(24 * 60 * 60)).await;
    }
}

pub async fn run_caldav_sync_worker(pool: SqlitePool) {
    loop {
        let acc_repo = AccountRepository::new(&pool);
        if let Ok(accounts) = acc_repo.list_all().await {
            for account in accounts {
                let domain = account.email.split('@').nth(1).unwrap_or(&account.imap_host);
                let dav_url = match imap::discover_dav_endpoint(domain, "caldav").await {
                    Some(url) => url,
                    None => { tracing::warn!("Could not discover CalDAV endpoint for {}", account.email); continue; }
                };

                let password = match auth::get_password(&account.id, "imap") {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!("Skipping CalDAV sync for {}: Keychain access failed ({})", account.email, e);
                        continue;
                    }
                };

                if let Ok(client) = caldav::CalDavClient::new(&dav_url, &account.email, &password) {
                    let engine = caldav::SyncEngine::new(client);
                    let cal_repo = CalendarRepository::new(&pool);
                    if let Ok(calendars) = engine.discover_full_chain(&dav_url).await {
                        for cal in calendars {
                            let old_token = cal_repo.get_calendars_for_account(&account.id).await.ok()
                                .and_then(|cals| cals.into_iter().find(|c| c.url == cal.url))
                                .and_then(|c| c.sync_token);

                            if let Ok((changed, deleted, new_token)) = engine.sync_collection(&cal, old_token.as_deref()).await {
                                let cal_id = cal_repo.upsert_calendar(&account.id, &cal.url, &cal.display_name, cal.ctag.as_deref(), Some(&new_token)).await.map(|c| c.id).unwrap_or(0);
                                for evt in changed {
                                    let _ = cal_repo.upsert_event(cal_id, &evt.uid, &evt.etag, &evt.url, &evt.ical_data, None).await;
                                }
                                for href in deleted {
                                    let _ = cal_repo.delete_event_by_url(cal_id, &href).await;
                                }
                            } else {
                                let _ = cal_repo.upsert_calendar(&account.id, &cal.url, &cal.display_name, cal.ctag.as_deref(), cal.sync_token.as_deref()).await;
                            }
                        }
                    }
                }
            }
        }
        sleep(Duration::from_secs(60 * 15)).await;
    }
}

pub async fn run_carddav_sync_worker(pool: SqlitePool) {
	loop {
		let acc_repo = AccountRepository::new(&pool);
		if let Ok(accounts) = acc_repo.list_all().await {
			for account in accounts {
				let domain = account.email.split('@').nth(1).unwrap_or(&account.imap_host);
				let dav_url = match imap::discover_dav_endpoint(domain, "carddav").await {
					Some(url) => url,
					None => { tracing::warn!("Could not discover CardDAV endpoint for {}", account.email); continue; }
				};

				let password = match auth::get_password(&account.id, "imap") {
					Ok(p) => p,
					Err(e) => {
						tracing::warn!("Skipping CardDAV sync for {}: Keychain access failed ({})", account.email, e);
						continue;
					}
				};

				if let Ok(client) = carddav::CardDavClient::new(&dav_url, &account.email, &password) {
					let engine = carddav::SyncEngine::new(client);
					let contact_repo = ContactRepository::new(&pool);
					if let Ok(books) = engine.discover_full_chain(&dav_url).await {
						for book in books {
							let old_token = contact_repo.get_address_books_for_account(&account.id).await.ok()
							.and_then(|books| books.into_iter().find(|b| b.url == book.url))
							.and_then(|b| b.sync_token);

							if let Ok((changed, deleted, new_token)) = engine.sync_collection(&book, old_token.as_deref()).await {
								let book_id = contact_repo.upsert_address_book(&account.id, &book.url, &book.display_name, book.ctag.as_deref(), Some(&new_token)).await.map(|b| b.id).unwrap_or(0);
								for c in changed {
									let _ = contact_repo.upsert_contact(book_id, &c.uid, &c.etag, &c.url, &c.vcard_data, None).await;
								}
								for href in deleted {
									let _ = contact_repo.delete_contact_by_url(book_id, &href).await;
								}
							} else {
								let _ = contact_repo.upsert_address_book(&account.id, &book.url, &book.display_name, book.ctag.as_deref(), book.sync_token.as_deref()).await;
							}
						}
					}
				}
			}
		}
		sleep(Duration::from_secs(60 * 15)).await;
	}
}

/// Enforces the user's sync window preference (e.g. LAST_30_DAYS) by permanently
/// deleting older messages from the local DB and garbage collecting their blobs.
pub async fn run_sync_window_pruner(pool: SqlitePool, blob_store: BlobStore) {
    loop {
        sleep(Duration::from_secs(60 * 60 * 24)).await;
        let acc_repo = AccountRepository::new(&pool);
        let msg_repo = MessageRepository::new(&pool);
        if let Ok(accounts) = acc_repo.list_all().await {
            for acc in accounts {
                let cutoff_days = match acc.sync_window.as_str() {
                    "LAST_30_DAYS" => 30,
                    "LAST_6_MONTHS" => 180,
                    _ => continue,
                };
                let cutoff_timestamp =
                    chrono::Utc::now().timestamp() - (cutoff_days * 24 * 60 * 60);
                if let Ok(deleted) = msg_repo.delete_older_than(&acc.id, cutoff_timestamp).await {
                    if deleted > 0 {
                        tracing::info!(
                            account_id = acc.id,
                            deleted = deleted,
                            "Pruned old messages"
                        );
                    }
                }
            }
        }
        if let Ok(active_hashes) = msg_repo.list_all_blob_hashes().await {
            let _ = blob_store.garbage_collect(&active_hashes).await;
        }
    }
}
