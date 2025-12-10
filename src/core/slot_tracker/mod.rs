use crate::core::types::{Slot, Transaction, TransactionInfo};
use crate::data_sources::solana_rpc::SolanaRpc;
use crate::data_sources::yellowstone_grpc::YellowstoneGrpc;
use crate::storage::cache::multi_cache::MultiCache;
use crate::utils::errors::{IndexerError, Result};
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct SlotTracker {
    yellowstone: Option<Arc<YellowstoneGrpc>>,
    rpc: Arc<SolanaRpc>,
    cache: Arc<MultiCache>,
    slot_tx: mpsc::Sender<Slot>,
    tx_tx: mpsc::Sender<TransactionInfo>,
}

impl SlotTracker {
    pub fn new(
        yellowstone: Option<Arc<YellowstoneGrpc>>,
        rpc: Arc<SolanaRpc>,
        cache: Arc<MultiCache>,
        slot_tx: mpsc::Sender<Slot>,
        tx_tx: mpsc::Sender<TransactionInfo>,
    ) -> Self {
        Self {
            yellowstone,
            rpc,
            cache,
            slot_tx,
            tx_tx,
        }
    }

    pub async fn start(&self) -> Result<()> {
        if let Some(yellowstone) = &self.yellowstone {
            match yellowstone.subscribe_with_transactions().await {
                Ok((slot_stream, tx_stream)) => {
                    tracing::info!("Using Yellowstone gRPC (real-time streaming)");
                    
                    match self.stream_from_yellowstone(slot_stream, tx_stream).await {
                        Ok(_) => return Ok(()),
                        Err(e) => {
                            tracing::warn!("Yellowstone stream failed: {}, falling back to RPC", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Yellowstone connection failed: {}, using RPC polling", e);
                }
            }
        } else {
            tracing::info!("No Yellowstone configured, using RPC polling");
        }
        tracing::info!("Using RPC polling (fallback mode)");
        self.poll_from_rpc().await
    }

    async fn stream_from_yellowstone(
        &self,
        mut slot_stream: mpsc::Receiver<Slot>,
        mut tx_stream: mpsc::Receiver<TransactionInfo>,
    ) -> Result<()> {
        loop {
            tokio::select! {
                slot = slot_stream.recv() => {
                    let Some(slot) = slot else {
                        return Err(IndexerError::RpcError("Yellowstone slot stream closed".into()));
                    };
                    
                    tracing::debug!("Received slot from Yellowstone: {}", slot.slot);
                    
                    if self.slot_tx.send(slot.clone()).await.is_err() {
                        tracing::error!("Failed to send slot to pipeline");
                        continue;
                    }
                    
                    if let Err(e) = self.cache.store_slot(slot).await {
                        tracing::error!("Failed to cache slot: {}", e);
                    }
                }
                tx = tx_stream.recv() => {
                    let Some(tx) = tx else {
                        return Err(IndexerError::RpcError("Yellowstone tx stream closed".into()));
                    };
                    
                    tracing::debug!("Received tx from Yellowstone: {}", tx.signature);
                    
                    if self.tx_tx.send(tx.clone()).await.is_err() {
                        tracing::error!("Failed to send tx to pipeline");
                        continue;
                    }
                    
                    let transaction = Transaction {
                        signature: tx.signature,
                        slot: tx.slot,
                        block_time: Some(tx.timestamp),
                        fee: tx.fee,
                        success: tx.success,
                        accounts: tx.accounts,
                    };
                    
                    if let Err(e) = self.cache.store_transaction(transaction).await {
                        tracing::error!("Failed to cache transaction: {}", e);
                    }
                }
            }
        }
    }

    async fn poll_from_rpc(&self) -> Result<()> {
        let mut slot_stream = self.rpc.subscribe_slots().await?;

        while let Some(slot) = slot_stream.recv().await {
            tracing::debug!("Received slot from RPC: {}", slot.slot);

            // Fetch transactions for this block
            if let Ok(transactions) = self.rpc.get_block_with_transactions(slot.slot).await {
                for tx in transactions {
                    if self.tx_tx.send(tx.clone()).await.is_err() {
                        continue;
                    }
                    
                    let transaction = Transaction {
                        signature: tx.signature,
                        slot: tx.slot,
                        block_time: Some(tx.timestamp),
                        fee: tx.fee,
                        success: tx.success,
                        accounts: tx.accounts,
                    };
                    
                    let _ = self.cache.store_transaction(transaction).await;
                }
            }

            if self.slot_tx.send(slot.clone()).await.is_err() {
                continue;
            }
            
            let _ = self.cache.store_slot(slot).await;
        }

        Ok(())
    }
}
