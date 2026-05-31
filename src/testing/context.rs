use crate::context::AppContext;
use crate::storage::cache::multi_cache::MultiCache;
use crate::testing::mock_db::MockDatabase;
use crate::utils::config::{CacheConfig, Config, RpcConfig, StorageConfig};
use crate::utils::metrics::IndexerMetrics;
use std::path::PathBuf;
use std::sync::Arc;

/// Builds an `AppContext` for unit tests (no real DB or network).
pub fn test_context(
    wallets: Vec<String>,
    watch_accounts: Vec<String>,
    api_port: Option<u16>,
) -> AppContext {
    let metrics = IndexerMetrics::new();
    let db = Arc::new(MockDatabase::with_wallets(wallets));
    let cache = Arc::new(MultiCache::new(10, 10, 10, db, metrics.clone()));
    AppContext::from_parts(
        Config {
            rpc: RpcConfig {
                solana_rpc_url: "http://localhost".into(),
                enrichment_rpc_url: None,
                yellowstone_grpc_url: None,
                yellowstone_grpc_token: None,
                yellowstone_tx_accounts: vec![],
            },
            storage: StorageConfig {
                sqlite_path: PathBuf::from("test.db"),
                postgres_url: None,
            },
            cache: CacheConfig {
                l1_size: 10,
                l2_size: 10,
                l3_size: 10,
            },
            watch_accounts,
            api_port,
            api_key: None,
            api_bind_localhost: false,
            slot_enrich_min_interval_ms: 0,
        },
        cache,
        "http://localhost",
        metrics,
    )
}
