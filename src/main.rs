//! Sulix Intelligence — 个人创业者的 AI 战略情报助手
//!
//! 管线按 4-Agent 架构组织：
//!   init()            — 配置/DB/EntityDb/CSS
//!   agent_signal()    — 源抓取/去重/丰富/实体提取
//!   agent_research()  — 分流/聚类/分析/蓝军/认知引擎/BeliefDb
//!   agent_publish()   — Premium 报告/HTML/Chronicle/看板

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use std::fs;
use std::path::Path;
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
mod domain;
mod engine;
mod enricher;
mod entity;
mod event_log;
mod fetcher;
mod hermes;
mod llm;
mod orchestrator;
mod pipeline;
mod premium;
mod question_engine;
mod renderer;
mod source;
mod twitter;
mod publishing;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("🚀 Sulix Intelligence — 启动");

    // Agent 0: 初始化
    let (config, api_key, db, catalog, data_dir, today, mut entity_db) = init().await?;

    // Signal Agent: 抓取 → 去重 → 丰富 → 实体提取
    let Some((new_articles, source_statuses, entity_db_fetched)) =
        agent_signal(&config, &api_key, &db, &catalog, &today, entity_db).await?
    else {
        return Ok(());
    };
    entity_db = entity_db_fetched;

    // Research Agent (+ Memory Agent): 分流 → 聚类 → 分析 → 认知引擎 → BeliefDb
    let source_statuses_clone = source_statuses.clone();
    let research = agent_research(
        &config,
        &api_key,
        &db,
        &catalog,
        &data_dir,
        &today,
        new_articles,
        source_statuses_clone,
    )
    .await?;

    // Publishing Agent: Premium → 渲染 → Chronicle → 看板
    publishing::agent_publish(
        &config,
        &api_key,
        &db,
        &catalog,
        &data_dir,
        &today,
        &mut entity_db,
        research,
        source_statuses,
    )
    .await?;

    log::info!("✅ Sulix Intelligence 执行完成");
    Ok(())
}

// ===== Agent 0: 初始化 =====

/// 加载配置、初始化数据库、加载 EntitySanctionDb、生成设计 CSS
async fn init() -> Result<(
    config::Config,
    String,
    db::Database,
    catalog::DataCatalog,
    PathBuf,
    String,
    entity::EntitySanctionDb,
)> {
    // 加载配置
    let mut config = config::Config::from_file("config.toml")?;
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

    // 加载特殊专题
    let special_topics = source::load_special_topics(&config.output.vault_path);
    if !special_topics.is_empty() {
        log::info!("📌 加载 {} 个特殊专题", special_topics.len());
        for st in &special_topics {
            log::info!("  - {} (flash: {})", st.title, st.is_flash);
        }
    }

    // 初始化数据库
    let db_path = get_db_path(&config);
    let db = db::Database::open(&db_path)?;
    log::info!("数据库已连接: {}", db_path.display());

    // 初始化认知审计链
    let data_dir = config
        .storage
        .as_ref()
        .and_then(|s| s.data_dir.as_deref())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let catalog = catalog::DataCatalog::new(&data_dir, &today);

    // 加载 EntitySanctionDb
    let entity_db_path = data_dir.join("entity_db.json");
    let entity_db = if entity_db_path.exists() {
        match entity::EntitySanctionDb::load_from_file(&entity_db_path.to_string_lossy()) {
            Ok(db) => {
                log::info!("🗃️ EntitySanctionDb 已加载: {} 个实体", db.sanctioned.len());
                db
            }
            Err(e) => {
                let backup = format!(
                    "{}.corrupt.{}",
                    entity_db_path.to_string_lossy(),
                    chrono::Utc::now().format("%Y%m%d_%H%M%S")
                );
                log::warn!("⚠️ EntitySanctionDb 加载失败 ({}), 备份到 {} 后重建", e, backup);
                let _ = std::fs::rename(&entity_db_path, &backup);
                entity::EntitySanctionDb::new()
            }
        }
    } else {
        log::info!("🗃️ EntitySanctionDb 新实例");
        entity::EntitySanctionDb::new()
    };

    // 写入设计令牌 CSS
    let vault_base = PathBuf::from(&config.output.vault_path);
    fs::create_dir_all(&vault_base)?;
    fs::write(
        vault_base.join("design.css"),
        sulix_intel::design::generate_full_css(),
    )?;
    log::info!("🎨 design.css 已生成");

    Ok((config, api_key, db, catalog, data_dir, today, entity_db))
}

// ===== Signal Agent: 抓取 → 去重 → 丰富 =====

/// 源抓取 → Pipeline 清洗去重 → SQLite 去重 → Trend 写入 → 证据快照 → 丰富 → 实体提取
/// 返回 None 表示今日无新文章（管线提前终止）
async fn agent_signal(
    config: &config::Config,
    _api_key: &str,
    db: &db::Database,
    catalog: &catalog::DataCatalog,
    today: &str,
    mut entity_db: entity::EntitySanctionDb,
) -> Result<
    Option<(
        Vec<fetcher::Article>,
        Vec<(String, bool, usize)>,
        entity::EntitySanctionDb,
    )>,
> {
    // 源抓取
    log::info!("开始拉取信号源...");
    let enabled_sources: Vec<&config::SourceConfig> =
        config.sources.iter().filter(|s| s.enabled).collect();
    let mut all_signals = Vec::new();
    let mut source_statuses: Vec<(String, bool, usize)> = Vec::new();
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

    // Pipeline 清洗去重
    let before_pipeline = all_signals.len();
    pipeline::run_pipeline_with_config(&mut all_signals, config.dedup.as_ref())?;
    log::info!(
        "Pipeline: {} → {} 条（清洗/合规/去重）",
        before_pipeline,
        all_signals.len()
    );
    catalog.save_step(1, "raw_signals", &all_signals)?;

    // Article 转换 + SQLite 去重
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
        return Ok(None);
    }

    println!("\n📋 === 今日新增 {} 篇 ===\n", new_articles.len());
    for a in &new_articles {
        println!("  [{}/{}] {}", a.category, a.source, a.title);
    }

    // Trend Layer 写入
    {
        use std::collections::HashMap;
        let mut cat_counts: HashMap<&str, u32> = HashMap::new();
        for a in &new_articles {
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

    // 证据快照
    let vault_path = &config.output.vault_path;
    for article in &new_articles {
        if article.content.is_some() || article.summary.is_some() {
            let signal = crate::source::RawSignal {
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

    // Wikipedia 注入 + 正文提取
    enricher::enrich_with_wikipedia(&mut new_articles, 3).await;
    catalog.save_step(3, "enriched_signals", &new_articles)?;
    fetcher::enrich_articles_content(&mut new_articles, 5).await;

    // 实体提取
    for article in &new_articles {
        let combined = format!(
            "{} {}",
            article.title,
            article.summary.as_deref().unwrap_or("")
        );
        let names = entity::extract_entities_from_text(&combined);
        for name in &names {
            let exists = entity_db.sanctioned.values().any(|e| e.name == *name)
                || entity_db.unsanctioned.values().any(|e| e.name == *name);
            if !exists {
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

    Ok(Some((new_articles, source_statuses, entity_db)))
}

use publishing::ResearchOutput;

// ===== Research Agent (+ Memory Agent): 分流 → 聚类 → 分析 → 认知引擎 → BeliefDb =====

/// Scan Agent 分流 → LLM 预去重 → 聚类 → 主题分析 → 蓝军验证 → DiGraph 引擎 → BeliefDb
/// 返回 ResearchOutput 供 Publishing Agent 使用
#[allow(clippy::too_many_arguments)]
async fn agent_research(
    config: &config::Config,
    api_key: &str,
    _db: &db::Database,
    catalog: &catalog::DataCatalog,
    data_dir: &Path,
    today: &str,
    new_articles: Vec<fetcher::Article>,
    _source_statuses: Vec<(String, bool, usize)>,
) -> Result<ResearchOutput> {
    // 分组 + Scan Agent
    let grouped = llm::group_by_category(&new_articles);
    let total_new = new_articles.len();
    let triage = if let Some(ref sc) = config.scan_agent {
        if sc.enabled && !grouped.is_empty() {
            match agent::scan::scan_and_triage(
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

    if triage.insight.is_empty() && triage.watchlist.is_empty() {
        println!("\n📋 今日 {} 篇全部进入 Signal Memory。\n", total_new);
        return Ok(ResearchOutput {
            themes: vec![],
            analyses: vec![],
            analyses_zh: vec![],
            decisions: vec![],
            triage,
            total_new,
            new_articles: vec![],
            question_matches: vec![],
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
    let themes = if insight_articles.is_empty() {
        vec![]
    } else {
        clusterer::cluster_articles(&insight_articles, api_key, &config.llm).await?
    };
    catalog.save_step(5, "themes", &themes)?;

    // 主题分析 + 蓝军验证
    let mut analyses = Vec::new();
    for theme in &themes {
        log::info!(
            "🔍 分析主题: {} ({} 条证据)",
            theme.title,
            theme.articles.len()
        );
        let mut analysis =
            clusterer::analyze_theme(theme, api_key, &config.llm, "en", config.prompts.as_ref())
                .await?;
        let (assumptions, adverse, next_tests, open_questions) =
            clusterer::challenge_theme(&analysis, api_key, &config.llm, config.prompts.as_ref())
                .await?;
        analysis.assumptions = assumptions;
        analysis.adverse = adverse;
        analysis.next_tests = next_tests;
        analysis.open_questions = open_questions;
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
        match clusterer::analyze_theme(theme, api_key, &config.llm, "zh", config.prompts.as_ref())
            .await
        {
            Ok(mut a) => {
                if let Ok((assumptions, adverse, next_tests, open_questions)) =
                    clusterer::challenge_theme(&a, api_key, &config.llm, config.prompts.as_ref())
                        .await
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

    // DiGraph 认知引擎 + BeliefDb
    let decisions;
    let question_matches: Vec<crate::question_engine::QuestionMatch>;
    {
        use crate::orchestrator::{
            blue_team_edge, BENode, BlueTeamNode, ClusterNode, DENode, DiGraph, GraphContext,
            QENode, RouteResult,
        };
        let mut ctx = GraphContext::new(config.clone(), api_key.to_string());
        ctx.current_themes = themes.clone();
        ctx.current_analyses = analyses.clone();

        let mut graph = DiGraph::new();
        graph.add_node(Box::new(ClusterNode { name: "Cluster" }));
        graph.add_node(Box::new(BlueTeamNode { name: "BlueTeam" }));
        graph.add_node(Box::new(QENode { name: "QE" }));
        graph.add_node(Box::new(BENode { name: "BE" }));
        graph.add_node(Box::new(DENode { name: "DE" }));
        graph.add_edge(
            "Cluster",
            "BlueTeam",
            Arc::new(|_| RouteResult::ProceedTo("BlueTeam".into())),
        );
        graph.add_edge("BlueTeam", "QE", blue_team_edge("QE"));
        graph.add_edge(
            "QE",
            "BE",
            Arc::new(|_| RouteResult::ProceedTo("BE".into())),
        );
        graph.add_edge(
            "BE",
            "DE",
            Arc::new(|_| RouteResult::ProceedTo("DE".into())),
        );
        graph.set_entry("Cluster");
        if let Err(e) = graph.run(&mut ctx) {
            log::warn!("⚠️ GraphFlow 认知引擎异常: {}", e);
        }
        decisions = ctx.decisions;
        if !decisions.is_empty() {
            log::info!("🧠 DiGraph 认知引擎: {} 个决策项", decisions.len());
        }

        // BeliefDb 持久化（含损坏备份保护）
        let belief_db_path = data_dir.join("belief_db.json");
        let mut belief_db = if belief_db_path.exists() {
            match crate::belief_engine::BeliefDb::load_from_file(&belief_db_path.to_string_lossy())
            {
                Ok(db) => db,
                Err(e) => {
                    let backup = format!(
                        "{}.corrupt.{}",
                        belief_db_path.to_string_lossy(),
                        chrono::Utc::now().format("%Y%m%d_%H%M%S")
                    );
                    log::error!("⚠️ BeliefDb 加载失败 ({}), 备份到 {} 后重建", e, backup);
                    let _ = std::fs::rename(&belief_db_path, &backup);
                    crate::belief_engine::BeliefDb::new(today)
                }
            }
        } else {
            let d = crate::belief_engine::BeliefDb::new(today);
            log::info!("🧠 BeliefDb 新实例");
            d
        };
        belief_db.snapshot_date = today.to_string();
        belief_db.apply_updates(&ctx.belief_updates);
        if let Err(e) = belief_db.save_to_file(&belief_db_path.to_string_lossy()) {
            log::warn!("⚠️ BeliefDb 保存失败: {}", e);
        }

        // 收集 QuestionEngine 匹配结果供 ASI 使用
        question_matches = ctx
            .question_matches
            .into_iter()
            .flatten()
            .collect();
    }

    // 返回研究结果供 Publishing Agent 使用
    Ok(ResearchOutput {
        themes,
        analyses,
        analyses_zh,
        decisions,
        triage,
        total_new,
        new_articles,
        question_matches,
    })
}

// ===== [agent_publish moved to src/publishing.rs] =====
fn get_db_path(config: &config::Config) -> PathBuf {
    let data_dir = config
        .storage
        .as_ref()
        .and_then(|s| s.data_dir.as_deref())
        .unwrap_or("data");
    PathBuf::from(data_dir).join("intel.db")
}
