//! Sulix Intelligence — 个人创业者的 AI 战略情报助手
//!
//! 管线：RSS 抓取 → 去重 → 全文提取 → 主题聚类 → 影响分析 → 咨询简报

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

mod agent;
mod catalog;
mod clusterer;
mod config;
mod db;
mod enricher;
mod fetcher;
mod llm;
mod renderer;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("🚀 Sulix Intelligence — 启动");

    // 1. 加载配置
    let config = config::Config::from_file("config.toml")?;
    let api_key = config.get_api_key()?;
    log::info!("配置加载完成: {} 个数据源, LLM 模型: {}", config.sources.len(), config.llm.model);

    // 2. 初始化数据库
    let db_path = get_db_path(&config);
    let db = db::Database::open(&db_path)?;
    log::info!("数据库已连接: {}", db_path.display());

    // 3. 初始化认知审计链
    let data_dir = config.storage.as_ref().and_then(|s| s.data_dir.as_deref())
        .map(PathBuf::from).unwrap_or_else(|| PathBuf::from("data"));
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let catalog = catalog::DataCatalog::new(&data_dir, &today);

    // 4. 拉取 RSS 源
    log::info!("开始拉取 RSS 源...");
    let mut articles = fetcher::fetch_all_sources(&config.sources).await?;
    log::info!("拉取完成: {} 篇文章", articles.len());
    catalog.save_step(1, "raw_signals", &articles)?;

    // 5. Delta 去重 + SQLite 去重
    let before_dedup = articles.len();
    fetcher::dedup_by_title(&mut articles, 0.75);
    if articles.len() < before_dedup {
        log::info!("🔀 Delta 去重: {} → {} 篇", before_dedup, articles.len());
    }
    let mut new_articles = db.dedup_and_insert(&articles)?;
    drop(articles);
    catalog.save_step(2, "unique_signals", &new_articles)?;

    if new_articles.is_empty() {
        log::info!("今日无新文章，跳过分析。");
        return Ok(());
    }

    println!("\n📋 === 今日新增 {} 篇 ===\n", new_articles.len());
    for a in &new_articles {
        println!("  [{}/{}] {}", a.category, a.source, a.title);
    }

    // 6. Wikipedia 上下文注入 + 正文提取
    enricher::enrich_with_wikipedia(&mut new_articles, 3).await;
    catalog.save_step(3, "enriched_signals", &new_articles)?;
    fetcher::enrich_articles_content(&mut new_articles, 5).await;

    // 7. 分组 + Scan Agent v1.1 信号标记 + 三层分流
    let grouped = llm::group_by_category(&new_articles);
    let total_new = new_articles.len();
    let triage = if let Some(ref sc) = config.scan_agent {
        if sc.enabled && !grouped.is_empty() {
            match agent::scan::scan_and_triage(&grouped, &api_key, &config.llm).await {
                Ok(t) => {
                    log::info!("Scan v1.1: 🟢Insight:{} 🟡Watchlist:{} 🔵Memory:{}",
                        t.insight.len(), t.watchlist.len(), t.signal_memory.len());
                    t
                }
                Err(e) => {
                    log::warn!("Scan Agent 失败 ({}), 全部进入 Insight", e);
                    agent::scan::TriageResult {
                        insight: new_articles.clone(),
                        watchlist: vec![],
                        signal_memory: vec![],
                    }
                }
            }
        } else {
            agent::scan::TriageResult {
                insight: new_articles.clone(),
                watchlist: vec![],
                signal_memory: vec![],
            }
        }
    } else {
        agent::scan::TriageResult {
            insight: new_articles.clone(),
            watchlist: vec![],
            signal_memory: vec![],
        }
    };

    catalog.save_step(4, "triage", &triage)?;

    // 即使 Insight 为空，Watchlist 也可能有内容
    if triage.insight.is_empty() && triage.watchlist.is_empty() {
        println!("\n📋 今日 {} 篇全部进入 Signal Memory。\n", total_new);
        return Ok(());
    }

    // ==================== 聚类分析（只对 Insight 层） ====================
    log::info!("📊 开始主题聚类 (Insight: {} 篇)...", triage.insight.len());
    let themes = if triage.insight.is_empty() {
        vec![]
    } else {
        clusterer::cluster_articles(&triage.insight, &api_key, &config.llm).await?
    };
    catalog.save_step(5, "themes", &themes)?;

    let mut analyses = Vec::new();
    for theme in &themes {
        log::info!("🔍 分析主题: {} ({} 条证据)", theme.title, theme.articles.len());
        analyses.push(clusterer::analyze_theme(theme, &api_key, &config.llm).await?);
    }
    catalog.save_step(6, "theme_analyses", &analyses)?;

    let summary = clusterer::synthesize(&themes, &analyses);
    log::info!("✅ 聚类完成: {} 个主题, {} 篇文章", summary.theme_count, summary.total_articles);
    catalog.save_step(7, "summary", &summary)?;

    // 认知校准
    use llm::VerticalAnalysis;
    let empty_analysis: Vec<VerticalAnalysis> = Vec::new();
    let calibration_text = agent::calibration::calibrate(&empty_analysis, &api_key, &config.llm).await?;
    catalog.save_step(8, "calibration", &calibration_text)?;

    // 渲染咨询简报
    let report = renderer::render_daily_report(
        &themes, &analyses, &summary,
        Some(&calibration_text),
        if triage.watchlist.is_empty() { None } else { Some(&triage.watchlist) },
    )?;
    log::info!("📝 简报渲染完成 ({} 字符)", report.len());

    // 写入 Vault
    let report_dir = PathBuf::from(&config.output.vault_path);
    fs::create_dir_all(&report_dir)?;
    let report_path = report_dir.join(format!("{}.md", today));
    fs::write(&report_path, &report)?;

    // 记录到数据库
    db.record_report(&today, &report, total_new)?;

    // Decay Agent 记忆墓地
    if let Some(ref grave_config) = config.graveyard {
        if grave_config.enabled {
            let all_articles: Vec<fetcher::Article> = triage.insight.iter().chain(triage.watchlist.iter()).cloned().collect();
            if let Ok(decay_report) = agent::decay::run_maintenance(&db, &all_articles, &api_key, &config.llm, grave_config).await {
                if decay_report.buried > 0 || !decay_report.wakeups.is_empty() {
                    log::info!("🪦 Decay: {} 埋葬, {} 唤醒", decay_report.buried, decay_report.wakeups.len());
                }
            }
        }
    }

    println!("\n✅ 简报已生成: {}", report_path.display());
    log::info!("✅ Sulix Intelligence 执行完成");
    Ok(())
}

fn get_db_path(config: &config::Config) -> PathBuf {
    let data_dir = config.storage.as_ref().and_then(|s| s.data_dir.as_deref()).unwrap_or("data");
    PathBuf::from(data_dir).join("intel.db")
}
