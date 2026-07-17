use crate::config::Config;
use crate::pool::ProxyPool;
use crate::pool::scorer::recalculate_score;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tokio::sync::Semaphore;
use tracing::{debug, trace, warn};

pub async fn run_health_check(pool: Arc<ProxyPool>, config: Arc<Config>) {
    let semaphore = Arc::new(Semaphore::new(config.validation_concurrency));
    let mut handles = Vec::new();

    for entry in pool.iter_all() {
        let addr = *entry.key();
        let proxy = entry.value().clone();
        let permit = semaphore.clone().acquire_owned();
        let pool = pool.clone();
        let config = config.clone();

        handles.push(tokio::spawn(async move {
            let _permit = permit.await;
            validate_one(&pool, &config, addr, &proxy).await;
        }));
    }

    for h in handles { let _ = h.await; }
}

async fn validate_one(
    pool: &ProxyPool,
    config: &Config,
    addr: std::net::SocketAddr,
    proxy: &crate::pool::proxy::Proxy,
) {
    let proxy_scheme = match proxy.protocol {
        crate::pool::proxy::Protocol::Http => "http",
        crate::pool::proxy::Protocol::Https => "https",
        crate::pool::proxy::Protocol::Socks4 => "socks4",
        crate::pool::proxy::Protocol::Socks5 => "socks5",
    };
    let proxy_url = format!("{}://{}", proxy_scheme, addr);

    let proxy_obj = match reqwest::Proxy::all(&proxy_url) {
        Ok(p) => p,
        Err(_) => { trace!("Bad proxy: {proxy_url}"); return; }
    };

    let client = match reqwest::Client::builder()
        .timeout(config.validation_timeout)
        .proxy(proxy_obj)
        .no_proxy()  // 确保不走上游10808代理
        .build()
    {
        Ok(c) => c,
        Err(_) => { trace!("Client build fail: {addr}"); return; }
    };

    // 依次尝试多个校验URL
    for test_url in &config.test_urls {
        let start = Instant::now();
        match client.get(test_url).send().await {
            Ok(resp) => {
                let latency = start.elapsed().as_millis() as u64;

                if resp.status().as_u16() != config.test_expected_status {
                    continue; // 状态码不对，试下一个URL
                }

                let body = match resp.text().await {
                    Ok(b) => b,
                    Err(_) => continue,
                };

                if !config.test_expected_body.is_empty() && !body.contains(&config.test_expected_body) {
                    continue; // 内容不对，试下一个URL
                }

                // 通过！
                proxy.record_success(latency);
                recalculate_score(proxy);
                pool.release_cooldown(&addr);
                return;
            }
            Err(e) => {
                trace!("{} via {}: {e}", addr, test_url);
                continue; // 连接失败，试下一个URL
            }
        }
    }

    // 所有URL都失败
    record_fail(pool, config, addr, proxy);
}

fn record_fail(pool: &ProxyPool, config: &Config, addr: std::net::SocketAddr, proxy: &crate::pool::proxy::Proxy) {
    proxy.record_failure();
    recalculate_score(proxy);

    let fails = proxy.consecutive_failures();
    if fails >= config.max_consecutive_failures {
        pool.cooldown(&addr);
        debug!("COOLDOWN {} ({})", addr, fails);
    }
    let total = proxy.total_checks.load(Ordering::Relaxed);
    if total >= 10 && proxy.success_rate() < 0.05 {
        pool.remove(&addr);
        warn!("EVICT {}", addr);
    }
}
