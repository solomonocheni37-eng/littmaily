# Littmaily 📬

[![CI](https://github.com/solomonocheni37-eng/littmaily/actions/workflows/ci.yml/badge.svg)](https://github.com/solomonocheni37-eng/littmaily/actions/workflows/ci.yml)

**Littmaily** is a local-first, privacy-focused desktop email, calendar, and contacts client built with **Rust (Tauri v2)**, **SolidJS**, and **SQLCipher**.

Your mailbox is encrypted at rest on your machine, remote images are proxied through a hardened SSRF-safe backend so trackers never see you, and the UI is engineered for compositor-driven 60fps scrolling on every platform — including Linux/WebKitGTK.

---

## ✨ Key Features

### 📧 Advanced Email Engine
- **Smart Threading:** Groups emails by `References` / `In-Reply-To` headers with gateway suffix normalization (Gmail/Outlook mutations handled).
- **Optimistic UI + Offline Queue:** Instant local updates for read/star/delete/move/archive; failed IMAP operations queue in `pending_actions` and retry on reconnect.
- **Sync Windows:** Configurable retention (30 days / 6 months / 100 messages / everything) with automatic local pruning and blob GC.
- **Scheduled Sending:** 10-second undo-send delay managed by the Rust outbox worker (retry-capped at 5 attempts).
- **Virtualized List:** `@tanstack/solid-virtual` rendering with viewport-snippet prefetch (IMAP partial fetch, first 2KB only).

### 📖 Reading Pane — Shadow DOM Renderer
- Email HTML renders inside a **Shadow DOM** attached to a plain `<div>`: full style isolation *without* iframe cross-context compositing costs or postMessage latency.
- **Compositor-friendly scrolling:** nothing inside the scroll container is force-promoted to a GPU layer at rest; `will-change: transform` exists only transiently during the 350ms zoom animation.
- Zoom (50–200%) with math-only dimension updates from cached base measurements; hand-tool pan mode; scroll-anchor compensation keeps your viewport stable when late-loading images grow content above it.
- `mailto:` links open a prefilled Compose window; external links require explicit user confirmation.
- Accessible: the Shadow host is a labelled `region`; failed images are clickable, dashed retry placeholders.

### 🖼️ Private Image Pipeline
Remote images **never** load directly in the webview:

1. **Rewrite:** the Rust parser replaces remote `src`/`background` with inert 1×1 placeholders (`data-src` keeps the original URL).
2. **Opt-in:** nothing fetches until the user clicks *Load Images* (privacy invariant).
3. **Prewarm + lazy display:** one fire-and-forget `prewarm_image_cache` call fills the disk cache for the whole email in DOM order; an IntersectionObserver (600px prefetch margin) displays images as they approach the viewport — usually straight from disk.
4. **Proxy:** SSRF-validated on every redirect hop (IANA-reserved ranges blocked), 5MB cap, magic-byte MIME sniffing, per-URL session dedup, bounded concurrency (16).
5. **Deliver:** bytes are written to `$APPLOCALDATA/littmaily/cache/<sha256(url)>.<ext>` and streamed to the webview via the **Tauri asset protocol** — no base64 strings in IPC or the DOM.
6. **Optimize:** oversized JPEG/PNG (>2048px) are downscaled on a background blocking thread and swapped in atomically (temp file + rename, `.opt` sidecar marker).

Reopening an email is instant and offline-capable: the cache is URL-keyed and shared across sessions.

### 🛡️ Hardcore Privacy & Security
- **Three-layer HTML sanitization (defense in depth):**
  1. *Rust:* ammonia allowlists (tags/attributes/URL schemes) + lol_html DOM rewriting (remote images → placeholders, VML/XML namespace stripping) + CSS scrubbing (`@import`, `url()`, `position: fixed`, `z-index`).
  2. *Frontend:* DOM pass removing `<script>`/`<noscript>` (incl. SVG namespace), every `on*` handler attribute, and `javascript:` URLs.
  3. *Runtime:* MutationObserver watchdog inside the Shadow DOM removes any script node injected after render.
- **No CSP — deliberate, documented tradeoff:** WebKitGTK's srcdoc/CSP handling produced endless stylesheet refusals; the three layers above replace it. If the ammonia configuration ever changes, re-run script-injection tests against the reading pane.
- **Encryption at rest:** SQLCipher (AES-256-GCM) database; master key in the OS-native keychain; content-addressed, deduplicated, encrypted blob store with garbage collection.
- **Attachment safety:** Unicode bidi (RTLO) spoof stripping, MIME-forced extensions, executable-download confirmation dialog.
- **Local image cache is plaintext by design** (the asset protocol streams it); it never leaves the machine. See `PRIVACY.md`.

### 📅 CalDAV & CardDAV
- RFC 6578 (`sync-collection`) incremental sync, RFC 6764 discovery chains.
- Fault-tolerant parsers that unfold malformed long lines (broken UIDs) from legacy servers.

### 🎨 Premium UI/UX
- Swipe-to-delete gestures, full keyboard navigation (`j/k/enter/o/e`, `mod+k` search), unified FTS5 search with snippet highlighting, Quill rich-text compose with draft autosave and undo-send.

---

## 🏗️ Architecture

```
┌──────────────────────────── Frontend (SolidJS) ────────────────────────────┐
│ Views → Hooks → IPC wrappers (core/ipc/*) → types/generated.ts (specta)    │
│ Reading pane: Shadow DOM host <div> + IntersectionObserver lazy images     │
│ List pane: virtualized rows, contained paint (contain: layout style paint) │
└──────────────────────────────────┬─────────────────────────────────────────┘
                                   │ typed Tauri IPC (tauri-specta)
┌──────────────────────────────────▼─────────────────────────────────────────┐
│ crates/tauri-app   commands, background workers, image proxy, sanitization │
│ crates/email-core  async IMAP/SMTP, OAuth2 PKCE, MIME, threading   (MIT)    │
│ crates/caldav      CalDAV client + sync engine                     (MIT)    │
│ crates/carddav     CardDAV client + sync engine                    (MIT)    │
│ crates/storage     SQLCipher schema, FTS5 triggers, repos, blobs   (BSL)    │
│ crates/crypto      AES-256-GCM + OS keychain                       (BSL)    │
└────────────────────────────────────────────────────────────────────────────┘
```

### IPC contract
Every command is annotated `#[tauri::command] #[specta::specta]`. `frontend/src/core/types/generated.ts` is **generated, never hand-edited**: after changing any command, run

```bash
cd crates/tauri-app && cargo test export_typescript_types
```

CI enforces this with a drift gate (see below).

### Performance invariants (do not regress)
- **Never** add `translateZ(0)` / permanent `will-change` inside either scroll container. On Linux (DMA-BUF disabled for crash safety) giant promoted layers re-raster during scroll and destroy realtime scrolling.
- Layer promotion is allowed only transiently (zoom animation), released afterwards.
- Batch all layout reads before writes; the zoom animation loop must stay math-only (base dimensions cached, updated by ResizeObserver).
- The inbox list stays virtualized; never render the full inbox DOM.
- Image decode and DNS resolution stay off the Rust async workers (`spawn_blocking` / async DNS).

---

## 📂 Repository Structure & Licensing Model

Littmaily uses a **dual-licensing model** to balance community contribution with commercial protection.

### 🟢 MIT Licensed (Open Source Protocol Engines)
* [`crates/email-core/`](./crates/email-core) — Async IMAP/SMTP, OAuth2 PKCE, MIME parsing, threading.
* [`crates/caldav/`](./crates/caldav) — CalDAV client, XML sync engine, iCalendar parsing.
* [`crates/carddav/`](./crates/carddav) — CardDAV client, XML sync engine, vCard parsing.

### 🔴 BSL 1.1 Licensed (The Product & Secret Sauce)
* `crates/storage/`, `crates/crypto/`, `crates/tauri-app/`, `frontend/`
* Change Date: **2030-07-09** (then Apache-2.0). Alternative commercial licensing: solomonocheni37@gmail.com.

---

## 🛠️ Getting Started

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (Edition 2024)
- [Node.js](https://nodejs.org/) v20.19+ / v22+
- [Tauri v2 CLI](https://v2.tauri.app/start/prerequisites/)
- **Linux:** `sudo apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev`

### Run
```bash
git clone https://github.com/solomonocheni37-eng/littmaily.git
cd littmaily
npm install --prefix frontend
npm run dev          # cargo tauri dev (frontend HMR + Rust backend)
```

### Production build
```bash
npm run build        # cargo tauri build (runs frontend:build first)
```

---

## ✅ Testing & CI

| Suite | Command | Gates |
|---|---|---|
| Frontend types + bundle | `npm run frontend:build` (`tsc -b && vite build`) | strict TS, no unused locals |
| Protocol crates | `cargo test --workspace --exclude tauri-app` | BDD unit/integration tests (network tests self-skip without creds) |
| App crate | `cargo test -p tauri-app` | outbox worker state machine + specta TS export |
| Bindings drift | `git diff --exit-code -- frontend/src/core/types/generated.ts` | IPC contract stays in sync |

GitHub Actions (`.github/workflows/ci.yml`) runs all of the above on every push/PR:
- **frontend** — Node 22, `npm ci`, typecheck + build.
- **rust-libs** — tests for email-core / caldav / carddav / storage / crypto.
- **tauri-app** — WebKitGTK system deps, workspace clippy (ratchet: warnings tolerated until legacy cleanup lands), tests, specta drift gate.

`cargo clippy -D warnings` and `cargo fmt --check` are intentionally **not** gates yet; clean the legacy surface in a dedicated commit, then tighten (ratchet rule).

---

##  Design Decisions & Tradeoffs
- **No CSP in the reading pane** (WebKitGTK stylesheet refusals); replaced by three sanitization layers.
- **Plaintext local image cache** so the asset protocol can stream it; local-only, documented in `PRIVACY.md`.
- **DMA-BUF renderer disabled on Linux** to avoid Nvidia/Mesa segfaults; X11/XWayland forced to keep the hardware compositor for smooth scrolling.
- **`generated.ts` is generated code**; the specta export test is the single source of truth for the IPC contract.

---

## 📄 License
Business Source License 1.1 (product crates + frontend) / MIT (protocol engines). See [LICENSE](./LICENSE) and [CONTRIBUTING.md](./CONTRIBUTING.md).
