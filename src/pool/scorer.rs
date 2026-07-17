use super::proxy::Proxy;

/// Recalculate and update a proxy's score based on its current metrics.
/// Score formula (0-10000 fixed-point):
///   success_rate * 40 + anonymity * 15 + latency_factor * 25 + freshness * 20
pub fn recalculate_score(proxy: &Proxy) -> f64 {
    let success = proxy.success_rate();               // 0.0–1.0
    let anonymity = proxy.anonymity.score();           // 0.3–1.0
    let latency_ms = proxy.avg_latency_ms();

    // Latency factor: 0 at >5000ms, 1.0 at <50ms, linear in between
    let latency_factor = if latency_ms <= 50.0 {
        1.0
    } else if latency_ms >= 5000.0 {
        0.0
    } else {
        1.0 - (latency_ms - 50.0) / 4950.0
    };

    // Freshness: elapsed since last success (0 = very stale, 1 = < 60s ago)
    let freshness = proxy.last_success.lock()
        .map(|ls| {
            let elapsed_secs = (chrono::Utc::now() - ls).num_seconds() as f64;
            if elapsed_secs <= 60.0 { 1.0 }
            else if elapsed_secs >= 1800.0 { 0.0 }
            else { 1.0 - (elapsed_secs - 60.0) / 1740.0 }
        })
        .unwrap_or(0.0);

    let raw = success * 40.0 + anonymity * 15.0 + latency_factor * 25.0 + freshness * 20.0;
    let score = raw.clamp(0.0, 100.0);
    proxy.score.store((score * 100.0) as u64, std::sync::atomic::Ordering::Relaxed);
    score
}
