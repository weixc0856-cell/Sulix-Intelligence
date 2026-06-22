//! RSS 源适配器（抄 RSSHub handler 模式：抓取 → 标准化 → 输出 RawSignal）
//!
//! 从 fetcher.rs 提取的 RSS 抓取逻辑，包装为独立适配器。

use std::io::Cursor;

use anyhow::Result;

use crate::config::SourceConfig;
use crate::source::RawSignal;

/// 获取 RSS 源的唯一系统标识（优先用 id，fallback 到 name 的 hash）
fn get_source_id(config: &SourceConfig) -> String {
    config.id.clone().unwrap_or_else(|| {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        config.name.hash(&mut hasher);
        format!("src_{:x}", hasher.finish())
    })
}

/// 抓取单个 RSS 源并输出标准化 RawSignal
pub async fn fetch_rss(config: &SourceConfig) -> Result<Vec<RawSignal>> {
    let client = reqwest::Client::builder()
        .user_agent("Sulix-Intel/0.1")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    log::debug!("抓取 RSS [{}] → {}", config.name, config.url);

    let response = client.get(&config.url).send().await?;
    let bytes = response.bytes().await?;
    let feed = feed_rs::parser::parse(Cursor::new(&bytes))?;

    let source_id = get_source_id(config);
    let signals: Vec<RawSignal> = feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let title = entry.title.map(|t| t.content).unwrap_or_default();
            if title.is_empty() {
                return None;
            }
            let url = entry
                .links
                .iter()
                .find(|l| l.rel.as_deref() == Some("alternate") || l.rel.is_none())
                .or_else(|| entry.links.first())
                .map(|l| l.href.clone())?;

            let id = simple_hash(&url);
            let content = entry
                .content
                .and_then(|c| c.body)
                .or_else(|| entry.summary.as_ref().map(|s| s.content.clone()));
            let summary = entry.summary.map(|s| s.content);

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
            })
        })
        .collect();

    log::info!("✅ [RSS/{}] → {} 条信号", config.name, signals.len());
    Ok(signals)
}

/// 简单的 URL hash 作为 ID
fn simple_hash(url: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
