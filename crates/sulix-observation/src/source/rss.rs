//! RSS 源适配器（抄 RSSHub handler 模式：抓取 → 标准化 → 输出 RawSignal）
//!
//! 支持：
//! - 正向关键词过滤（keywords）
//! - 反向黑名单（exclude_keywords）
//! - 日期范围过滤（date_range）

use std::io::Cursor;

use anyhow::Result;
use chrono::{Duration, Utc};

use sulix_config::SourceConfig;
use crate::source::RawSignal;

/// 解析日期范围配置 ("d3" → 3天, "w1" → 7天, "m1" → 30天)
pub fn parse_date_range(s: &str) -> Duration {
    let s = s.trim().to_lowercase();
    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str.parse().unwrap_or(7);
    match unit {
        "h" => Duration::hours(num),
        "d" => Duration::days(num),
        "w" => Duration::weeks(num),
        "m" => Duration::days(num * 30),
        _ => Duration::days(7),
    }
}

/// 检查文本是否匹配任一关键词（不区分大小写）
fn matches_keywords(text: &str, keywords: &[String]) -> bool {
    let lower = text.to_lowercase();
    keywords.iter().any(|kw| lower.contains(&kw.to_lowercase()))
}

/// 带指数退避重试的 HTTP 抓取（抄 RSSHub 重试策略）
async fn fetch_with_retry(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let mut last_error = None;
    for attempt in 0..3 {
        if attempt > 0 {
            let delay = std::time::Duration::from_secs(2u64.pow(attempt));
            tokio::time::sleep(delay).await;
        }
        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return Ok(resp.bytes().await?.to_vec());
                }
                // 429/503/502 可重试
                if status.as_u16() == 429 || status.as_u16() == 502 || status.as_u16() == 503 {
                    last_error = Some(anyhow::anyhow!("HTTP {}", status));
                    continue;
                }
                // 4xx 其他不重试
                return Err(anyhow::anyhow!("HTTP {}", status));
            }
            Err(e) => {
                last_error = Some(e.into());
                // 网络错误重试
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("请求失败")))
}

/// 获取 RSSHub 基础 URL（优先使用环境变量 RSSHUB_BASE_URL，否则默认 rsshub.app）
fn get_rsshub_base() -> String {
    std::env::var("RSSHUB_BASE_URL").unwrap_or_else(|_| "https://rsshub.app".into())
}

/// 抓取单个 RSS 源并输出标准化 RawSignal
pub async fn fetch_rss(config: &SourceConfig, date_range: &str) -> Result<Vec<RawSignal>> {
    // 使用全局 HTTP Client（复用连接池，统一 User-Agent）
    // SEC 需要含联系邮箱的 User-Agent
    let client = if config.url.contains("sec.gov") {
        reqwest::Client::builder()
            .user_agent("SulixIntel/3.0 (weixc0856@gmail.com)")
            .timeout(std::time::Duration::from_secs(30))
            .build()?
    } else {
        crate::client::global_client().clone()
    };

    // RSSHub URL 重写：用环境变量 RSSHUB_BASE_URL 替换 rsshub.app
    let actual_url = if config.url.contains("rsshub.app") {
        let base = get_rsshub_base();
        config.url.replace("https://rsshub.app", &base)
    } else {
        config.url.clone()
    };

    // Phase 3: 尝试从缓存读取
    let cache = crate::client::global_cache();
    let cache_key = format!("rss:{}", actual_url);
    if let Some(cached) = cache.get(&cache_key) {
        log::debug!("📦 RSS 缓存命中 [{}]", config.name);
        // 缓存命中，直接解析
        let bytes = cached.into_bytes();
        return parse_feed_bytes(&bytes, config, date_range);
    }

    log::debug!("抓取 RSS [{}] → {}", config.name, actual_url);

    let bytes = fetch_with_retry(&client, &actual_url).await?;
    // 写入缓存（RSS 的 TTL 由 LayeredCache 默认 60s 控制）
    if let Ok(text) = String::from_utf8(bytes.clone()) {
        cache.set(cache_key, text);
    }

    parse_feed_bytes(&bytes, config, date_range)
}

/// 解析 RSS feed 字节为 RawSignal 列表
fn parse_feed_bytes(
    bytes: &[u8],
    config: &SourceConfig,
    date_range: &str,
) -> Result<Vec<RawSignal>> {
    let feed = feed_rs::parser::parse(Cursor::new(bytes))?;
    let cutoff = Utc::now() - parse_date_range(date_range);
    let source_id = get_source_id(config);

    let signals: Vec<RawSignal> = feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let title = entry.title.map(|t| t.content).unwrap_or_default();
            if title.is_empty() {
                return None;
            }

            // 日期范围过滤（在循环内提前熔断，避免创建对象）
            if let Some(ref published) = entry.published {
                if published.with_timezone(&Utc) < cutoff {
                    return None; // 过期文章，直接丢弃
                }
            }

            let url = entry
                .links
                .iter()
                .find(|l| l.rel.as_deref() == Some("alternate") || l.rel.is_none())
                .or_else(|| entry.links.first())
                .map(|l| l.href.clone())?;

            // 构建用于过滤的文本
            let summary = entry.summary.as_ref().map(|s| s.content.clone());
            let content = entry
                .content
                .and_then(|c| c.body)
                .or_else(|| summary.clone());

            // YouTube RSS 平稳退化：content 为空时用 title 代替，防止被误判为噪音
            let content = if content
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
            {
                log::debug!("🎬 YouTube RSS 退化: '{}' → 用标题填充 content", title);
                Some(title.clone())
            } else {
                content
            };
            let filter_text = format!("{} {}", &title, content.as_deref().unwrap_or(""));

            // 反向黑名单（exclude_keywords）：匹配任一即熔断
            if let Some(ref exclude) = config.exclude_keywords {
                if matches_keywords(&filter_text, exclude) {
                    log::debug!("🔴 反向熔断 [{}]: {}", config.name, title);
                    return None;
                }
            }

            // 正向关键词过滤（keywords）：如果配置了，至少匹配一个才保留
            if let Some(ref keywords) = config.keywords {
                if !keywords.is_empty() && !matches_keywords(&filter_text, keywords) {
                    return None; // 未匹配任何正向关键词
                }
            }

            let id = simple_hash(&url);

            Some(RawSignal {
                id,
                title,
                url,
                content,
                summary,
                published_at: entry.published.map(|d| d.fixed_offset()),
                source: config.name.clone(),
                source_id: source_id.clone(),
                category: config.category.clone(),
                metrics: None,
                requires_sanitization: false,
                is_internal: config.is_internal(),
            })
        })
        .collect();

    log::info!("✅ [RSS/{}] → {} 条信号", config.name, signals.len());
    Ok(signals)
}

fn get_source_id(config: &SourceConfig) -> String {
    config.id.clone().unwrap_or_else(|| {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        config.name.hash(&mut hasher);
        format!("src_{:x}", hasher.finish())
    })
}

fn simple_hash(url: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_parse_date_range_hours() {
        let d = parse_date_range("6h");
        assert_eq!(d, Duration::hours(6));
    }

    #[test]
    fn test_parse_date_range_days() {
        let d = parse_date_range("3d");
        assert_eq!(d, Duration::days(3));
    }

    #[test]
    fn test_parse_date_range_weeks() {
        let d = parse_date_range("2w");
        assert_eq!(d, Duration::weeks(2));
    }

    #[test]
    fn test_parse_date_range_months() {
        let d = parse_date_range("1m");
        assert_eq!(d, Duration::days(30));
    }

    #[test]
    fn test_parse_date_range_default() {
        let d = parse_date_range("invalid");
        assert_eq!(d, Duration::days(7));
    }

    #[test]
    fn test_matches_keywords_positive() {
        let text = "US tightens semiconductor export controls on China";
        let keywords = vec!["semiconductor".to_string(), "export".to_string()];
        assert!(matches_keywords(text, &keywords));
    }

    #[test]
    fn test_matches_keywords_negative() {
        let text = "Federal reserve maintains interest rates";
        let keywords = vec!["semiconductor".to_string(), "AI chip".to_string()];
        assert!(!matches_keywords(text, &keywords));
    }

    #[test]
    fn test_matches_keywords_case_insensitive() {
        let text = "AI Chip Export Restrictions";
        let keywords = vec!["ai chip".to_string()];
        assert!(matches_keywords(text, &keywords));
    }

    #[test]
    fn test_matches_keywords_empty_keywords() {
        let text = "Any text";
        let keywords: Vec<String> = vec![];
        assert!(!matches_keywords(text, &keywords));
    }

    #[test]
    fn test_simple_hash_consistent() {
        let h1 = simple_hash("https://example.com/rss");
        let h2 = simple_hash("https://example.com/rss");
        assert_eq!(h1, h2, "same URL should produce same hash");
    }

    #[test]
    fn test_simple_hash_different() {
        let h1 = simple_hash("https://url1.com");
        let h2 = simple_hash("https://url2.com");
        assert_ne!(h1, h2, "different URLs should produce different hashes");
    }
}

