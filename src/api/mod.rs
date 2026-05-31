use axum::{
    extract::{Path, State},
    http::StatusCode,
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
pub struct ApiState {
    cache: Arc<MultiCache>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

pub fn router(cache: Arc<MultiCache>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/slots/latest", get(latest_slot))
        .route("/slots/:number", get(slot_by_number))
        .route("/transactions/:signature", get(transaction_by_sig))
        .route("/accounts/:address", get(account_by_address))
        .with_state(ApiState { cache })
}

/// Standalone HTTP server (Ctrl+C stops). Binds `0.0.0.0` — use behind a firewall in production.
pub async fn serve(cache: Arc<MultiCache>, port: u16) -> Result<()> {
    let shutdown_tx = shutdown::channel();
    shutdown::spawn_on_ctrl_c(
        shutdown_tx.clone(),
        "Shutdown signal received, stopping HTTP API...",
    );
    serve_until_shutdown(cache, port, shutdown_tx.subscribe()).await
}

/// HTTP server until the shared shutdown broadcast fires (used by `indexer start` + `API_PORT`).
pub async fn serve_until_shutdown(
    cache: Arc<MultiCache>,
    port: u16,
    mut shutdown: broadcast::Receiver<()>,
) -> Result<()> {
    let app = router(cache);
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| IndexerError::ConfigError(format!("Failed to bind {addr}: {e}")))?;

    tracing::info!("HTTP API listening on http://{addr} (dev: no auth; bind all interfaces)");

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
    use crate::core::types::{Slot, SlotStatus, Transaction};
    use crate::storage::database::DatabaseStorage;
    use crate::testing::mock_db::MockDatabase;
    use tower::ServiceExt;

    fn sample_slot(n: u64) -> Slot {
        Slot {
            slot: n,
            parent: Some(n - 1),
            status: SlotStatus::Confirmed,
            timestamp: 1,
            block_hash: None,
            block_height: None,
        }
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let cache = Arc::new(MultiCache::new(10, 10, 10, Arc::new(MockDatabase::new())));
        let app = router(cache);

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
        let cache = Arc::new(MultiCache::new(10, 10, 10, db));
        let app = router(cache);

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
        let cache = Arc::new(MultiCache::new(10, 10, 10, Arc::new(MockDatabase::new())));
        let app = router(cache);

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
        let cache = Arc::new(MultiCache::new(10, 10, 10, db));
        let app = router(cache);

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
}
