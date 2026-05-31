use axum::{
    extract::{Path, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use crate::data_sources::solana_rpc::SolanaRpc;
use crate::data_sources::YellowstoneSource;
use crate::storage::cache::multi_cache::MultiCache;
use crate::utils::errors::{IndexerError, Result};
use crate::utils::shutdown;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

const READINESS_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct ReadinessDeps {
    pub cache: Arc<MultiCache>,
    pub rpc: Arc<SolanaRpc>,
    pub yellowstone: Option<Arc<dyn YellowstoneSource>>,
    /// When set (indexer running), readiness requires an active gRPC stream.
    pub yellowstone_connected: Option<Arc<AtomicBool>>,
}

#[derive(Clone)]
pub struct ApiServeConfig {
    pub cache: Arc<MultiCache>,
    pub port: u16,
    pub api_key: Option<String>,
    pub bind_localhost: bool,
    pub readiness: Option<ReadinessDeps>,
}

#[derive(Clone)]
struct ApiState {
    cache: Arc<MultiCache>,
    api_key: Option<String>,
    readiness: Option<ReadinessDeps>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct ReadinessResponse {
    status: &'static str,
    checks: HashMap<String, CheckResult>,
}

#[derive(Serialize)]
struct CheckResult {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

pub fn router(cache: Arc<MultiCache>, api_key: Option<String>, readiness: Option<ReadinessDeps>) -> Router {
    let state = ApiState {
        cache,
        api_key,
        readiness,
    };
    let mut app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
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
        readiness,
    } = config;

    let host = if bind_localhost { "127.0.0.1" } else { "0.0.0.0" };
    let addr = format!("{host}:{port}");
    let app = router(cache, api_key.clone(), readiness);
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

/// Liveness — process is up (no dependency checks).
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// Readiness — DB, RPC, and gRPC (when configured) must be healthy.
async fn ready(State(state): State<ApiState>) -> Response {
    let Some(deps) = &state.readiness else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadinessResponse {
                status: "not_ready",
                checks: HashMap::from([(
                    "config".into(),
                    CheckResult {
                        ok: false,
                        detail: Some("readiness dependencies not configured".into()),
                    },
                )]),
            }),
        )
            .into_response();
    };

    let checks = run_readiness_checks(deps).await;
    let all_ok = checks.values().all(|c| c.ok);
    let status = if all_ok { "ready" } else { "not_ready" };
    let code = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (code, Json(ReadinessResponse { status, checks })).into_response()
}

async fn run_readiness_checks(deps: &ReadinessDeps) -> HashMap<String, CheckResult> {
    let mut checks = HashMap::new();

    checks.insert(
        "database".into(),
        match deps.cache.ping_db().await {
            Ok(()) => CheckResult {
                ok: true,
                detail: None,
            },
            Err(e) => CheckResult {
                ok: false,
                detail: Some(e.to_string()),
            },
        },
    );

    checks.insert(
        "rpc".into(),
        match tokio::time::timeout(READINESS_TIMEOUT, deps.rpc.current_slot()).await {
            Ok(Ok(slot)) => CheckResult {
                ok: true,
                detail: Some(format!("chain head slot {slot}")),
            },
            Ok(Err(e)) => CheckResult {
                ok: false,
                detail: Some(e.to_string()),
            },
            Err(_) => CheckResult {
                ok: false,
                detail: Some(format!(
                    "RPC check timed out after {}s",
                    READINESS_TIMEOUT.as_secs()
                )),
            },
        },
    );

    checks.insert("grpc".into(), check_grpc(deps).await);

    checks
}

async fn check_grpc(deps: &ReadinessDeps) -> CheckResult {
    let Some(yellowstone) = &deps.yellowstone else {
        return CheckResult {
            ok: true,
            detail: Some("not configured".into()),
        };
    };

    if let Some(flag) = &deps.yellowstone_connected {
        if flag.load(Ordering::Relaxed) {
            return CheckResult {
                ok: true,
                detail: Some("stream connected".into()),
            };
        }
        return CheckResult {
            ok: false,
            detail: Some("gRPC configured but stream not connected".into()),
        };
    }

    match tokio::time::timeout(READINESS_TIMEOUT, yellowstone.health_ping()).await {
        Ok(Ok(())) => CheckResult {
            ok: true,
            detail: Some("connectivity ok".into()),
        },
        Ok(Err(e)) => CheckResult {
            ok: false,
            detail: Some(e.to_string()),
        },
        Err(_) => CheckResult {
            ok: false,
            detail: Some(format!(
                "gRPC ping timed out after {}s",
                READINESS_TIMEOUT.as_secs()
            )),
        },
    }
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

    fn test_router(db: Arc<MockDatabase>) -> Router {
        router(test_cache(db), None, None)
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = test_router(Arc::new(MockDatabase::new()));

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
    async fn ready_without_deps_returns_503() {
        let app = test_router(Arc::new(MockDatabase::new()));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn latest_slot_returns_json() {
        let db = Arc::new(MockDatabase::new());
        db.store_slot(&sample_slot(42)).await.unwrap();
        let app = test_router(db);

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
        let app = test_router(Arc::new(MockDatabase::new()));

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
        let app = test_router(db);

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
            None,
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
