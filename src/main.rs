//! Sulix Intelligence — 个人创业者的 AI 战略情报助手
//!
//! 管线：RSS 抓取 → 去重 → 全文提取 → 主题聚类 → 影响分析 → 咨询简报

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

mod agent;
mod archive;
mod catalog;
mod client;
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
    let mut config = config::Config::from_file("config.toml")?;
    // CI 环境变量覆盖：VAULT_PATH → config.output.vault_path
    if let Ok(ci_path) = std::env::var("VAULT_PATH") {
        log::info!("⚙️ CI 覆盖 vault_path: {}", ci_path);
        config.output.vault_path = ci_path;
    }
    let api_key = config.get_api_key()?;
    log::info!(
        "配置加载完成: {} 个数据源, LLM 模型: {}",
        config.sources.len(),
        config.llm.model
    );

    // 2. 初始化数据库
    let db_path = get_db_path(&config);
    let db = db::Database::open(&db_path)?;
    log::info!("数据库已连接: {}", db_path.display());

    // 3. 初始化认知审计链
    let data_dir = config
        .storage
        .as_ref()
        .and_then(|s| s.data_dir.as_deref())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let catalog = catalog::DataCatalog::new(&data_dir, &today);

    // 4. Source Adapter: 遍历所有源，用对应适配器抓取
    log::info!("开始拉取信号源...");
    let enabled_sources: Vec<&config::SourceConfig> =
        config.sources.iter().filter(|s| s.enabled).collect();
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
    log::info!(
        "Pipeline: {} → {} 条（清洗/合规/去重）",
        before_pipeline,
        all_signals.len()
    );
    catalog.save_step(1, "raw_signals", &all_signals)?;

    // 6. RawSignal → Article 转换 + SQLite 去重
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
        })
        .collect();
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
        log::info!(
            "🔍 分析主题: {} ({} 条证据)",
            theme.title,
            theme.articles.len()
        );
        let mut analysis = clusterer::analyze_theme(theme, &api_key, &config.llm, "en").await?;
        // Phase 1: 蓝军挑战
        let (assumptions, adverse, next_tests, open_questions) =
            clusterer::challenge_theme(&analysis, &api_key, &config.llm).await?;
        analysis.assumptions = assumptions;
        analysis.adverse = adverse;
        analysis.next_tests = next_tests;
        analysis.open_questions = open_questions;
        // 如果蓝军发现承重假设证据弱，降级信号强度
        let weak_bearing = analysis
            .assumptions
            .iter()
            .any(|a| a.load_bearing && a.evidence_strength == "weak");
        if weak_bearing && analysis.signal_strength >= 3 {
            analysis.signal_strength -= 2;
            log::info!("🔵 蓝军降级: {} (承重假设证据弱)", theme.title);
        }
        analyses.push(analysis);
    }
    catalog.save_step(6, "theme_analyses", &analyses)?;

    // 双轨：生成繁体中文版分析
    let mut analyses_zh = Vec::new();
    for theme in &themes {
        match clusterer::analyze_theme(theme, &api_key, &config.llm, "zh").await {
            Ok(mut a) => {
                if let Ok((assumptions, adverse, next_tests, open_questions)) =
                    clusterer::challenge_theme(&a, &api_key, &config.llm).await
                {
                    a.assumptions = assumptions;
                    a.adverse = adverse;
                    a.next_tests = next_tests;
                    a.open_questions = open_questions;
                }
                let weak = a
                    .assumptions
                    .iter()
                    .any(|x| x.load_bearing && x.evidence_strength == "weak");
                if weak && a.signal_strength >= 3 {
                    a.signal_strength -= 2;
                }
                analyses_zh.push(a);
            }
            Err(e) => log::warn!("⚠️ 中文分析失败 [{}]: {}", theme.title, e),
        }
    }
    log::info!("✅ 中文分析完成: {} 篇", analyses_zh.len());

    let summary = clusterer::synthesize(&themes, &analyses);
    log::info!(
        "✅ 聚类完成: {} 个主题, {} 篇文章",
        summary.theme_count,
        summary.total_articles
    );
    catalog.save_step(7, "summary", &summary)?;

    // 认知校准（传入真数据，非空 vec；跳过校准不中断管线）
    let calibration_text = if !analyses.is_empty() {
        let calibration_input: Vec<llm::VerticalAnalysis> = analyses
            .iter()
            .map(|ta| llm::VerticalAnalysis {
                category: ta.theme_title.clone(),
                articles: vec![],
            })
            .collect();
        agent::calibration::calibrate(&calibration_input, &api_key, &config.llm).await?
    } else {
        String::new()
    };
    catalog.save_step(8, "calibration", &calibration_text)?;

    // Markdown 渲染已停用（仅作计数日志）
    log::info!(
        "📝 分析主题: {} 个, 信号: {} 条",
        analyses.len(),
        themes.iter().map(|t| t.articles.len()).sum::<usize>() + triage.watchlist.len()
    );

    // === 目录路由写入（/en/ = 英文, /zh/ = 繁体中文）===
    let report_dir = PathBuf::from(&config.output.vault_path);
    let en_root = report_dir.join("en");
    let zh_root = report_dir.join("zh");
    let month_dir = en_root.join(&today[..7]);

    // 英文日详情
    if let Ok(html) = renderer::render_html_report(&themes, &analyses, &today) {
        fs::create_dir_all(&month_dir)?;
        let _ = fs::write(month_dir.join("index.html"), &html);
        log::info!("📄 EN 简报写入: {}", month_dir.join("index.html").display());
    }
    // === 编年史看板初始化（在双语写入之前） ===
    let db_dir = data_dir.join(&today[..7]);
    fs::create_dir_all(&db_dir)
        .unwrap_or_else(|e| log::warn!("无法创建数据目录 {:?}: {}", db_dir, e));
    let chronicle_path = data_dir.join("database.json");
    let mut chronicle = archive::ChronicleDb::load(&chronicle_path)?;

    // === 繁体中文版 ===
    if !analyses_zh.is_empty() {
        let zh_dir = report_dir.join("zh").join(&today[..7]);
        fs::create_dir_all(&zh_dir)
            .unwrap_or_else(|e| log::warn!("无法创建中文目录 {:?}: {}", zh_dir, e));

        if let Ok(zh_html) = renderer::render_html_report(&themes, &analyses_zh, &today) {
            if let Err(e) = fs::write(zh_dir.join("index.html"), &zh_html) {
                log::warn!("写入中文 HTML 失败 {:?}: {}", zh_dir.join("index.html"), e);
            }
            log::info!("🌏 中文简报已生成");
        }
        // 中文 Chronicle 条目
        for a in &analyses_zh {
            let mut entities: Vec<String> = Vec::new();
            for fb in &a.fact_base {
                for word in fb.evidence.split_whitespace() {
                    let upper = word.to_uppercase();
                    if [
                        "TSMC",
                        "ASML",
                        "NVIDIA",
                        "OPENAI",
                        "ANTHROPIC",
                        "GOOGLE",
                        "META",
                        "MICROSOFT",
                        "INTEL",
                        "AMD",
                        "ARM",
                        "HBM",
                    ]
                    .contains(&upper.as_str())
                        && !entities.contains(&upper)
                    {
                        entities.push(upper);
                    }
                }
            }
            chronicle.push(archive::ChronicleEntry {
                date: today.clone(),
                topic: a.theme_title.clone(),
                headline: a.bluf.clone(),
                entities,
                signal_strength: a.signal_strength,
                language: "zh".into(),
            });
        }
    }

    // 记录到数据库（只记计数，不存 Markdown 全文）
    db.record_report(&today, &format!("Daily brief - {} topics", analyses.len()), total_new)?;

    // Decay Agent 记忆墓地
    if let Some(ref grave_config) = config.graveyard {
        if grave_config.enabled {
            let all_articles: Vec<fetcher::Article> = triage
                .insight
                .iter()
                .chain(triage.watchlist.iter())
                .cloned()
                .collect();
            if let Ok(decay_report) = agent::decay::run_maintenance(
                &db,
                &all_articles,
                &api_key,
                &config.llm,
                grave_config,
            )
            .await
            {
                if decay_report.buried > 0 || !decay_report.wakeups.is_empty() {
                    log::info!(
                        "🪦 Decay: {} 埋葬, {} 唤醒",
                        decay_report.buried,
                        decay_report.wakeups.len()
                    );
                }
            }
        }
    }

    // EN 条目
    for analysis in &analyses {
        let mut entities: Vec<String> = Vec::new();
        for fb in &analysis.fact_base {
            for word in fb.evidence.split_whitespace() {
                let upper = word.to_uppercase();
                if [
                    "TSMC",
                    "ASML",
                    "NVIDIA",
                    "INTEL",
                    "AMD",
                    "ARM",
                    "HBM",
                    "OPENAI",
                    "ANTHROPIC",
                    "GOOGLE",
                    "META",
                    "MICROSOFT",
                ]
                .contains(&upper.as_str())
                    && !entities.contains(&upper)
                {
                    entities.push(upper);
                }
            }
        }
        chronicle.push(archive::ChronicleEntry {
            date: today.clone(),
            topic: analysis.theme_title.clone(),
            headline: analysis.bluf.clone(),
            entities,
            signal_strength: analysis.signal_strength,
            language: "en".into(),
        });
    }
    chronicle.save(&chronicle_path)?;

    // 重写总索引页
    // 编年史看板：写入 /en/ + 复制到 root（默认英文）
    let sorted = chronicle.sorted();
    let archive_html = renderer::render_archive_dashboard(&sorted)?;

    let en_root = report_dir.join("en");
    fs::create_dir_all(&en_root)?;
    let en_archive = en_root.join("index.html");
    fs::write(&en_archive, &archive_html)?;
    // 复制到根目录（intel.getsulix.com/ 默认显示英文）
    fs::write(report_dir.join("index.html"), &archive_html)?;
    log::info!("📚 编年史看板: {} 条 → EN + root", sorted.len());

    // 中文看板（已由上方 ZH 区块写入 zh/{month}/index.html）
    // 再写入 /zh/index.html
    let zh_root = report_dir.join("zh");
    fs::create_dir_all(&zh_root)?;
    let zh_entries = chronicle.sorted_by_lang("zh");
    if !zh_entries.is_empty() {
        if let Ok(zh_archive) = renderer::render_archive_dashboard(&zh_entries) {
            fs::write(zh_root.join("index.html"), &zh_archive)?;
            log::info!("📚 中文看板: {} 条 → {}", zh_entries.len(), zh_root.join("index.html").display());
        }
    }

    println!("\n✅ EN 简报: {}", month_dir.join("index.html").display());
    println!("✅ 看板: {}", en_root.join("index.html").display());
    log::info!("✅ Sulix Intelligence 执行完成");
    Ok(())
}

fn get_db_path(config: &config::Config) -> PathBuf {
    let data_dir = config
        .storage
        .as_ref()
        .and_then(|s| s.data_dir.as_deref())
        .unwrap_or("data");
    PathBuf::from(data_dir).join("intel.db")
}
