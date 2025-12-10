use crate::core::types::Transaction;
use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;

pub struct L2Transactions {
    cache: Arc<Cache<String, Transaction>>,
}

impl L2Transactions {
    pub fn new(max_size: usize) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_size as u64)
            .time_to_live(Duration::from_secs(3600)) 
            .build();
        
        Self {
            cache: Arc::new(cache),
        }
    }

    pub async fn get(&self, signature: &str) -> Option<Transaction> {
        self.cache.get(signature).await
    }

    pub async fn insert(&self, tx: Transaction) {
        self.cache.insert(tx.signature.clone(), tx).await;
    }
}