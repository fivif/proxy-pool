use crate::pool::proxy::Protocol;
use std::net::{SocketAddr, ToSocketAddrs};

pub struct ProxySource {
    pub name: &'static str,
    pub url: &'static str,
    pub default_protocol: Protocol,
    pub is_api: bool,
}

pub fn default_sources() -> Vec<ProxySource> {
    vec![
        ProxySource {
            name: "proxyscrape-api",
            url: "https://api.proxyscrape.com/v4/free-proxy-list/get?request=display_proxies&proxy_format=protocolipport&format=text&timeout=20000",
            default_protocol: Protocol::Http,
            is_api: true,
        },
        ProxySource {
            name: "thespeedx-http",
            url: "https://raw.githubusercontent.com/TheSpeedX/PROXY-List/master/http.txt",
            default_protocol: Protocol::Http,
            is_api: false,
        },
        ProxySource {
            name: "jetkai-proxies",
            url: "https://raw.githubusercontent.com/jetkai/proxy-list/main/online-proxies/txt/proxies.txt",
            default_protocol: Protocol::Http,
            is_api: false,
        },
        ProxySource {
            name: "monosans-http",
            url: "https://raw.githubusercontent.com/monosans/proxy-list/main/proxies/http.txt",
            default_protocol: Protocol::Http,
            is_api: false,
        },
        ProxySource {
            name: "hookzof-socks5",
            url: "https://raw.githubusercontent.com/hookzof/socks5_list/master/proxy.txt",
            default_protocol: Protocol::Socks5,
            is_api: false,
        },
        ProxySource {
            name: "roosterkid-https",
            url: "https://raw.githubusercontent.com/roosterkid/openproxylist/main/HTTPS.txt",
            default_protocol: Protocol::Https,
            is_api: false,
        },
        ProxySource {
            name: "shiftytr-http",
            url: "https://raw.githubusercontent.com/ShiftyTR/Proxy-List/master/http.txt",
            default_protocol: Protocol::Http,
            is_api: false,
        },
        ProxySource {
            name: "aliilapro-http",
            url: "https://raw.githubusercontent.com/ALIILAPRO/Proxy/main/http.txt",
            default_protocol: Protocol::Http,
            is_api: false,
        },
        ProxySource {
            name: "sunny9577-proxies",
            url: "https://raw.githubusercontent.com/sunny9577/proxy-scraper/master/proxies.txt",
            default_protocol: Protocol::Http,
            is_api: false,
        },
        ProxySource {
            name: "openproxylist-api",
            url: "https://api.openproxylist.xyz/http.txt",
            default_protocol: Protocol::Http,
            is_api: true,
        },
    ]
}

/// Parse a raw text body of `ip:port` lines (one per line) into SocketAddr list.
pub fn parse_ip_port_lines(body: &str) -> Vec<(SocketAddr, Protocol)> {
    let mut results = Vec::with_capacity(body.len() / 20);
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        // Handle `protocol://ip:port` format
        let cleaned = if let Some(idx) = line.find("://") {
            &line[idx + 3..]
        } else {
            line
        };
        // Handle "ip:port#country" or "ip:port description" formats
        let ip_port = cleaned.split(&[' ', '#', '\t'][..]).next().unwrap_or(cleaned);
        // Handle country flag prefixes like "🇺🇸 ip:port ..."
        let ip_port = ip_port.split_whitespace().last().unwrap_or(ip_port);

        if let Ok(addr) = parse_socket(ip_port) {
            let proto = if line.starts_with("socks5://") {
                Protocol::Socks5
            } else if line.starts_with("socks4://") {
                Protocol::Socks4
            } else if line.starts_with("https://") {
                Protocol::Https
            } else {
                Protocol::Http
            };
            results.push((addr, proto));
        }
    }
    results
}

fn parse_socket(s: &str) -> Result<SocketAddr, ()> {
    if let Ok(addr) = s.parse::<SocketAddr>() {
        return Ok(addr);
    }
    // Fallback: try ToSocketAddrs
    s.to_socket_addrs().ok().and_then(|mut iter| iter.next()).ok_or(())
}
