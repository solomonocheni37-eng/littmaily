# Contributing to Littmaily

Thank you for your interest in contributing to Littmaily! Whether you are fixing a bug in our MIME parser, adding support for a quirky CalDAV server, or improving the SolidJS frontend, your help is highly appreciated.

## ⚖️ The Dual-License Model (Important)

Littmaily uses a dual-licensing model. **By submitting a Pull Request, you agree to license your contribution under the license applicable to the specific directory you are modifying.**

### 🟢 MIT Licensed (Protocol Engines)
The following crates are open-source under the **MIT License**. We highly encourage community forks, usage, and contributions to these networking engines:
* `crates/email-core/` (IMAP, SMTP, OAuth2, MIME, Threading)
* `crates/caldav/` (CalDAV sync, iCalendar parsing)
* `crates/carddav/` (CardDAV sync, vCard parsing)

### 🔴 BSL 1.1 Licensed (Product & Secret Sauce)
The core application, local-first architecture, and UI are protected under the **Business Source License (BSL 1.1)**. 
* `crates/storage/`, `crates/crypto/`, `crates/tauri-app/`, `crates/notes/`, `crates/alarm/`
* `frontend/` (SolidJS UI)

*By contributing to these directories, you agree that your contributions are licensed under the BSL 1.1. This prevents competitors from forking the product to build rival commercial clients, while guaranteeing that the code will eventually become Open Source (Apache 2.0) on the Change Date (2030-07-09).*

---

## 🛠️ Development Setup

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (Edition 2024)
- [Node.js](https://nodejs.org/) (v20+) & npm
- [Tauri v2 CLI](https://v2.tauri.app/start/prerequisites/)
- **Linux users:** Ensure you have the WebKit and GTK headers installed:
  ```bash
  git clone https://github.com/solomonocheni37-eng/littmaily.git
  sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
