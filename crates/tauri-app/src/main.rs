// FILE: ./crates/tauri-app/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod error;
mod outbox_worker;
mod services;
mod state;
mod util;

use crate::state::AppState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use storage::db::init_file_pool;
use tauri::{Emitter, Manager};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::DialogExt;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn main() {
    // CRITICAL LINUX WORKAROUND: WebKitGTK frequently crashes on systems with incompatible
    // GPU drivers (common in VMs, Wayland, or older distros) when using hardware compositing
    // or DMABUF rendering. Disabling these forces software rendering, ensuring stability
    // at the cost of some graphical performance.
    #[cfg(target_os = "linux")]
    unsafe {
        // CRITICAL PRODUCTION FIX:
        // The previous workaround forced CPU software rendering to prevent Wayland crashes,
        // which mathematically guarantees scrolling jank (the CPU cannot repaint at 60fps+).
        // Instead, we force X11 via XWayland. This bypasses native Wayland GPU crashes
        // while preserving the hardware compositor required for buttery smooth scrolling.
        std::env::set_var("GDK_BACKEND", "x11");

        // Disable DMA-BUF to prevent specific Nvidia/Mesa segfaults without killing OpenGL
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    // rustls requires a crypto provider to be installed globally before any TLS connections
    // are attempted. Failing to do this will cause panics on the first HTTPS request.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("littmaily")
        .join("logs");
    std::fs::create_dir_all(&log_dir).expect("Failed to create log directory");

    let file_appender = rolling::daily(&log_dir, "littmaily.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::new(
            "info,sqlx=warn,reqwest=warn,async_imap=warn,html5ever=error",
        ))
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    // Global panic hook to ensure Rust panics are logged to the file before the process aborts.
    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        tracing::error!(
            panic.payload = payload,
            panic.location = location,
            "APPLICATION PANIC"
        );
    }));

    tracing::info!("Littmaily starting up...");

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init());

    builder
        .setup(|app| {
            let app_handle = app.handle().clone();
            //Initialize the Updater Plugin (Desktop only)
            #[cfg(desktop)]
            let _ = app_handle.plugin(tauri_plugin_updater::Builder::new().build());

            let app_handle_for_deep_link = app_handle.clone();

            // Intercept OS-level deep link redirects (e.g., littmaily://oauth/callback)
            // and emit them as Tauri events so the frontend OAuth flow can complete.
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    if url.scheme() == "littmaily" {
                        let mut code = None;
                        let mut state = None;
                        for (k, v) in url.query_pairs() {
                            if k == "code" {
                                code = Some(v.to_string());
                            }
                            if k == "state" {
                                state = Some(v.to_string());
                            }
                        }
                        if let (Some(c), Some(s)) = (code, state) {
                            let _ = app_handle_for_deep_link.emit(
                                "oauth:deep-link-callback",
                                serde_json::json!({ "code": c, "state": s }),
                            );
                        }
                    }
                }
            });

            let master_key = match crypto::MasterKeyManager::get_or_create_key() {
                Ok(key) => key,
                Err(e) => {
                    let error_msg = format!(
                        "Failed to access system keychain.\n\
                        On Linux, ensure a Secret Service provider (like gnome-keyring or keepassxc) is running.\n\
                        Error: {}", e
                    );
                    app_handle
                        .dialog()
                        .message(&error_msg)
                        .title("Critical Security Error")
                        .blocking_show();
                    std::process::exit(1);
                }
            };

            // We use OnceLock because the DB requires asynchronous initialization
            // (SQLCipher decryption and migrations) which cannot be done in the
            // synchronous tauri::Builder::setup closure without blocking the UI thread.
            let pool_cell = Arc::new(OnceLock::new());
            let blob_cell = Arc::new(OnceLock::new());
            let pool_clone = pool_cell.clone();
            let blob_clone = blob_cell.clone();
            let sync_workers = Arc::new(Mutex::new(HashMap::new()));
            let sync_workers_for_manager = sync_workers.clone();

            tauri::async_runtime::spawn(async move {
                tracing::info!("[MAIN] Async DB initialization task started.");
                let db_path = dirs::data_local_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("littmaily")
                    .join("app.db");
                let db_key_hex = hex::encode(master_key).to_uppercase();

                let pool = match init_file_pool(&db_path, &db_key_hex).await {
                    Ok(p) => p,
                    Err(e) => {
                        // If the DB is corrupted, offer the user a chance to reset it
                        // rather than permanently bricking the app.
                        let error_msg = format!(
                            "Failed to initialize the local database.\n\
                            This usually happens if the disk is full or the database file is corrupted.\n\
                            Error: {}\n\
                            Click 'Reset' to delete the corrupted database and start fresh (this will erase local cached emails).", e
                        );
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        let db_path_clone = db_path.clone();
                        app_handle
                            .dialog()
                            .message(&error_msg)
                            .title("Critical Database Error")
                            .kind(tauri_plugin_dialog::MessageDialogKind::Error)
                            .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                                "Reset App Data".to_string(),
                                "Exit".to_string(),
                            ))
                            .show(move |is_reset| {
                                let _ = tx.send(is_reset);
                            });
                        match rx.await {
                            Ok(true) => {
                                let _ = std::fs::remove_file(&db_path_clone);
                                let _ = std::fs::remove_file(format!("{}-wal", db_path_clone.display()));
                                let _ = std::fs::remove_file(format!("{}-shm", db_path_clone.display()));
                                match init_file_pool(&db_path_clone, &db_key_hex).await {
                                    Ok(p) => p,
                                    Err(e2) => {
                                        tracing::error!("Failed to reset and re-initialize DB: {}", e2);
                                        std::process::exit(1);
                                    }
                                }
                            }
                            _ => std::process::exit(1),
                        }
                    }
                };

                tracing::info!("[MAIN] Injecting pool into AppState OnceLock...");
                let _ = pool_clone.set(pool.clone());
                tracing::info!("[MAIN] Pool injection success: {}", pool_clone.get().is_some());

                let blob_dir = app_handle
                    .path()
                    .app_local_data_dir()
                    .unwrap_or_else(|_| dirs::data_local_dir().unwrap_or_default())
                    .join("littmaily")
                    .join("blobs");
                let blob_store = storage::blob::BlobStore::new(blob_dir, master_key);
                if let Err(e) = blob_store.init().await {
                    let error_msg = format!(
                        "Failed to initialize the local attachment store.\n\
                        This usually happens if the disk is full or permissions are denied.\n\
                        Error: {}", e
                    );
                    app_handle
                        .dialog()
                        .message(&error_msg)
                        .title("Critical Storage Error")
                        .kind(tauri_plugin_dialog::MessageDialogKind::Error)
                        .blocking_show();
                    std::process::exit(1);
                }
                let _ = blob_clone.set(blob_store.clone());

                services::workers::spawn_all_workers(
                    app_handle.clone(),
                    pool,
                    blob_store,
                    sync_workers_for_manager,
                );
            });

            app.manage(AppState {
                pool: pool_cell,
                blob_store: blob_cell,
                pending_flow: std::sync::Mutex::new(None),
                sync_workers,
            });

            // ==========================================
            // System Tray Icon Setup
            // ==========================================
            let show = MenuItem::with_id(app, "show", "Show Littmaily", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            let mut tray_builder = TrayIconBuilder::with_id("main-tray")
                .tooltip("Littmaily")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } => {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                });

            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            let _tray = tray_builder.build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_db_ready,
            commands::start_oauth2_login,
            commands::complete_oauth2_login,
            commands::queue_email,
            commands::cancel_scheduled_email,
            commands::save_draft,
            commands::get_drafts,
            commands::delete_draft,
            commands::get_calendar_events,
            commands::get_contacts,
            commands::get_thread_messages,
            commands::unified_search,
            commands::discover_email_settings,
            commands::add_account,
            commands::list_accounts,
            commands::delete_account,
            commands::get_mailboxes,
            commands::get_emails_paginated,
            commands::fetch_email_body,
            commands::get_cached_email_body,
            commands::update_email_state,
            commands::get_attachment_path,
            commands::get_attachment_base64,
            commands::check_for_new_emails,
            commands::proxy_remote_image,
            commands::create_folder,
            commands::delete_folder,
            commands::rename_folder,
            commands::open_logs_folder,
            commands::fetch_viewport_snippets,
            commands::update_sync_window,
            commands::backfill_older_emails,
            commands::save_attachment_dialog,
            commands::fetch_email_attachment,
            commands::update_badge_count,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod specta_export {
    use super::*;
    use specta_typescript::Typescript;
    use tauri_specta::{collect_commands, Builder};

    // This test ensures the TypeScript bindings stay in sync with the Rust IPC commands.
    // It runs during `cargo test` and overwrites the generated.ts file in the frontend.
    #[test]
    fn export_typescript_types() {
        let builder = Builder::<tauri::Wry>::new().commands(collect_commands![
            commands::check_db_ready,
            commands::start_oauth2_login,
            commands::complete_oauth2_login,
            commands::queue_email,
            commands::cancel_scheduled_email,
            commands::save_draft,
            commands::get_drafts,
            commands::get_thread_messages,
            commands::delete_draft,
            commands::get_calendar_events,
            commands::get_contacts,
            commands::unified_search,
            commands::discover_email_settings,
            commands::add_account,
            commands::list_accounts,
            commands::delete_account,
            commands::get_mailboxes,
            commands::get_emails_paginated,
            commands::fetch_email_body,
            commands::get_cached_email_body,
            commands::update_email_state,
            commands::get_attachment_path,
            commands::get_attachment_base64,
            commands::check_for_new_emails,
            commands::proxy_remote_image,
            commands::create_folder,
            commands::delete_folder,
            commands::rename_folder,
            commands::open_logs_folder,
            commands::fetch_viewport_snippets,
            commands::update_sync_window,
            commands::backfill_older_emails,
            commands::save_attachment_dialog,
            commands::fetch_email_attachment,
            commands::update_badge_count,
        ]);
        let export_path = "../../frontend/src/core/types/generated.ts";
        builder
            .export(
                Typescript::default()
                    .header("// ð ¯ Auto-generated by tauri-specta. DO NOT EDIT MANUALLY.\n"),
                export_path,
            )
            .expect("Failed to export typescript types");
        println!("â  Successfully exported TS types to {}", export_path);
    }
}
