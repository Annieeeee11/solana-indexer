use crate::data_sources::solana_rpc::SolanaRpc;
use crate::data_sources::{AccountSource, SlotSource};
use crate::storage::cache::multi_cache::MultiCache;
use crate::storage::factory::create_storage;
use crate::utils::config::Config;
use crate::utils::errors::Result;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppContext {
    pub config: Config,
    pub cache: Arc<MultiCache>,
    rpc: Arc<SolanaRpc>,
}

impl AppContext {
    pub async fn new() -> Result<Self> {
        let config = Config::load()?;
        let db = create_storage(&config.storage).await?;
        let cache = Arc::new(MultiCache::new(
            config.cache.l1_size,
            config.cache.l2_size,
            config.cache.l3_size,
            db,
        ));
        let rpc = Arc::new(SolanaRpc::new(&config.rpc.solana_rpc_url));

        Ok(Self { config, cache, rpc })
    }

    #[cfg(test)]
    pub fn from_parts(
        config: Config,
        cache: Arc<MultiCache>,
        rpc_url: &str,
    ) -> Self {
        Self {
            config,
            cache,
            rpc: Arc::new(SolanaRpc::new(rpc_url)),
        }
    }

    pub fn account_source(&self) -> Arc<dyn AccountSource> {
        Arc::clone(&self.rpc) as Arc<dyn AccountSource>
    }

    pub fn slot_source(&self) -> Arc<dyn SlotSource> {
        Arc::clone(&self.rpc) as Arc<dyn SlotSource>
    }
}
