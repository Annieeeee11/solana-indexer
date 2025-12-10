use crate::data_sources::solana_rpc::SolanaRpc;
use crate::storage::cache::multi_cache::MultiCache;
use crate::utils::errors::Result;
use std::sync::Arc;
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

    pub async fn start(&self) -> Result<()> {
        let mut ticker = interval(Duration::from_secs(POLL_INTERVAL_SECS));
        let accounts = self.accounts_to_watch.clone();

        loop {
            ticker.tick().await;

            for address in &accounts {
                match self.rpc.get_account(address).await {
                    Ok(account) => {
                        if let Some(previous) = self.cache.get_account(address).await? {
                            if previous.lamports != account.lamports
                                || previous.data != account.data
                            {
                                tracing::info!("Account {} changed", address);
                            }
                        }
                        self.cache.store_account(account).await?;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch account {}: {}", address, e);
                    }
                }
            }
        }
    }
}

