use crate::models::*;
use chrono::Utc;
use sqlx::SqlitePool;
use std::collections::HashSet;
use uuid::Uuid;

pub struct AccountRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> AccountRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        email: &str,
        provider: &str,
        imap_host: &str,
        imap_port: u16,
        smtp_host: &str,
        smtp_port: u16,
        auth_method: &str,
        oauth_client_id: Option<&str>,
        oauth_client_secret: Option<&str>,
        oauth_token_url: Option<&str>,
    ) -> Result<Account, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now().timestamp();
        let sync_window = "LAST_30_DAYS";
        sqlx::query("INSERT INTO accounts (id, email, provider, imap_host, imap_port, smtp_host, smtp_port, auth_method, oauth_client_id, oauth_client_secret, oauth_token_url, created_at, sync_window) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id).bind(email).bind(provider).bind(imap_host).bind(imap_port as i32).bind(smtp_host).bind(smtp_port as i32).bind(auth_method).bind(oauth_client_id).bind(oauth_client_secret).bind(oauth_token_url).bind(created_at).bind(sync_window)
            .execute(self.pool).await?;
        Ok(Account {
            id,
            email: email.to_string(),
            provider: provider.to_string(),
            imap_host: imap_host.to_string(),
            imap_port: imap_port as i32,
            smtp_host: smtp_host.to_string(),
            smtp_port: smtp_port as i32,
            auth_method: auth_method.to_string(),
            oauth_client_id: oauth_client_id.map(String::from),
            oauth_client_secret: oauth_client_secret.map(String::from),
            oauth_token_url: oauth_token_url.map(String::from),
            sync_window: sync_window.to_string(),
            created_at,
        })
    }

    pub async fn get_by_id(&self, id: &str) -> Result<Option<Account>, sqlx::Error> {
        sqlx::query_as::<_, Account>("SELECT id, email, provider, imap_host, imap_port, smtp_host, smtp_port, auth_method, oauth_client_id, oauth_client_secret, oauth_token_url, created_at, sync_window FROM accounts WHERE id = ?").bind(id).fetch_optional(self.pool).await
    }

    pub async fn get_by_email(&self, email: &str) -> Result<Option<Account>, sqlx::Error> {
        sqlx::query_as::<_, Account>("SELECT id, email, provider, imap_host, imap_port, smtp_host, smtp_port, auth_method, oauth_client_id, oauth_client_secret, oauth_token_url, created_at, sync_window FROM accounts WHERE email = ?").bind(email).fetch_optional(self.pool).await
    }

    pub async fn list_all(&self) -> Result<Vec<Account>, sqlx::Error> {
        sqlx::query_as::<_, Account>("SELECT id, email, provider, imap_host, imap_port, smtp_host, smtp_port, auth_method, oauth_client_id, oauth_client_secret, oauth_token_url, created_at, sync_window FROM accounts").fetch_all(self.pool).await
    }

    pub async fn delete(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM accounts WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_sync_window(&self, id: &str, sync_window: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE accounts SET sync_window = ? WHERE id = ?")
            .bind(sync_window)
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}

pub struct MailboxRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> MailboxRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(
        &self,
        account_id: &str,
        name: &str,
        delimiter: Option<&str>,
        attributes: &[String],
    ) -> Result<Mailbox, sqlx::Error> {
        let attrs_json = serde_json::to_string(attributes).unwrap_or_else(|_| "[]".to_string());
        sqlx::query("INSERT INTO mailboxes (account_id, name, delimiter, attributes) VALUES (?, ?, ?, ?) ON CONFLICT(account_id, name) DO UPDATE SET delimiter = excluded.delimiter, attributes = excluded.attributes").bind(account_id).bind(name).bind(delimiter).bind(&attrs_json).execute(self.pool).await?;
        self.get_by_name(account_id, name)
            .await
            .map(|opt| opt.unwrap())
    }

    pub async fn get_by_name(
        &self,
        account_id: &str,
        name: &str,
    ) -> Result<Option<Mailbox>, sqlx::Error> {
        // Calculates unread count dynamically via a correlated subquery.
        // Avoids maintaining a separate counter column that could drift out of sync during bulk flag updates.
        sqlx::query_as::<_, Mailbox>("SELECT m.id, m.account_id, m.name, m.delimiter, m.attributes, m.uid_validity, m.uid_next, m.highest_modseq, COALESCE((SELECT COUNT(*) FROM messages WHERE account_id = m.account_id AND mailbox_name = m.name AND flags NOT LIKE '%Seen%'), 0) as unread_count FROM mailboxes m WHERE m.account_id = ? AND m.name = ?").bind(account_id).bind(name).fetch_optional(self.pool).await
    }

    pub async fn list_for_account(&self, account_id: &str) -> Result<Vec<Mailbox>, sqlx::Error> {
        sqlx::query_as::<_, Mailbox>("SELECT m.id, m.account_id, m.name, m.delimiter, m.attributes, m.uid_validity, m.uid_next, m.highest_modseq, COALESCE((SELECT COUNT(*) FROM messages WHERE account_id = m.account_id AND mailbox_name = m.name AND flags NOT LIKE '%Seen%'), 0) as unread_count FROM mailboxes m WHERE m.account_id = ?").bind(account_id).fetch_all(self.pool).await
    }

    pub async fn delete_by_name(&self, account_id: &str, name: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM mailboxes WHERE account_id = ? AND name = ?")
            .bind(account_id)
            .bind(name)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn rename(
        &self,
        account_id: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE mailboxes SET name = ? WHERE account_id = ? AND name = ?")
            .bind(new_name)
            .bind(account_id)
            .bind(old_name)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}

pub struct MessageRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> MessageRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(
        &self,
        account_id: &str,
        mailbox_name: &str,
        uid: i32,
        subject: Option<&str>,
        sender: Option<&str>,
        date: Option<&str>,
        date_timestamp: Option<i64>,
        flags: &[String],
        size: i32,
        has_attachments: bool,
        snippet: Option<&str>,
        blob_hash: Option<&str>,
        attachment_names: Option<&str>,
        message_id: Option<&str>,
        in_reply_to: Option<&str>,
        references_json: Option<&str>,
        thread_id: Option<&str>,
        thread_subject: Option<&str>,
    ) -> Result<Message, sqlx::Error> {
        let flags_json = serde_json::to_string(flags).unwrap_or_else(|_| "[]".to_string());
        sqlx::query("INSERT INTO messages (account_id, mailbox_name, uid, subject, sender, date, date_timestamp, flags, size, has_attachments, snippet, blob_hash, attachment_names, message_id, in_reply_to, references_json, thread_id, thread_subject) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(account_id, mailbox_name, uid) DO UPDATE SET subject=excluded.subject, sender=excluded.sender, date=excluded.date, date_timestamp=excluded.date_timestamp, flags=excluded.flags, size=excluded.size, has_attachments=excluded.has_attachments, snippet=excluded.snippet, blob_hash=excluded.blob_hash, attachment_names=excluded.attachment_names, message_id=excluded.message_id, in_reply_to=excluded.in_reply_to, references_json=excluded.references_json, thread_id=excluded.thread_id, thread_subject=excluded.thread_subject")
            .bind(account_id).bind(mailbox_name).bind(uid).bind(subject).bind(sender).bind(date).bind(date_timestamp).bind(&flags_json).bind(size).bind(has_attachments).bind(snippet).bind(blob_hash).bind(attachment_names).bind(message_id).bind(in_reply_to).bind(references_json).bind(thread_id).bind(thread_subject)
            .execute(self.pool).await?;
        sqlx::query_as::<_, Message>("SELECT id, account_id, mailbox_name, uid, subject, sender, date, date_timestamp, flags, size, has_attachments, snippet, blob_hash, attachment_names, message_id, in_reply_to, references_json, thread_id, thread_subject, NULL as thread_count FROM messages WHERE account_id = ? AND mailbox_name = ? AND uid = ?").bind(account_id).bind(mailbox_name).bind(uid).fetch_one(self.pool).await
    }

    pub async fn update_snippet(
        &self,
        account_id: &str,
        mailbox_name: &str,
        uid: i32,
        snippet: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE messages SET snippet = ? WHERE account_id = ? AND mailbox_name = ? AND uid = ?",
        )
            .bind(snippet)
            .bind(account_id)
            .bind(mailbox_name)
            .bind(uid)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn search_with_highlight(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<MessageSearchRow>, sqlx::Error> {
        let sql = r#"SELECT m.id, m.account_id, m.mailbox_name, m.uid, m.subject, m.sender, m.date, m.date_timestamp, m.flags, m.size, m.has_attachments, m.snippet, m.blob_hash, m.attachment_names, m.message_id, m.in_reply_to, m.references_json, m.thread_id, m.thread_subject, NULL as thread_count, snippet(email_fts, -1, '<mark>', '</mark>', '...', 32) as highlight FROM email_fts fts JOIN messages m ON m.id = fts.rowid WHERE email_fts MATCH ? ORDER BY rank LIMIT ?"#;
        sqlx::query_as::<_, MessageSearchRow>(sql)
            .bind(query)
            .bind(limit)
            .fetch_all(self.pool)
            .await
    }

    pub async fn list_cursor(
        &self,
        account_id: &str,
        mailbox_name: &str,
        before_uid: Option<i32>,
        limit: i64,
    ) -> Result<Vec<Message>, sqlx::Error> {
        let cols = "id, account_id, mailbox_name, uid, subject, sender, date, date_timestamp, flags, size, has_attachments, snippet, blob_hash, attachment_names, message_id, in_reply_to, references_json, thread_id, thread_subject, NULL as thread_count";
        if let Some(cursor) = before_uid {
            sqlx::query_as::<_, Message>(&format!("SELECT {} FROM messages WHERE account_id = ? AND mailbox_name = ? AND uid < ? ORDER BY uid DESC LIMIT ?", cols)).bind(account_id).bind(mailbox_name).bind(cursor).bind(limit).fetch_all(self.pool).await
        } else {
            sqlx::query_as::<_, Message>(&format!("SELECT {} FROM messages WHERE account_id = ? AND mailbox_name = ? ORDER BY uid DESC LIMIT ?", cols)).bind(account_id).bind(mailbox_name).bind(limit).fetch_all(self.pool).await
        }
    }

    pub async fn list_threads_cursor(
        &self,
        account_id: &str,
        mailbox_name: &str,
        before_id: Option<i64>,
        limit: i64,
    ) -> Result<Vec<Message>, sqlx::Error> {
        // Virtual mailboxes (e.g., __STARRED__) are handled by dynamically injecting the WHERE clause
        // since they don't correspond to a single physical mailbox_name.
        let (where_clause, needs_mailbox_bind) = match mailbox_name {
            "__STARRED__" => ("flags LIKE '%\"Flagged\"%'", false),
            "__ARCHIVE__" => (
                "(mailbox_name LIKE '%Archive%' OR mailbox_name LIKE '%All Mail%' OR mailbox_name = 'Archive')",
                false,
            ),
            "__SPAM__" => (
                "(mailbox_name LIKE '%Spam%' OR mailbox_name LIKE '%Junk%' OR mailbox_name = 'Spam' OR mailbox_name = 'Junk')",
                false,
            ),
            _ => ("mailbox_name = ?", true),
        };

        // Uses SQL window functions to partition messages by thread_id, selecting only the most recent message
        // per thread (rn = 1) while also computing the total message count for that thread.
        // COALESCE(NULLIF(thread_id, ''), CAST(id AS TEXT)) ensures unthreaded messages don't all get
        // lumped into a single NULL partition; each gets its own partition based on its unique ID.
        let sql = format!(
            r#"
WITH Threaded AS (
SELECT *,
ROW_NUMBER() OVER(PARTITION BY COALESCE(NULLIF(thread_id, ''), CAST(id AS TEXT)) ORDER BY date_timestamp DESC, id DESC) as rn,
COUNT(*) OVER(PARTITION BY COALESCE(NULLIF(thread_id, ''), CAST(id AS TEXT))) as thread_count
FROM messages WHERE account_id = ? AND {}
)
SELECT id, account_id, mailbox_name, uid, subject, sender, date, date_timestamp, flags, size, has_attachments, snippet, blob_hash, attachment_names, message_id, in_reply_to, references_json, thread_id, thread_subject, thread_count
FROM Threaded WHERE rn = 1 AND (? IS NULL OR id < ?)
ORDER BY date_timestamp DESC, id DESC LIMIT ?
"#,
            where_clause
        );
        let mut query = sqlx::query_as::<_, Message>(&sql).bind(account_id);
        if needs_mailbox_bind {
            query = query.bind(mailbox_name);
        }
        query = query.bind(before_id).bind(before_id).bind(limit);
        query.fetch_all(self.pool).await
    }

    pub async fn get_thread_messages(&self, thread_id: &str) -> Result<Vec<Message>, sqlx::Error> {
        sqlx::query_as::<_, Message>("SELECT id, account_id, mailbox_name, uid, subject, sender, date, date_timestamp, flags, size, has_attachments, snippet, blob_hash, attachment_names, message_id, in_reply_to, references_json, thread_id, thread_subject, NULL as thread_count FROM messages WHERE thread_id = ? ORDER BY date_timestamp ASC")
            .bind(thread_id).fetch_all(self.pool).await
    }

    pub async fn get_by_uid(
        &self,
        account_id: &str,
        mailbox_name: &str,
        uid: i32,
    ) -> Result<Option<Message>, sqlx::Error> {
        sqlx::query_as::<_, Message>("SELECT id, account_id, mailbox_name, uid, subject, sender, date, date_timestamp, flags, size, has_attachments, snippet, blob_hash, attachment_names, message_id, in_reply_to, references_json, thread_id, thread_subject, NULL as thread_count FROM messages WHERE account_id = ? AND mailbox_name = ? AND uid = ?").bind(account_id).bind(mailbox_name).bind(uid).fetch_optional(self.pool).await
    }

    pub async fn update_blob_hash(
        &self,
        account_id: &str,
        mailbox_name: &str,
        uid: i32,
        blob_hash: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE messages SET blob_hash = ? WHERE account_id = ? AND mailbox_name = ? AND uid = ?").bind(blob_hash).bind(account_id).bind(mailbox_name).bind(uid).execute(self.pool).await?;
        Ok(())
    }

    pub async fn update_flags(
        &self,
        account_id: &str,
        mailbox_name: &str,
        uid: i32,
        flags: &[String],
    ) -> Result<(), sqlx::Error> {
        let flags_json = serde_json::to_string(flags).unwrap_or_else(|_| "[]".to_string());
        sqlx::query(
            "UPDATE messages SET flags = ? WHERE account_id = ? AND mailbox_name = ? AND uid = ?",
        )
            .bind(&flags_json)
            .bind(account_id)
            .bind(mailbox_name)
            .bind(uid)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_by_uid(
        &self,
        account_id: &str,
        mailbox_name: &str,
        uid: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM messages WHERE account_id = ? AND mailbox_name = ? AND uid = ?")
            .bind(account_id)
            .bind(mailbox_name)
            .bind(uid)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn move_to_mailbox(
        &self,
        account_id: &str,
        src_mailbox: &str,
        dest_mailbox: &str,
        uid: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE messages SET mailbox_name = ? WHERE account_id = ? AND mailbox_name = ? AND uid = ?").bind(dest_mailbox).bind(account_id).bind(src_mailbox).bind(uid).execute(self.pool).await?;
        Ok(())
    }

    pub async fn list_all_blob_hashes(&self) -> Result<HashSet<String>, sqlx::Error> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT blob_hash FROM messages WHERE blob_hash IS NOT NULL")
                .fetch_all(self.pool)
                .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn delete_older_than(
        &self,
        account_id: &str,
        cutoff_timestamp: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM messages WHERE account_id = ? AND date_timestamp < ? AND date_timestamp IS NOT NULL").bind(account_id).bind(cutoff_timestamp).execute(self.pool).await?;
        Ok(result.rows_affected())
    }
}

pub struct OutboxRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> OutboxRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn enqueue(
        &self,
        account_id: &str,
        raw_mime: &[u8],
        envelope_from: &str,
        envelope_to: &[String],
        subject: Option<&str>,
        body: Option<&str>,
        scheduled_for: Option<i64>,
    ) -> Result<OutboxMessage, sqlx::Error> {
        let to_json = serde_json::to_string(envelope_to).unwrap_or_else(|_| "[]".to_string());
        let created_at = Utc::now().timestamp();
        sqlx::query("INSERT INTO outbox (account_id, raw_mime, envelope_from, envelope_to, subject, body, created_at, scheduled_for) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(account_id).bind(raw_mime).bind(envelope_from).bind(&to_json)
            .bind(subject).bind(body).bind(created_at).bind(scheduled_for)
            .execute(self.pool).await?;
        sqlx::query_as::<_, OutboxMessage>("SELECT id, account_id, status, retry_count, last_error, raw_mime, envelope_from, envelope_to, subject, body, created_at, sent_at, scheduled_for FROM outbox WHERE account_id = ? ORDER BY id DESC LIMIT 1").bind(account_id).fetch_one(self.pool).await
    }

    pub async fn save_draft(
        &self,
        account_id: &str,
        raw_mime: &[u8],
        envelope_from: &str,
        envelope_to: &[String],
        subject: Option<&str>,
        body: Option<&str>,
        draft_id: Option<i64>,
    ) -> Result<OutboxMessage, sqlx::Error> {
        let to_json = serde_json::to_string(envelope_to).unwrap_or_else(|_| "[]".to_string());
        let created_at = Utc::now().timestamp();
        if let Some(id) = draft_id {
            sqlx::query("UPDATE outbox SET raw_mime = ?, envelope_to = ?, subject = ?, body = ? WHERE id = ? AND status = 'draft'").bind(raw_mime).bind(&to_json).bind(subject).bind(body).bind(id).execute(self.pool).await?;
            return self.get_by_id(id).await;
        }
        sqlx::query("INSERT INTO outbox (account_id, raw_mime, envelope_from, envelope_to, subject, body, status, created_at) VALUES (?, ?, ?, ?, ?, ?, 'draft', ?)").bind(account_id).bind(raw_mime).bind(envelope_from).bind(&to_json).bind(subject).bind(body).bind(created_at).execute(self.pool).await?;
        sqlx::query_as::<_, OutboxMessage>("SELECT id, account_id, status, retry_count, last_error, raw_mime, envelope_from, envelope_to, subject, body, created_at, sent_at, scheduled_for FROM outbox WHERE account_id = ? ORDER BY id DESC LIMIT 1").bind(account_id).fetch_one(self.pool).await
    }

    pub async fn get_by_id(&self, id: i64) -> Result<OutboxMessage, sqlx::Error> {
        sqlx::query_as::<_, OutboxMessage>("SELECT id, account_id, status, retry_count, last_error, raw_mime, envelope_from, envelope_to, subject, body, created_at, sent_at, scheduled_for FROM outbox WHERE id = ?").bind(id).fetch_one(self.pool).await
    }

    pub async fn get_drafts(&self, account_id: &str) -> Result<Vec<OutboxMessage>, sqlx::Error> {
        sqlx::query_as::<_, OutboxMessage>("SELECT id, account_id, status, retry_count, last_error, raw_mime, envelope_from, envelope_to, subject, body, created_at, sent_at, scheduled_for FROM outbox WHERE account_id = ? AND status = 'draft' ORDER BY created_at DESC").bind(account_id).fetch_all(self.pool).await
    }

    pub async fn delete_draft(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM outbox WHERE id = ? AND status = 'draft'")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_pending(&self, limit: i64) -> Result<Vec<OutboxMessage>, sqlx::Error> {
        let now = Utc::now().timestamp();
        // Fetches messages that are either pending or have failed fewer than 5 times,
        // and are either unscheduled or scheduled for a time in the past.
        sqlx::query_as::<_, OutboxMessage>(
            "SELECT id, account_id, status, retry_count, last_error, raw_mime, envelope_from, envelope_to, subject, body, created_at, sent_at, scheduled_for
FROM outbox
WHERE (status = 'pending' OR (status = 'failed' AND retry_count < 5))
AND (scheduled_for IS NULL OR scheduled_for <= ?)
ORDER BY created_at ASC LIMIT ?"
        )
            .bind(now).bind(limit).fetch_all(self.pool).await
    }

    pub async fn mark_sent(&self, id: i64) -> Result<(), sqlx::Error> {
        let sent_at = Utc::now().timestamp();
        sqlx::query("UPDATE outbox SET status = 'sent', sent_at = ? WHERE id = ?")
            .bind(sent_at)
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_failed(&self, id: i64, error: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE outbox SET status = 'failed', last_error = ?, retry_count = retry_count + 1 WHERE id = ?").bind(error).bind(id).execute(self.pool).await?;
        Ok(())
    }

    pub async fn cancel_scheduled(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM outbox WHERE id = ? AND status = 'pending'")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}

pub struct CalendarRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> CalendarRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_calendar(
        &self,
        account_id: &str,
        url: &str,
        display_name: &str,
        ctag: Option<&str>,
        sync_token: Option<&str>,
    ) -> Result<CalendarRecord, sqlx::Error> {
        sqlx::query("INSERT INTO calendars (account_id, url, display_name, ctag, sync_token) VALUES (?, ?, ?, ?, ?) ON CONFLICT(account_id, url) DO UPDATE SET display_name=excluded.display_name, ctag=excluded.ctag, sync_token=excluded.sync_token").bind(account_id).bind(url).bind(display_name).bind(ctag).bind(sync_token).execute(self.pool).await?;
        sqlx::query_as::<_, CalendarRecord>("SELECT id, account_id, url, display_name, ctag, sync_token FROM calendars WHERE account_id = ? AND url = ?").bind(account_id).bind(url).fetch_one(self.pool).await
    }

    pub async fn get_calendars_for_account(
        &self,
        account_id: &str,
    ) -> Result<Vec<CalendarRecord>, sqlx::Error> {
        sqlx::query_as::<_, CalendarRecord>("SELECT id, account_id, url, display_name, ctag, sync_token FROM calendars WHERE account_id = ?").bind(account_id).fetch_all(self.pool).await
    }

    pub async fn upsert_event(
        &self,
        calendar_id: i64,
        uid: &str,
        etag: &str,
        url: &str,
        ical_data: &str,
        last_modified: Option<i64>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO calendar_events (calendar_id, uid, etag, url, ical_data, last_modified) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(calendar_id, url) DO UPDATE SET uid=excluded.uid, etag=excluded.etag, ical_data=excluded.ical_data, last_modified=excluded.last_modified").bind(calendar_id).bind(uid).bind(etag).bind(url).bind(ical_data).bind(last_modified).execute(self.pool).await?;
        Ok(())
    }

    pub async fn delete_event_by_url(
        &self,
        calendar_id: i64,
        url: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM calendar_events WHERE calendar_id = ? AND url = ?")
            .bind(calendar_id)
            .bind(url)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_events_for_calendar(
        &self,
        calendar_id: i64,
    ) -> Result<Vec<CalendarEventRecord>, sqlx::Error> {
        sqlx::query_as::<_, CalendarEventRecord>("SELECT id, calendar_id, uid, etag, url, ical_data, last_modified FROM calendar_events WHERE calendar_id = ?").bind(calendar_id).fetch_all(self.pool).await
    }

    pub async fn search_events(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<CalendarEventRecord>, sqlx::Error> {
        sqlx::query_as::<_, CalendarEventRecord>("SELECT c.id, c.calendar_id, c.uid, c.etag, c.url, c.ical_data, c.last_modified FROM calendar_events_fts fts JOIN calendar_events c ON c.id = fts.rowid WHERE calendar_events_fts MATCH ? ORDER BY rank LIMIT ?").bind(query).bind(limit).fetch_all(self.pool).await
    }
}

pub struct ContactRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> ContactRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_address_book(
        &self,
        account_id: &str,
        url: &str,
        display_name: &str,
        ctag: Option<&str>,
        sync_token: Option<&str>,
    ) -> Result<AddressBookRecord, sqlx::Error> {
        sqlx::query("INSERT INTO address_books (account_id, url, display_name, ctag, sync_token) VALUES (?, ?, ?, ?, ?) ON CONFLICT(account_id, url) DO UPDATE SET display_name=excluded.display_name, ctag=excluded.ctag, sync_token=excluded.sync_token").bind(account_id).bind(url).bind(display_name).bind(ctag).bind(sync_token).execute(self.pool).await?;
        sqlx::query_as::<_, AddressBookRecord>("SELECT id, account_id, url, display_name, ctag, sync_token FROM address_books WHERE account_id = ? AND url = ?").bind(account_id).bind(url).fetch_one(self.pool).await
    }

    pub async fn get_address_books_for_account(
        &self,
        account_id: &str,
    ) -> Result<Vec<AddressBookRecord>, sqlx::Error> {
        sqlx::query_as::<_, AddressBookRecord>("SELECT id, account_id, url, display_name, ctag, sync_token FROM address_books WHERE account_id = ?").bind(account_id).fetch_all(self.pool).await
    }

    pub async fn upsert_contact(
        &self,
        address_book_id: i64,
        uid: &str,
        etag: &str,
        url: &str,
        vcard_data: &str,
        last_modified: Option<i64>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO contacts (address_book_id, uid, etag, url, vcard_data, last_modified) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(address_book_id, url) DO UPDATE SET uid=excluded.uid, etag=excluded.etag, vcard_data=excluded.vcard_data, last_modified=excluded.last_modified").bind(address_book_id).bind(uid).bind(etag).bind(url).bind(vcard_data).bind(last_modified).execute(self.pool).await?;
        Ok(())
    }

    pub async fn delete_contact_by_url(
        &self,
        address_book_id: i64,
        url: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM contacts WHERE address_book_id = ? AND url = ?")
            .bind(address_book_id)
            .bind(url)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_contacts_for_address_book(
        &self,
        address_book_id: i64,
    ) -> Result<Vec<ContactRecord>, sqlx::Error> {
        sqlx::query_as::<_, ContactRecord>("SELECT id, address_book_id, uid, etag, url, vcard_data, last_modified FROM contacts WHERE address_book_id = ?").bind(address_book_id).fetch_all(self.pool).await
    }

    pub async fn search_contacts(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<ContactRecord>, sqlx::Error> {
        sqlx::query_as::<_, ContactRecord>("SELECT c.id, c.address_book_id, c.uid, c.etag, c.url, c.vcard_data, c.last_modified FROM contacts_fts fts JOIN contacts c ON c.id = fts.rowid WHERE contacts_fts MATCH ? ORDER BY rank LIMIT ?").bind(query).bind(limit).fetch_all(self.pool).await
    }
}

pub struct PendingActionRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> PendingActionRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn queue_action(
        &self,
        account_id: &str,
        mailbox_name: &str,
        uid: i32,
        action: &str,
        dest_mailbox: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let created_at = Utc::now().timestamp();
        sqlx::query("INSERT INTO pending_actions (account_id, mailbox_name, uid, action, dest_mailbox, created_at) VALUES (?, ?, ?, ?, ?, ?)").bind(account_id).bind(mailbox_name).bind(uid).bind(action).bind(dest_mailbox).bind(created_at).execute(self.pool).await?;
        Ok(())
    }

    pub async fn get_pending_actions(
        &self,
        account_id: &str,
    ) -> Result<Vec<PendingAction>, sqlx::Error> {
        sqlx::query_as::<_, PendingAction>("SELECT id, account_id, mailbox_name, uid, action, dest_mailbox, created_at FROM pending_actions WHERE account_id = ? ORDER BY created_at ASC").bind(account_id).fetch_all(self.pool).await
    }

    pub async fn delete_action(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM pending_actions WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
