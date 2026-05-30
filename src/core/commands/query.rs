use colored::*;
use crate::context::AppContext;
use crate::utils::cli_animations::Cli;
use crate::utils::errors::Result;

pub async fn query_latest() -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    match ctx.cache.get_latest_slot().await? {
        Some(slot) => {
            Cli::success("Latest slot (L1 → DB fallback)");
            Cli::slot(&slot, None);
        }
        None => Cli::warning("No slots indexed yet. Run `indexer start` first."),
    }
    Ok(())
}

pub async fn query_slot(number: u64) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    match ctx.cache.get_slot(number).await? {
        Some(slot) => {
            Cli::success(&format!("Slot {} (L1 → DB fallback)", number));
            Cli::slot(&slot, None);
        }
        None => Cli::warning(&format!("Slot {} not found in cache or DB", number)),
    }
    Ok(())
}

pub async fn query_tx(signature: String) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    match ctx.cache.get_transaction(&signature).await? {
        Some(tx) => {
            Cli::success("Transaction (L2 → DB fallback)");
            println!();
            println!(
                "    {} {}",
                "Sig:".bright_white(),
                tx.signature.bright_cyan()
            );
            println!(
                "    {} {}  {} {}",
                "Slot:".bright_white(),
                tx.slot.to_string().bright_yellow(),
                "Fee:".bright_white(),
                tx.fee
            );
            println!(
                "    {} {}",
                "Success:".bright_white(),
                if tx.success {
                    "yes".bright_green()
                } else {
                    "no".bright_red()
                }
            );
            println!(
                "    {} {}",
                "Accounts:".bright_white(),
                tx.accounts.len()
            );
            println!();
        }
        None => Cli::warning("Transaction not found in cache or DB"),
    }
    Ok(())
}

pub async fn query_account(address: String) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    match ctx.cache.get_account(&address).await? {
        Some(acc) => {
            Cli::success("Account (L3 → DB fallback)");
            Cli::account(&acc);
        }
        None => Cli::warning("Account not found in cache or DB"),
    }
    Ok(())
}
