//! Sulix Intelligence — 个人创业者的 AI 战略情报助手
//!
//! 管线按 4-Agent 架构组织：
//!   init()            — 配置/DB/EntityDb/CSS
//!   agent_signal()    — 源抓取/去重/丰富/实体提取
//!   agent_research()  — 分流/聚类/分析/蓝军/认知引擎/BeliefDb
//!   agent_publish()   — Premium 报告/HTML/Chronicle/看板

use anyhow::Result;
use std::path::PathBuf;

use sulix_intel::agent;
use sulix_intel::catalog;
use sulix_intel::clusterer;
use sulix_intel::config;
use sulix_intel::db;
use sulix_intel::engine;
use sulix_intel::entity;
use sulix_intel::fetcher;
use sulix_intel::llm;
use sulix_intel::pipeline;
use sulix_intel::publishing;
use sulix_intel::enricher;
use sulix_intel::source;
use sulix_intel::storage;
use sulix_intel::engine::pipeline_health::StageStatus;

/// 信号源抓取计数
#[derive(Debug, Clone)]
struct SourceStatus {
    signal_count: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let start = std::time::Instant::now();
    log::info!("🚀 Sulix Intelligence — 启动");

    // Agent 0: 初始化
    let (config, api_key, db, catalog, data_dir, today, mut entity_db) = init().await?;

    let mut report = sulix_intel::engine::pipeline_health::PipelineReport::new(&today);

    // Signal Agent: 抓取 → 去重 → 丰富 → 实体提取
    let signal_result = agent_signal(&config, &db, &catalog, &today, entity_db).await;
    report.add_stage(
        "agent_signal",
        0,
        0,
        if signal_result.is_ok() {
            StageStatus::Success
        } else {
            StageStatus::Failed
        },
    );

    let Some((new_articles, source_statuses, entity_db_fetched)) = signal_result? else {
        log::warn!("Pipeline: 0 new articles — done");
        report.status = sulix_intel::engine::pipeline_health::PipelineStatus::StoppedEarly;
        report.add_stage("agent_research", 0, 0, StageStatus::Skipped);
        report.add_stage("agent_publish", 0, 0, StageStatus::Skipped);
        report.duration_seconds = start.elapsed().as_secs_f64();
        let _ = report.save(&data_dir);
        return Ok(());
    };
    entity_db = entity_db_fetched;

    // Research Agent: 分流 → 聚类 → 分析 → 认知引擎 → BeliefDb
    let new_article_count = new_articles.len(); // 保存在 new_articles 被 move 前
    // 快照分类计数（在 new_articles move 前）
    let category_snapshot: std::collections::HashMap<String, usize> = {
        let mut m = std::collections::HashMap::new();
        for a in &new_articles {
            if !a.is_internal {
                *m.entry(a.category.clone()).or_insert(0) += 1;
            }
        }
        m
    };
    let total_signals: usize = source_statuses.iter().map(|s| s.signal_count).sum();

    let research = agent_research(
        &config,
        &api_key,
        &catalog,
        new_articles,
    )
    .await?;
    // 填充报告聚合字段（前端消费稳定接口）
    report.observation_count = Some(total_signals);
    report.signal_count = Some(new_article_count);
    report.theme_count = Some(research.themes.len());
    if !category_snapshot.is_empty() {
        report.category_counts = Some(category_snapshot);
    }
    if research.themes.is_empty() {
        report.status = sulix_intel::engine::pipeline_health::PipelineStatus::NoOutput;
    }
    report.add_stage(
        "agent_research",
        total_signals,
        research.themes.len(),
        if research.themes.is_empty() {
            StageStatus::Skipped
        } else {
            StageStatus::Success
        },
    );

    // Publishing Agent: 5-stage publish → returns ArtifactSet
    let artifacts = publishing::agent_publish(
        &config,
        &api_key,
        &db,
        &catalog,
        &data_dir,
        &today,
        &mut entity_db,
        research,
    )
    .await?;
    report.add_stage("agent_publish", 0, 0, StageStatus::Success);

    report.duration_seconds = start.elapsed().as_secs_f64();

    // Verify: if config.r2 is configured but missing env vars, skip R2
    // Delivery publisher handles this: local write + R2 (if available by env)
    let publish_report = sulix_intel::delivery::publisher::publish(
        artifacts,
        &config,
        &data_dir,
        &today,
    ).await?;

    report.status = if publish_report.rejected_count > 0 {
        sulix_intel::engine::pipeline_health::PipelineStatus::PartialFailure
    } else {
        sulix_intel::engine::pipeline_health::PipelineStatus::Success
    };
    // Use duplicated status if publish had issues
    #[allow(unused_assignments)]
    {
        let _ = &publish_report;
    }

    // 跳过旧 R2/manifest/frontend sync 代码——已由 delivery::publisher 接管
    log::info!(
        "✅ Sulix Intelligence 执行完成 ({:.1}s)",
        report.duration_seconds
    );

    // After delivery publish: report dual-write (data/ + public/ + vault)
    // Note: report duration_seconds is now set, so this is the final version
    sulix_intel::artifact::report::save_report(
        &report,
        &data_dir,
        config.output.frontend_public_dir.as_deref(),
    )?;

    // Validate output contract
    validate_output_contract(&config, &report).await;

    // Non-zero exit if objects were rejected by schema validation
    if publish_report.rejected_count > 0 {
        anyhow::bail!(
            "{} object(s) rejected by schema validation. See data/rejected/{}/",
            publish_report.rejected_count, today
        );
    }

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
    let entity_db = storage::with_corrupt_recovery(
        &entity_db_path,
        |p| {
            let db = entity::EntitySanctionDb::load_from_file(&p.to_string_lossy())?;
            log::info!("🗃️ EntitySanctionDb 已加载: {} 个实体", db.sanctioned.len());
            Ok(db)
        },
        || {
            log::info!("🗃️ EntitySanctionDb 新实例");
            entity::EntitySanctionDb::new()
        },
    );

    Ok((config, api_key, db, catalog, data_dir, today, entity_db))
}

// ===== Signal Agent: 抓取 → 去重 → 丰富 =====

/// 源抓取 → Pipeline 清洗去重 → SQLite 去重 → Trend 写入 → 证据快照 → 丰富 → 实体提取
/// 返回 None 表示今日无新文章（管线提前终止）
async fn agent_signal(
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
    // 源抓取
    log::info!("开始拉取信号源...");
    let enabled_sources: Vec<&config::SourceConfig> =
        config.sources.iter().filter(|s| s.enabled).collect();
    let mut all_signals = Vec::new();
    let mut source_statuses: Vec<SourceStatus> = Vec::new();
    let date_range = &config.output.date_range;
    for sc in &enabled_sources {
        match source::fetch_source(sc, date_range).await {
            Ok(mut signals) => {
                log::info!("  [{}] → {} 条信号", sc.name, signals.len());
                source_statuses.push(SourceStatus { signal_count: signals.len() });
                all_signals.append(&mut signals);
            }
            Err(e) => {
                log::warn!("⚠️ [{}] 抓取失败: {}", sc.name, e);
                source_statuses.push(SourceStatus { signal_count: 0 });
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
            let signal = sulix_intel::source::RawSignal {
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

// ===== Pipeline Step: 主题分析 + 蓝军验证 =====

/// 对单个主题执行分析（英文或中文）。
///
/// Pipeline Step 封装：让"主题分析"成为可测试、可替换的独立步骤。
/// - analyze_theme 失败 → 跳过（返回 None），避免整个管线崩溃
/// - challenge_theme 失败 → 继续（使用无蓝军分析）
/// 
/// 注意：蓝军降级（signal_strength 扣减）已移至 BlueTeamNode 执行，
/// 此处不再处理。这样保证了降级逻辑在 Graph 编排内原子化执行。
async fn analyze_and_validate(
    theme: &sulix_intel::domain::theme::Theme,
    api_key: &str,
    llm_config: &config::LlmConfig,
    prompts: Option<&config::PromptsConfig>,
    language: &str,
) -> Option<sulix_intel::domain::theme::ThemeAnalysis> {
    let mut analysis = match crate::engine::analysis::analyze_theme(theme, api_key, llm_config, language, prompts).await {
        Ok(a) => a,
        Err(e) => {
            log::warn!("⚠️ 主题分析失败 [{}|{}]: {}", language, theme.title, e);
            return None;
        }
    };
    match crate::engine::analysis::challenge_theme(&analysis, api_key, llm_config, prompts).await {
        Ok((assumptions, adverse, next_tests, open_questions)) => {
            analysis.assumptions = assumptions;
            analysis.adverse = adverse;
            analysis.next_tests = next_tests;
            analysis.open_questions = open_questions;
        }
        Err(e) => log::warn!("⚠️ 蓝军验证失败 [{}|{}], 使用无蓝军分析: {}", language, theme.title, e),
    }
    Some(analysis)
}

// ===== Research Agent (+ Memory Agent): 分流 → 聚类 → 分析 → 认知引擎 → BeliefDb =====

/// Scan Agent 分流 → LLM 预去重 → 聚类 → 主题分析 → 蓝军验证 → DiGraph 引擎 → BeliefDb
/// 返回 ResearchOutput 供 Publishing Agent 使用
#[allow(clippy::too_many_arguments)]
async fn agent_research(
    config: &config::Config,
    api_key: &str,
    catalog: &catalog::DataCatalog,
    new_articles: Vec<fetcher::Article>,
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
        if let Some(a) = analyze_and_validate(theme, api_key, &config.llm, config.prompts.as_ref(), "en").await {
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
        if let Some(a) = analyze_and_validate(theme, api_key, &config.llm, config.prompts.as_ref(), "zh").await {
            analyses_zh.push(a);
        }
    }
    log::info!("✅ 中文分析完成: {} 篇", analyses_zh.len());

    // QuestionEngine 已移除（原始终返回空列表）。保留 question_engine.rs 模块供未来启用。
    // 返回研究结果供 Publishing Agent 使用
    Ok(ResearchOutput {
        themes,
        analyses,
        analyses_zh,
        triage,
        new_articles,
    })
}

// ===== Phase 0: Output Contract Validation =====

/// Validate that all required output artifacts exist and are internally consistent.
///
/// This is the **contract enforcement layer** between sulix-engine and sulix-web.
/// It checks:
///   1. Manifest file exists and is valid JSON
///   2. At least one MDX output directory has content (daily / thesis / research)
///   3. Pipeline report is not in Failed state
///
/// Non-fatal: warnings are logged but the pipeline does not fail.
async fn validate_output_contract(
    config: &config::Config,
    report: &sulix_intel::engine::pipeline_health::PipelineReport,
) {
    let mut issues: Vec<String> = Vec::new();

    // 1. Check manifest exists
    if let Some(ref mdx_out) = config.output.mdx_dir {
        let manifest_path = std::path::PathBuf::from(mdx_out).join("manifest.json");
        if !manifest_path.exists() {
            issues.push(format!("Manifest not found: {}", manifest_path.display()));
        } else {
            match std::fs::read_to_string(&manifest_path) {
                Ok(content) => {
                    if serde_json::from_str::<serde_json::Value>(&content).is_err() {
                        issues.push(format!("Manifest is not valid JSON: {}", manifest_path.display()));
                    }
                }
                Err(e) => issues.push(format!("Manifest unreadable: {}: {}", manifest_path.display(), e)),
            }
        }

        // 2. Check MDX output directories have content
        let required_dirs = ["daily", "thesis", "research", "investigation"];
        for dir_name in &required_dirs {
            let dir_path = std::path::PathBuf::from(mdx_out).join(dir_name);
            if dir_path.exists() {
                if let Ok(entries) = std::fs::read_dir(&dir_path) {
                    let md_count = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
                        .count();
                    if md_count == 0 {
                        issues.push(format!("MDX dir '{}' exists but contains 0 .md files", dir_name));
                    }
                }
            } else {
                // Only warn if the pipeline completed successfully — zero-output runs are fine
                if report.status == sulix_intel::engine::pipeline_health::PipelineStatus::Success {
                    issues.push(format!("MDX dir '{}' does not exist", dir_name));
                }
            }
        }
    }

    // 3. Check pipeline report isn't Failed
    if report.status == sulix_intel::engine::pipeline_health::PipelineStatus::PartialFailure
        || report.status == sulix_intel::engine::pipeline_health::PipelineStatus::StoppedEarly
    {
        issues.push(format!("Pipeline finished with status: {:?}", report.status));
    }

    // 4. Check frontend content sync if configured
    if let Some(ref fe_public_dir) = config.output.frontend_public_dir {
        let fe_path = std::path::Path::new(fe_public_dir);
        if !fe_path.exists() {
            issues.push(format!(
                "Frontend public dir configured but does not exist: {}",
                fe_public_dir
            ));
        }
    }

    if issues.is_empty() {
        log::info!("✅ Output contract validation passed");
    } else {
        log::warn!("⚠️ Output contract issues:");
        for issue in &issues {
            log::warn!("  - {}", issue);
        }
    }
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
