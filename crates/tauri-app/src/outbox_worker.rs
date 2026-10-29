// FILE: ./crates/tauri-app/src/outbox_worker.rs
use async_trait::async_trait;
use sqlx::SqlitePool;
use storage::models::{Account, OutboxMessage};
use storage::repository::{AccountRepository, OutboxRepository};
use tokio::time::{Duration, sleep};

/// Abstracts the SMTP sending logic to allow mocking in unit tests
/// without hitting the network or requiring live credentials.
#[async_trait]
pub trait MessageSender: Send + Sync {
    async fn send(&self, account: &Account, msg: &OutboxMessage) -> Result<(), String>;
    async fn on_success(&self, account: &Account, msg: &OutboxMessage) -> Result<(), String>;
}

pub struct OutboxWorker<S: MessageSender> {
    pool: SqlitePool,
    sender: S,
}

impl<S: MessageSender> OutboxWorker<S> {
    pub fn new(pool: SqlitePool, sender: S) -> Self {
        Self { pool, sender }
    }

    pub async fn run(&self) {
        loop {
            self.process_pending().await;
            // Poll every 2 seconds to balance responsiveness with CPU/battery usage.
            sleep(Duration::from_secs(2)).await;
        }
    }

    pub async fn process_pending(&self) {
        let repo = OutboxRepository::new(&self.pool);
        let account_repo = AccountRepository::new(&self.pool);
        let pending = repo.get_pending(10).await.unwrap_or_default();
        for msg in pending {
            if let Ok(Some(account)) = account_repo.get_by_id(&msg.account_id).await {
                match self.sender.send(&account, &msg).await {
                    Ok(_) => {
                        let _ = self.sender.on_success(&account, &msg).await;
                        let _ = repo.mark_sent(msg.id).await;
                    }
                    Err(e) => {
                        let _ = repo.mark_failed(msg.id, &e).await;
                    }
                }
            } else {
                let _ = repo.mark_failed(msg.id, "Account not found").await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use storage::db::init_test_pool;

    struct MockSender {
        send_count: Arc<AtomicUsize>,
        fail: bool,
    }

    #[async_trait]
    impl MessageSender for MockSender {
        async fn send(&self, _account: &Account, _msg: &OutboxMessage) -> Result<(), String> {
            self.send_count.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err("Mock SMTP Error".into())
            } else {
                Ok(())
            }
        }

        async fn on_success(&self, _account: &Account, _msg: &OutboxMessage) -> Result<(), String> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_outbox_worker_success_transitions_to_sent() {
        let (pool, _temp_dir) = init_test_pool().await.unwrap();
        let acc_repo = AccountRepository::new(&pool);
        let outbox_repo = OutboxRepository::new(&pool);
        let acc = acc_repo
            .create(
                "test@example.com",
                "test",
                "imap",
                993,
                "smtp",
                587,
                "password",
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let msg = outbox_repo
            .enqueue(
                &acc.id,
                b"Raw MIME",
                "test@example.com",
                &["dest@example.com".into()],
                Some("Subj"),
                None,
                None,
            )
            .await
            .unwrap();
        let send_count = Arc::new(AtomicUsize::new(0));
        let sender = MockSender {
            send_count: send_count.clone(),
            fail: false,
        };
        let worker = OutboxWorker::new(pool.clone(), sender);
        worker.process_pending().await;
        assert_eq!(send_count.load(Ordering::SeqCst), 1);
        let updated = outbox_repo.get_by_id(msg.id).await.unwrap();
        assert_eq!(updated.status, "sent");
    }

    #[tokio::test]
    async fn test_outbox_worker_respects_retry_limit() {
        let (pool, _temp_dir) = init_test_pool().await.unwrap();
        let acc_repo = AccountRepository::new(&pool);
        let outbox_repo = OutboxRepository::new(&pool);
        let acc = acc_repo
            .create(
                "test@example.com",
                "test",
                "imap",
                993,
                "smtp",
                587,
                "password",
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Capture the message so we can query its exact DB state later.
        let msg = outbox_repo
            .enqueue(
                &acc.id,
                b"Raw MIME",
                "test@example.com",
                &["dest@example.com".into()],
                Some("Subj"),
                None,
                None,
            )
            .await
            .unwrap();

        let sender = MockSender {
            send_count: Arc::new(AtomicUsize::new(0)),
            fail: true,
        };
        let worker = OutboxWorker::new(pool.clone(), sender);

        // Simulate 5 failed loop iterations. After this the message has
        // retry_count = 5 and status = 'failed'.
        for _ in 0..5 {
            worker.process_pending().await;
        }

        // Assert the raw DB state via get_by_id. We must NOT use get_pending
        // here: its query intentionally excludes messages with retry_count >= 5,
        // so a message that has exactly hit the limit is invisible to it.
        let updated_msg = outbox_repo.get_by_id(msg.id).await.unwrap();
        assert_eq!(
            updated_msg.retry_count, 5,
            "Retry count should exactly hit the limit"
        );
        assert_eq!(
            updated_msg.status, "failed",
            "Status should be marked as failed"
        );

        // 6th run should NOT pick it up because the DB query filters
        // retry_count < 5. This prevents infinite retry loops for permanently
        // failed messages (e.g., invalid recipient).
        worker.process_pending().await;
        let pending_after = outbox_repo.get_pending(10).await.unwrap();
        assert_eq!(
            pending_after.len(),
            0,
            "Worker should ignore messages that exceeded the retry limit"
        );
    }
}
