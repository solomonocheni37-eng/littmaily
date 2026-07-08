use crate::{
    ImapStream, MessageHeader, OwnedFlag, connect_account, decode_mime_header,
    oauth::{Credentials, TokenStore},
};
use async_imap::Session;
use chrono::{Duration, Utc};
use futures::StreamExt;
use std::time::Duration as StdDuration;
use tokio::sync::{broadcast, mpsc};
use tokio::time::sleep;

const IMAP_IDLE_TIMEOUT_SECS: u64 = 25 * 60;

#[derive(Debug)]
pub enum SyncCommand {
    Shutdown,
    Pause,
    Resume,
    ForceResync,
}

#[derive(Debug, Clone)]
pub enum SyncEvent {
    Connected,
    Disconnected(String),
    NewMessages(Vec<MessageHeader>),
    StateSync(Vec<(u32, Vec<OwnedFlag>)>),
    SyncComplete,
    Error(String),
}

/// Background worker that maintains a persistent IMAP connection using the IDLE command.
/// Handles exponential backoff on connection failures, sync window truncation,
/// and emits events for new messages and state changes (flag updates/deletions).
pub struct SyncWorker<S: TokenStore + 'static> {
    credentials: Credentials<S>,
    host: String,
    port: u16,
    mailbox: String,
    cmd_rx: mpsc::Receiver<SyncCommand>,
    event_tx: broadcast::Sender<SyncEvent>,
    last_uid_next: u32,
    is_paused: bool,
    pub sync_window: String,
}

impl<S: TokenStore + 'static> SyncWorker<S> {
    pub fn new(
        credentials: Credentials<S>,
        host: String,
        port: u16,
        mailbox: String,
        cmd_rx: mpsc::Receiver<SyncCommand>,
        event_tx: broadcast::Sender<SyncEvent>,
        sync_window: String,
    ) -> Self {
        Self {
            credentials,
            host,
            port,
            mailbox,
            cmd_rx,
            event_tx,
            last_uid_next: 1,
            is_paused: false,
            sync_window,
        }
    }

    pub async fn run(mut self) {
        let mut backoff = StdDuration::from_secs(1);
        let max_backoff = StdDuration::from_secs(15 * 60);

        loop {
            if self.process_commands() {
                return;
            }

            let mut session = match connect_account(&self.host, self.port, &self.credentials).await
            {
                Ok(s) => s,
                Err(e) => {
                    self.emit(SyncEvent::Error(format!("Connection failed: {}", e)));
                    self.emit(SyncEvent::Disconnected(e.to_string()));
                    self.backoff(&mut backoff, max_backoff).await;
                    continue;
                }
            };
            self.emit(SyncEvent::Connected);
            backoff = StdDuration::from_secs(1);

            let mailbox_data = match session.select(&self.mailbox).await {
                Ok(data) => data,
                Err(e) => {
                    self.emit(SyncEvent::Error(format!(
                        "SELECT {} failed: {}",
                        self.mailbox, e
                    )));
                    self.backoff(&mut backoff, max_backoff).await;
                    continue;
                }
            };

            if self.last_uid_next <= 1 {
                let uid_next = mailbox_data.uid_next.unwrap_or(1);
                match self.sync_window.as_str() {
                    "LAST_100_MESSAGES" => {
                        self.last_uid_next = if uid_next > 100 {
                            uid_next.saturating_sub(100)
                        } else {
                            1
                        };
                    }
                    "LAST_30_DAYS" | "LAST_6_MONTHS" => {
                        let days = if self.sync_window == "LAST_30_DAYS" {
                            30
                        } else {
                            180
                        };
                        let cutoff = Utc::now() - Duration::days(days);
                        let imap_date = cutoff.format("%d-%b-%Y").to_string();
                        let query = format!("SINCE {}", imap_date);
                        match session.uid_search(&query).await {
                            Ok(uids) => {
                                let mut sorted_uids: Vec<u32> = uids.into_iter().collect();
                                sorted_uids.sort_unstable();
                                self.last_uid_next =
                                    sorted_uids.first().copied().unwrap_or(uid_next);
                            }
                            Err(_) => self.last_uid_next = 1, // Fallback
                        }
                    }
                    _ => self.last_uid_next = 1, // EVERYTHING
                }
            }

            loop {
                if self.process_commands() {
                    return;
                }
                if self.is_paused {
                    sleep(StdDuration::from_secs(5)).await;
                    continue;
                }
                if let Err(e) = self.fetch_new_messages(&mut session).await {
                    self.emit(SyncEvent::Error(format!("Fetch error: {}", e)));
                    break;
                }
                match self.fetch_state_sync(&mut session).await {
                    Ok(updates) => {
                        if !updates.is_empty() {
                            self.emit(SyncEvent::StateSync(updates));
                        }
                    }
                    Err(e) => {
                        self.emit(SyncEvent::Error(format!("State sync error: {}", e)));
                        break;
                    }
                }
                self.emit(SyncEvent::SyncComplete);

                let mut idle_handle = session.idle();
                if let Err(e) = idle_handle.init().await {
                    self.emit(SyncEvent::Error(format!("IDLE init error: {}", e)));
                    break;
                }

                let (idle_future, _stop_source) =
                    idle_handle.wait_with_timeout(StdDuration::from_secs(IMAP_IDLE_TIMEOUT_SECS));

                match idle_future.await {
                    Ok(_) => {}
                    Err(e) => {
                        self.emit(SyncEvent::Error(format!("IDLE wait error: {}", e)));
                        break;
                    }
                }

                match idle_handle.done().await {
                    Ok(recovered_session) => session = recovered_session,
                    Err(e) => {
                        self.emit(SyncEvent::Error(format!("IDLE done failed: {}", e)));
                        break;
                    }
                }
            }
            self.emit(SyncEvent::Disconnected("Connection lost or reset".into()));
            self.backoff(&mut backoff, max_backoff).await;
        }
    }

    async fn fetch_new_messages(
        &mut self,
        session: &mut Session<ImapStream>,
    ) -> Result<(), String> {
        if self.last_uid_next == 0 {
            return Ok(());
        }
        let range = format!("{}:*", self.last_uid_next);
        let mut stream = session
            .uid_fetch(&range, "(UID FLAGS ENVELOPE RFC822.SIZE BODYSTRUCTURE BODY.PEEK[HEADER.FIELDS (REFERENCES)])")
            .await
            .map_err(|e| e.to_string())?;

        let mut new_headers = Vec::new();
        let mut highest_uid_seen = self.last_uid_next.saturating_sub(1);

        while let Some(fetch_result) = stream.next().await {
            let fetch = fetch_result.map_err(|e| e.to_string())?;
            if let Some(uid) = fetch.uid {
                if uid >= self.last_uid_next {
                    if uid > highest_uid_seen {
                        highest_uid_seen = uid;
                    }
                    if let Some(header) = Self::parse_fetch_to_header(&fetch) {
                        new_headers.push(header);
                    }
                }
            }
        }

        if !new_headers.is_empty() {
            self.emit(SyncEvent::NewMessages(new_headers));
        }
        self.last_uid_next = highest_uid_seen + 1;
        Ok(())
    }

    async fn fetch_state_sync(
        &mut self,
        session: &mut Session<ImapStream>,
    ) -> Result<Vec<(u32, Vec<OwnedFlag>)>, String> {
        // CRITICAL FIX: Only sync flags for the most recent 2000 messages to prevent O(N) bandwidth waste.
        // Older messages rarely change flags, and fetching them all on every IDLE cycle is prohibitively slow.
        let start_uid = if self.last_uid_next > 2000 { self.last_uid_next.saturating_sub(2000) } else { 1 };
        let range = format!("{}:*", start_uid);
        let mut stream = session
            .uid_fetch(&range, "(UID FLAGS)")
            .await
            .map_err(|e| e.to_string())?;

        let mut updates = Vec::new();
        while let Some(fetch_result) = stream.next().await {
            let fetch = fetch_result.map_err(|e| e.to_string())?;
            if let Some(uid) = fetch.uid {
                let flags: Vec<OwnedFlag> = fetch.flags().map(|f| (&f).into()).collect();
                updates.push((uid, flags));
            }
        }
        Ok(updates)
    }

    pub fn parse_fetch_to_header(fetch: &async_imap::types::Fetch) -> Option<MessageHeader> {
        let uid = fetch.uid?;
        let envelope = fetch.envelope()?;
        let subject = envelope
            .subject
            .as_ref()
            .map(|cow| decode_mime_header(cow))
            .unwrap_or_default();
        let from = envelope
            .from
            .as_ref()
            .and_then(|v| v.first())
            .map(|addr| {
                let name = addr.name.as_ref().map(|cow| decode_mime_header(cow));
                let mailbox = addr
                    .mailbox
                    .as_ref()
                    .map(|cow| decode_mime_header(cow))
                    .unwrap_or_default();
                let host = addr
                    .host
                    .as_ref()
                    .map(|cow| decode_mime_header(cow))
                    .unwrap_or_default();
                let email = if host.is_empty() {
                    mailbox
                } else {
                    format!("{}@{}", mailbox, host)
                };
                match name {
                    Some(n) => format!("{} <{}>", n, email),
                    None => email,
                }
            })
            .unwrap_or_default();
        let date = envelope.date.as_ref().map(|cow| decode_mime_header(cow));
        let flags: Vec<OwnedFlag> = fetch.flags().map(|f| (&f).into()).collect();
        let size = fetch.size.unwrap_or(0);
        let attachment_names = crate::extract_attachment_names(fetch);
        let threading = crate::threading::ThreadingFields::from_imap_fetch(&fetch, &subject, uid);

        Some(MessageHeader {
            uid,
            subject,
            from,
            date,
            flags,
            size,
            attachment_names,
            snippet: crate::extract_snippet(fetch),
            message_id: threading.message_id,
            in_reply_to: threading.in_reply_to,
            references: threading.references,
            thread_id: threading.thread_id,
            thread_subject: threading.thread_subject,
        })
    }

    fn process_commands(&mut self) -> bool {
        let mut should_shutdown = false;
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            match cmd {
                SyncCommand::Pause => self.is_paused = true,
                SyncCommand::Resume => self.is_paused = false,
                SyncCommand::ForceResync => self.last_uid_next = 1,
                SyncCommand::Shutdown => should_shutdown = true,
            }
        }
        should_shutdown
    }

    async fn backoff(&mut self, current: &mut StdDuration, max: StdDuration) {
        tokio::select! {
            _ = sleep(*current) => {},
            cmd = self.cmd_rx.recv() => { if let Some(SyncCommand::Shutdown) = cmd { /* Will exit on next loop */ } }
        }
        *current = (*current * 2).min(max);
    }

    fn emit(&self, event: SyncEvent) {
        let _ = self.event_tx.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::MemoryStore;
    use tokio::sync::{broadcast, mpsc};

    fn setup_worker() -> (SyncWorker<MemoryStore>, mpsc::Sender<SyncCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(10);
        let (event_tx, _event_rx) = broadcast::channel(100);
        let creds = Credentials::Password {
            full_name: "Test User".into(),
            email: "test@example.com".into(),
            password: zeroize::Zeroizing::new("password123".into()),
        };
        let worker = SyncWorker::new(
            creds,
            "imap.example.com".into(),
            993,
            "INBOX".into(),
            cmd_rx,
            event_tx,
            "LAST_30_DAYS".into(),
        );
        (worker, cmd_tx)
    }

    #[tokio::test]
    async fn given_shutdown_command_when_processed_then_returns_true() {
        let (mut worker, cmd_tx) = setup_worker();
        cmd_tx.send(SyncCommand::Shutdown).await.unwrap();
        let should_shutdown = worker.process_commands();
        assert!(should_shutdown, "Worker should signal shutdown");
    }

    #[tokio::test]
    async fn given_pause_command_when_processed_then_sets_is_paused() {
        let (mut worker, cmd_tx) = setup_worker();
        cmd_tx.send(SyncCommand::Pause).await.unwrap();
        worker.process_commands();
        assert!(worker.is_paused, "Worker should be paused");
    }

    #[tokio::test]
    async fn given_resume_command_when_processed_then_clears_is_paused() {
        let (mut worker, cmd_tx) = setup_worker();
        worker.is_paused = true;
        cmd_tx.send(SyncCommand::Resume).await.unwrap();
        worker.process_commands();
        assert!(!worker.is_paused, "Worker should be resumed");
    }

    #[tokio::test]
    async fn given_force_resync_command_when_processed_then_resets_uid_next() {
        let (mut worker, cmd_tx) = setup_worker();
        worker.last_uid_next = 54321; // Simulate existing state
        cmd_tx.send(SyncCommand::ForceResync).await.unwrap();
        worker.process_commands();
        assert_eq!(
            worker.last_uid_next, 1,
            "UID NEXT should reset to 1 for full resync"
        );
    }
}
