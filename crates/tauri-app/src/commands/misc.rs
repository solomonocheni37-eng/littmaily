use crate::error::AppError;
use crate::state::AppState;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::OnceLock;
use tauri::{State, Manager};
use reqwest::Url;

const IMAGE_PROXY_TIMEOUT_SECS: u64 = 15;
// 5MB limit prevents memory exhaustion from malicious emails embedding massive base64 images
// or infinite-length streaming responses from compromised CDNs.
const IMAGE_PROXY_MAX_BYTES: usize = 5 * 1024 * 1024;

static SHARED_IMAGE_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Returns a singleton HTTP client configured specifically for image proxying.
///
/// Forces HTTP/1.1 because HTTP/2 multiplexing can cause head-of-line blocking or
/// connection resets with some flaky image CDNs. Connection pooling is enabled to
/// reuse TLS handshakes when an email contains multiple images from the same domain.
fn get_image_client() -> Result<&'static reqwest::Client, AppError> {
    if let Some(c) = SHARED_IMAGE_CLIENT.get() {
        return Ok(c);
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(IMAGE_PROXY_TIMEOUT_SECS))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
        .redirect(reqwest::redirect::Policy::none()) // We handle redirects manually to enforce SSRF checks on every hop
        .http1_only()
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::System(format!("Failed to build shared image client: {}", e)))?;
    Ok(SHARED_IMAGE_CLIENT.get_or_init(|| client))
}

/// Validates that an IP address is not part of a private, loopback, or link-local network.
///
/// This is a critical SSRF (Server-Side Request Forgery) defense. It prevents malicious emails
/// from using the image proxy to scan the user's local network (e.g., `192.168.x.x`, `100.64.x.x` CGNAT,
/// IPv6 link-local `fe80::`, or localhost).
fn is_safe_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            if ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local() || ipv4.is_broadcast() || ipv4.is_unspecified() {
                return false;
            }
            // Block specific IANA reserved ranges that `is_private()` might miss in older Rust versions
            if ipv4.octets()[0] == 100 && (ipv4.octets()[1] & 0xc0) == 64 { return false; } // CGNAT
            if ipv4.octets()[0] == 192 && ipv4.octets()[1] == 0 && ipv4.octets()[2] == 0 { return false; } // IETF Protocol
            if ipv4.octets()[0] == 192 && ipv4.octets()[1] == 0 && ipv4.octets()[2] == 2 { return false; } // TEST-NET-1
            if ipv4.octets()[0] == 198 && ipv4.octets()[1] == 51 && ipv4.octets()[2] == 100 { return false; } // TEST-NET-2
            if ipv4.octets()[0] == 203 && ipv4.octets()[1] == 0 && ipv4.octets()[2] == 113 { return false; } // TEST-NET-3
            true
        }
        IpAddr::V6(ipv6) => {
            if ipv6.is_loopback() || ipv6.is_unspecified() { return false; }
            let segments = ipv6.segments();
            if segments[0] & 0xffc0 == 0xfe80 { return false; } // Link-local
            if segments[0] & 0xfe00 == 0xfc00 { return false; } // Unique-local
            // Check for IPv4-mapped IPv6 addresses (e.g., ::ffff:192.168.1.1)
            if segments[0] == 0 && segments[1] == 0 && segments[2] == 0 && segments[3] == 0 && segments[4] == 0 && segments[5] == 0xffff {
                let v4_octets = [(segments[6] >> 8) as u8, segments[6] as u8, (segments[7] >> 8) as u8, segments[7] as u8];
                let v4 = std::net::Ipv4Addr::new(v4_octets[0], v4_octets[1], v4_octets[2], v4_octets[3]);
                return is_safe_ip(IpAddr::V4(v4));
            }
            true
        }
    }
}

/// Resolves the hostname and validates every resulting IP against the SSRF blocklist.
fn validate_url_safety(url_str: &str) -> Result<(), String> {
    let url = Url::parse(url_str).map_err(|e| e.to_string())?;
    let host = url.host_str().ok_or("No host")?;
    let port = url.port_or_known_default().unwrap_or(443);
    let socket_addrs = format!("{}:{}", host, port)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed: {}", e))?;
    for addr in socket_addrs {
        if !is_safe_ip(addr.ip()) {
            return Err(format!("Blocked unsafe IP: {}", addr.ip()));
        }
    }
    Ok(())
}

/// Polls the background database initialization task to determine if the UI can begin rendering.
#[tauri::command]
#[specta::specta]
pub async fn check_db_ready(state: State<'_, AppState>) -> Result<bool, AppError> {
    let is_ready = state.pool.get().is_some();
    tracing::info!("[BACKEND] check_db_ready called. Pool ready: {}", is_ready);
    Ok(is_ready)
}

/// Fetches a remote image, validates it for SSRF, and returns it as a base64 data URI.
///
/// If the image fails to load, exceeds the size limit, or triggers an SSRF block,
/// it returns a 1x1 transparent GIF fallback pixel to prevent broken UI layout.
#[tauri::command]
#[specta::specta]
pub async fn proxy_remote_image(url: String) -> Result<String, AppError> {
    let fallback_pixel = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7".to_string();
    let client = get_image_client()?;

    let parsed_url = match reqwest::Url::parse(&url) {
        Ok(u) => u,
        Err(_) => {
            // Attempt to fix malformed URLs with unencoded spaces
            let fixed_url = url.replace(" ", "%20");
            match reqwest::Url::parse(&fixed_url) {
                Ok(u) => u,
                Err(e) => {
                    tracing::warn!("Image proxy invalid URL {}: {}", url, e);
                    return Ok(fallback_pixel);
                }
            }
        }
    };

    let mut current_url = parsed_url;
    let mut resp = None;

    // Manual redirect loop to enforce SSRF checks on every hop
    for _ in 0..5 {
        if let Err(e) = validate_url_safety(current_url.as_str()) {
            tracing::warn!("Image proxy SSRF blocked for {}: {}", current_url, e);
            return Ok(fallback_pixel);
        }
        let r = match client.get(current_url.clone()).header("Accept", "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8").send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Image proxy network error for {}: {:?}", current_url, e);
                return Ok(fallback_pixel);
            }
        };
        if r.status().is_redirection() {
            if let Some(loc) = r.headers().get(reqwest::header::LOCATION) {
                if let Ok(loc_str) = loc.to_str() {
                    current_url = match Url::parse(loc_str) {
                        Ok(u) => u,
                        Err(_) => match current_url.join(loc_str) {
                            Ok(u) => u,
                            Err(_) => return Ok(fallback_pixel),
                        }
                    };
                    continue;
                }
            }
        }
        resp = Some(r);
        break;
    }

    let mut resp = match resp {
        Some(r) => r,
        None => return Ok(fallback_pixel),
    };

    if !resp.status().is_success() {
        tracing::warn!("Image proxy HTTP error for {}: {}", url, resp.status());
        return Ok(fallback_pixel);
    }

    let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_lowercase();
    let mut bytes = Vec::new();
    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                bytes.extend_from_slice(&chunk);
                if bytes.len() > IMAGE_PROXY_MAX_BYTES {
                    tracing::warn!("Image proxy size limit exceeded for {}", url);
                    return Ok(fallback_pixel);
                }
            }
            Ok(None) => break,
            Err(e) => {
                let err_str = e.to_string();
                // Tolerate premature connection closes from flaky CDNs if we already have some data
                if !bytes.is_empty() && (err_str.contains("close_notify") || err_str.contains("UnexpectedEof") || err_str.contains("unexpected eof") || err_str.contains("error decoding response body")) {
                    break;
                }
                tracing::warn!("Image proxy chunk read error for {}: {}", url, err_str);
                return Ok(fallback_pixel);
            }
        }
    }

    if bytes.is_empty() { return Ok(fallback_pixel); }

    // Magic byte sniffing to determine the true MIME type, as email clients and CDNs
    // frequently lie about Content-Type (e.g., serving PNGs as application/octet-stream).
    let mime_type = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) { "image/jpeg" }
    else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) { "image/png" }
    else if bytes.starts_with(&[0x47, 0x49, 0x46, 0x38]) { "image/gif" }
    else if bytes.get(8..12) == Some(b"WEBP".as_slice()) && bytes.starts_with(&[0x52, 0x49, 0x46, 0x46]) { "image/webp" }
    else if bytes.starts_with(b"<?xml") || bytes.starts_with(b"<svg") || bytes.windows(4).any(|w| w == b"<svg") { "image/svg+xml" }
    else if content_type.contains("icon") || bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) { "image/x-icon" }
    else if content_type.starts_with("image/") { content_type.split(';').next().unwrap_or("image/png") }
    else {
        let url_lower = url.to_lowercase();
        if url_lower.contains(".jpg") || url_lower.contains(".jpeg") { "image/jpeg" }
        else if url_lower.contains(".png") { "image/png" }
        else if url_lower.contains(".gif") { "image/gif" }
        else if url_lower.contains(".webp") { "image/webp" }
        else if url_lower.contains(".svg") { "image/svg+xml" }
        else {
            tracing::warn!("Image proxy unknown mime type for {}. Content-Type: {}", url, content_type);
            return Ok(fallback_pixel);
        }
    };

    let b64 = STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime_type, b64))
}

/// Opens the OS file explorer to the local log directory for debugging.
#[tauri::command]
#[specta::specta]
pub fn open_logs_folder() -> Result<(), AppError> {
    let log_dir = dirs::data_local_dir()
        .ok_or_else(|| AppError::System("Could not resolve local data directory".into()))?
        .join("littmaily")
        .join("logs");
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir).map_err(|e| AppError::System(e.to_string()))?;
    }
    open::that(log_dir).map_err(|e| AppError::System(format!("Failed to open folder: {}", e)))?;
    Ok(())
}

/// Updates the OS-native badge count and window title fallbacks.
///
/// Tauri 2 does not yet have a unified native badge API for Windows/Linux taskbars,
/// so we mutate the window title as a fallback to show the unread count.
#[tauri::command]
#[specta::specta]
pub async fn update_badge_count(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), AppError> {
    let pool = state.pool.get().ok_or_else(|| AppError::System("DB not ready".into()))?;
    let unread_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE flags NOT LIKE '%\"Seen\"%'"
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or(0);

    if let Some(tray) = app_handle.tray_by_id("main-tray") {
        let tooltip = if unread_count > 0 {
            format!("Littmaily ({} unread)", unread_count)
        } else {
            "Littmaily".to_string()
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }

    if let Some(window) = app_handle.get_webview_window("main") {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            // Natively updates the macOS Dock badge and Linux Unity Launcher API (GNOME Dash to Dock, KDE Plasma)
            let badge = if unread_count > 0 { Some(unread_count) } else { None };
            let _ = window.set_badge_count(badge);
        }

        #[cfg(target_os = "windows")]
        {
            // Windows taskbar fallback
            if unread_count > 0 {
                let _ = window.set_title(&format!("Littmaily ({})", unread_count));
            } else {
                let _ = window.set_title("Littmaily");
            }
        }
    }

    Ok(())
}
