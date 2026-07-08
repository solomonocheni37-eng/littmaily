// ./crates/tauri-app/src/services/notification.rs
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

pub async fn send_notification(
    app_handle: &AppHandle,
    title: &str,
    body: &str,
    badge_count: Option<i32>,
) -> Result<(), String> {
    let _ = app_handle
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();

    // Update badge count (macOS and Windows)
    #[cfg(target_os = "macos")]
    {
        use tauri::menu::MenuItemExt;
        if let Some(count) = badge_count {
            let badge = if count > 0 {
                count.to_string()
            } else {
                String::new()
            };
            let _ = app_handle.set_badge_count(Some(count as i64));
        }
    }
    #[cfg(target_os = "windows")]
    {
        // Windows doesn't have a native badge API in Tauri 2 yet.
        // We mutate the window title as a fallback to show the unread count.
        if let Some(count) = badge_count {
            if count > 0 {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.set_title(&format!("Littmaily ({})", count));
                }
            } else {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.set_title("Littmaily");
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        // Linux badge via window urgency hint
        if let Some(count) = badge_count {
            if count > 0 {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.set_focus();
                }
            }
        }
    }
    Ok(())
}

pub async fn update_badge_count(app_handle: &AppHandle, count: i32) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = app_handle.set_badge_count(Some(count as i64));
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(window) = app_handle.get_webview_window("main") {
            if count > 0 {
                let _ = window.set_title(&format!("Littmaily ({})", count));
            } else {
                let _ = window.set_title("Littmaily");
            }
        }
    }
    Ok(())
}
