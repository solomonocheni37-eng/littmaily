// FILE: ./crates/tauri-app/src/commands/misc.rs
use crate::error::AppError;
use crate::state::AppState;
use futures::StreamExt;
use reqwest::Url;
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tauri::{Manager, State};
use tokio::sync::OnceCell;

const IMAGE_PROXY_TIMEOUT_SECS: u64 = 15;
// 5MB limit prevents memory exhaustion from malicious emails embedding massive
// base64 images or infinite-length streaming responses from compromised CDNs.
const IMAGE_PROXY_MAX_BYTES: usize = 5 * 1024 * 1024;
// Images wider than this are downscaled — but ONLY on the background
// optimization path (Lever 3), never on the user-visible fast path.
const IMAGE_MAX_WIDTH: u32 = 2048;
const IMAGE_BATCH_MAX_URLS: usize = 500;
// Lever 5: raised from 8 to 16. Safe now because (a) fetching is decoupled
// from the user-visible path via prewarm, and (b) DNS + decode no longer
// occupy async worker threads (Lever 2).
const IMAGE_BATCH_CONCURRENCY: usize = 16;

const CACHE_EXTENSIONS: [&str; 6] = ["jpg", "png", "gif", "webp", "svg", "ico"];

static SHARED_IMAGE_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Session-long dedup map: url -> shared task cell. Guarantees exactly ONE
/// fetch per URL per session, even when the background prewarm and a
/// viewport-triggered batch request the same URL concurrently. The second
/// caller awaits the first caller's result instead of re-fetching.
static IMAGE_TASKS: OnceLock<std::sync::Mutex<HashMap<String, Arc<OnceCell<String>>>>> =
    OnceLock::new();

/// Returns a singleton HTTP client configured specifically for image proxying.
///
/// Forces HTTP/1.1 because HTTP/2 multiplexing can cause head-of-line blocking
/// or connection resets with some flaky image CDNs. Connection pooling is
/// enabled to reuse TLS handshakes when an email contains multiple images from
/// the same domain.
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
/// This is a critical SSRF (Server-Side Request Forgery) defense. It prevents malicious
/// emails from using the image proxy to scan the user's local network.
fn is_safe_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            if ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local() || ipv4.is_broadcast() || ipv4.is_unspecified() {
                return false;
            }
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
///
/// Lever 2: `tokio::net::lookup_host` runs getaddrinfo on tokio's blocking pool,
/// so DNS resolution no longer parks async worker threads (the old synchronous
/// `ToSocketAddrs` did, which serialized batches behind slow resolvers).
async fn validate_url_safety(url_str: &str) -> Result<(), String> {
    let url = Url::parse(url_str).map_err(|e| e.to_string())?;
    let host = url.host_str().ok_or("No host")?;
    let port = url.port_or_known_default().unwrap_or(443);
    let socket_addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("DNS resolution failed: {}", e))?;
    for addr in socket_addrs {
        if !is_safe_ip(addr.ip()) {
            return Err(format!("Blocked unsafe IP: {}", addr.ip()));
        }
    }
    Ok(())
}

fn fallback_pixel() -> String {
    "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7".to_string()
}

/// The plaintext cache directory shared with `get_attachment_path`.
/// Already covered by the assetProtocol scope in tauri.conf.json.
fn image_cache_dir(state: &State<'_, AppState>) -> Result<PathBuf, AppError> {
    let blob_store = state
        .blob_store
        .get()
        .ok_or_else(|| AppError::System("Blob store not ready".into()))?;
    Ok(blob_store.base_dir().parent().unwrap().join("cache"))
}

fn ext_for_mime(mime: &str) -> &'static str {
    match mime {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/x-icon" => "ico",
        _ => "png",
    }
}

fn cache_url_hash(url: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Looks for an already-cached file for this URL hash across known extensions.
fn lookup_cached_image(dir: &PathBuf, hash: &str) -> Option<PathBuf> {
    for ext in CACHE_EXTENSIONS.iter() {
        let p = dir.join(format!("{}.{}", hash, ext));
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Magic-byte sniffing only (no Content-Type / URL fallbacks).
/// Every branch returns Option<&'static str> because if/else is an
/// expression in Rust: all branches must agree on one type.
fn sniff_magic(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        Some("image/png")
    } else if bytes.starts_with(&[0x47, 0x49, 0x46, 0x38]) {
        Some("image/gif")
    } else if bytes.get(8..12) == Some(b"WEBP".as_slice())
        && bytes.starts_with(&[0x52, 0x49, 0x46, 0x46])
    {
        Some("image/webp")
    } else if bytes.starts_with(b"<?xml")
        || bytes.starts_with(b"<svg")
        || bytes.windows(4).any(|w| w == b"<svg")
    {
        Some("image/svg+xml")
    } else if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        Some("image/x-icon")
    } else {
        None
    }
}

/// Full MIME determination: magic bytes first (CDNs frequently lie about
/// Content-Type), then Content-Type, then URL extension heuristics.
fn sniff_mime(bytes: &[u8], content_type: &str, url: &str) -> Option<String> {
    if let Some(m) = sniff_magic(bytes) {
        return Some(m.to_string());
    }
    if content_type.contains("icon") {
        return Some("image/x-icon".to_string());
    }
    if content_type.starts_with("image/") {
        return Some(content_type.split(';').next().unwrap_or("image/png").to_string());
    }
    let url_lower = url.to_lowercase();
    if url_lower.contains(".jpg") || url_lower.contains(".jpeg") { Some("image/jpeg".into()) }
    else if url_lower.contains(".png") { Some("image/png".into()) }
    else if url_lower.contains(".gif") { Some("image/gif".into()) }
    else if url_lower.contains(".webp") { Some("image/webp".into()) }
    else if url_lower.contains(".svg") { Some("image/svg+xml".into()) }
    else {
        tracing::warn!("Image proxy unknown mime type for {}. Content-Type: {}", url, content_type);
        None
    }
}

/// SSRF-safe fetch core: redirect loop with per-hop validation, chunked read
/// with a hard size cap, and tolerant handling of premature CDN closes.
async fn fetch_remote_image(url: &str) -> Option<(Vec<u8>, String)> {
    let client = get_image_client().ok()?;
    let parsed_url = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => {
            let fixed_url = url.replace(" ", "%20");
            match reqwest::Url::parse(&fixed_url) {
                Ok(u) => u,
                Err(e) => {
                    tracing::warn!("Image proxy invalid URL {}: {}", url, e);
                    return None;
                }
            }
        }
    };
    let mut current_url = parsed_url;
    let mut resp = None;
    for _ in 0..5 {
        if let Err(e) = validate_url_safety(current_url.as_str()).await {
            tracing::warn!("Image proxy SSRF blocked for {}: {}", current_url, e);
            return None;
        }
        let r = match client.get(current_url.clone()).header("Accept", "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8").send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Image proxy network error for {}: {:?}", current_url, e);
                return None;
            }
        };
        if r.status().is_redirection() {
            if let Some(loc) = r.headers().get(reqwest::header::LOCATION) {
                if let Ok(loc_str) = loc.to_str() {
                    current_url = match Url::parse(loc_str) {
                        Ok(u) => u,
                        Err(_) => match current_url.join(loc_str) {
                            Ok(u) => u,
                            Err(_) => return None,
                        },
                    };
                    continue;
                }
            }
        }
        resp = Some(r);
        break;
    }
    let mut resp = resp?;
    if !resp.status().is_success() {
        tracing::warn!("Image proxy HTTP error for {}: {}", url, resp.status());
        return None;
    }
    let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_lowercase();
    let mut bytes = Vec::new();
    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                bytes.extend_from_slice(&chunk);
                if bytes.len() > IMAGE_PROXY_MAX_BYTES {
                    tracing::warn!("Image proxy size limit exceeded for {}", url);
                    return None;
                }
            }
            Ok(None) => break,
            Err(e) => {
                let err_str = e.to_string();
                if !bytes.is_empty() && (err_str.contains("close_notify") || err_str.contains("UnexpectedEof") || err_str.contains("unexpected eof") || err_str.contains("error decoding response body")) {
                    break;
                }
                tracing::warn!("Image proxy chunk read error for {}: {}", url, err_str);
                return None;
            }
        }
    }
    if bytes.is_empty() { return None; }
    let mime = sniff_mime(&bytes, &content_type, url)?;
    Some((bytes, mime))
}

// ==========================================
// Lever 3: background optimization path
// ==========================================

/// Decodes and, if needed, downscales a cached image OFF the user-visible path.
/// Writes the result to a temp file and atomically renames it over the
/// original, so readers never observe a partial file. A `.opt` sidecar marks
/// "already checked" so later opens skip the decode entirely.
fn optimize_from_bytes(path: &PathBuf, bytes: &[u8], mime: &str) {
    let marker = path.with_extension("opt");
    if marker.exists() {
        return;
    }
    match mime {
        "image/jpeg" | "image/png" => {
            if let Ok(img) = image::load_from_memory(bytes) {
                if img.width() > IMAGE_MAX_WIDTH {
                    let scaled = img.resize(IMAGE_MAX_WIDTH, u32::MAX, image::imageops::FilterType::Triangle);
                    let mut out: Vec<u8> = Vec::new();
                    let format = if mime == "image/jpeg" {
                        image::ImageFormat::Jpeg
                    } else {
                        image::ImageFormat::Png
                    };
                    let mut cursor = std::io::Cursor::new(&mut out);
                    if scaled.write_to(&mut cursor, format).is_ok() && !out.is_empty() {
                        let tmp = path.with_extension("tmpopt");
                        if std::fs::write(&tmp, &out).is_ok() {
                            let _ = std::fs::rename(&tmp, path);
                        } else {
                            let _ = std::fs::remove_file(&tmp);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    // Mark as checked even when no rewrite was needed (decode was a no-op).
    let _ = std::fs::File::create(&marker);
}

/// Optimizer for bytes we already hold in memory (fresh fetch).
fn spawn_optimize_raw(path: PathBuf, bytes: Vec<u8>, mime: String) {
    tokio::task::spawn_blocking(move || {
        optimize_from_bytes(&path, &bytes, &mime);
    });
}

/// Optimizer for cache hits (reopens): reads the file on a blocking thread.
fn spawn_optimize_file(path: PathBuf) {
    if path.with_extension("opt").exists() {
        return;
    }
    tokio::task::spawn_blocking(move || {
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => return,
        };
        let mime = sniff_magic(&bytes).unwrap_or("image/png").to_string();
        optimize_from_bytes(&path, &bytes, &mime);
    });
}

// ==========================================
// Per-URL task dedup + cache pipeline
// ==========================================

fn image_task_cell(url: &str) -> Arc<OnceCell<String>> {
    let map = IMAGE_TASKS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut guard = match map.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard
        .entry(url.to_string())
        .or_insert_with(|| Arc::new(OnceCell::new()))
        .clone()
}

/// Cache lookup -> SSRF-safe fetch -> RAW write (fast path) -> background
/// optimize. Returns the cache path on success or the fallback data URI.
async fn fetch_and_cache(url: String, dir: PathBuf) -> String {
    let hash = cache_url_hash(&url);
    if let Some(path) = lookup_cached_image(&dir, &hash) {
        spawn_optimize_file(path.clone());
        return path.to_string_lossy().into_owned();
    }
    match fetch_remote_image(&url).await {
        Some((bytes, mime)) => {
            let ext = ext_for_mime(&mime);
            let path = dir.join(format!("{}.{}", hash, ext));
            if tokio::fs::create_dir_all(&dir).await.is_err() {
                return fallback_pixel();
            }
            // FAST PATH: raw bytes hit disk now; the webview can stream them
            // immediately. Downscaling happens behind us (Lever 3).
            if tokio::fs::write(&path, &bytes).await.is_err() {
                return fallback_pixel();
            }
            let path_str = path.to_string_lossy().into_owned();
            spawn_optimize_raw(path, bytes, mime);
            path_str
        }
        None => fallback_pixel(),
    }
}

async fn proxy_one_image(url: String, dir: PathBuf) -> (String, String) {
    let cell = image_task_cell(&url);
    let value = cell
        .get_or_init(|| fetch_and_cache(url.clone(), dir.clone()))
        .await
        .clone();
    (url, value)
}

fn capped_unique(urls: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    urls.into_iter()
        .filter(|u| seen.insert(u.clone()))
        .take(IMAGE_BATCH_MAX_URLS)
        .collect()
}

// ==========================================
// Commands
// ==========================================

/// Polls the background database initialization task to determine if the UI can begin rendering.
#[tauri::command]
#[specta::specta]
pub async fn check_db_ready(state: State<'_, AppState>) -> Result<bool, AppError> {
    let is_ready = state.pool.get().is_some();
    tracing::info!("[BACKEND] check_db_ready called. Pool ready: {}", is_ready);
    Ok(is_ready)
}

/// Fetches a single remote image through the SSRF-safe proxy (retry path).
/// Returns a cache path (frontend converts to asset URL) or the fallback pixel.
#[tauri::command]
#[specta::specta]
pub async fn proxy_remote_image(
    state: State<'_, AppState>,
    url: String,
) -> Result<String, AppError> {
    let dir = image_cache_dir(&state)?;
    Ok(proxy_one_image(url, dir).await.1)
}

/// Batched proxy for viewport-triggered image batches. One IPC round-trip,
/// bounded concurrency, per-URL disk cache, session-wide fetch dedup.
#[tauri::command]
#[specta::specta]
pub async fn proxy_remote_images(
    state: State<'_, AppState>,
    urls: Vec<String>,
) -> Result<HashMap<String, String>, AppError> {
    let dir = image_cache_dir(&state)?;
    let unique = capped_unique(urls);
    if unique.is_empty() {
        return Ok(HashMap::new());
    }
    let results = futures::stream::iter(unique.into_iter().map(|u| {
        let d = dir.clone();
        async move { proxy_one_image(u, d).await }
    }))
    .buffer_unordered(IMAGE_BATCH_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;
    Ok(results.into_iter().collect())
}

/// Lever 1: fire-and-forget prewarm of the image disk cache for every remote
/// image in an email. Returns immediately; fetching runs in a background task
/// in DOM order (buffer_unordered pulls the iterator in order, so above-fold
/// images start first). The viewport observer then almost always hits cache.
#[tauri::command]
#[specta::specta]
pub async fn prewarm_image_cache(
    state: State<'_, AppState>,
    urls: Vec<String>,
) -> Result<(), AppError> {
    let dir = image_cache_dir(&state)?;
    let unique = capped_unique(urls);
    if unique.is_empty() {
        return Ok(());
    }
    tauri::async_runtime::spawn(async move {
        let _ = futures::stream::iter(unique.into_iter().map(|u| {
            let d = dir.clone();
            async move { proxy_one_image(u, d).await }
        }))
        .buffer_unordered(IMAGE_BATCH_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
        tracing::info!("[IMAGE-PROXY] prewarm pass complete");
    });
    Ok(())
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
            let badge = if unread_count > 0 { Some(unread_count) } else { None };
            let _ = window.set_badge_count(badge);
        }
        #[cfg(target_os = "windows")]
        {
            if unread_count > 0 {
                let _ = window.set_title(&format!("Littmaily ({})", unread_count));
            } else {
                let _ = window.set_title("Littmaily");
            }
        }
    }
    Ok(())
}
