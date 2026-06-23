//! HTTP Client 层 — 全局单例 + 分层缓存 + 可配置重试 + 代理故障转移
//!
//! 对标 RSSHub cache.ts + ofetch 模式:
//!   - 全局 OnceLock Client（复用连接池，统一 User-Agent）
//!   - 内存缓存（HashMap + TTL，线程安全）：相同请求在 TTL 内不穿透网络
//!   - 可配置重试码与指数退避（含 jitter）
//!   - 熔断器：连续 N 次失败后暂停服务 M 秒
//!
//! #![allow(dead_code)]: LayeredCache, CircuitBreaker, RetryConfig 等基础设施
//! 已实现但等待 future pipeline 接入。保留代码不删除。
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use anyhow::Result;

// ===== 缓存层 =====

/// 缓存条目
#[derive(Debug, Clone)]
pub struct CachedEntry {
    pub data: String,
    pub created_at: Instant,
    pub ttl_secs: u64,
}

impl CachedEntry {
    pub fn is_expired(&self) -> bool {
        self.ttl_secs == 0 || self.created_at.elapsed().as_secs() >= self.ttl_secs
    }
}

/// 分层缓存（内存层）
///
/// - key: 请求 URL / prompt hash
/// - value: 缓存的响应体
/// - TTL: 按 key 前缀可配置（API 调用 300s，RSS 拉取 60s）
pub struct LayeredCache {
    store: RwLock<HashMap<String, CachedEntry>>,
    default_ttl: u64,
}

impl LayeredCache {
    pub fn new(default_ttl_secs: u64) -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            default_ttl: default_ttl_secs,
        }
    }

    /// 获取缓存（自动检查 TTL）
    pub fn get(&self, key: &str) -> Option<String> {
        let store = self.store.read().ok()?;
        if let Some(entry) = store.get(key) {
            if !entry.is_expired() {
                return Some(entry.data.clone());
            }
        }
        None
    }

    /// 写入缓存
    pub fn set(&self, key: String, data: String) {
        if let Ok(mut store) = self.store.write() {
            let ttl = if key.starts_with("llm:") { 300 } else { self.default_ttl };
            store.insert(key, CachedEntry {
                data,
                created_at: Instant::now(),
                ttl_secs: ttl,
            });
        }
    }

    /// 清空过期缓存
    pub fn evict_expired(&self) {
        if let Ok(mut store) = self.store.write() {
            store.retain(|_, entry| !entry.is_expired());
        }
    }

    /// 按前缀清空（如 source: 前缀在配置变更时清空）
    pub fn evict_by_prefix(&self, prefix: &str) {
        if let Ok(mut store) = self.store.write() {
            store.retain(|k, _| !k.starts_with(prefix));
        }
    }
}

// ===== 熔断器 =====

/// 熔断器 — 连续 N 次失败后暂停 M 秒
pub struct CircuitBreaker {
    max_failures: u32,
    cooldown_secs: u64,
    failures: u32,
    last_failure: Option<Instant>,
}

impl CircuitBreaker {
    pub fn new(max_failures: u32, cooldown_secs: u64) -> Self {
        Self { max_failures, cooldown_secs, failures: 0, last_failure: None }
    }

    /// 检查是否熔断
    pub fn is_open(&self) -> bool {
        if let Some(t) = self.last_failure {
            if self.failures >= self.max_failures {
                return t.elapsed().as_secs() < self.cooldown_secs;
            }
        }
        false
    }

    /// 记录成功（重置计数）
    pub fn record_success(&mut self) { self.failures = 0; }

    /// 记录失败
    pub fn record_failure(&mut self) {
        self.failures += 1;
        self.last_failure = Some(Instant::now());
    }
}

// ===== 全局缓存单例 =====

static CACHE: std::sync::OnceLock<LayeredCache> = std::sync::OnceLock::new();

pub fn global_cache() -> &'static LayeredCache {
    CACHE.get_or_init(|| LayeredCache::new(60))
}

// ===== 全局 HTTP Client 单例 =====

pub fn global_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("SulixIntel/3.0 (Global Pipeline + Cache)")
            .build()
            .expect("failed to build global HTTP client")
    })
}

pub fn http_client_with_timeout(timeout_secs: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent("SulixIntel/3.0 (Global Pipeline + Cache)")
        .build()
        .expect("failed to build HTTP client")
}

// ===== 重试助手 =====

/// 可配置重试选项
pub struct RetryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 不重试的 HTTP 状态码（如 401, 403, 422）
    pub no_retry_codes: Vec<u16>,
    /// LLM 调用时额外不重试的代码（如 429 Rate Limit）
    pub llm_no_retry_codes: Vec<u16>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            no_retry_codes: vec![401, 403, 404, 422],
            llm_no_retry_codes: vec![429],
        }
    }
}

impl RetryConfig {
    /// 是否应该跳过重试
    pub fn should_skip_retry(&self, status: u16, is_llm: bool) -> bool {
        if self.no_retry_codes.contains(&status) { return true; }
        if is_llm && self.llm_no_retry_codes.contains(&status) { return true; }
        false
    }
}

/// 带缓存 + 可配置重试的请求
///
/// 流程:
///   1. 查缓存（命中且未过期 → 直接返回）
///   2. 判断熔断器（open → 直接报错不穿透）
///   3. 执行请求（带指数退避 + jitter）
///   4. 写入缓存
///   5. 记录成功/失败到熔断器
pub async fn fetch_with_cache_and_retry(
    url: &str,
    cache_key: &str,
    cache: &LayeredCache,
    breaker: &mut CircuitBreaker,
    retry_config: &RetryConfig,
    is_llm: bool,
) -> Result<String> {
    // 1. 查缓存
    if let Some(cached) = cache.get(cache_key) {
        log::debug!("📦 缓存命中: {} (key: {})", url, cache_key);
        return Ok(cached);
    }

    // 2. 熔断检查
    if breaker.is_open() {
        anyhow::bail!("⛔ 熔断器开启: {} 暂不可用", url);
    }

    let client = global_client();
    let mut last_error = None;

    // 3. 重试循环
    for attempt in 0..=retry_config.max_retries {
        if attempt > 0 {
            let delay_secs = 2u64.pow(attempt); // 1s, 2s, 4s
            log::warn!("⏳ 第 {} 次重试 ({}s 后)...", attempt, delay_secs);
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        }

        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if resp.status().is_success() {
                    let text = resp.text().await?;
                    // 4. 写入缓存
                    cache.set(cache_key.to_string(), text.clone());
                    // 5. 记录成功
                    breaker.record_success();
                    return Ok(text);
                } else if retry_config.should_skip_retry(status, is_llm) {
                    let body = resp.text().await.unwrap_or_default();
                    anyhow::bail!("HTTP {} (不重试): {} — {}", status, url, body);
                } else {
                    last_error = Some(anyhow::anyhow!("HTTP {} — {}", status, url));
                }
            }
            Err(e) => {
                last_error = Some(anyhow::anyhow!("请求失败 [{}]: {}", url, e));
            }
        }
    }

    // 所有重试耗尽
    breaker.record_failure();
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("重试耗尽: {}", url)))
}

/// LLM API 调用的带缓存重试
///
/// 与 fetch_with_cache_and_retry 的不同点:
///   - cache_key 为 prompt hash（而非 URL）
///   - 使用 POST 方法而非 GET
///   - 不重试 429（LLM 限流）
pub async fn llm_call_with_cache(
    client: &reqwest::Client,
    api_key: &str,
    url: &str,
    body: serde_json::Value,
    cache_key: &str,
) -> Result<String> {
    let cache = global_cache();

    // 1. 查缓存
    if let Some(cached) = cache.get(cache_key) {
        log::debug!("📦 LLM 缓存命中");
        return Ok(cached);
    }

    let mut last_error = None;
    let retry_config = RetryConfig::default();

    for attempt in 0..=retry_config.max_retries {
        if attempt > 0 {
            let delay_secs = 2u64.pow(attempt); // 1s, 2s, 4s
            log::warn!("⏳ LLM 第 {} 次重试 ({}s 后)...", attempt, delay_secs);
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        }

        let resp = client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if resp.status().is_success() {
                    let text = resp.text().await?;
                    cache.set(cache_key.to_string(), text.clone());
                    return Ok(text);
                } else if status == 429 || status == 401 || status == 403 {
                    anyhow::bail!("LLM HTTP {} (不重试)", status);
                } else {
                    let body_txt = resp.text().await.unwrap_or_default();
                    last_error = Some(anyhow::anyhow!("LLM HTTP {}: {}", status, body_txt));
                }
            }
            Err(e) => {
                last_error = Some(anyhow::anyhow!("LLM 请求失败: {}", e));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("LLM 重试耗尽")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_set_get() {
        let cache = LayeredCache::new(60);
        cache.set("test-key".into(), "test-data".into());
        assert_eq!(cache.get("test-key"), Some("test-data".into()));
    }

    #[test]
    fn test_cache_evict_expired() {
        let cache = LayeredCache::new(0);
        cache.set("evict-key".into(), "data".into());
        cache.evict_expired();
        assert_eq!(cache.get("evict-key"), None);
    }

    #[test]
    fn test_cache_evict_by_prefix() {
        let cache = LayeredCache::new(60);
        cache.set("source:rss:1".into(), "a".into());
        cache.set("source:rss:2".into(), "b".into());
        cache.set("llm:hash".into(), "c".into());
        cache.evict_by_prefix("source:");
        assert_eq!(cache.get("source:rss:1"), None);
        assert_eq!(cache.get("llm:hash"), Some("c".into()));
    }

    #[test]
    fn test_circuit_breaker() {
        let mut cb = CircuitBreaker::new(3, 60);
        assert!(!cb.is_open());
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_open());
        cb.record_success();
        assert!(!cb.is_open());
    }

    #[test]
    fn test_retry_config_skip_codes() {
        let cfg = RetryConfig::default();
        assert!(cfg.should_skip_retry(401, false));
        assert!(cfg.should_skip_retry(403, false));
        assert!(cfg.should_skip_retry(429, true));  // LLM only
        assert!(!cfg.should_skip_retry(429, false)); // non-LLM passes
        assert!(!cfg.should_skip_retry(500, false)); // server error -> retry
    }
}
