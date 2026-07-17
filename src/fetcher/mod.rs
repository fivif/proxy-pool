pub mod sources;

use crate::config::Config;
use crate::pool::proxy::Protocol;
use crate::pool::ProxyPool;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, error, info};

pub async fn fetch_all(pool: Arc<ProxyPool>, config: Arc<Config>) -> usize {
    let sources = sources::default_sources();

    let mut client_builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30));

    // 如果配置了上游代理，通过它访问 GitHub 等被墙的源
    if !config.fetch_upstream_proxy.is_empty() {
        if let Ok(proxy) = reqwest::Proxy::all(&config.fetch_upstream_proxy) {
            client_builder = client_builder.proxy(proxy);
            info!(target: "fetcher", "🔀 Fetch via upstream: {}", config.fetch_upstream_proxy);
        } else {
            error!(target: "fetcher", "Bad upstream proxy: {}", config.fetch_upstream_proxy);
        }
    }

    let client = client_builder.build().expect("Failed to build fetch client");

    let semaphore = Arc::new(Semaphore::new(5));
    let mut handles = Vec::new();
    let mut total = 0usize;

    for source in sources {
        let client = client.clone();
        let pool = pool.clone();
        let permit = semaphore.clone().acquire_owned();

        handles.push(tokio::spawn(async move {
            let _permit = permit.await;
            match client.get(source.url).send().await {
                Ok(resp) => match resp.text().await {
                    Ok(body) => {
                        let entries = sources::parse_ip_port_lines(&body);
                        let count = entries.len();
                        let typed: Vec<(SocketAddr, Protocol)> = entries.into_iter()
                            .map(|(addr, p)| {
                                if matches!(p, Protocol::Http) && !matches!(source.default_protocol, Protocol::Http) {
                                    (addr, source.default_protocol)
                                } else {
                                    (addr, p)
                                }
                            })
                            .collect();
                        let added = pool.batch_insert(typed);
                        if added > 0 {
                            info!(target: "fetcher", "{} → {} parsed, {} new", source.name, count, added);
                        } else {
                            debug!("{} → {} parsed, {} added", source.name, count, added);
                        }
                        added
                    }
                    Err(e) => {
                        error!("{} body error: {e}", source.name);
                        0
                    }
                },
                Err(e) => {
                    error!("{} fetch error: {e}", source.name);
                    0
                }
            }
        }));
    }

    for h in handles {
        if let Ok(n) = h.await {
            total += n;
        }
    }

    info!(target: "fetcher", "Fetch cycle complete: {total} new proxies added");
    total
}
