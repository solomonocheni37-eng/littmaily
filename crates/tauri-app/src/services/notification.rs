// ./crates/tauri-app/src/services/notification.rs
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

pub async fn send_notification(
    app_handle: &AppHandle,
    title: &str,
    body: &str,
    _badge_count: Option<i32>, // Handled globally by SyncListener calling update_badge_count
) -> Result<(), String> {
    let _ = app_handle
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();

    Ok(())
}

pub async fn update_badge_count(app_handle: &AppHandle, count: i32) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let badge = if count > 0 { Some(count as i64) } else { None };
            let _ = window.set_badge_count(badge);
        }

        #[cfg(target_os = "windows")]
        {
            if count > 0 {
                let _ = window.set_title(&format!("Littmaily ({})", count));
            } else {
                let _ = window.set_title("Littmaily");
            }
        }
    }
    Ok(())
}
