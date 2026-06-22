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
mod pipeline;
mod renderer;
mod source;
mod template;

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

    // 4. Source Adapter: 遍历所有源，用对应适配器抓取
    log::info!("开始拉取信号源...");
    let enabled_sources: Vec<&config::SourceConfig> = config.sources.iter().filter(|s| s.enabled).collect();
    let mut all_signals = Vec::new();
    let mut source_statuses: Vec<(String, bool, usize)> = Vec::new(); // (name, success, count)
    let date_range = &config.output.date_range;
    for sc in &enabled_sources {
        match source::fetch_source(sc, date_range).await {
            Ok(mut signals) => {
                log::info!("  [{}] → {} 条信号", sc.name, signals.len());
                source_statuses.push((sc.name.clone(), true, signals.len()));
                all_signals.append(&mut signals);
            }
            Err(e) => {
                log::warn!("⚠️ [{}] 抓取失败: {}", sc.name, e);
                source_statuses.push((sc.name.clone(), false, 0));
            }
        }
    }
    log::info!("拉取完成: 共 {} 条原始信号", all_signals.len());

    // 5. Pipeline: 清洗 → 合规过滤 → 去重
    let before_pipeline = all_signals.len();
    pipeline::run_pipeline(&mut all_signals)?;
    log::info!("Pipeline: {} → {} 条（清洗/合规/去重）", before_pipeline, all_signals.len());
    catalog.save_step(1, "raw_signals", &all_signals)?;

    // 6. RawSignal → Article 转换 + SQLite 去重
    let articles: Vec<fetcher::Article> = all_signals.into_iter().map(|s| fetcher::Article {
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
    }).collect();
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

    // 7. Wikipedia 上下文注入 + 正文提取
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
        let mut analysis = clusterer::analyze_theme(theme, &api_key, &config.llm).await?;
        // Phase 1: 蓝军挑战
        let (assumptions, adverse, next_tests, open_questions) = clusterer::challenge_theme(&analysis, &api_key, &config.llm).await?;
        analysis.assumptions = assumptions;
        analysis.adverse = adverse;
        analysis.next_tests = next_tests;
        analysis.open_questions = open_questions;
        // 如果蓝军发现承重假设证据弱，降级信号强度
        let weak_bearing = analysis.assumptions.iter().any(|a| a.load_bearing && a.evidence_strength == "weak");
        if weak_bearing && analysis.signal_strength >= 3 {
            analysis.signal_strength -= 2;
            log::info!("🔵 蓝军降级: {} (承重假设证据弱)", theme.title);
        }
        analyses.push(analysis);
    }
    catalog.save_step(6, "theme_analyses", &analyses)?;

    let summary = clusterer::synthesize(&themes, &analyses);
    log::info!("✅ 聚类完成: {} 个主题, {} 篇文章", summary.theme_count, summary.total_articles);
    catalog.save_step(7, "summary", &summary)?;

    // 认知校准（传入真数据，非空 vec；跳过校准不中断管线）
    let calibration_text = if !analyses.is_empty() {
        let calibration_input: Vec<llm::VerticalAnalysis> = analyses.iter().map(|ta| {
            llm::VerticalAnalysis {
                category: ta.theme_title.clone(),
                articles: vec![],
            }
        }).collect();
        agent::calibration::calibrate(&calibration_input, &api_key, &config.llm).await?
    } else {
        String::new()
    };
    catalog.save_step(8, "calibration", &calibration_text)?;

    // 渲染战略分析报告
    let analysis_report = renderer::render_analysis_report(
        &themes, &analyses, &summary,
        Some(&calibration_text),
        if triage.watchlist.is_empty() { None } else { Some(&triage.watchlist) },
        &source_statuses,
    )?;
    log::info!("📝 分析报告渲染完成 ({} 字符)", analysis_report.len());

    // 渲染每日信号聚合
    let aggregation = renderer::render_signal_aggregation(
        &themes, &analyses,
        if triage.watchlist.is_empty() { None } else { Some(&triage.watchlist) },
    )?;
    log::info!("📋 信号聚合渲染完成 ({} 条信号)",
        themes.iter().map(|t| t.articles.len()).sum::<usize>() + triage.watchlist.len());

    // 写入 Vault（双文件）
    // 按月归档（抄 Daily-News-Briefing: {archive}/{YYYY-MM}/{filename}）
    let report_dir = PathBuf::from(&config.output.vault_path);
    let month_dir = report_dir.join(&today[..7]); // YYYY-MM
    fs::create_dir_all(&month_dir)?;
    let analysis_path = month_dir.join(format!("{}-分析.md", today));
    fs::write(&analysis_path, &analysis_report)?;
    let aggregation_path = month_dir.join(format!("{}-聚合.md", today));
    fs::write(&aggregation_path, &aggregation)?;
    // 同时写入 vault 根目录一份最新版（方便快速打开）
    let latest_analysis = report_dir.join(format!("{}-分析.md", today));
    let latest_aggregation = report_dir.join(format!("{}-聚合.md", today));
    let _ = fs::write(&latest_analysis, &analysis_report);
    let _ = fs::write(&latest_aggregation, &aggregation);

    // 记录到数据库
    db.record_report(&today, &analysis_report, total_new)?;

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

    println!("\n✅ 分析报告: {}", analysis_path.display());
    println!("✅ 信号聚合: {}", aggregation_path.display());
    log::info!("✅ Sulix Intelligence 执行完成");
    Ok(())
}

fn get_db_path(config: &config::Config) -> PathBuf {
    let data_dir = config.storage.as_ref().and_then(|s| s.data_dir.as_deref()).unwrap_or("data");
    PathBuf::from(data_dir).join("intel.db")
}
