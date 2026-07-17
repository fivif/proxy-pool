use crate::config::Config;
use crate::pool::ProxyPool;
use axum::{Json, Router, extract::State, response::Html, routing::get};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<ProxyPool>,
    pub config: Arc<RwLock<Config>>,
}

pub fn build_router(pool: Arc<ProxyPool>, config: Arc<RwLock<Config>>) -> Router {
    let state = AppState { pool, config };
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(dashboard))
        .route("/dashboard", get(dashboard))
        .route("/api/proxies", get(list_proxies))
        .route("/api/proxy/random", get(random_proxy))
        .route("/api/stats", get(stats))
        .route("/api/config", get(get_config).post(update_config))
        .route("/health", get(health))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

async fn dashboard() -> Html<&'static str> {
    Html(include_str!("dashboard.html"))
}

#[derive(Serialize)]
struct StatsResponse {
    total: usize,
    active: usize,
    cooldown: usize,
    avg_score: f64,
}

async fn stats(State(state): State<AppState>) -> Json<StatsResponse> {
    let total = state.pool.total_count();
    let active = state.pool.active_count();
    let cooldown = state.pool.cooldown_count();
    let avg_score = if total > 0 {
        let snapshots = state.pool.active_snapshots(Some(1000));
        snapshots.iter().map(|s| s.score).sum::<f64>() / snapshots.len() as f64
    } else {
        0.0
    };
    Json(StatsResponse { total, active, cooldown, avg_score })
}

async fn list_proxies(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Vec<crate::pool::proxy::ProxySnapshot>> {
    let limit = params.get("limit").and_then(|v| v.parse::<usize>().ok());
    Json(state.pool.active_snapshots(limit))
}

async fn random_proxy(State(state): State<AppState>) -> Json<Option<crate::pool::proxy::ProxySnapshot>> {
    Json(state.pool.random_best())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

// ===== Config API =====

#[derive(Serialize)]
struct ConfigResponse {
    fetch_upstream_proxy: String,
    fetch_interval_secs: u64,
    health_check_interval_secs: u64,
    max_pool_size: usize,
    cooldown_ttl_secs: u64,
    listen_addr: String,
}

async fn get_config(State(state): State<AppState>) -> Json<ConfigResponse> {
    let cfg = state.config.read().unwrap();
    Json(ConfigResponse {
        fetch_upstream_proxy: cfg.fetch_upstream_proxy.clone(),
        fetch_interval_secs: cfg.fetch_interval.as_secs(),
        health_check_interval_secs: cfg.health_check_interval.as_secs(),
        max_pool_size: cfg.max_pool_size,
        cooldown_ttl_secs: cfg.cooldown_ttl.as_secs(),
        listen_addr: cfg.listen_addr.clone(),
    })
}

#[derive(Deserialize)]
struct UpdateConfigRequest {
    fetch_upstream_proxy: Option<String>,
    fetch_interval_secs: Option<u64>,
    max_pool_size: Option<usize>,
}

async fn update_config(
    State(state): State<AppState>,
    Json(req): Json<UpdateConfigRequest>,
) -> Json<ConfigResponse> {
    let mut cfg = state.config.write().unwrap();
    if let Some(v) = req.fetch_upstream_proxy {
        cfg.fetch_upstream_proxy = v;
    }
    if let Some(v) = req.fetch_interval_secs {
        cfg.fetch_interval = std::time::Duration::from_secs(v.max(30)); // min 30s
    }
    if let Some(v) = req.max_pool_size {
        cfg.max_pool_size = v.min(50000);
    }
    Json(ConfigResponse {
        fetch_upstream_proxy: cfg.fetch_upstream_proxy.clone(),
        fetch_interval_secs: cfg.fetch_interval.as_secs(),
        health_check_interval_secs: cfg.health_check_interval.as_secs(),
        max_pool_size: cfg.max_pool_size,
        cooldown_ttl_secs: cfg.cooldown_ttl.as_secs(),
        listen_addr: cfg.listen_addr.clone(),
    })
}
