use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicBool, Ordering};

/// Anonymity level of a proxy server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anonymity {
    Elite,
    Anonymous,
    Transparent,
    Unknown,
}

impl Anonymity {
    pub fn score(&self) -> f64 {
        match self {
            Self::Elite => 1.0,
            Self::Anonymous => 0.7,
            Self::Transparent => 0.3,
            Self::Unknown => 0.5,
        }
    }
}

/// Supported proxy protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Http,
    Https,
    Socks4,
    Socks5,
}

/// A proxy entry in the pool.
pub struct Proxy {
    pub addr: SocketAddr,
    pub protocol: Protocol,
    pub anonymity: Anonymity,
    pub country: String,

    // Atomic counters for lock-free stats
    pub total_checks: AtomicU64,
    pub success_checks: AtomicU64,
    pub consecutive_failures: AtomicU32,
    pub avg_latency_ms: AtomicU64,   // stored as micros for precision, divided on read
    pub score: AtomicU64,           // fixed-point score: 0–10000 maps to 0.0–100.0

    pub first_seen: DateTime<Utc>,
    pub last_checked: parking_lot::Mutex<DateTime<Utc>>,
    pub last_success: parking_lot::Mutex<Option<DateTime<Utc>>>,
    pub in_cooldown: AtomicBool,
}

impl Proxy {
    pub fn new(addr: SocketAddr, protocol: Protocol) -> Self {
        Self {
            addr,
            protocol,
            anonymity: Anonymity::Unknown,
            country: String::new(),
            total_checks: AtomicU64::new(0),
            success_checks: AtomicU64::new(0),
            consecutive_failures: AtomicU32::new(0),
            avg_latency_ms: AtomicU64::new(0),
            score: AtomicU64::new(5000), // start at 50.0
            first_seen: Utc::now(),
            last_checked: parking_lot::Mutex::new(Utc::now()),
            last_success: parking_lot::Mutex::new(None),
            in_cooldown: AtomicBool::new(false),
        }
    }

    /// Get the current success rate (0.0–1.0).
    #[inline]
    pub fn success_rate(&self) -> f64 {
        let total = self.total_checks.load(Ordering::Relaxed);
        if total == 0 { return 0.0; }
        self.success_checks.load(Ordering::Relaxed) as f64 / total as f64
    }

    /// Get the average latency in milliseconds.
    #[inline]
    pub fn avg_latency_ms(&self) -> f64 {
        self.avg_latency_ms.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Get the current score (0.0–100.0).
    #[inline]
    pub fn score(&self) -> f64 {
        self.score.load(Ordering::Relaxed) as f64 / 100.0
    }

    /// Get consecutive failures.
    #[inline]
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures.load(Ordering::Relaxed)
    }

    /// Record a successful check, updating latencies and counters.
    pub fn record_success(&self, latency_ms: u64) {
        let old_avg = self.avg_latency_ms.load(Ordering::Relaxed);
        let new_avg = if old_avg == 0 {
            latency_ms * 1000
        } else {
            (old_avg * 7 + latency_ms * 1000) / 8 // EMA smoothing
        };
        self.avg_latency_ms.store(new_avg, Ordering::Relaxed);

        self.total_checks.fetch_add(1, Ordering::Relaxed);
        self.success_checks.fetch_add(1, Ordering::Relaxed);
        self.consecutive_failures.store(0, Ordering::Relaxed);
        *self.last_success.lock() = Some(Utc::now());
    }

    /// Record a failed check.
    pub fn record_failure(&self) {
        self.total_checks.fetch_add(1, Ordering::Relaxed);
        self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
    }
}

/// Lightweight snapshot of a proxy for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySnapshot {
    pub addr: String,
    pub protocol: String,
    pub anonymity: String,
    pub country: String,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub score: f64,
    pub total_checks: u64,
    pub consecutive_failures: u32,
    pub in_cooldown: bool,
}

impl From<&Proxy> for ProxySnapshot {
    fn from(p: &Proxy) -> Self {
        Self {
            addr: p.addr.to_string(),
            protocol: format!("{:?}", p.protocol).to_lowercase(),
            anonymity: format!("{:?}", p.anonymity).to_lowercase(),
            country: p.country.clone(),
            success_rate: p.success_rate(),
            avg_latency_ms: p.avg_latency_ms(),
            score: p.score(),
            total_checks: p.total_checks.load(Ordering::Relaxed),
            consecutive_failures: p.consecutive_failures(),
            in_cooldown: p.in_cooldown.load(Ordering::Relaxed),
        }
    }
}
