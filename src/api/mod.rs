use crate::pool::ProxyPool;
use axum::{Json, Router, extract::State, response::Html, routing::get};
use serde::Serialize;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<ProxyPool>,
}

pub fn build_router(pool: Arc<ProxyPool>) -> Router {
    let state = AppState { pool };
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(dashboard))  // 🎨 首页 = 管理面板
        .route("/dashboard", get(dashboard))
        .route("/api/proxies", get(list_proxies))
        .route("/api/proxy/random", get(random_proxy))
        .route("/api/stats", get(stats))
        .route("/health", get(health))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

/// 🎨 服务内置管理面板
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
