use crate::core::channels;
use crate::core::types::{AccountState, Slot, SlotStatus, TransactionInfo};
use crate::data_sources::{AccountSource, SlotSource};
use crate::utils::errors::{IndexerError, Result};
use crate::utils::metrics::IndexerMetrics;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcBlockConfig;
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{EncodedTransaction, TransactionDetails, UiInstruction, UiMessage, UiTransactionEncoding};
use tokio::sync::mpsc;
use tokio::time::Duration;

/// Default polling interval in milliseconds for RPC slot subscription
const DEFAULT_POLL_INTERVAL_MS: u64 = 400;

pub struct SolanaRpc {
    client: Arc<RpcClient>,
    poll_interval: Duration,
    metrics: Arc<IndexerMetrics>,
}

impl SolanaRpc {
    pub fn new(url: &str, metrics: Arc<IndexerMetrics>) -> Self {
        Self {
            client: Arc::new(RpcClient::new(url.to_string())),
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
            metrics,
        }
    }

    pub async fn current_slot(&self) -> Result<u64> {
        self.client
            .get_slot()
            .await
            .map_err(|e| {
                self.metrics.rpc_errors.fetch_add(1, Ordering::Relaxed);
                IndexerError::RpcError(e.to_string())
            })
    }

    pub async fn subscribe_slots(&self) -> Result<mpsc::Receiver<Slot>> {
        let (tx, rx) = channels::slot_channel();
        let client = self.client.clone();
        let poll_interval = self.poll_interval;

        tokio::spawn(async move {
            let mut last = 0u64;
            loop {
                if let Ok(current) = client.get_slot().await {
                    if current > last {
                        let (hash, height) = client.get_block(current).await
                            .map(|b| (Some(b.blockhash.to_string()), b.block_height))
                            .unwrap_or((None, None));

                        let slot = Slot {
                            slot: current,
                            parent: Some(last),
                            status: SlotStatus::Confirmed,
                            timestamp: chrono::Utc::now().timestamp(),
                            block_hash: hash,
                            block_height: height,
                        };

                        if tx.send(slot).await.is_err() { break; }
                        last = current;
                    }
                }
                tokio::time::sleep(poll_interval).await;
            }
        });

        Ok(rx)
    }

    async fn fetch_account_state(&self, address: &str) -> Result<AccountState> {
        let pubkey: Pubkey = address.parse()
            .map_err(|e| IndexerError::RpcError(format!("Invalid address: {}", e)))?;

        let account = self.client.get_account(&pubkey).await
            .map_err(|e| IndexerError::RpcError(format!("RPC error: {}", e)))?;

        let slot = match self.client.get_slot().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Could not fetch current slot for account {}: {}", address, e);
                0
            }
        };

        Ok(AccountState {
            address: address.to_string(),
            slot,
            lamports: account.lamports,
            owner: account.owner.to_string(),
            executable: account.executable,
            data: account.data,
            rent_epoch: account.rent_epoch,
        })
    }

    pub async fn get_block_with_transactions(&self, slot: u64) -> Result<Vec<TransactionInfo>> {
        let config = RpcBlockConfig {
            encoding: Some(UiTransactionEncoding::JsonParsed),
            transaction_details: Some(TransactionDetails::Full),
            rewards: Some(false),
            commitment: None,
            max_supported_transaction_version: Some(0),
        };

        let block = match self.client.get_block_with_config(slot, config).await {
            Ok(b) => b,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("skipped") || err_str.contains("not available") {
                    tracing::debug!("Block {} skipped or not available", slot);
                } else {
                    tracing::warn!("Failed to fetch block {}: {}", slot, e);
                }
                return Ok(vec![]);
            }
        };

        let mut txs = Vec::new();
        for tx in block.transactions.unwrap_or_default() {
            let Some(meta) = &tx.meta else { continue };

            let EncodedTransaction::Json(ui_tx) = &tx.transaction else { continue };
            let Some(sig) = ui_tx.signatures.first() else { continue };

            let accounts: Vec<String> = match &ui_tx.message {
                UiMessage::Parsed(p) => p.account_keys.iter().map(|k| k.pubkey.as_str()).map(String::from).collect(),
                UiMessage::Raw(r) => r.account_keys.clone(),
            };

            let program = match &ui_tx.message {
                UiMessage::Parsed(p) => {
                    p.instructions.first()
                        .and_then(|ix| match ix {
                            UiInstruction::Compiled(compiled) => {
                                accounts.get(compiled.program_id_index as usize).cloned()
                            }
                            UiInstruction::Parsed(_parsed) => {
                                // For parsed instructions, we can't easily extract program_id
                                // The first account is typically the program
                                accounts.get(0).cloned()
                            }
                        })
                        .or_else(|| accounts.get(0).cloned())
                        .unwrap_or_else(|| "Unknown".into())
                }
                UiMessage::Raw(r) => {
                    r.instructions.first()
                        .and_then(|ix| accounts.get(ix.program_id_index as usize))
                        .cloned()
                        .unwrap_or_else(|| "Unknown".into())
                }
            };

            let instruction_count = match &ui_tx.message {
                UiMessage::Parsed(p) => p.instructions.len(),
                UiMessage::Raw(r) => r.instructions.len(),
            };

            txs.push(TransactionInfo {
                signature: sig.clone(),
                slot,
                success: meta.err.is_none(),
                fee: meta.fee,
                program,
                instructions: instruction_count,
                compute_units: meta.compute_units_consumed.clone().unwrap_or(0),
                accounts,
                timestamp: chrono::Utc::now().timestamp(),
            });
        }

        Ok(txs)
    }

    pub async fn get_leader_at_slot(&self, slot: u64) -> Result<String> {
        self.client
            .get_slot_leaders(slot, 1)
            .await
            .map_err(|e| {
                self.metrics.rpc_errors.fetch_add(1, Ordering::Relaxed);
                IndexerError::RpcError(e.to_string())
            })?
            .first()
            .map(|l| l.to_string())
            .ok_or_else(|| IndexerError::RpcError("No leader".into()))
    }

    pub async fn enrich_slot_block_metadata(&self, slot: &mut Slot) -> Result<()> {
        if slot.block_hash.is_some() && slot.block_height.is_some() {
            return Ok(());
        }

        match self.client.get_block(slot.slot).await {
            Ok(block) => {
                slot.block_hash = Some(block.blockhash.to_string());
                slot.block_height = block.block_height;
            }
            Err(e) => {
                self.metrics.rpc_errors.fetch_add(1, Ordering::Relaxed);
                let err_str = e.to_string();
                if err_str.contains("skipped") || err_str.contains("not available") {
                    tracing::debug!("Block {} not available for metadata enrichment", slot.slot);
                } else {
                    tracing::debug!("Failed to enrich slot {} metadata: {}", slot.slot, e);
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl AccountSource for SolanaRpc {
    async fn get_account(&self, address: &str) -> Result<AccountState> {
        self.fetch_account_state(address).await
    }
}

#[async_trait::async_trait]
impl SlotSource for SolanaRpc {
    async fn subscribe_slots(&self) -> Result<mpsc::Receiver<Slot>> {
        SolanaRpc::subscribe_slots(self).await
    }

    async fn get_block_with_transactions(&self, slot: u64) -> Result<Vec<TransactionInfo>> {
        SolanaRpc::get_block_with_transactions(self, slot).await
    }

    async fn get_leader_at_slot(&self, slot: u64) -> Result<String> {
        SolanaRpc::get_leader_at_slot(self, slot).await
    }

    async fn enrich_slot_block_metadata(&self, slot: &mut Slot) -> Result<()> {
        SolanaRpc::enrich_slot_block_metadata(self, slot).await
    }
}
