use crate::data_sources::solana_rpc::SolanaRpc;
use crate::data_sources::yellowstone_grpc::YellowstoneGrpc;
use crate::data_sources::{AccountSource, SlotSource, YellowstoneSource};
use crate::storage::cache::multi_cache::MultiCache;
use crate::storage::factory::create_storage;
use crate::utils::config::Config;
use crate::utils::errors::Result;
use crate::utils::metrics::IndexerMetrics;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct AppContext {
    pub config: Config,
    pub cache: Arc<MultiCache>,
    pub metrics: Arc<IndexerMetrics>,
    rpc: Arc<SolanaRpc>,
    yellowstone: Option<Arc<dyn YellowstoneSource>>,
}

impl AppContext {
    pub async fn new() -> Result<Self> {
        let config = Config::load()?;
        config.warn_if_misconfigured();

        let metrics = IndexerMetrics::new();
        let db = create_storage(&config.storage).await?;
        let cache = Arc::new(MultiCache::new(
            config.cache.l1_size,
            config.cache.l2_size,
            config.cache.l3_size,
            db,
            metrics.clone(),
        ));
        let rpc = Arc::new(SolanaRpc::new(&config.rpc.solana_rpc_url, metrics.clone()));
        let yellowstone = config
            .rpc
            .yellowstone_grpc_url
            .as_ref()
            .map(|url| {
                tracing::info!("Yellowstone gRPC configured at {url}");
                Arc::new(YellowstoneGrpc::new(
                    url,
                    config.rpc.yellowstone_grpc_token.clone(),
                )) as Arc<dyn YellowstoneSource>
            });

        Ok(Self {
            config,
            cache,
            metrics,
            rpc,
            yellowstone,
        })
    }

    pub fn slot_enrich_interval(&self) -> Duration {
        Duration::from_millis(self.config.slot_enrich_min_interval_ms)
    }

    #[cfg(test)]
    pub fn from_parts(
        config: Config,
        cache: Arc<MultiCache>,
        rpc_url: &str,
        metrics: Arc<IndexerMetrics>,
    ) -> Self {
        Self {
            config,
            cache,
            metrics: metrics.clone(),
            rpc: Arc::new(SolanaRpc::new(rpc_url, metrics)),
            yellowstone: None,
        }
    }

    pub fn account_source(&self) -> Arc<dyn AccountSource> {
        Arc::clone(&self.rpc) as Arc<dyn AccountSource>
    }

    pub fn slot_source(&self) -> Arc<dyn SlotSource> {
        Arc::clone(&self.rpc) as Arc<dyn SlotSource>
    }

    pub fn yellowstone_source(&self) -> Option<Arc<dyn YellowstoneSource>> {
        self.yellowstone.clone()
    }

    pub fn streaming_mode_label(&self) -> &'static str {
        if self.yellowstone.is_some() {
            "Yellowstone gRPC primary (RPC fallback if unavailable)"
        } else {
            "RPC polling (set YELLOWSTONE_GRPC_URL for gRPC streaming)"
        }
    }
}
