use crate::core::channels;
use crate::core::types::{Slot, SlotStatus, TransactionInfo};
use crate::utils::errors::{IndexerError, Result};
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Once;
use std::time::Duration;
use tokio::sync::mpsc;
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcBuilder};
use yellowstone_grpc_proto::prelude::*;
use yellowstone_grpc_proto::prelude::subscribe_update::UpdateOneof;

static RUSTLS_PROVIDER: Once = Once::new();

fn ensure_rustls_provider() {
    RUSTLS_PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn normalize_grpc_url(url: &str) -> String {
    let url = url.trim();
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{url}")
    }
}

pub struct YellowstoneGrpc {
    url: String,
    token: Option<String>,
}

impl YellowstoneGrpc {
    pub fn new(url: &str, token: Option<String>) -> Self {
        Self {
            url: url.to_string(),
            token,
        }
    }

    fn slot_filter() -> HashMap<String, SubscribeRequestFilterSlots> {
        let mut m = HashMap::new();
        m.insert("slots".into(), SubscribeRequestFilterSlots {
            filter_by_commitment: Some(true),
            interslot_updates: Some(false),
        });
        m
    }

    fn tx_filter() -> HashMap<String, SubscribeRequestFilterTransactions> {
        // Empty: providers like RPCFast reject unfiltered mainnet tx subscriptions.
        HashMap::new()
     }

    fn parse_slot(s: &SubscribeUpdateSlot) -> Slot {
        Slot {
            slot: s.slot,
            parent: s.parent,
            status: match s.status {
                0 => SlotStatus::Processed,
                1 => SlotStatus::Confirmed,
                _ => SlotStatus::Finalized,
            },
            timestamp: chrono::Utc::now().timestamp(),
            block_hash: None,
            block_height: None,
        }
    }

    fn parse_tx(t: &SubscribeUpdateTransaction) -> Option<TransactionInfo> {
        let tx = t.transaction.as_ref()?;
        let meta = tx.meta.as_ref();
        let msg = tx.transaction.as_ref()?.message.as_ref()?;

        let accounts: Vec<String> = msg.account_keys.iter()
            .map(|k| bs58::encode(k).into_string())
            .collect();

        let program = msg.instructions.first()
            .and_then(|ix| accounts.get(ix.program_id_index as usize))
            .cloned()
            .unwrap_or_else(|| "Unknown".into());

        Some(TransactionInfo {
            signature: bs58::encode(&tx.signature).into_string(),
            slot: t.slot,
            success: meta.map(|m| m.err.is_none()).unwrap_or(false),
            fee: meta.map(|m| m.fee).unwrap_or(0),
            program,
            instructions: msg.instructions.len(),
            compute_units: meta.and_then(|m| m.compute_units_consumed).unwrap_or(0),
            accounts,
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    pub async fn subscribe_with_transactions(&self) -> Result<(mpsc::Receiver<Slot>, mpsc::Receiver<TransactionInfo>)> {
        let (slot_tx, slot_rx) = channels::slot_channel();
        let (tx_tx, tx_rx) = channels::transaction_channel();

        let url = normalize_grpc_url(&self.url);
        let mut builder = GeyserGrpcBuilder::from_shared(url.clone())
            .map_err(|e| IndexerError::ConfigError(e.to_string()))?;

        if let Some(t) = &self.token {
            builder = builder.x_token(Some(t.clone()))
                .map_err(|e| IndexerError::ConfigError(e.to_string()))?;
        }

        if url.starts_with("https://") {
            ensure_rustls_provider();
            builder = builder
                .tls_config(ClientTlsConfig::new().with_native_roots())
                .map_err(|e| IndexerError::ConfigError(e.to_string()))?;
        }

        builder = builder
            .connect_timeout(Duration::from_secs(15))
            .http2_keep_alive_interval(Duration::from_secs(30));

        let mut client = builder.connect().await
            .map_err(|e| IndexerError::ConfigError(e.to_string()))?;

        let (mut sink, mut stream) = client.subscribe().await
            .map_err(|e| IndexerError::RpcError(e.to_string()))?;

        let request = SubscribeRequest {
            slots: Self::slot_filter(),
            transactions: Self::tx_filter(),
            commitment: Some(CommitmentLevel::Confirmed as i32),
            ..Default::default()
        };

        sink.send(request).await.map_err(|e| IndexerError::RpcError(e.to_string()))?;

        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                let Ok(msg) = msg else {
                    tracing::error!("Stream error");
                    break;
                };

                match msg.update_oneof {
                    Some(UpdateOneof::Slot(s)) => {
                        if slot_tx.send(Self::parse_slot(&s)).await.is_err() {
                            break;
                        }
                    }
                    Some(UpdateOneof::Transaction(t)) => {
                        if let Some(info) = Self::parse_tx(&t) {
                            if tx_tx.send(info).await.is_err() {
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        Ok((slot_rx, tx_rx))
    }
}

#[async_trait::async_trait]
impl crate::data_sources::YellowstoneSource for YellowstoneGrpc {
    async fn subscribe_with_transactions(
        &self,
    ) -> Result<(mpsc::Receiver<Slot>, mpsc::Receiver<TransactionInfo>)> {
        YellowstoneGrpc::subscribe_with_transactions(self).await
    }
}