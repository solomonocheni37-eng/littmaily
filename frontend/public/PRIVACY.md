# Littmaily Privacy Policy

**Last Updated:** July 2026

Littmaily is a local-first, privacy-focused desktop application. We believe your emails, contacts, and calendar events belong to you, not to us. 

## 1. Zero Telemetry & No Analytics
Littmaily does not collect, transmit, or store any usage data, analytics, or crash reports. We have no servers, and we do not track your behavior within the app.

## 2. Local-First Encrypted Storage
All your data (emails, attachments, contacts, calendar events) is downloaded and stored exclusively on your local device. 
- **Database Encryption:** The local SQLite database is encrypted at rest using **SQLCipher** (AES-256-GCM).
- **Key Management:** The master encryption key is securely generated and stored in your operating system's native credential manager (macOS Keychain, Windows Credential Manager, or Linux Secret Service).
- **Blob Store:** Attachments are stored in a content-addressed, encrypted blob store on your local disk.

## 3. Network Connections
Littmaily only connects to the internet to communicate with the email, calendar, and contacts providers you explicitly configure (e.g., Gmail, Outlook, iCloud, or your custom IMAP/CalDAV server).
- **OAuth2 Authentication:** When using "Sign in with Browser", OAuth tokens are exchanged directly between your browser and the provider. Tokens are saved locally and never touch Littmaily servers.
- **SSRF-Protected Image Proxy:** To prevent email tracking pixels and malicious payloads, remote images are not loaded directly by the UI. Instead, they are fetched via a hardened Rust backend proxy that strips trackers, enforces strict MIME-type validation, and blocks Server-Side Request Forgery (SSRF) attacks against your local network.

## 4. Deep Links
Littmaily registers a custom URI scheme (`littmaily://`) solely to intercept OAuth2 redirect callbacks from your system browser. This callback is processed entirely locally and is never transmitted elsewhere.

## 5. Open Source & Auditable
Littmaily's core protocol engines and cryptography implementations are open-source (MIT Licensed) and available for public audit on GitHub.

This can be updated anytime.
