mod config;
mod pool;
mod checker;
mod fetcher;
mod api;
mod storage;

use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,proxy_pool=debug"))
        )
        .with_target(false)
        .with_thread_ids(true)
        .with_line_number(false)
        .init();

    let config = Arc::new(config::Config::default());
    let pool = Arc::new(pool::ProxyPool::new(config.clone()));

    info!("🚀 Proxy Pool starting on {}", config.listen_addr);
    info!("   Max: {} | Check: {:?} | Fetch: {:?} | Cooldown: {:?} | Concurrency: {}",
        config.max_pool_size, config.health_check_interval, config.fetch_interval,
        config.cooldown_ttl, config.validation_concurrency);

    // Restore from previous session
    let snapshot_path = std::env::temp_dir().join("proxy-pool-snapshot.json");
    if snapshot_path.exists() {
        match storage::load_snapshots(&snapshot_path) {
            Ok(snapshots) => {
                let n = storage::restore_from_snapshots(&pool, &snapshots);
                info!("📂 Restored {n} proxies from snapshot");
            }
            Err(e) => warn!("Snapshot load failed: {e}"),
        }
    }

    // Start HTTP server FIRST
    let router = api::build_router(pool.clone());
    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await.unwrap();
    info!("🌐 API listening on http://{}", config.listen_addr);

    let server_pool = pool.clone();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Initial fetch in background
    let fetch_pool = pool.clone();
    let fetch_config = config.clone();
    tokio::spawn(async move {
        info!("🔍 Initial proxy fetch...");
        let n = fetcher::fetch_all(fetch_pool.clone(), fetch_config.clone()).await;
        info!("✅ Initial fetch: {n} new proxies");

        if fetch_pool.total_count() > 0 {
            info!("🩺 Initial health check...");
            checker::run_health_check(fetch_pool.clone(), fetch_config.clone()).await;
            info!("✅ Health check done. Active: {}", fetch_pool.active_count());
        }
    });

    // Periodic health check + maintenance
    let hc_pool = pool.clone();
    let hc_config = config.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(hc_config.health_check_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            checker::run_health_check(hc_pool.clone(), hc_config.clone()).await;
            hc_pool.evict_low_quality();
            hc_pool.evict_expired_cooldowns();
            info!("📊 Pool: {}/{} active | {} cooling | {} total",
                hc_pool.active_count(), hc_pool.active_count() + hc_pool.cooldown_count(),
                hc_pool.cooldown_count(), hc_pool.total_count());
        }
    });

    // Periodic fetch
    let fp = pool.clone();
    let fc = config.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(fc.fetch_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            fetcher::fetch_all(fp.clone(), fc.clone()).await;
        }
    });

    // Snapshot persistence (60s)
    let sp = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let snapshots = sp.active_snapshots(Some(5000));
            if let Err(e) = storage::save_snapshots(&snapshot_path, &snapshots) {
                warn!("Snapshot save failed: {e}");
            }
        }
    });

    // Keep main alive
    tokio::signal::ctrl_c().await.unwrap();
    info!("Shutting down...");
}
