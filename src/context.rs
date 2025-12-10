use crate::data_sources::solana_rpc::SolanaRpc;
use crate::storage::cache::multi_cache::MultiCache;
use crate::storage::database::DatabaseStorage;
use crate::storage::factory::create_storage;
use crate::utils::config::Config;
use std::sync::Arc;

pub struct AppContext {
    pub config: Config,
    pub db: Arc<dyn DatabaseStorage>,
    pub cache: Arc<MultiCache>,
    pub rpc: Arc<SolanaRpc>,
}

impl AppContext {
    pub async fn new() -> anyhow::Result<Self> {
        let config = Config::load()?;
        let db = create_storage(&config.storage).await?;
        let cache = Arc::new(MultiCache::new(
            config.cache.l1_size,
            config.cache.l2_size,
            db.clone(),
        ));
        let rpc = Arc::new(SolanaRpc::new(&config.rpc.solana_rpc_url));

        Ok(Self { config, db, cache, rpc })
    }

    pub async fn db_only() -> anyhow::Result<(Config, Arc<dyn DatabaseStorage>)> {
        let config = Config::load()?;
        let db = create_storage(&config.storage).await?;
        Ok((config, db))
    }
}