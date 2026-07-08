use crate::error::AppError;
use crate::services::auth::{delete_password, set_password};
use crate::state::{AddAccountPayload, AppState};
use email_core::discovery::{discover_provider, ProviderConfig};
use email_core::oauth::FileStore;
use email_core::sync_worker::SyncCommand;
use storage::models::Account;
use storage::repository::{AccountRepository, MessageRepository};
use tauri::State;
use email_core::oauth::TokenStore;

/// Discovers IMAP/SMTP settings for an email address using the 4-step fallback chain.
#[tauri::command]
#[specta::specta]
pub async fn discover_email_settings(email: String) -> Result<ProviderConfig, AppError> {
    discover_provider(&email)
        .await
        .map_err(|e| AppError::Network(e))
}

/// Creates a new account, securely stores credentials, and migrates OAuth tokens if necessary.
#[tauri::command]
#[specta::specta]
pub async fn add_account(
    state: State<'_, AppState>,
    payload: AddAccountPayload,
) -> Result<Account, AppError> {
    let pool = state
        .pool
        .get()
        .ok_or_else(|| AppError::System("Database is still initializing...".into()))?;
    let repo = AccountRepository::new(pool);
    let account = repo
        .create(
            &payload.email,
            &payload.provider,
            &payload.imap_host,
            payload.imap_port,
            &payload.smtp_host,
            payload.smtp_port,
            &payload.auth_method,
            payload.oauth_client_id.as_deref(),
            payload.oauth_client_secret.as_deref(),
            payload.oauth_token_url.as_deref(),
        )
        .await?;
    let sw = payload
        .sync_window
        .unwrap_or_else(|| "LAST_30_DAYS".to_string());
    repo.update_sync_window(&account.id, &sw).await?;

    if payload.auth_method == "password" {
        let pwd = payload
            .password
            .ok_or_else(|| AppError::BadRequest("Password required".into()))?;
        // Store IMAP and SMTP passwords separately in the OS keychain to allow
        // independent rotation if a provider requires different app passwords.
        set_password(&account.id, "imap", &pwd)?;
        set_password(&account.id, "smtp", &pwd)?;
    } else if payload.auth_method == "oauth2" {
        // During the OAuth flow, tokens are temporarily saved under the user's email address
        // because the database UUID doesn't exist yet. Now that the DB row is created,
        // we rename the token file to the permanent UUID to stabilize the path.
        let _ = FileStore::rename(&payload.email, &account.id);
    }
    Ok(account)
}

#[tauri::command]
#[specta::specta]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, AppError> {
    let pool = state
        .pool
        .get()
        .ok_or_else(|| AppError::System("Database is still initializing...".into()))?;
    Ok(AccountRepository::new(pool).list_all().await?)
}

/// Updates the sync window and forces the background IMAP worker to reset its UID tracking.
#[tauri::command]
#[specta::specta]
pub async fn update_sync_window(
    state: State<'_, AppState>,
    account_id: String,
    sync_window: String,
) -> Result<(), AppError> {
    let pool = state
        .pool
        .get()
        .ok_or_else(|| AppError::System("DB not ready".into()))?;
    AccountRepository::new(pool)
        .update_sync_window(&account_id, &sync_window)
        .await?;

    let cmd_tx = {
        let workers = match state.sync_workers.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        workers.get(&account_id).map(|handle| handle.cmd_tx.clone())
    };

    // Changing the sync window invalidates the current UID NEXT tracking.
    // We must send a ForceResync command so the worker drops its cached state
    // and re-evaluates the date/UID boundaries on the next IDLE cycle.
    if let Some(tx) = cmd_tx {
        let _ = tx.send(SyncCommand::ForceResync).await;
    }
    Ok(())
}

/// Deletes an account and all associated data, recovering from mutex poisoning if necessary.
#[tauri::command]
#[specta::specta]
pub async fn delete_account(state: State<'_, AppState>, account_id: String) -> Result<(), AppError> {
    // Recover from mutex poisoning: if a background thread panicked while holding the
    // sync workers lock, we take the inner value rather than crashing the UI thread.
    let worker_handle = {
        let mut workers = match state.sync_workers.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::error!("Sync workers mutex poisoned during account deletion! Recovering...");
                poisoned.into_inner()
            }
        };
        workers.remove(&account_id)
    };

    if let Some(handle) = worker_handle {
        let _ = handle.cmd_tx.send(SyncCommand::Shutdown).await;
        handle.task_handle.abort();
    }

    let pool = state
        .pool
        .get()
        .ok_or_else(|| AppError::System("Database is still initializing...".into()))?;

    // Manual cascade deletion. While foreign keys handle messages/events,
    // we explicitly delete them here to ensure the transaction completes
    // even if the schema's ON DELETE CASCADE was bypassed or altered.
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM messages WHERE account_id = ?")
        .bind(&account_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM calendar_events WHERE calendar_id IN (SELECT id FROM calendars WHERE account_id = ?)")
        .bind(&account_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM contacts WHERE address_book_id IN (SELECT id FROM address_books WHERE account_id = ?)")
        .bind(&account_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM accounts WHERE id = ?")
        .bind(&account_id).execute(&mut *tx).await?;
    tx.commit().await?;

    // Wipe OS keychain entries for all services
    let _ = delete_password(&account_id, "imap");
    let _ = delete_password(&account_id, "smtp");
    let _ = delete_password(&account_id, "caldav");
    let _ = delete_password(&account_id, "carddav");

    // Wipe OAuth token file
    if let Ok(store) = FileStore::new(&account_id) {
        let _ = store.clear().await;
    }

    // Spawn blob garbage collection in the background.
    // Scanning the blob directory and deleting files can take seconds on large accounts;
    // doing it synchronously would freeze the UI during account deletion.
    if let Some(blob_store) = state.blob_store.get() {
        let blob_store_owned = blob_store.clone();
        let pool_clone = pool.clone();
        tauri::async_runtime::spawn(async move {
            let msg_repo = MessageRepository::new(&pool_clone);
            if let Ok(active_hashes) = msg_repo.list_all_blob_hashes().await {
                let _ = blob_store_owned.garbage_collect(&active_hashes).await;
            }
        });
    }
    Ok(())
}
