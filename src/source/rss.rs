//! RSS 源适配器（抄 RSSHub handler 模式：抓取 → 标准化 → 输出 RawSignal）
//!
//! 支持：
//! - 正向关键词过滤（keywords）
//! - 反向黑名单（exclude_keywords）
//! - 日期范围过滤（date_range）

use std::io::Cursor;

use anyhow::Result;
use chrono::{Duration, Utc};

use crate::config::SourceConfig;
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

/// 抓取单个 RSS 源并输出标准化 RawSignal
pub async fn fetch_rss(config: &SourceConfig, date_range: &str) -> Result<Vec<RawSignal>> {
    let client = reqwest::Client::builder()
        .user_agent("Sulix-Intel/0.1")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    log::debug!("抓取 RSS [{}] → {}", config.name, config.url);

    let bytes = fetch_with_retry(&client, &config.url).await?;
    let feed = feed_rs::parser::parse(Cursor::new(&bytes))?;

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
