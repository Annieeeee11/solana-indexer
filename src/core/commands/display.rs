use crate::core::account_watcher::AccountWatcher;
use crate::core::types::{AccountState, Slot, TransactionInfo};
use crate::utils::cli_animations::Cli;
use crate::utils::errors::Result;
use std::sync::Arc;

pub fn slot_and_tx_handlers() -> (
    Arc<dyn Fn(Slot, Option<String>) + Send + Sync>,
    Arc<dyn Fn(TransactionInfo) + Send + Sync>,
) {
    let on_slot = Arc::new(|slot: Slot, leader: Option<String>| {
        Cli::slot(&slot, leader.as_deref())
    });
    let on_tx = Arc::new(|tx: TransactionInfo| {
        Cli::transaction(
            &tx.signature,
            tx.slot,
            tx.success,
            tx.fee,
            &tx.program,
            tx.instructions,
            tx.compute_units,
        )
    });
    (on_slot, on_tx)
}

pub fn account_change_handler() -> Arc<dyn Fn(&str, &AccountState, &AccountState) + Send + Sync> {
    Arc::new(|addr, prev, curr| {
        Cli::account_change(addr, prev.lamports, curr.lamports, curr.slot);
    })
}

pub async fn run_watcher_with_cli(watcher: &AccountWatcher) -> Result<()> {
    let on_change = account_change_handler();
    watcher
        .run(move |addr, prev, curr| on_change(addr, prev, curr))
        .await
}
