use colored::*;
use crate::context::AppContext;
use crate::utils::cli_animations::Cli;
use crate::utils::errors::Result;

pub async fn wallet_add(address: String, name: Option<String>) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    ctx.cache.add_wallet(address.clone(), name).await?;
    Cli::success(&format!("Added: {}", address));
    Ok(())
}

pub async fn wallet_remove(address: String) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    ctx.cache.remove_wallet(&address).await?;
    Cli::success(&format!("Removed: {}", address));
    Ok(())
}

pub async fn wallet_list(detailed: bool) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    let wallets = ctx.cache.list_wallets(true).await?;

    if wallets.is_empty() {
        Cli::warning("No wallets.");
        return Ok(());
    }

    println!();
    for (addr, name, _) in wallets {
        let n = name.as_deref().unwrap_or("unnamed");
        if detailed {
            println!(
                "    {} {}",
                addr.bright_white(),
                format!("({})", n).bright_black()
            );
        } else {
            Cli::wallet(&addr, n);
        }
    }
    println!();

    Ok(())
}
