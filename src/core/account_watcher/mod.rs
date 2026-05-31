use crate::core::types::AccountState;
use crate::data_sources::AccountSource;
use crate::storage::cache::multi_cache::MultiCache;
use crate::utils::errors::Result;
use crate::utils::shutdown;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};

#[cfg(not(test))]
const POLL_INTERVAL_SECS: u64 = 5;

#[cfg(test)]
const POLL_INTERVAL_SECS: u64 = 1;

pub struct AccountWatcher {
    accounts_source: Arc<dyn AccountSource>,
    cache: Arc<MultiCache>,
    accounts_to_watch: Vec<String>,
}

impl AccountWatcher {
    pub fn with_accounts(
        accounts_source: Arc<dyn AccountSource>,
        cache: Arc<MultiCache>,
        accounts: Vec<String>,
    ) -> Self {
        Self {
            accounts_source,
            cache,
            accounts_to_watch: accounts,
        }
    }

    pub async fn fetch_account(&self, address: &str) -> Result<AccountState> {
        let account = self.accounts_source.get_account(address).await?;
        self.cache.store_account(account.clone()).await?;
        Ok(account)
    }

    pub async fn seed_accounts(&self) -> Result<()> {
        for address in &self.accounts_to_watch {
            if let Err(e) = self.fetch_account(address).await {
                tracing::warn!("Failed to seed account {address}: {e}");
            }
        }
        Ok(())
    }

    pub async fn run<F>(&self, on_change: F) -> Result<()>
    where
        F: FnMut(&str, &AccountState, &AccountState),
    {
        let shutdown_tx = shutdown::channel();
        shutdown::spawn_on_ctrl_c(
            shutdown_tx.clone(),
            "Shutdown signal received, stopping account watcher...",
        );
        self.run_until(on_change, shutdown_tx.subscribe()).await
    }

    pub async fn run_until<F>(
        &self,
        mut on_change: F,
        mut shutdown: broadcast::Receiver<()>,
    ) -> Result<()>
    where
        F: FnMut(&str, &AccountState, &AccountState),
    {
        let mut ticker = interval(Duration::from_secs(POLL_INTERVAL_SECS));
        let accounts = self.accounts_to_watch.clone();

        loop {
            tokio::select! {
                biased;
                _ = shutdown.recv() => {
                    tracing::info!("Account watcher stopping");
                    return Ok(());
                }
                _ = ticker.tick() => {
                    for address in &accounts {
                        match self.accounts_source.get_account(address).await {
                            Ok(current) => {
                                if let Some(previous) = self.cache.get_account(address).await? {
                                    if previous.lamports != current.lamports
                                        || previous.data != current.data
                                    {
                                        on_change(address, &previous, &current);
                                    }
                                }
                                self.cache.store_account(current).await?;
                            }
                            Err(e) => {
                                tracing::warn!("Failed to fetch account {address}: {e}");
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mock_sources::{sample_account, MockAccountSource};
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn detects_lamport_change_from_mock_source() {
        let source = Arc::new(MockAccountSource::new());
        source.insert(sample_account("addr1", 1_000_000_000));

        let cache = Arc::new(MultiCache::new(
            10,
            10,
            10,
            Arc::new(crate::testing::mock_db::MockDatabase::new()),
        ));

        let watcher = AccountWatcher::with_accounts(
            source.clone(),
            cache.clone(),
            vec!["addr1".into()],
        );
        watcher.seed_accounts().await.unwrap();

        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let watch_task = tokio::spawn(async move {
            watcher
                .run_until(
                    move |addr, prev, curr| {
                        assert_eq!(addr, "addr1");
                        assert_eq!(prev.lamports, 1_000_000_000);
                        assert_eq!(curr.lamports, 2_000_000_000);
                        let _ = shutdown_tx.send(());
                    },
                    shutdown_rx,
                )
                .await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        source.set_lamports("addr1", 2_000_000_000);

        timeout(Duration::from_secs(2), watch_task)
            .await
            .expect("watcher should detect change within poll interval")
            .expect("watch task should not panic")
            .expect("watch task ok");
    }
}
