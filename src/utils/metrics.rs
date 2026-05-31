use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// In-process counters exposed via structured `tracing` logs (no Prometheus dependency).
#[derive(Debug, Default)]
pub struct IndexerMetrics {
    pub slots_ingested: AtomicU64,
    pub txs_ingested: AtomicU64,
    pub l1_hits: AtomicU64,
    pub l1_misses: AtomicU64,
    pub l2_hits: AtomicU64,
    pub l2_misses: AtomicU64,
    pub l3_hits: AtomicU64,
    pub l3_misses: AtomicU64,
    pub rpc_errors: AtomicU64,
    pub enrich_rate_limited: AtomicU64,
    pub enrich_success: AtomicU64,
}

impl IndexerMetrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn log_snapshot(&self) {
        tracing::info!(
            slots_ingested = self.slots_ingested.load(Ordering::Relaxed),
            txs_ingested = self.txs_ingested.load(Ordering::Relaxed),
            l1_hits = self.l1_hits.load(Ordering::Relaxed),
            l1_misses = self.l1_misses.load(Ordering::Relaxed),
            l2_hits = self.l2_hits.load(Ordering::Relaxed),
            l2_misses = self.l2_misses.load(Ordering::Relaxed),
            l3_hits = self.l3_hits.load(Ordering::Relaxed),
            l3_misses = self.l3_misses.load(Ordering::Relaxed),
            rpc_errors = self.rpc_errors.load(Ordering::Relaxed),
            enrich_rate_limited = self.enrich_rate_limited.load(Ordering::Relaxed),
            enrich_success = self.enrich_success.load(Ordering::Relaxed),
            "indexer metrics snapshot"
        );
    }

    pub fn maybe_log_periodic(&self, every: u64) {
        let n = self.slots_ingested.load(Ordering::Relaxed);
        if n > 0 && n % every == 0 {
            self.log_snapshot();
        }
    }
}
