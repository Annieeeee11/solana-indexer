use crate::core::types::Slot;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Limits how often Yellowstone slots trigger RPC `get_block` enrichment.
pub struct EnrichmentLimiter {
    min_interval: Duration,
    last_attempt: Mutex<Instant>,
}

impl EnrichmentLimiter {
    pub fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            last_attempt: Mutex::new(Instant::now() - min_interval),
        }
    }

    /// Returns true when metadata is missing and the min interval since the last RPC has passed.
    pub fn should_enrich(&self, slot: &Slot) -> bool {
        if slot.block_hash.is_some() && slot.block_height.is_some() {
            return false;
        }

        let mut last = self.last_attempt.lock().expect("enrichment limiter lock");
        if last.elapsed() < self.min_interval {
            return false;
        }
        *last = Instant::now();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixtures::sample_slot;

    #[test]
    fn skips_when_metadata_present() {
        let limiter = EnrichmentLimiter::new(Duration::from_secs(10));
        let mut slot = sample_slot(1);
        slot.block_hash = Some("hash".into());
        slot.block_height = Some(1);
        assert!(!limiter.should_enrich(&slot));
    }

    #[test]
    fn rate_limits_back_to_back_calls() {
        let limiter = EnrichmentLimiter::new(Duration::from_secs(60));
        let slot = sample_slot(1);
        assert!(limiter.should_enrich(&slot));
        assert!(!limiter.should_enrich(&slot));
    }
}
