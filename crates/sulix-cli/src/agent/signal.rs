//! Signal Agent — 源抓取/去重/实体提取
//!
//! 新管线 Observation 输入的入口。
//! 职责：fetch sources → sanitize → dedup → extract entities → return Articles

use anyhow::Result;

use crate::db;
use sulix_config as config;
use sulix_observation::{fetcher, source};

/// 信号源抓取计数
#[derive(Debug, Clone)]
pub struct SourceStatus {
    #[allow(dead_code)]
    pub signal_count: usize,
}

/// 获取所有启用的信号源
async fn fetch_all_sources(
    config: &config::Config,
    date_range: &str,
) -> (Vec<source::RawSignal>, Vec<SourceStatus>) {
    log::info!("开始拉取信号源...");
    let enabled_sources: Vec<&config::SourceConfig> =
        config.sources.iter().filter(|s| s.enabled).collect();
    let mut all_signals = Vec::new();
    let mut source_statuses: Vec<SourceStatus> = Vec::new();
    for sc in &enabled_sources {
        match source::fetch_source(sc, date_range).await {
            Ok(mut signals) => {
                log::info!("  [{}] → {} 条信号", sc.name, signals.len());
                source_statuses.push(SourceStatus {
                    signal_count: signals.len(),
                });
                all_signals.append(&mut signals);
            }
            Err(e) => {
                log::warn!("⚠️ [{}] 抓取失败: {}", sc.name, e);
                source_statuses.push(SourceStatus { signal_count: 0 });
            }
        }
    }
    log::info!("拉取完成: 共 {} 条原始信号", all_signals.len());
    (all_signals, source_statuses)
}

/// 源抓取 → 去重 → 实体提取
/// 返回 None 表示今日无新文章
pub async fn agent_signal(
    config: &config::Config,
    db: &db::Database,
    _today: &str,
) -> Result<Option<Vec<fetcher::Article>>> {
    let date_range = &config.output.date_range;
    let (all_signals, _source_statuses) = fetch_all_sources(config, date_range).await;

    // 转换为 Article
    let articles: Vec<fetcher::Article> = all_signals
        .into_iter()
        .map(|s| fetcher::Article {
            id: s.id,
            source: s.source,
            title: s.title,
            url: s.url,
            content: s.content,
            summary: s.summary,
            published_at: s.published_at,
            category: s.category,
            wiki_summary: None,
            evidence_type: String::new(),
            is_internal: s.is_internal,
        })
        .collect();

    // SQLite 去重
    let new_articles = db.dedup_and_insert(&articles)?;
    drop(articles);

    if new_articles.is_empty() {
        log::info!("今日无新文章");
        return Ok(None);
    }

    log::info!("📋 新文章: {} 篇", new_articles.len());
    for a in &new_articles {
        log::info!("  [{}/{}] {}", a.category, a.source, a.title);
    }

    Ok(Some(new_articles))
}
