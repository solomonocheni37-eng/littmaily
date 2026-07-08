// FILE: ./crates/storage/src/db.rs
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

// FTS5 virtual tables are configured as "external content" tables.
// They don't store data themselves; instead, triggers keep them in sync with the main tables.
const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
id TEXT PRIMARY KEY, email TEXT NOT NULL UNIQUE, provider TEXT NOT NULL,
imap_host TEXT NOT NULL, imap_port INTEGER NOT NULL,
smtp_host TEXT NOT NULL, smtp_port INTEGER NOT NULL, created_at INTEGER NOT NULL,
auth_method TEXT NOT NULL DEFAULT 'password',
oauth_client_id TEXT, oauth_client_secret TEXT, oauth_token_url TEXT,
sync_window TEXT NOT NULL DEFAULT 'LAST_30_DAYS'
);
CREATE TABLE IF NOT EXISTS mailboxes (
id INTEGER PRIMARY KEY AUTOINCREMENT, account_id TEXT NOT NULL, name TEXT NOT NULL,
delimiter TEXT, attributes TEXT NOT NULL, uid_validity INTEGER, uid_next INTEGER, highest_modseq INTEGER,
FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE CASCADE, UNIQUE(account_id, name)
);
CREATE TABLE IF NOT EXISTS messages (
id INTEGER PRIMARY KEY AUTOINCREMENT, account_id TEXT NOT NULL, mailbox_name TEXT NOT NULL,
uid INTEGER NOT NULL, subject TEXT, sender TEXT, date TEXT, date_timestamp INTEGER, flags TEXT NOT NULL,
size INTEGER NOT NULL, has_attachments BOOLEAN NOT NULL DEFAULT 0, snippet TEXT,
blob_hash TEXT, attachment_names TEXT,
message_id TEXT, in_reply_to TEXT, references_json TEXT, thread_id TEXT, thread_subject TEXT,
FOREIGN KEY(account_id, mailbox_name) REFERENCES mailboxes(account_id, name) ON DELETE CASCADE,
UNIQUE(account_id, mailbox_name, uid)
);
CREATE TABLE IF NOT EXISTS attachments (
id INTEGER PRIMARY KEY AUTOINCREMENT, message_id INTEGER NOT NULL, filename TEXT,
mime_type TEXT NOT NULL, size INTEGER NOT NULL, blob_hash TEXT NOT NULL,
FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS outbox (
id INTEGER PRIMARY KEY AUTOINCREMENT, account_id TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending',
retry_count INTEGER NOT NULL DEFAULT 0, last_error TEXT, raw_mime BLOB NOT NULL,
envelope_from TEXT NOT NULL, envelope_to TEXT NOT NULL, subject TEXT, body TEXT, created_at INTEGER NOT NULL, sent_at INTEGER,
FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS calendars (
id INTEGER PRIMARY KEY AUTOINCREMENT, account_id TEXT NOT NULL, url TEXT NOT NULL,
display_name TEXT NOT NULL, ctag TEXT, sync_token TEXT,
FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE CASCADE, UNIQUE(account_id, url)
);
CREATE TABLE IF NOT EXISTS calendar_events (
id INTEGER PRIMARY KEY AUTOINCREMENT, calendar_id INTEGER NOT NULL, uid TEXT NOT NULL,
etag TEXT NOT NULL, url TEXT NOT NULL, ical_data TEXT NOT NULL, last_modified INTEGER,
FOREIGN KEY(calendar_id) REFERENCES calendars(id) ON DELETE CASCADE, UNIQUE(calendar_id, url)
);
CREATE TABLE IF NOT EXISTS address_books (
id INTEGER PRIMARY KEY AUTOINCREMENT, account_id TEXT NOT NULL, url TEXT NOT NULL,
display_name TEXT NOT NULL, ctag TEXT, sync_token TEXT,
FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE CASCADE, UNIQUE(account_id, url)
);
CREATE TABLE IF NOT EXISTS contacts (
id INTEGER PRIMARY KEY AUTOINCREMENT, address_book_id INTEGER NOT NULL, uid TEXT NOT NULL,
etag TEXT NOT NULL, url TEXT NOT NULL, vcard_data TEXT NOT NULL, last_modified INTEGER,
FOREIGN KEY(address_book_id) REFERENCES address_books(id) ON DELETE CASCADE, UNIQUE(address_book_id, url)
);
CREATE TABLE IF NOT EXISTS pending_actions (
id INTEGER PRIMARY KEY AUTOINCREMENT,
account_id TEXT NOT NULL,
mailbox_name TEXT NOT NULL,
uid INTEGER NOT NULL,
action TEXT NOT NULL,
dest_mailbox TEXT,
created_at INTEGER NOT NULL,
FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE CASCADE
);
CREATE VIRTUAL TABLE IF NOT EXISTS email_fts USING fts5(
subject, sender, snippet, attachment_names,
content='messages', content_rowid='id'
);
CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
INSERT INTO email_fts(rowid, subject, sender, snippet, attachment_names) VALUES (new.id, new.subject, new.sender, new.snippet, new.attachment_names);
END;
CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
INSERT INTO email_fts(email_fts, rowid, subject, sender, snippet, attachment_names) VALUES('delete', old.id, old.subject, old.sender, old.snippet, old.attachment_names);
END;
CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
INSERT INTO email_fts(email_fts, rowid, subject, sender, snippet, attachment_names) VALUES('delete', old.id, old.subject, old.sender, old.snippet, old.attachment_names);
INSERT INTO email_fts(rowid, subject, sender, snippet, attachment_names) VALUES (new.id, new.subject, new.sender, new.snippet, new.attachment_names);
END;
CREATE VIRTUAL TABLE IF NOT EXISTS calendar_events_fts USING fts5(ical_data, content='calendar_events', content_rowid='id');
CREATE TRIGGER IF NOT EXISTS calendar_events_ai AFTER INSERT ON calendar_events BEGIN INSERT INTO calendar_events_fts(rowid, ical_data) VALUES (new.id, new.ical_data); END;
CREATE TRIGGER IF NOT EXISTS calendar_events_ad AFTER DELETE ON calendar_events BEGIN INSERT INTO calendar_events_fts(calendar_events_fts, rowid, ical_data) VALUES('delete', old.id, old.ical_data); END;
CREATE TRIGGER IF NOT EXISTS calendar_events_au AFTER UPDATE ON calendar_events BEGIN INSERT INTO calendar_events_fts(calendar_events_fts, rowid, ical_data) VALUES('delete', old.id, old.ical_data); INSERT INTO calendar_events_fts(rowid, ical_data) VALUES (new.id, new.ical_data); END;
CREATE VIRTUAL TABLE IF NOT EXISTS contacts_fts USING fts5(vcard_data, content='contacts', content_rowid='id');
CREATE TRIGGER IF NOT EXISTS contacts_ai AFTER INSERT ON contacts BEGIN INSERT INTO contacts_fts(rowid, vcard_data) VALUES (new.id, new.vcard_data); END;
CREATE TRIGGER IF NOT EXISTS contacts_ad AFTER DELETE ON contacts BEGIN INSERT INTO contacts_fts(contacts_fts, rowid, vcard_data) VALUES('delete', old.id, old.vcard_data); END;
CREATE TRIGGER IF NOT EXISTS contacts_au AFTER UPDATE ON contacts BEGIN INSERT INTO contacts_fts(contacts_fts, rowid, vcard_data) VALUES('delete', old.id, old.vcard_data); INSERT INTO contacts_fts(rowid, vcard_data) VALUES (new.id, new.vcard_data); END;
CREATE INDEX IF NOT EXISTS idx_outbox_status_created ON outbox(status, created_at);
"#;

pub struct Migration {
    pub version: i64,
    pub description: &'static str,
    pub sql: &'static str,
}

pub fn get_migrations() -> Vec<Migration> {
    vec![
        Migration { version: 2, description: "Baseline versioning", sql: "" },
        Migration {
            version: 3,
            description: "FTS & Attachment Names",
            sql: r#"
ALTER TABLE messages ADD COLUMN attachment_names TEXT;
DROP TABLE IF EXISTS email_fts;
CREATE VIRTUAL TABLE email_fts USING fts5(subject, sender, snippet, attachment_names, content='messages', content_rowid='id');
CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN INSERT INTO email_fts(rowid, subject, sender, snippet, attachment_names) VALUES (new.id, new.subject, new.sender, new.snippet, new.attachment_names); END;
CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN INSERT INTO email_fts(email_fts, rowid, subject, sender, snippet, attachment_names) VALUES('delete', old.id, old.subject, old.sender, old.snippet, old.attachment_names); END;
CREATE TRIGGER messages_au AFTER UPDATE ON messages BEGIN INSERT INTO email_fts(email_fts, rowid, subject, sender, snippet, attachment_names) VALUES('delete', old.id, old.subject, old.sender, old.snippet, old.attachment_names); INSERT INTO email_fts(rowid, subject, sender, snippet, attachment_names) VALUES (new.id, new.subject, new.sender, new.snippet, new.attachment_names); END;
"#,
        },
        Migration {
            version: 4,
            description: "OAuth Support",
            sql: r#"
ALTER TABLE accounts ADD COLUMN auth_method TEXT NOT NULL DEFAULT 'password';
ALTER TABLE accounts ADD COLUMN oauth_client_id TEXT;
ALTER TABLE accounts ADD COLUMN oauth_client_secret TEXT;
ALTER TABLE accounts ADD COLUMN oauth_token_url TEXT;
"#,
        },
        Migration { version: 5, description: "Outbox Body Column", sql: "ALTER TABLE outbox ADD COLUMN body TEXT;" },
        Migration {
            version: 6,
            description: "Pending Actions Queue",
            sql: "CREATE TABLE IF NOT EXISTS pending_actions (id INTEGER PRIMARY KEY AUTOINCREMENT, account_id TEXT NOT NULL, mailbox_name TEXT NOT NULL, uid INTEGER NOT NULL, action TEXT NOT NULL, dest_mailbox TEXT, created_at INTEGER NOT NULL, FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE CASCADE);",
        },
        Migration {
            version: 7,
            description: "Sync Window & Timestamps",
            sql: r#"
ALTER TABLE accounts ADD COLUMN sync_window TEXT NOT NULL DEFAULT 'LAST_30_DAYS';
ALTER TABLE messages ADD COLUMN date_timestamp INTEGER;
"#,
        },
        Migration { version: 8, description: "v8 Placeholder", sql: "" },
        Migration {
            version: 9,
            description: "Threading Fields",
            sql: r#"
ALTER TABLE messages ADD COLUMN message_id TEXT;
ALTER TABLE messages ADD COLUMN in_reply_to TEXT;
ALTER TABLE messages ADD COLUMN references_json TEXT;
ALTER TABLE messages ADD COLUMN thread_id TEXT;
ALTER TABLE messages ADD COLUMN thread_subject TEXT;
"#,
        },
        Migration { version: 10, description: "Scheduled Sending", sql: "ALTER TABLE outbox ADD COLUMN scheduled_for INTEGER;" },
    ]
}

/// Initializes an in-memory or temporary file-backed SQLCipher pool for testing.
/// Uses a hardcoded hex key to avoid OS keychain dependencies in CI environments.
pub async fn init_test_pool() -> Result<(SqlitePool, tempfile::TempDir), sqlx::Error> {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let url = format!("sqlite://{}?mode=rwc", db_path.display());
    let test_key_hex = "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF";

    // The key is passed as a hex string wrapped in x'...' syntax, which is the standard
    // SQLCipher way to provide a raw hex key rather than a passphrase.
    let options = SqliteConnectOptions::from_str(&url)?
        .pragma("key", format!("\"x'{}'\"", test_key_hex))
        .pragma("journal_mode", "WAL")
        .pragma("synchronous", "NORMAL")
        .pragma("cache_size", "-20000")
        .pragma("temp_store", "MEMORY")
        .pragma("foreign_keys", "ON")
        .pragma("busy_timeout", "5000");

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;

    sqlx::query(SCHEMA_SQL).execute(&pool).await?;

    for migration in get_migrations() {
        if !migration.sql.trim().is_empty() {
            let _ = sqlx::raw_sql(migration.sql).execute(&pool).await;
        }
        sqlx::query(&format!("PRAGMA user_version = {};", migration.version))
            .execute(&pool)
            .await?;
    }
    Ok((pool, temp_dir))
}

/// Connects to the production SQLCipher database, applies the baseline schema,
/// and sequentially applies any pending migrations based on the `user_version` pragma.
pub async fn init_file_pool(path: &Path, db_key_hex: &str) -> Result<SqlitePool, sqlx::Error> {
    tracing::info!("[DB INIT] 1. Creating directories...");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create DB directory");
    }
    let url = format!("sqlite://{}?mode=rwc", path.display());
    tracing::info!("[DB INIT] 2. Building connection options...");
    let options = SqliteConnectOptions::from_str(&url)?
        .pragma("key", format!("\"x'{}'\"", db_key_hex))
        .pragma("journal_mode", "WAL")
        .pragma("synchronous", "NORMAL")
        .pragma("cache_size", "-20000")
        .pragma("temp_store", "MEMORY")
        .pragma("foreign_keys", "ON")
        .pragma("busy_timeout", "5000");

    tracing::info!("[DB INIT] 3. Connecting pool (SQLCipher decryption happens here)...");
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    tracing::info!("[DB INIT] 4. Pool connected. Executing SCHEMA_SQL...");
    sqlx::query(SCHEMA_SQL).execute(&pool).await?;
    tracing::info!("[DB INIT] 5. SCHEMA_SQL complete. Fetching user_version...");
    let mut current_version: i64 = sqlx::query_scalar("PRAGMA user_version;")
        .fetch_optional(&pool)
        .await?
        .unwrap_or(0);
    tracing::info!("[DB INIT] 6. Current user_version is {}", current_version);

    for migration in get_migrations() {
        if current_version < migration.version {
            tracing::info!("[DB INIT] Migrating to v{} ({})...", migration.version, migration.description);
            if !migration.sql.trim().is_empty() {
                // Use raw_sql to execute multiple statements safely.
                // Ignore errors to mimic legacy `let _ = ` resilience for partial states.
                let _ = sqlx::raw_sql(migration.sql).execute(&pool).await;
            }
            sqlx::query(&format!("PRAGMA user_version = {};", migration.version))
                .execute(&pool)
                .await?;
            current_version = migration.version;
        }
    }
    tracing::info!("[DB INIT] 7. ALL MIGRATIONS COMPLETE. Pool is ready.");
    Ok(pool)
}

/// Rebuilds FTS indexes in the background if the main tables have data but the FTS tables are empty.
/// This recovers from corrupted FTS states or partial migration failures without blocking app startup.
/// Also creates performance indexes in the background to avoid blocking the initial UI render.
pub async fn run_background_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let msg_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages;")
        .fetch_optional(pool)
        .await?
        .unwrap_or(0);
    let fts_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM email_fts;")
        .fetch_optional(pool)
        .await?
        .unwrap_or(0);

    if msg_count > 0 && fts_count == 0 {
        println!("[Migration] Rebuilding email FTS index in background...");
        let _ = sqlx::query("INSERT INTO email_fts(email_fts) VALUES('rebuild');")
            .execute(pool)
            .await;
    }

    let cal_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM calendar_events;")
        .fetch_optional(pool)
        .await?
        .unwrap_or(0);
    let cal_fts_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM calendar_events_fts;")
        .fetch_optional(pool)
        .await?
        .unwrap_or(0);

    if cal_count > 0 && cal_fts_count == 0 {
        let _ =
            sqlx::query("INSERT INTO calendar_events_fts(calendar_events_fts) VALUES('rebuild');")
                .execute(pool)
                .await;
    }

    let con_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM contacts;")
        .fetch_optional(pool)
        .await?
        .unwrap_or(0);
    let con_fts_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM contacts_fts;")
        .fetch_optional(pool)
        .await?
        .unwrap_or(0);

    if con_count > 0 && con_fts_count == 0 {
        let _ = sqlx::query("INSERT INTO contacts_fts(contacts_fts) VALUES('rebuild');")
            .execute(pool)
            .await;
    }

    // Ensure performance indexes exist in the background so they don't block startup
    let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_outbox_status_created ON outbox(status, created_at);")
        .execute(pool)
        .await;
    let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_thread_id ON messages(thread_id);")
        .execute(pool)
        .await;
    let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(account_id, mailbox_name, date_timestamp DESC);")
        .execute(pool)
        .await;
    Ok(())
}
