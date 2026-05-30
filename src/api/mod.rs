use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use crate::storage::cache::multi_cache::MultiCache;
use crate::utils::errors::{IndexerError, Result};
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone)]
struct ApiState {
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
        .route("/slots/{number}", get(slot_by_number))
        .route("/transactions/{signature}", get(transaction_by_sig))
        .route("/accounts/{address}", get(account_by_address))
        .with_state(ApiState { cache })
}

pub async fn serve(cache: Arc<MultiCache>, port: u16) -> Result<()> {
    let app = router(cache);
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| IndexerError::ConfigError(format!("Failed to bind {addr}: {e}")))?;

    tracing::info!("HTTP API listening on http://{addr}");

    axum::serve(
        listener,
        app.into_make_service(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
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

async fn slot_by_number(
    State(state): State<ApiState>,
    Path(number): Path<u64>,
) -> Response {
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
