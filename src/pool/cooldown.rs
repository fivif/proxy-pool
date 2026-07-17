use dashmap::DashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Manages proxies in cooldown with TTL-based expiration.
pub struct CooldownPool {
    entries: DashMap<SocketAddr, Instant>,
    ttl: Duration,
}

impl CooldownPool {
    pub fn new(ttl: Duration) -> Self {
        Self { entries: DashMap::new(), ttl }
    }

    /// Put a proxy into cooldown. Returns true if it was already cooling.
    pub fn put(&self, addr: SocketAddr) -> bool {
        self.entries.insert(addr, Instant::now()).is_some()
    }

    /// Remove from cooldown (e.g., on successful re-validation).
    pub fn remove(&self, addr: &SocketAddr) -> bool {
        self.entries.remove(addr).is_some()
    }

    /// Check if an address is currently in cooldown (auto-evicts expired).
    pub fn is_cooling(&self, addr: &SocketAddr) -> bool {
        if let Some(entry) = self.entries.get(addr) {
            if entry.value().elapsed() < self.ttl {
                return true;
            }
            drop(entry);
            self.entries.remove(addr);
        }
        false
    }

    /// Get count of proxies in cooldown.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Evict all expired entries; returns number evicted.
    pub fn evict_expired(&self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, v| v.elapsed() < self.ttl);
        before - self.entries.len()
    }
}
