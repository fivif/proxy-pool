use crate::pool::proxy::{Protocol, ProxySnapshot};
use std::path::Path;

/// Persist pool snapshots as JSON file for crash recovery.
pub fn save_snapshots(path: &Path, snapshots: &[ProxySnapshot]) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(snapshots)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load snapshots from JSON file.
pub fn load_snapshots(path: &Path) -> std::io::Result<Vec<ProxySnapshot>> {
    let data = std::fs::read_to_string(path)?;
    let snapshots: Vec<ProxySnapshot> = serde_json::from_str(&data)?;
    Ok(snapshots)
}

/// Restore proxies from snapshots into the pool on startup.
pub fn restore_from_snapshots(
    pool: &crate::pool::ProxyPool,
    snapshots: &[ProxySnapshot],
) -> usize {
    let mut count = 0;
    for s in snapshots {
        if let Ok(addr) = s.addr.parse::<std::net::SocketAddr>() {
            let proto = match s.protocol.as_str() {
                "http" => Protocol::Http,
                "https" => Protocol::Https,
                "socks4" => Protocol::Socks4,
                "socks5" => Protocol::Socks5,
                _ => Protocol::Http,
            };
            let anonymity = match s.anonymity.as_str() {
                "elite" => crate::pool::proxy::Anonymity::Elite,
                "anonymous" => crate::pool::proxy::Anonymity::Anonymous,
                "transparent" => crate::pool::proxy::Anonymity::Transparent,
                _ => crate::pool::proxy::Anonymity::Unknown,
            };
            pool.upsert(addr, proto, anonymity, s.country.clone());
            count += 1;
        }
    }
    count
}
