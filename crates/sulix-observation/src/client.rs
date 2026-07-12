//! HTTP Client 层 — 全局单例 + 分层缓存
//!
//! 对标 RSSHub cache.ts 模式:
//!   - 全局 OnceLock Client（复用连接池，统一 User-Agent）
//!   - 内存缓存（HashMap + TTL，线程安全）：相同请求在 TTL 内不穿透网络

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

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
            let ttl = if key.starts_with("llm:") {
                300
            } else {
                self.default_ttl
            };
            store.insert(
                key,
                CachedEntry {
                    data,
                    created_at: Instant::now(),
                    ttl_secs: ttl,
                },
            );
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_set_get() {
        let cache = LayeredCache::new(60);
        cache.set("test-key".into(), "test-data".into());
        assert_eq!(cache.get("test-key"), Some("test-data".into()));
    }
}

