use crate::api;
use crate::context::AppContext;
use crate::utils::cli_animations::Cli;
use crate::utils::errors::Result;

pub async fn serve(port: Option<u16>) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    let port = port.or(ctx.config.api_port).unwrap_or(8080);

    Cli::success(&format!("Starting HTTP query API on port {port}"));
    Cli::info("Endpoints: /health, /slots/latest, /slots/{{n}}, /transactions/{{sig}}, /accounts/{{addr}}");
    Cli::info("Ctrl+C to stop");

    api::serve(ctx.cache, port).await
}
