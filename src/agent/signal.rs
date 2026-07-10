//! Signal Agent — 源抓取/去重/丰富/实体提取
//!
//! 从 main.rs 搬迁至此。纯搬迁，不改语义。

use anyhow::Result;

use crate::{catalog, config, db, enricher, entity, fetcher, pipeline, source};

/// 信号源抓取计数
#[derive(Debug, Clone)]
pub struct SourceStatus {
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

/// Pipeline 清洗 → SQLite 去重 → Trend → 证据快照 → 丰富
/// 返回 None 表示今日无新文章
async fn process_signal_articles(
    signals: Vec<source::RawSignal>,
    config: &config::Config,
    db: &db::Database,
    catalog: &catalog::DataCatalog,
    today: &str,
) -> Result<Option<Vec<fetcher::Article>>> {
    // Pipeline 清洗去重
    let before_pipeline = signals.len();
    let mut sigs = signals;
    pipeline::run_pipeline_with_config(&mut sigs, config.dedup.as_ref())?;
    log::info!(
        "Pipeline: {} → {} 条（清洗/合规/去重）",
        before_pipeline,
        sigs.len()
    );
    catalog.save_step(1, "raw_signals", &sigs)?;

    // Article 转换 + SQLite 去重
    let articles: Vec<fetcher::Article> = sigs
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
    let mut new_articles = db.dedup_and_insert(&articles)?;
    drop(articles);
    catalog.save_step(2, "unique_signals", &new_articles)?;

    if new_articles.is_empty() {
        log::info!("今日无新文章, 跳过分析。");
        return Ok(None);
    }

    println!("\n📋 === 今日新增 {} 篇 ===\n", new_articles.len());
    for a in &new_articles {
        println!("  [{}/{}] {}", a.category, a.source, a.title);
    }

    // Trend Layer 写入
    write_trend_layer(&new_articles, db, today);

    // 证据快照
    capture_evidence_snapshots(&new_articles, &config.output.vault_path);

    // Wikipedia 注入 + 正文提取
    enricher::enrich_with_wikipedia(&mut new_articles, 3).await;
    catalog.save_step(3, "enriched_signals", &new_articles)?;
    fetcher::enrich_articles_content(&mut new_articles, 5).await;

    Ok(Some(new_articles))
}

/// 写入 Trend Layer 分类统计
fn write_trend_layer(articles: &[fetcher::Article], db: &db::Database, today: &str) {
    use std::collections::HashMap;
    let mut cat_counts: HashMap<&str, u32> = HashMap::new();
    for a in articles {
        *cat_counts.entry(&a.category).or_insert(0) += 1;
    }
    let stats: Vec<db::CategoryStat> = cat_counts
        .into_iter()
        .map(|(cat, count)| db::CategoryStat {
            category: cat.to_string(),
            article_count: count,
        })
        .collect();
    if let Err(e) = db.upsert_daily_stats(today, &stats) {
        log::warn!("⚠️ Trend Layer 写入失败: {}", e);
    }
}

/// 捕获证据快照（SVI >= 7 的原始信号存档）
fn capture_evidence_snapshots(articles: &[fetcher::Article], vault_path: &str) {
    for article in articles {
        if article.content.is_some() || article.summary.is_some() {
            let signal = source::RawSignal {
                id: article.id.clone(),
                title: article.title.clone(),
                url: article.url.clone(),
                source: article.source.clone(),
                source_id: article.source.clone(),
                category: article.category.clone(),
                content: article.content.clone(),
                summary: article.summary.clone(),
                published_at: article.published_at,
                metrics: None,
                requires_sanitization: false,
                is_internal: article.is_internal,
            };
            if let Err(e) = pipeline::capture_evidence_snapshot(&signal, 5, vault_path) {
                log::warn!("⚠️ 证据快照写入失败 [{}]: {}", article.id, e);
            }
        }
    }
}

/// 从文章提取实体并更新 EntitySanctionDb
fn extract_entities_from_articles(
    articles: &[fetcher::Article],
    entity_db: &mut entity::EntitySanctionDb,
) {
    for article in articles {
        let combined = format!(
            "{} {}",
            article.title,
            article.summary.as_deref().unwrap_or("")
        );
        let names = entity::extract_entities_from_text(&combined);
        for name in &names {
            if !entity_db.name_exists(name) {
                let ent = entity::Entity {
                    id: format!(
                        "ent-{}-{}",
                        chrono::Utc::now().timestamp(),
                        entity_db.unsanctioned.len()
                    ),
                    entity_type: entity::EntityType::Unknown,
                    name: name.clone(),
                    aliases: vec![],
                    sanctioned: false,
                    external_refs: vec![entity::ExternalRef {
                        source: article.source.clone(),
                        external_id: article.id.clone(),
                        url: Some(article.url.clone()),
                    }],
                    relationships: vec![],
                };
                entity_db.add_entity(ent);
            }
        }
    }
    let entity_count = entity_db.sanctioned.len() + entity_db.unsanctioned.len();
    log::info!(
        "🗃️ EntitySanctionDb: {} 实体 (sanctioned: {}, unsanctioned: {})",
        entity_count,
        entity_db.sanctioned.len(),
        entity_db.unsanctioned.len()
    );
}

/// 源抓取 → Pipeline 清洗去重 → SQLite 去重 → Trend 写入 → 证据快照 → 丰富 → 实体提取
/// 返回 None 表示今日无新文章（管线提前终止）
pub async fn agent_signal(
    config: &config::Config,
    db: &db::Database,
    catalog: &catalog::DataCatalog,
    today: &str,
    mut entity_db: entity::EntitySanctionDb,
) -> Result<
    Option<(
        Vec<fetcher::Article>,
        Vec<SourceStatus>,
        entity::EntitySanctionDb,
    )>,
> {
    let date_range = &config.output.date_range;
    let (all_signals, source_statuses) = fetch_all_sources(config, date_range).await;

    let Some(new_articles) =
        process_signal_articles(all_signals, config, db, catalog, today).await?
    else {
        return Ok(None);
    };

    // 实体提取
    extract_entities_from_articles(&new_articles, &mut entity_db);

    Ok(Some((new_articles, source_statuses, entity_db)))
}
