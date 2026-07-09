# Littmaily 📬

**Littmaily** is a local-first, privacy-focused desktop email, calendar, and contacts client built with **Rust (Tauri v2)**, **SolidJS**, and **SQLCipher**. 

Designed for users who demand absolute privacy and speed, Littmaily encrypts your local database, proxies remote images to prevent tracking, handles complex CalDAV/CardDAV syncs seamlessly in the background, and provides a premium "Superhuman-style" UI.

## ✨ Key Features

### 📧 Advanced Email Engine
- **Smart Threading:** Groups emails by `References` / `In-Reply-To` headers with gateway suffix normalization (RFC 5256).
- **Optimistic UI + Offline Queue:** Instant UI updates for read/star/delete/move. Actions are queued locally and synced via a background worker when connectivity is restored.
- **Sync Windows:** Configurable retention policies (e.g., 30 days, 6 months) with automatic local pruning.
- **Scheduled Sending:** 10-second undo-send delay managed by the Rust outbox worker.

### 🛡️ Hardcore Privacy & Security
- **SSRF-Protected Image Proxy:** Remote images are fetched via a hardened Rust proxy that blocks IANA reserved ranges, enforces 5MB limits, and uses magic-byte sniffing to verify true MIME types.
- **AES-256-GCM Blob Store:** Content-addressed, deduplicated attachment storage with automatic garbage collection.
- **SQLCipher Encryption:** The entire local database is encrypted at rest, with keys securely stored in the OS-native Keychain (`keyring`).
- **HTML Sanitization:** Ammonia-powered HTML sanitization with CID-to-data-URI rewriting to neutralize tracking pixels and malicious payloads.

### 📅 CalDAV & CardDAV
- **Robust Sync Engines:** Full support for RFC 6578 (`sync-collection`) and RFC 6764 discovery chains.
- **Fault-Tolerant Parsing:** Custom vCard/iCalendar parsers that automatically unfold malformed long lines (e.g., broken UIDs) commonly found in legacy servers.

### 🎨 Premium UI/UX
- **Virtualized Rendering:** Blazing-fast email lists using `@tanstack/solid-virtual`.
- **Native Gestures:** Swipe-to-delete, spring-physics zoom controls, and full keyboard navigation (j/k/enter).
- **Unified FTS5 Search:** Concurrent full-text search with snippet highlighting across emails, calendar events, and contacts simultaneously.
- **Rich Text Editing:** Integrated Quill editor for composing beautiful HTML emails.

---

## 📂 Repository Structure & Licensing Model

Littmaily utilizes a **dual-licensing model** to balance community contribution with commercial protection.

### 🟢 MIT Licensed (Open Source Protocol Engines)
We encourage the Rust community to use, fork, and contribute to these networking and parsing engines.
* [`crates/email-core/`](./crates/email-core) - Async IMAP/SMTP, OAuth2 PKCE, MIME parsing, and Threading.
* [`crates/caldav/`](./crates/caldav) - CalDAV client, XML sync engine, and iCalendar parsing.
* [`crates/carddav/`](./crates/carddav) - CardDAV client, XML sync engine, and vCard parsing.

### 🔴 BSL 1.1 Licensed (The Product & Secret Sauce)
The core application, local-first architecture, and UI are protected under the **Business Source License (BSL) 1.1**. This prevents competitors from forking the product to build rival commercial clients, while guaranteeing that the code will eventually become Open Source (Apache 2.0) on the Change Date (**2030-07-09**).
* `crates/storage/` - SQLCipher schema, FTS5 triggers, Repositories, Blob Store.
* `crates/crypto/` - AES-256-GCM encryption, OS Keychain integration.
* `crates/tauri-app/` - Tauri IPC bridge (via `tauri-specta`), background sync workers.
* `crates/notes/` & `crates/alarm/` - App-specific features.
* `frontend/` - SolidJS UI, Tailwind styling, Components.

*For alternative commercial licensing of the BSL components, please contact solomonocheni37@gmail.com.*

---

## 🛠️ Getting Started

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (Edition 2024)
- [Node.js](https://nodejs.org/) (v20+) & npm
- [Tauri v2 CLI](https://v2.tauri.app/start/prerequisites/)
- **Linux users:** Ensure you have `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, and `libssl-dev` installed.

### Setup & Run

1. **Clone the repository**
   ```bash
   git clone https://github.com/solomonocheni37-eng/littmaily.git
   cd littmaily
