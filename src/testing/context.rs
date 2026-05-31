use crate::context::AppContext;
use crate::storage::cache::multi_cache::MultiCache;
use crate::testing::mock_db::MockDatabase;
use crate::utils::config::{CacheConfig, Config, RpcConfig, StorageConfig};
use std::path::PathBuf;
use std::sync::Arc;

/// Builds an `AppContext` for unit tests (no real DB or network).
pub fn test_context(
    wallets: Vec<String>,
    watch_accounts: Vec<String>,
    api_port: Option<u16>,
) -> AppContext {
    let db = Arc::new(MockDatabase::with_wallets(wallets));
    AppContext::from_parts(
        Config {
            rpc: RpcConfig {
                solana_rpc_url: "http://localhost".into(),
                yellowstone_grpc_url: None,
                yellowstone_grpc_token: None,
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
        },
        Arc::new(MultiCache::new(10, 10, 10, db)),
        "http://localhost",
    )
}
