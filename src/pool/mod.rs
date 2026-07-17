pub mod proxy;
pub mod cooldown;
pub mod scorer;

use crate::config::Config;
use crate::pool::cooldown::CooldownPool;
use crate::pool::proxy::{Anonymity, Protocol, Proxy, ProxySnapshot};
use crate::pool::scorer::recalculate_score;
use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::warn;

pub struct ProxyPool {
    proxies: DashMap<SocketAddr, Arc<Proxy>>,
    cooldown: CooldownPool,
    config: Arc<Config>,
}

impl ProxyPool {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            proxies: DashMap::with_capacity(config.max_pool_size),
            cooldown: CooldownPool::new(config.cooldown_ttl),
            config,
        }
    }

    pub fn upsert(&self, addr: SocketAddr, protocol: Protocol, anonymity: Anonymity, country: String) {
        if self.cooldown.is_cooling(&addr) || self.proxies.contains_key(&addr) {
            return;
        }
        while self.proxies.len() >= self.config.max_pool_size {
            if let Some(worst) = self.find_worst_proxy() {
                self.proxies.remove(&worst);
            } else { break; }
        }
        let mut proxy = Proxy::new(addr, protocol);
        proxy.anonymity = anonymity;
        proxy.country = country;
        recalculate_score(&proxy);
        self.proxies.insert(addr, Arc::new(proxy));
    }

    pub fn batch_insert(&self, entries: Vec<(SocketAddr, Protocol)>) -> usize {
        let mut count = 0;
        for (addr, proto) in entries {
            if self.cooldown.is_cooling(&addr) || self.proxies.contains_key(&addr) { continue; }
            if self.proxies.len() >= self.config.max_pool_size { break; }
            let proxy = Arc::new(Proxy::new(addr, proto));
            recalculate_score(&proxy);
            self.proxies.insert(addr, proxy);
            count += 1;
        }
        count
    }

    pub fn remove(&self, addr: &SocketAddr) {
        self.proxies.remove(addr);
        self.cooldown.remove(addr);
    }

    pub fn cooldown(&self, addr: &SocketAddr) {
        if let Some(r) = self.proxies.get(addr) {
            r.value().in_cooldown.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.cooldown.put(*addr);
    }

    pub fn release_cooldown(&self, addr: &SocketAddr) {
        self.cooldown.remove(addr);
        if let Some(r) = self.proxies.get(addr) {
            r.value().in_cooldown.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Iterate all proxy entries for the health checker.
    pub fn iter_all(&self) -> dashmap::iter::Iter<'_, SocketAddr, Arc<Proxy>> {
        self.proxies.iter()
    }

    pub fn active_snapshots(&self, limit: Option<usize>) -> Vec<ProxySnapshot> {
        let mut all: Vec<_> = self.proxies.iter()
            .filter(|r| !r.value().in_cooldown.load(std::sync::atomic::Ordering::Relaxed))
            .map(|r| ProxySnapshot::from(r.value().as_ref()))
            .collect();
        all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        if let Some(n) = limit { all.truncate(n); }
        all
    }

    pub fn random_best(&self) -> Option<ProxySnapshot> {
        let active: Vec<_> = self.proxies.iter()
            .filter(|r| !r.value().in_cooldown.load(std::sync::atomic::Ordering::Relaxed))
            .map(|r| r.value().clone())
            .collect();
        if active.is_empty() { return None; }
        let total_weight: f64 = active.iter().map(|p| p.score()).sum();
        if total_weight <= 0.0 {
            use rand::Rng;
            let idx = rand::thread_rng().gen_range(0..active.len());
            return Some(ProxySnapshot::from(active[idx].as_ref()));
        }
        use rand::Rng;
        let mut pick = rand::thread_rng().gen_range(0.0..1.0) * total_weight;
        for p in &active {
            pick -= p.score();
            if pick <= 0.0 { return Some(ProxySnapshot::from(p.as_ref())); }
        }
        Some(ProxySnapshot::from(active.last()?.as_ref()))
    }

    pub fn total_count(&self) -> usize { self.proxies.len() }
    pub fn active_count(&self) -> usize {
        self.proxies.iter().filter(|r| !r.value().in_cooldown.load(std::sync::atomic::Ordering::Relaxed)).count()
    }
    pub fn cooldown_count(&self) -> usize { self.cooldown.len() }

    pub fn evict_low_quality(&self) -> usize {
        let min_rate = self.config.min_success_rate;
        let mut to_remove = Vec::new();
        for entry in self.proxies.iter() {
            let p = entry.value();
            if p.total_checks.load(std::sync::atomic::Ordering::Relaxed) >= 5
                && p.success_rate() < min_rate {
                to_remove.push(*entry.key());
            }
        }
        let cnt = to_remove.len();
        for addr in to_remove { self.proxies.remove(&addr); }
        if cnt > 0 { warn!("Evicted {} low-quality proxies", cnt); }
        cnt
    }

    pub fn evict_expired_cooldowns(&self) -> usize { self.cooldown.evict_expired() }

    fn find_worst_proxy(&self) -> Option<SocketAddr> {
        self.proxies.iter()
            .min_by(|a, b| a.value().score().partial_cmp(&b.value().score()).unwrap_or(std::cmp::Ordering::Equal))
            .map(|r| *r.key())
    }
}
