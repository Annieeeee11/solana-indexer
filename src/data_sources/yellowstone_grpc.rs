use crate::core::types::{AccountState, Slot, SlotStatus, TransactionInfo};
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

const ACCOUNT_CHANNEL_CAPACITY: usize = 1000;

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
    tx_accounts: Vec<String>,
}

impl YellowstoneGrpc {
    pub fn new(url: &str, token: Option<String>, tx_accounts: Vec<String>) -> Self {
        Self {
            url: url.to_string(),
            token,
            tx_accounts,
        }
    }

    fn build_client(&self) -> Result<GeyserGrpcBuilder> {
        let url = normalize_grpc_url(&self.url);
        let mut builder = GeyserGrpcBuilder::from_shared(url.clone())
            .map_err(|e| IndexerError::ConfigError(e.to_string()))?;

        if let Some(t) = &self.token {
            builder = builder
                .x_token(Some(t.clone()))
                .map_err(|e| IndexerError::ConfigError(e.to_string()))?;
        }

        if url.starts_with("https://") {
            ensure_rustls_provider();
            builder = builder
                .tls_config(ClientTlsConfig::new().with_native_roots())
                .map_err(|e| IndexerError::ConfigError(e.to_string()))?;
        }

        Ok(builder
            .connect_timeout(Duration::from_secs(15))
            .http2_keep_alive_interval(Duration::from_secs(30)))
    }

    fn slot_filter() -> HashMap<String, SubscribeRequestFilterSlots> {
        let mut m = HashMap::new();
        m.insert(
            "slots".into(),
            SubscribeRequestFilterSlots {
                filter_by_commitment: Some(true),
                interslot_updates: Some(false),
            },
        );
        m
    }

    fn tx_filter(accounts: &[String]) -> HashMap<String, SubscribeRequestFilterTransactions> {
        if accounts.is_empty() {
            return HashMap::new();
        }

        let mut m = HashMap::new();
        m.insert(
            "txs".into(),
            SubscribeRequestFilterTransactions {
                vote: Some(false),
                account_include: accounts.to_vec(),
                ..Default::default()
            },
        );
        m
    }

    fn account_filter(accounts: &[String]) -> HashMap<String, SubscribeRequestFilterAccounts> {
        let mut m = HashMap::new();
        m.insert(
            "accounts".into(),
            SubscribeRequestFilterAccounts {
                account: accounts.to_vec(),
                owner: vec![],
                filters: vec![],
                nonempty_txn_signature: None,
            },
        );
        m
    }

    fn parse_slot(s: &SubscribeUpdateSlot) -> Slot {
        let parent = s.parent.filter(|&p| p > 0);
        Slot {
            slot: s.slot,
            parent,
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

        let accounts: Vec<String> = msg
            .account_keys
            .iter()
            .map(|k| bs58::encode(k).into_string())
            .collect();

        let program = msg
            .instructions
            .first()
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

    fn parse_account(update: &SubscribeUpdateAccount) -> Option<AccountState> {
        let info = update.account.as_ref()?;
        Some(AccountState {
            address: bs58::encode(&info.pubkey).into_string(),
            slot: update.slot,
            lamports: info.lamports,
            owner: bs58::encode(&info.owner).into_string(),
            executable: info.executable,
            data: info.data.clone(),
            rent_epoch: info.rent_epoch,
        })
    }

    pub async fn subscribe_with_transactions(
        &self,
    ) -> Result<(mpsc::Receiver<Slot>, mpsc::Receiver<TransactionInfo>)> {
        let (slot_tx, slot_rx) = crate::core::channels::slot_channel();
        let (tx_tx, tx_rx) = crate::core::channels::transaction_channel();

        let mut client = self
            .build_client()?
            .connect()
            .await
            .map_err(|e| IndexerError::ConfigError(e.to_string()))?;

        let (mut sink, mut stream) = client
            .subscribe()
            .await
            .map_err(|e| IndexerError::RpcError(e.to_string()))?;

        let tx_filter = Self::tx_filter(&self.tx_accounts);
        if tx_filter.is_empty() {
            tracing::debug!("Yellowstone tx stream disabled (set YELLOWSTONE_TX_ACCOUNTS to enable)");
        }

        let request = SubscribeRequest {
            slots: Self::slot_filter(),
            transactions: tx_filter,
            commitment: Some(CommitmentLevel::Confirmed as i32),
            ..Default::default()
        };

        sink.send(request)
            .await
            .map_err(|e| IndexerError::RpcError(e.to_string()))?;

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

    pub async fn subscribe_accounts(
        &self,
        accounts: &[String],
    ) -> Result<mpsc::Receiver<AccountState>> {
        if accounts.is_empty() {
            return Err(IndexerError::ConfigError(
                "account subscription requires at least one address".into(),
            ));
        }

        let (account_tx, account_rx) = mpsc::channel(ACCOUNT_CHANNEL_CAPACITY);
        let mut client = self
            .build_client()?
            .connect()
            .await
            .map_err(|e| IndexerError::ConfigError(e.to_string()))?;

        let (mut sink, mut stream) = client
            .subscribe()
            .await
            .map_err(|e| IndexerError::RpcError(e.to_string()))?;

        let request = SubscribeRequest {
            accounts: Self::account_filter(accounts),
            commitment: Some(CommitmentLevel::Confirmed as i32),
            ..Default::default()
        };

        sink.send(request)
            .await
            .map_err(|e| IndexerError::RpcError(e.to_string()))?;

        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                let Ok(msg) = msg else {
                    tracing::error!("Account stream error");
                    break;
                };

                if let Some(UpdateOneof::Account(a)) = msg.update_oneof {
                    if let Some(state) = Self::parse_account(&a) {
                        if account_tx.send(state).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Ok(account_rx)
    }

    pub async fn health_ping(&self) -> Result<()> {
        self.build_client()?
            .connect()
            .await
            .map_err(|e| IndexerError::ConfigError(e.to_string()))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::data_sources::YellowstoneSource for YellowstoneGrpc {
    async fn subscribe_with_transactions(
        &self,
    ) -> Result<(mpsc::Receiver<Slot>, mpsc::Receiver<TransactionInfo>)> {
        YellowstoneGrpc::subscribe_with_transactions(self).await
    }

    async fn subscribe_accounts(
        &self,
        accounts: &[String],
    ) -> Result<mpsc::Receiver<AccountState>> {
        YellowstoneGrpc::subscribe_accounts(self, accounts).await
    }

    async fn health_ping(&self) -> Result<()> {
        YellowstoneGrpc::health_ping(self).await
    }
}
