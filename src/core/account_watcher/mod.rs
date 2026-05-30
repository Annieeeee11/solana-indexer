use crate::core::types::AccountState;
use crate::data_sources::solana_rpc::SolanaRpc;
use crate::storage::cache::multi_cache::MultiCache;
use crate::utils::errors::Result;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};

const POLL_INTERVAL_SECS: u64 = 5;

pub struct AccountWatcher {
    rpc: Arc<SolanaRpc>,
    cache: Arc<MultiCache>,
    accounts_to_watch: Vec<String>,
}

impl AccountWatcher {
    pub fn new(rpc: Arc<SolanaRpc>, cache: Arc<MultiCache>) -> Self {
        Self {
            rpc,
            cache,
            accounts_to_watch: vec![],
        }
    }

    pub fn add_account(&mut self, address: String) {
        self.accounts_to_watch.push(address);
    }

    pub fn with_accounts(rpc: Arc<SolanaRpc>, cache: Arc<MultiCache>, accounts: Vec<String>) -> Self {
        Self {
            rpc,
            cache,
            accounts_to_watch: accounts,
        }
    }

    /// Fetches one account and seeds the L3 cache (used before `run`).
    pub async fn fetch_account(&self, address: &str) -> Result<AccountState> {
        let account = self.rpc.get_account(address).await?;
        self.cache.store_account(account.clone()).await?;
        Ok(account)
    }

    /// Seeds L3 cache for all registered accounts.
    pub async fn seed_accounts(&self) -> Result<()> {
        for address in &self.accounts_to_watch {
            if let Err(e) = self.fetch_account(address).await {
                tracing::warn!("Failed to seed account {}: {}", address, e);
            }
        }
        Ok(())
    }

    /// Polls accounts until Ctrl+C (standalone commands).
    pub async fn run<F>(&self, on_change: F) -> Result<()>
    where
        F: FnMut(&str, &AccountState, &AccountState),
    {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let shutdown_task = tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = shutdown_tx.send(());
        });

        let result = self.run_until(on_change, shutdown_rx).await;
        shutdown_task.abort();
        result
    }

    /// Polls accounts until the shared shutdown broadcast fires (used by runtime).
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
                        match self.rpc.get_account(address).await {
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
                                tracing::warn!("Failed to fetch account {}: {}", address, e);
                            }
                        }
                    }
                }
            }
        }
    }
}
