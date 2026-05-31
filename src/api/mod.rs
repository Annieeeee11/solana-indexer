use axum::{
    extract::{Path, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use crate::storage::cache::multi_cache::MultiCache;
use crate::utils::errors::{IndexerError, Result};
use crate::utils::shutdown;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct ApiServeConfig {
    pub cache: Arc<MultiCache>,
    pub port: u16,
    pub api_key: Option<String>,
    pub bind_localhost: bool,
}

#[derive(Clone)]
struct ApiState {
    cache: Arc<MultiCache>,
    api_key: Option<String>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

pub fn router(cache: Arc<MultiCache>, api_key: Option<String>) -> Router {
    let state = ApiState { cache, api_key };
    let mut app = Router::new()
        .route("/health", get(health))
        .route("/slots/latest", get(latest_slot))
        .route("/slots/:number", get(slot_by_number))
        .route("/transactions/:signature", get(transaction_by_sig))
        .route("/accounts/:address", get(account_by_address));

    if state.api_key.is_some() {
        app = app.route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_key,
        ));
    }

    app.with_state(state)
}

async fn require_api_key(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    let Some(expected) = &state.api_key else {
        return Ok(next.run(request).await);
    };

    let authorized = request
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|k| k == expected)
        .unwrap_or(false)
        || request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|token| token == expected)
            .unwrap_or(false);

    if authorized {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Standalone HTTP server (Ctrl+C stops).
pub async fn serve(config: ApiServeConfig) -> Result<()> {
    let shutdown_tx = shutdown::channel();
    shutdown::spawn_on_ctrl_c(
        shutdown_tx.clone(),
        "Shutdown signal received, stopping HTTP API...",
    );
    serve_until_shutdown(config, shutdown_tx.subscribe()).await
}

/// HTTP server until the shared shutdown broadcast fires (used by `indexer start` + `API_PORT`).
pub async fn serve_until_shutdown(
    config: ApiServeConfig,
    mut shutdown: broadcast::Receiver<()>,
) -> Result<()> {
    let ApiServeConfig {
        cache,
        port,
        api_key,
        bind_localhost,
    } = config;

    let host = if bind_localhost { "127.0.0.1" } else { "0.0.0.0" };
    let addr = format!("{host}:{port}");
    let app = router(cache, api_key.clone());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| IndexerError::ConfigError(format!("Failed to bind {addr}: {e}")))?;

    match &api_key {
        Some(_) => tracing::info!("HTTP API listening on http://{addr} (API_KEY required)"),
        None => tracing::info!(
            "HTTP API listening on http://{addr} (no API_KEY — unauthenticated; dev only)"
        ),
    }

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(async move {
            let _ = shutdown.recv().await;
            tracing::info!("HTTP API shutting down");
        })
        .await
        .map_err(|e| IndexerError::ConfigError(format!("HTTP server error: {e}")))?;

    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn latest_slot(State(state): State<ApiState>) -> Response {
    match state.cache.get_latest_slot().await {
        Ok(Some(slot)) => Json(slot).into_response(),
        Ok(None) => not_found("No slots indexed yet"),
        Err(e) => api_error(e),
    }
}

async fn slot_by_number(State(state): State<ApiState>, Path(number): Path<u64>) -> Response {
    match state.cache.get_slot(number).await {
        Ok(Some(slot)) => Json(slot).into_response(),
        Ok(None) => not_found(&format!("Slot {number} not found")),
        Err(e) => api_error(e),
    }
}

async fn transaction_by_sig(
    State(state): State<ApiState>,
    Path(signature): Path<String>,
) -> Response {
    match state.cache.get_transaction(&signature).await {
        Ok(Some(tx)) => Json(tx).into_response(),
        Ok(None) => not_found("Transaction not found"),
        Err(e) => api_error(e),
    }
}

async fn account_by_address(
    State(state): State<ApiState>,
    Path(address): Path<String>,
) -> Response {
    match state.cache.get_account(&address).await {
        Ok(Some(account)) => Json(account).into_response(),
        Ok(None) => not_found("Account not found"),
        Err(e) => api_error(e),
    }
}

fn not_found(message: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorBody {
            error: message.to_string(),
        }),
    )
        .into_response()
}

fn api_error(err: IndexerError) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody {
            error: err.to_string(),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use crate::core::types::Transaction;
    use crate::storage::database::DatabaseStorage;
    use crate::testing::fixtures::sample_slot;
    use crate::testing::mock_db::MockDatabase;
    use crate::utils::metrics::IndexerMetrics;
    use tower::ServiceExt;

    fn test_cache(db: Arc<MockDatabase>) -> Arc<MultiCache> {
        Arc::new(MultiCache::new(10, 10, 10, db, IndexerMetrics::new()))
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = router(test_cache(Arc::new(MockDatabase::new())), None);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn latest_slot_returns_json() {
        let db = Arc::new(MockDatabase::new());
        db.store_slot(&sample_slot(42)).await.unwrap();
        let app = router(test_cache(db), None);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/slots/latest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn missing_slot_returns_404() {
        let app = router(test_cache(Arc::new(MockDatabase::new())), None);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/slots/999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn transaction_endpoint_returns_stored_tx() {
        let db = Arc::new(MockDatabase::new());
        let tx = Transaction {
            signature: "sigtest".into(),
            slot: 1,
            block_time: None,
            fee: 100,
            success: true,
            accounts: vec![],
        };
        db.store_transaction(tx).await.unwrap();
        let app = router(test_cache(db), None);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/transactions/sigtest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_required_when_configured() {
        let app = router(
            test_cache(Arc::new(MockDatabase::new())),
            Some("secret".into()),
        );

        let denied = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

        let ok = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("x-api-key", "secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ok.status(), StatusCode::OK);
    }
}
