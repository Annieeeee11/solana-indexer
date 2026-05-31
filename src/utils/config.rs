use std::path::PathBuf;

use crate::utils::errors::Result;

#[derive(Debug, Clone)]
pub struct Config {
    pub rpc: RpcConfig,
    pub storage: StorageConfig,
    pub cache: CacheConfig,
    /// Extra comma-separated addresses to watch on `indexer start`.
    pub watch_accounts: Vec<String>,
    /// HTTP query API port (`indexer serve`). Default 8080 when unset.
    pub api_port: Option<u16>,
    /// Optional bearer/API-key auth for the HTTP query API (`API_KEY`).
    pub api_key: Option<String>,
    /// When true, HTTP API binds `127.0.0.1` only (`API_BIND_LOCALHOST=1`).
    pub api_bind_localhost: bool,
    /// Min milliseconds between Yellowstone slot `get_block` enrichment RPC calls.
    pub slot_enrich_min_interval_ms: u64,
}

#[derive(Debug, Clone)]
pub struct RpcConfig {
    pub solana_rpc_url: String,
    pub yellowstone_grpc_url: Option<String>,
    pub yellowstone_grpc_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub sqlite_path: PathBuf,
    pub postgres_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub l1_size: usize,
    pub l2_size: usize,
    pub l3_size: usize,
}

impl Config {
    pub fn load() -> Result<Self> {
        dotenvy::dotenv().ok();

        Ok(Self {
            rpc: RpcConfig {
                solana_rpc_url: std::env::var("SOLANA_RPC_URL")
                    .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".into()),
                yellowstone_grpc_url: std::env::var("YELLOWSTONE_GRPC_URL").ok(),
                yellowstone_grpc_token: std::env::var("YELLOWSTONE_GRPC_TOKEN").ok(),
            },
            storage: StorageConfig {
                sqlite_path: std::env::var("SQLITE_DB_PATH")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| "indexer.db".into()),
                postgres_url: std::env::var("DATABASE_URL").ok(),
            },
            cache: CacheConfig {
                l1_size: std::env::var("CACHE_L1_SIZE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1000),
                l2_size: std::env::var("CACHE_L2_SIZE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(10000),
                l3_size: std::env::var("CACHE_L3_SIZE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(5000),
            },
            watch_accounts: std::env::var("WATCH_ACCOUNTS")
                .ok()
                .map(|s| {
                    s.split(',')
                        .map(str::trim)
                        .filter(|a| !a.is_empty())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
            api_port: std::env::var("API_PORT")
                .ok()
                .and_then(|v| v.parse().ok()),
            api_key: std::env::var("API_KEY").ok().filter(|s| !s.is_empty()),
            api_bind_localhost: std::env::var("API_BIND_LOCALHOST")
                .ok()
                .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true")),
            slot_enrich_min_interval_ms: std::env::var("SLOT_ENRICH_MIN_INTERVAL_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2000),
        })
    }

    /// Log warnings for common misconfiguration (non-fatal).
    pub fn warn_if_misconfigured(&self) {
        if let Some(url) = &self.rpc.yellowstone_grpc_url {
            let lower = url.to_lowercase();
            if lower.contains("mainnet-beta.solana.com")
                || lower.contains("/v1/")
            {
                tracing::warn!(
                    "YELLOWSTONE_GRPC_URL looks like an HTTP JSON-RPC URL; \
                     use a Geyser gRPC endpoint (host:port), not SOLANA_RPC_URL"
                );
            }
        }

        if self.api_port.is_some() && self.api_key.is_none() {
            tracing::warn!(
                "API_PORT is set but API_KEY is not — HTTP API accepts unauthenticated requests"
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_with_defaults() {
        let config = Config::load().expect("config should load");
        assert!(config.rpc.solana_rpc_url.contains("solana.com"));
        assert_eq!(config.cache.l1_size, 1000);
        assert_eq!(config.cache.l2_size, 10000);
        assert_eq!(config.cache.l3_size, 5000);
    }
}
