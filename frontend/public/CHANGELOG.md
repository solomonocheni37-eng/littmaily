# Changelog

## v0.1.9 (Latest)
### ✨ New Features
- **Lazy Attachment Loading:** Email bodies now load instantly. Large attachments are deferred and downloaded on-demand via IMAP partial fetching, saving massive amounts of bandwidth.
- **JSON Body Caching:** Re-opening previously viewed emails is now instantaneous (0ms parsing overhead) via encrypted local JSON caching.
- **Instant Background Sync UI:** The UI now updates in <100ms when the Rust background worker detects new emails via IMAP IDLE.

### 🐛 Bug Fixes
- Fixed a WebKitGTK crash on Linux Wayland by forcing X11/XWayland and disabling DMA-BUF.
- Resolved an issue where FTS5 search indexes could corrupt during rapid bulk syncs.

## v0.1.8
### ✨ New Features
- **Smart Threading:** Emails are now automatically grouped by `References` and `In-Reply-To` headers, with gateway suffix normalization.
- **Swipe-to-Delete:** Native gesture support for quickly archiving or deleting emails from the list pane.
- **Unified FTS5 Search:** Concurrent full-text search across emails, calendar events, and contacts with snippet highlighting.

## v0.1.0
- 🎉 Initial Release of Littmaily!
