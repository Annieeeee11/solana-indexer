use crate::api::{self, ApiServeConfig, ReadinessDeps};
use crate::context::AppContext;
use crate::utils::cli_animations::Cli;
use crate::utils::errors::Result;

pub async fn serve(port: Option<u16>) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    let port = port.or(ctx.config.api_port).unwrap_or(8080);

    Cli::success(&format!("Starting HTTP query API on port {port}"));
    Cli::info("Endpoints: /health, /ready, /slots/latest, /slots/{{n}}, /transactions/{{sig}}, /accounts/{{addr}}");
    if ctx.config.api_key.is_some() {
        Cli::info("Auth: set X-API-Key or Authorization: Bearer <API_KEY>");
    } else {
        Cli::warning("API_KEY not set — server accepts unauthenticated requests");
    }
    if ctx.config.api_bind_localhost {
        Cli::info("Binding to 127.0.0.1 only (API_BIND_LOCALHOST=1)");
    }
    Cli::info("Ctrl+C to stop");

    let readiness = ReadinessDeps {
        cache: ctx.cache.clone(),
        rpc: ctx.rpc_client(),
        yellowstone: ctx.yellowstone_source(),
        yellowstone_connected: None,
    };

    api::serve(ApiServeConfig {
        cache: ctx.cache,
        port,
        api_key: ctx.config.api_key,
        bind_localhost: ctx.config.api_bind_localhost,
        readiness: Some(readiness),
    })
    .await
}
