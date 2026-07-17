mod config;
mod pool;
mod checker;
mod fetcher;
mod api;
mod storage;

use std::sync::{Arc, RwLock};
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

    let config = Arc::new(RwLock::new(config::Config::default()));

    {
        let cfg = config.read().unwrap();
        info!("🚀 Proxy Pool starting on {}", cfg.listen_addr);
        info!("   Max: {} | Check: {:?} | Fetch: {:?} | Cooldown: {:?} | Concurrency: {}",
            cfg.max_pool_size, cfg.health_check_interval, cfg.fetch_interval,
            cfg.cooldown_ttl, cfg.validation_concurrency);
        if cfg.fetch_upstream_proxy.is_empty() {
            info!("   🌐 Fetch upstream: direct (no proxy)");
        } else {
            info!("   🔀 Fetch upstream: {}", cfg.fetch_upstream_proxy);
        }
    }

    let init_cfg = config.read().unwrap().clone();
    let pool = Arc::new(pool::ProxyPool::new(Arc::new(init_cfg)));

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
    let router = api::build_router(pool.clone(), config.clone());
    let listen_addr = config.read().unwrap().listen_addr.clone();
    let listener = tokio::net::TcpListener::bind(&listen_addr).await.unwrap();
    info!("🌐 API listening on http://{}", listen_addr);

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
            let cfg = fetch_config.read().unwrap().clone();
            info!("🩺 Initial health check...");
            checker::run_health_check(fetch_pool.clone(), cfg).await;
            info!("✅ Health check done. Active: {}", fetch_pool.active_count());
        }
    });

    // Periodic health check + maintenance
    let hc_pool = pool.clone();
    let hc_config = config.clone();
    tokio::spawn(async move {
        let hc_interval = hc_config.read().unwrap().health_check_interval;
        let mut interval = tokio::time::interval(hc_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let cfg = hc_config.read().unwrap().clone();
            checker::run_health_check(hc_pool.clone(), cfg).await;
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
        let fetch_interval = fc.read().unwrap().fetch_interval;
        let mut interval = tokio::time::interval(fetch_interval);
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
