# Changelog

## v0.1.11 (Latest)

### ✨ New Features
- **Resilient Image Loading:** Remote images that fail to load (flaky network,
  SSRF block, size limit) now render as clickable dashed placeholders instead
  of disappearing. Click a single broken image to retry it, or use the new
  "Retry all" banner to re-fetch everything at once.
- **Reliable Zoom Reset:** The zoom reset control now works consistently at any
  zoom level and re-syncs correctly when the message list pane is collapsed or
  expanded.
- **Load Images feedback:** The "Load Images" button now shows a live spinner
  and disables while images are being proxied.

### 🚀 Performance
- **Buttery-smooth scrolling:** Removed a global CSS transition rule that
  animated every virtualized row reposition, eliminated the animated iframe
  height resize, and batched resize messages to one per frame. Both the email
  list and reading pane now scroll as pure compositor operations.
- Added CSS containment and compositor hints to the list and reading panes.

### ⚙️ Backend Performance (Rust)
- **Reduced RAM usage by ~70–90 MB:** Lowered SQLCipher page cache from 20 MB
  to 4 MB per connection, reduced the connection pool from 5 to 2 connections,
  and added a 64 MB `mmap_size` pragma so cold pages are managed by the OS
  page cache (reclaimable under memory pressure) instead of SQLite's private
  cache.
- **Eliminated O(depth) allocations in MIME traversal:** Rewrote
  `get_filenames_recursive` and `traverse_bodystructure` from recursive to
  iterative stack-based traversal, removing one intermediate `Vec` allocation
  per nesting level on deeply nested multipart messages.
- **Reduced per-message allocations during sync:**
  - `clean_message_id` now truncates in-place (1 allocation instead of 3).
  - `normalize_subject` uses `eq_ignore_ascii_case` + `drain` instead of
    allocating a lowercased copy per loop iteration.
  - `decode_mime_header` pre-allocates result capacity to the input length.
  - `parse_references_header` pre-allocates the refs Vec and current-id buffer.
  - `extract_snippet` uses a new single-pass `collapse_whitespace` helper,
    eliminating the intermediate `Vec<&str>` from `split_whitespace().collect()`
    and the second pass from `.take().collect()`.
- **Pre-allocated sync worker buffers:** `new_headers` (capacity 64) and
  `updates` (capacity 256) in the IMAP sync worker avoid repeated
  reallocation during bulk fetches.
- **Release profile tuning:** Added `[profile.release]` with `lto = "fat"`,
  `codegen-units = 1`, `panic = "abort"`, and `strip = "symbols"` to the
  workspace, enabling cross-crate inlining (e.g. `clean_message_id` inlined
  into `fetch_headers`) and a smaller release binary.
- **Cold error paths:** Marked all `From<…> for AppError` conversions `#[cold]`
  so LLVM keeps error-handling code out of the hot instruction stream.
- **Bounds-safe magic-byte checks:** Replaced `&bytes[8..12]` slice indexing
  with `bytes.get(8..12)` in the image proxy and MIME sanitizer, eliminating
  a panic path the compiler had to guard.
- **Explicit ciphertext drop:** `BlobStore::load` now explicitly drops the
  encrypted buffer before returning plaintext, documenting the non-overlap
  guarantee for large attachments.

### 🐛 Bug Fixes
- Fixed ~188 "Refused to apply a stylesheet" errors and "non CSS MIME types"
  errors in the email viewer by removing the Content Security Policy that
  WebKitGTK was mis-applying to the sandboxed iframe, and by correctly
  stripping entire `@import` rules during HTML sanitization.
- Fixed a circular import that broke the frontend TypeScript build.

### 🔒 Security
- Email iframe isolation now relies on `sandbox="allow-scripts"` (without
  `allow-same-origin`) plus the Rust ammonia/lol_html sanitization pipeline,
  replacing the problematic CSP. No reduction in protection.

## v0.1.9

### ✨ New Features
- **Lazy Attachment Loading:** Email bodies now load instantly. Large
  attachments are deferred and downloaded on-demand via IMAP partial fetching,
  saving massive amounts of bandwidth.
- **JSON Body Caching:** Re-opening previously viewed emails is now
  instantaneous (0ms parsing overhead) via encrypted local JSON caching.
- **Instant Background Sync UI:** The UI now updates in <100ms when the Rust
  background worker detects new emails via IMAP IDLE.

### 🐛 Bug Fixes
- Fixed a WebKitGTK crash on Linux Wayland by forcing X11/XWayland and
  disabling DMA-BUF.
- Resolved an issue where FTS5 search indexes could corrupt during rapid bulk
  syncs.

## v0.1.8

### ✨ New Features
- **Smart Threading:** Emails are now automatically grouped by `References`
  and `In-Reply-To` headers, with gateway suffix normalization.
- **Swipe-to-Delete:** Native gesture support for quickly archiving or
  deleting emails from the list pane.
- **Unified FTS5 Search:** Concurrent full-text search across emails, calendar
  events, and contacts with snippet highlighting.

## v0.1.0

- 🎉 Initial Release of Littmaily!
