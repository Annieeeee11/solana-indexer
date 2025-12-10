use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub rpc: RpcConfig,
    pub storage: StorageConfig,
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub solana_rpc_url: String,
    pub yellowstone_grpc_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub sqlite_path: PathBuf,
    pub postgres_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub l1_size: usize,
    pub l2_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rpc: RpcConfig {
                solana_rpc_url: "https://api.mainnet-beta.solana.com".into(),
                yellowstone_grpc_url: None,
            },
            storage: StorageConfig {
                sqlite_path: "indexer.db".into(),
                postgres_url: None,
            },
            cache: CacheConfig {
                l1_size: 1000,
                l2_size: 10000,
            },
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();
        
        Ok(Self {
            rpc: RpcConfig {
                solana_rpc_url: std::env::var("SOLANA_RPC_URL")
                    .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".into()),
                yellowstone_grpc_url: std::env::var("YELLOWSTONE_GRPC_URL").ok(),
            },
            storage: StorageConfig {
                sqlite_path: std::env::var("SQLITE_DB_PATH")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| "indexer.db".into()),
                postgres_url: std::env::var("DATABASE_URL").ok(),
            },
            cache: CacheConfig {
                l1_size: std::env::var("CACHE_L1_SIZE").ok().and_then(|v| v.parse().ok()).unwrap_or(1000),
                l2_size: std::env::var("CACHE_L2_SIZE").ok().and_then(|v| v.parse().ok()).unwrap_or(10000),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let c = Config::default();
        assert!(c.rpc.solana_rpc_url.contains("solana.com"));
        assert_eq!(c.cache.l1_size, 1000);
    }

    #[test]
    fn loads() {
        assert!(Config::load().is_ok());
    }
}
