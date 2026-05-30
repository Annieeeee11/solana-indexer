use crate::data_sources::solana_rpc::SolanaRpc;
use crate::storage::cache::multi_cache::MultiCache;
use crate::storage::factory::create_storage;
use crate::utils::config::Config;
use crate::utils::errors::Result;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppContext {
    pub config: Config,
    pub cache: Arc<MultiCache>,
    pub rpc: Arc<SolanaRpc>,
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
}
