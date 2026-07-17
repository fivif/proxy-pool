use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Config {
    pub listen_addr: String,
    pub health_check_interval: Duration,
    pub validation_timeout: Duration,
    pub validation_concurrency: usize,
    pub fetch_interval: Duration,
    pub cooldown_ttl: Duration,
    pub max_pool_size: usize,
    pub min_success_rate: f64,
    pub max_consecutive_failures: u32,
    /// 校验URL列表，按优先级依次尝试，任一通过即认为代理有效
    pub test_urls: Vec<String>,
    pub test_expected_status: u16,
    /// 期望响应体中包含的关键字，空字符串则跳过内容校验
    pub test_expected_body: String,
    /// 上游代理地址，留空则直连。格式: http://127.0.0.1:7890 或 socks5://127.0.0.1:1080
    pub fetch_upstream_proxy: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:3000".into(),
            health_check_interval: Duration::from_secs(60),
            validation_timeout: Duration::from_secs(8),
            validation_concurrency: 256,
            fetch_interval: Duration::from_secs(300),
            cooldown_ttl: Duration::from_secs(120),
            max_pool_size: 5000,
            min_success_rate: 0.3,
            max_consecutive_failures: 3,
            test_urls: vec![
                "http://api.ipify.org/?format=json".into(),
                "https://api.ipify.org/?format=json".into(),
            ],
            test_expected_status: 200,
            test_expected_body: "ip".into(),
            fetch_upstream_proxy: "".into(),
        }
    }
}
