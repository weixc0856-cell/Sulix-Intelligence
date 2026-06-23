//! Sulix Intelligence — 个人创业者的 AI 战略情报助手
//!
//! 管线：RSS 抓取 → 去重 → 全文提取 → 主题聚类 → 影响分析 → 咨询简报

use std::sync::Arc;

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

mod agent;
mod archive;
mod belief_engine;
mod catalog;
mod client;
mod clusterer;
mod config;
mod db;
mod decision_engine;
mod design;
mod enricher;
mod entity;
mod fetcher;
mod llm;
mod orchestrator;
mod pipeline;
mod premium;
mod question_engine;
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

    // Phase 2: 加载特殊专题 / Flash Mode
    let special_topics = source::load_special_topics(&config.output.vault_path);
    if !special_topics.is_empty() {
        log::info!("📌 加载 {} 个特殊专题", special_topics.len());
        for st in &special_topics {
            log::info!("  - {} (flash: {})", st.title, st.is_flash);
        }
    }

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

    // Phase 3: 加载实体关系数据库（EntitySanctionDb）
    let entity_db_path = data_dir.join("entity_db.json");
    let mut entity_db = if entity_db_path.exists() {
        match entity::EntitySanctionDb::load_from_file(&entity_db_path.to_string_lossy()) {
            Ok(db) => {
                log::info!("🗃️ EntitySanctionDb 已加载: {} 个实体", db.sanctioned.len());
                db
            }
            Err(e) => {
                log::warn!("⚠️ EntitySanctionDb 加载失败: {}，创建新实例", e);
                entity::EntitySanctionDb::new()
            }
        }
    } else {
        log::info!("🗃️ EntitySanctionDb 新实例");
        entity::EntitySanctionDb::new()
    };
    // Phase 4: 接入认知引擎后，entity_db 将被传递到 DiGraph context
    let _entity_db = &mut entity_db;

    // 写入设计令牌 CSS（在抓取前，确保 HTML 引用的 design.css 存在）
    let vault_base = PathBuf::from(&config.output.vault_path);
    fs::create_dir_all(&vault_base)?;
    fs::write(vault_base.join("design.css"), sulix_intel::design::generate_full_css())?;
    log::info!("🎨 design.css 已生成");

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
            is_internal: s.is_internal,
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
            match agent::scan::scan_and_triage(&grouped, &api_key, &config.llm, config.prompts.as_ref()).await {
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
        let mut analysis = clusterer::analyze_theme(theme, &api_key, &config.llm, "en", config.prompts.as_ref()).await?;
        // Phase 1: 蓝军挑战
        let (assumptions, adverse, next_tests, open_questions) =
            clusterer::challenge_theme(&analysis, &api_key, &config.llm, config.prompts.as_ref()).await?;
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
        match clusterer::analyze_theme(theme, &api_key, &config.llm, "zh", config.prompts.as_ref()).await {
            Ok(mut a) => {
                if let Ok((assumptions, adverse, next_tests, open_questions)) =
                    clusterer::challenge_theme(&a, &api_key, &config.llm, config.prompts.as_ref()).await
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

    // === Phase 3: DiGraph 认知引擎管线 ===
    // Cluster -> BlueTeam(veto -> 回Cluster) -> QE -> BE -> DE
    let mut decisions = Vec::new();
    {
        use crate::orchestrator::{
            DiGraph, ClusterNode, BlueTeamNode, QENode, BENode, DENode, blue_team_edge,
            RouteResult, GraphContext,
        };

        let mut ctx = GraphContext::new(config.clone(), api_key.clone());
        ctx.current_themes = themes.clone();
        ctx.current_analyses = analyses.clone();

        let mut graph = DiGraph::new();
        graph.add_node(Box::new(ClusterNode { name: "Cluster" }));
        graph.add_node(Box::new(BlueTeamNode { name: "BlueTeam" }));
        graph.add_node(Box::new(QENode { name: "QE" }));
        graph.add_node(Box::new(BENode { name: "BE" }));
        graph.add_node(Box::new(DENode { name: "DE" }));

        // 蓝军条件边：blue team 检查 + veto 回滚
        graph.add_edge("Cluster", "BlueTeam", Arc::new(|_| RouteResult::ProceedTo("BlueTeam".into())));
        graph.add_edge("BlueTeam", "QE", blue_team_edge("QE"));
        graph.add_edge("QE", "BE", Arc::new(|_| RouteResult::ProceedTo("BE".into())));
        graph.add_edge("BE", "DE", Arc::new(|_| RouteResult::ProceedTo("DE".into())));

        graph.set_entry("Cluster");
        if let Err(e) = graph.run(&mut ctx) {
            log::warn!("⚠️ GraphFlow 认知引擎异常: {}", e);
        }

        decisions = ctx.decisions;
        if !decisions.is_empty() {
            log::info!("🧠 DiGraph 认知引擎: {} 个决策项", decisions.len());
        }
    }

    // === Premium: 多 Agent 深度研报（SVI ≥ 7 的主题）===
    let vault_base = PathBuf::from(&config.output.vault_path);
    let premium_dir = vault_base.join("premium");
    fs::create_dir_all(&premium_dir)?;
    let mut flash_headlines: Vec<String> = Vec::new();
    for (theme, analysis) in themes.iter().zip(analyses.iter()) {
        let svi = clusterer::calculate_svi(analysis, theme, &config.sources);
        if svi < 7 {
            continue;
        }
        let is_flash = svi >= 9;
        if is_flash {
            flash_headlines.push(theme.title.clone());
        }
        let theme_context: String = theme.articles.iter()
            .map(|a| format!("- [{}] {}: {}", a.source, a.title, a.summary.as_deref().unwrap_or("")))
            .collect::<Vec<_>>()
            .join("\n");
        match premium::generate_premium_report(theme, &theme_context, &api_key, &config.llm, config.prompts.as_ref()).await {
            Ok(report) => {
                if let Ok(html) = renderer::render_premium_report(&report) {
                    let slug = theme.title.to_lowercase().replace(' ', "-");
                    let path = premium_dir.join(format!("{}.html", slug));
                    fs::write(&path, &html)?;
                    log::info!("📖 Premium: {} → {}", theme.title, path.display());
                }
                // Phase 2: Substack 自动推送（失败不阻塞管线）
                if let Some(sub) = &config.substack {
                    if sub.enabled {
                        if let Err(e) = premium::push_to_substack(
                            &report, &sub.api_key, &sub.publication_url
                        ).await {
                            log::warn!("⚠️ Substack push failed [{}]: {}", theme.title, e);
                        } else {
                            log::info!("📬 Substack draft created: {}", theme.title);
                        }
                    }
                }
            }
            Err(e) => log::warn!("⚠️ Premium 研报失败 [{}]: {}", theme.title, e),
        }
    }

    let summary = clusterer::synthesize(&themes, &analyses);
    log::info!(
        "✅ 聚类完成: {} 个主题, {} 篇文章",
        summary.theme_count,
        summary.total_articles
    );
    catalog.save_step(7, "summary", &summary)?;

    // === Astro 前端资产：Markdown 输出 ===
    let content_dir = PathBuf::from("D:/Project/Sulix Intelligence/content/posts");
    fs::create_dir_all(&content_dir)?;
    for (theme, analysis) in themes.iter().zip(analyses.iter()) {
        let markdown = renderer::render_signal_markdown(theme, analysis, &today);
        let slug = theme.title.to_lowercase().replace(' ', "-");
        let md_path = content_dir.join(format!("{}-{}.md", today, slug));
        fs::write(&md_path, &markdown)?;
        log::info!("📝 Astro Markdown: {}", md_path.display());
    }

    // 认知校准（传入真数据，非空 vec；跳过校准不中断管线）
    let calibration_text = if !analyses.is_empty() {
        let calibration_input: Vec<llm::VerticalAnalysis> = analyses
            .iter()
            .map(|ta| llm::VerticalAnalysis {
                category: ta.theme_title.clone(),
                articles: vec![],
            })
            .collect();
        agent::calibration::calibrate(&calibration_input, &api_key, &config.llm, config.prompts.as_ref()).await?
    } else {
        String::new()
    };
    catalog.save_step(8, "calibration", &calibration_text)?;

    log::info!(
        "📝 分析主题: {} 个, 信号: {} 条",
        analyses.len(),
        themes.iter().map(|t| t.articles.len()).sum::<usize>() + triage.watchlist.len()
    );

    // === 目录路由写入（/en/ = 英文, /zh/ = 繁体中文）===
    let report_dir = PathBuf::from(&config.output.vault_path);
    let en_root = report_dir.join("en");
    let month_dir = en_root.join(&today[..7]);

    // 英文日详情
    let flash = flash_headlines.first().map(String::as_str);
    if let Ok(html) = renderer::render_html_report(&themes, &analyses, &today, Some(&calibration_text), &config.sources, flash, "en") {
        fs::create_dir_all(&month_dir)?;
        fs::write(month_dir.join("index.html"), &html)?;
        log::info!("📄 EN 简报写入: {}", month_dir.join("index.html").display());
        // Phase 2: 决策输出注入（如果认知引擎产生了决策）
        if !decisions.is_empty() {
            let decision_html = decision_engine::render_decision_html(&decisions);
            let path = month_dir.join("index.html");
            if let Ok(content) = std::fs::read_to_string(&path) {
                let updated = content.replacen("</main>", &format!("{}</main>", decision_html), 1);
                let _ = std::fs::write(&path, &updated);
                log::info!("🧠 决策区块注入: {} 项", decisions.len());
            }
        }
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

        if let Ok(zh_html) = renderer::render_html_report(&themes, &analyses_zh, &today, Some(&calibration_text), &config.sources, flash, "zh") {
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
    db.record_report(
        &today,
        &format!("Daily brief - {} topics", analyses.len()),
        total_new,
    )?;

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
            log::info!(
                "📚 中文看板: {} 条 → {}",
                zh_entries.len(),
                zh_root.join("index.html").display()
            );
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
