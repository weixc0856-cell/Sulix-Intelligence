//! Research Agent — 分流 → 聚类 → 分析 → 蓝军 → 认知引擎 → BeliefDb
//!
//! 从 main.rs 搬迁至此。纯搬迁，不改语义。

use std::path::PathBuf;

use anyhow::Result;

use crate::config;
use crate::catalog;
use crate::fetcher;
use crate::clusterer;
use crate::publishing;
use crate::pipeline;
use crate::llm;

/// Scan Agent 分流 → LLM 预去重 → 聚类 → 主题分析 → 蓝军验证 → DiGraph 引擎 → BeliefDb
/// 返回 ResearchOutput 供 Publishing Agent 使用
#[allow(clippy::too_many_arguments)]
pub async fn agent_research(
    config: &config::Config,
    api_key: &str,
    catalog: &catalog::DataCatalog,
    new_articles: Vec<fetcher::Article>,
) -> Result<publishing::ResearchOutput> {
    // 分组 + Scan Agent
    let grouped = llm::group_by_category(&new_articles);
    let total_new = new_articles.len();
    let triage = if let Some(ref sc) = config.scan_agent {
        if sc.enabled && !grouped.is_empty() {
            match crate::agent::scan::scan_and_triage(
                &grouped,
                api_key,
                &config.llm,
                config.prompts.as_ref(),
                &config.sources,
            )
            .await
            {
                Ok(t) => {
                    log::info!(
                        "Scan v1.1: 🟢Insight:{} 🟡Watchlist:{} 🔵Memory:{}",
                        t.insight.len(),
                        t.watchlist.len(),
                        t.signal_memory.len()
                    );
                    t
                }
                Err(e) => {
                    log::warn!("Scan Agent 失败 ({}), 全部进入 Insight", e);
                    crate::agent::scan::TriageResult {
                        insight: new_articles.clone(),
                        watchlist: vec![],
                        signal_memory: vec![],
                    }
                }
            }
        } else {
            crate::agent::scan::TriageResult {
                insight: new_articles.clone(),
                watchlist: vec![],
                signal_memory: vec![],
            }
        }
    } else {
        crate::agent::scan::TriageResult {
            insight: new_articles.clone(),
            watchlist: vec![],
            signal_memory: vec![],
        }
    };
    catalog.save_step(4, "triage", &triage)?;

    if triage.insight.is_empty() && triage.watchlist.is_empty() {
        println!("\n📋 今日 {} 篇全部进入 Signal Memory。\n", total_new);
        return Ok(publishing::ResearchOutput {
            themes: vec![],
            analyses: vec![],
            analyses_zh: vec![],
            triage,
            new_articles: vec![],
        });
    }

    // 聚类（只对 Insight 层）
    let mut insight_articles = triage.insight.clone();
    if let Some(ref nl) = config.news_layer {
        if nl.llm_prededup {
            let before = insight_articles.len();
            insight_articles = clusterer::llm_prededup(
                &insight_articles,
                api_key,
                &config.llm,
                config.prompts.as_ref(),
                nl.prededup_batch_size,
            )
            .await?;
            let removed = before - insight_articles.len();
            if removed > 0 {
                log::info!("🔍 LLM 预去重: 移除 {} 篇重复信号", removed);
            }
        }
    }
    log::info!(
        "📊 开始主题聚类 (Insight: {} 篇)...",
        insight_articles.len()
    );
    let mut themes = if insight_articles.is_empty() {
        vec![]
    } else {
        clusterer::cluster_articles(&insight_articles, api_key, &config.llm).await?
    };
    // 标准化处理顺序（确定性管线）：按主题标题字母排序
    themes.sort_by(|a, b| a.title.cmp(&b.title));
    catalog.save_step(5, "themes", &themes)?;

    // 主题分析 + 蓝军验证（英文）
    let mut analyses = Vec::new();
    for theme in &themes {
        log::info!("🔍 分析主题: {} ({} 条证据)", theme.title, theme.articles.len());
        if let Some(a) = publishing::analyze_and_validate(theme, api_key, &config.llm, config.prompts.as_ref(), "en").await {
            analyses.push(a);
        }
    }
    catalog.save_step(6, "theme_analyses", &analyses)?;

    // 主题证据快照（记录每篇文章所属主题 + 分析结论）
    if !themes.is_empty() && !analyses.is_empty() {
        if let Err(e) =
            pipeline::capture_topic_evidence(&themes, &analyses, &config.output.vault_path)
        {
            log::warn!("⚠️ 主题证据快照写入失败: {}", e);
        }
        log::info!("📸 主题证据快照: {} 篇", analyses.len());
    }

    // 中文分析
    let mut analyses_zh = Vec::new();
    for theme in &themes {
        if let Some(a) = publishing::analyze_and_validate(theme, api_key, &config.llm, config.prompts.as_ref(), "zh").await {
            analyses_zh.push(a);
        }
    }
    log::info!("✅ 中文分析完成: {} 篇", analyses_zh.len());

    Ok(publishing::ResearchOutput {
        themes,
        analyses,
        analyses_zh,
        triage,
        new_articles,
    })
}

pub fn get_db_path(config: &config::Config) -> PathBuf {
    let data_dir = config
        .storage
        .as_ref()
        .and_then(|s| s.data_dir.as_deref())
        .unwrap_or("data");
    PathBuf::from(data_dir).join("intel.db")
}
