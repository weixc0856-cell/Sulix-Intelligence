//! Sulix Intelligence — 个人创业者的 AI 战略情报助手
//!
//! 管线按 4-Agent 架构组织：
//!   init()            — 配置/DB/EntityDb
//!   agent_signal()    — 源抓取/去重/丰富/实体提取 (agent::signal)
//!   agent_research()  — 分流/聚类/分析/蓝军/认知引擎 (agent::research)
//!   agent_publish()   — Premium 报告/Chronicle/看板 (publishing)

use anyhow::Result;
use std::path::PathBuf;

use sulix_intel::agent;
use sulix_intel::config;
use sulix_intel::db;
use sulix_intel::engine::pipeline_health::{PipelineReport, PipelineStatus, StageStatus};
use sulix_intel::entity;
use sulix_intel::fetcher;
use sulix_intel::publishing;
use sulix_intel::source;
use sulix_intel::storage;

/// Init() 的命名返回值
struct InitContext {
    config: config::Config,
    api_key: String,
    db: db::Database,
    catalog: sulix_intel::catalog::DataCatalog,
    data_dir: PathBuf,
    today: String,
    entity_db: entity::EntitySanctionDb,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let start = std::time::Instant::now();
    let ctx = init().await?;
    let mut report = PipelineReport::new(&ctx.today);

    // Signal Agent: 抓取 → 去重 → 丰富 → 实体提取
    let signal_result = agent::signal::agent_signal(
        &ctx.config,
        &ctx.db,
        &ctx.catalog,
        &ctx.today,
        ctx.entity_db,
    )
    .await;
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

    let Some((new_articles, source_statuses, mut entity_db)) = signal_result? else {
        log::warn!("Pipeline: 0 new articles — done");
        report.status = PipelineStatus::StoppedEarly;
        report.add_stage("agent_research", 0, 0, StageStatus::Skipped);
        report.add_stage("agent_publish", 0, 0, StageStatus::Skipped);
        report.duration_seconds = start.elapsed().as_secs_f64();
        let _ = report.save(&ctx.data_dir);
        return Ok(());
    };

    // ===== Scan + Route: Signal Classification → 三路分叉 =====
    let article_count = new_articles.len();
    let total_raw_signals: usize = source_statuses.iter().map(|s| s.signal_count).sum();
    let grouped = sulix_intel::llm::group_by_category(&new_articles);

    let (research_signals, intel_signals, archive_count, triage) = agent::scan::classify_and_route(
        &grouped,
        &ctx.api_key,
        &ctx.config.llm,
        ctx.config.prompts.as_ref(),
        &ctx.config.sources,
    )
    .await?;

    let fallback_count = research_signals.iter()
        .chain(intel_signals.iter())
        .filter(|s| s.assessment.is_fallback)
        .count();
    report.fallback_assessed_count = Some(fallback_count);

    log::info!(
        "📊 Signal Funnel: {} raw → {} articles → Research:{} Intel:{} Archive:{}",
        total_raw_signals,
        article_count,
        research_signals.len(),
        intel_signals.len(),
        archive_count
    );

    // Layer 2: Daily Intel (score ≥ 3)
    let intel_assessments: Vec<sulix_intel::agent::scan::SignalAssessment> = intel_signals
        .iter()
        .map(|cs| cs.assessment.clone())
        .collect();
    let intel_output_dir = PathBuf::from(&ctx.config.output.vault_path)
        .join("intel")
        .join("daily");
    let intel_published = match sulix_intel::publishing::layer2::publish_intel(
        &intel_assessments,
        &ctx.today,
        &intel_output_dir,
    ) {
        Ok(n) => n,
        Err(e) => {
            log::error!("⚠️ Layer 2 intel publish failed: {}", e);
            report.add_stage(
                "layer2_intel",
                intel_assessments.len(),
                0,
                sulix_intel::engine::pipeline_health::StageStatus::Failed,
            );
            0
        }
    };

    // Layer 3: Research (score ≥ 7)
    let llm_calls_before =
        sulix_intel::llm::LLM_CALL_COUNT.load(std::sync::atomic::Ordering::Relaxed);
    let research = if !research_signals.is_empty() {
        let insight_articles: Vec<fetcher::Article> =
            triage.insight.iter().map(|t| t.article.clone()).collect();
        let r = agent::research::agent_research(
            &ctx.config,
            &ctx.api_key,
            &ctx.catalog,
            insight_articles,
        )
        .await?;
        report.theme_count = Some(r.themes.len());
        if r.themes.is_empty() {
            report.status = PipelineStatus::NoOutput;
        }
        report.add_stage(
            "agent_research",
            research_signals.len(),
            r.themes.len(),
            if r.themes.is_empty() {
                StageStatus::Skipped
            } else {
                StageStatus::Success
            },
        );
        r
    } else {
        report.add_stage("agent_research", 0, 0, StageStatus::Skipped);
        publishing::ResearchOutput {
            themes: vec![],
            analyses: vec![],
            analyses_zh: vec![],
            triage,
            new_articles: vec![],
        }
    };

    report.observation_count = Some(total_raw_signals);
    report.signal_count = Some(article_count);

    // Publishing Agent: 5-stage publish → ArtifactSet
    let artifacts = publishing::agent_publish(
        &ctx.config,
        &ctx.api_key,
        &ctx.db,
        &ctx.catalog,
        &ctx.data_dir,
        &ctx.today,
        &mut entity_db,
        research,
        &intel_signals,
    )
    .await?;
    report.add_stage("agent_publish", 0, 0, StageStatus::Success);
    report.duration_seconds = start.elapsed().as_secs_f64();

    // WAL checkpoint: flush all DB writes before uploading intel.db to R2
    if let Err(e) = ctx.db.checkpoint_wal() {
        log::warn!("⚠️ WAL checkpoint failed: {}", e);
    }

    // Assemble PublishBundle + deliver
    let intel_paths = collect_intel_paths(intel_published, &intel_output_dir);
    let llm_calls_total = sulix_intel::llm::LLM_CALL_COUNT
        .load(std::sync::atomic::Ordering::Relaxed)
        - llm_calls_before;

    let publish_report = sulix_intel::delivery::publisher::publish(
        sulix_intel::delivery::publisher::PublishBundle::new(
            artifacts,
            intel_paths,
            archive_count,
            total_raw_signals,
            article_count,
            llm_calls_total,
        ),
        &ctx.config,
        &ctx.data_dir,
        &ctx.today,
    )
    .await?;

    report.status = if publish_report.rejected_count > 0 {
        PipelineStatus::PartialFailure
    } else {
        PipelineStatus::Success
    };

    log::info!(
        "✅ Sulix Intelligence 执行完成 ({:.1}s)",
        report.duration_seconds
    );
    sulix_intel::artifact::report::save_report(
        &report,
        &ctx.data_dir,
        ctx.config.output.frontend_public_dir.as_deref(),
    )?;

    if publish_report.rejected_count > 0 {
        anyhow::bail!(
            "{} object(s) rejected by schema validation. See data/rejected/{}/",
            publish_report.rejected_count,
            ctx.today
        );
    }

    // R2 失败 bail 放在 report.save 之后，确保本地报告不丢
    if publish_report.r2_failed() {
        anyhow::bail!("R2 upload: {}", publish_report.r2_status);
    }

    Ok(())
}

/// 收集发布的 intel JSON 文件路径
fn collect_intel_paths(published: usize, output_dir: &std::path::Path) -> Vec<PathBuf> {
    if published == 0 {
        return vec![];
    }
    let mut paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                paths.push(path);
            }
        }
    }
    paths
}

// ===== 初始化 =====

/// 加载配置、初始化数据库、加载 EntitySanctionDb
async fn init() -> Result<InitContext> {
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
    let db_path = agent::research::get_db_path(&config);
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
    let catalog = sulix_intel::catalog::DataCatalog::new(&data_dir, &today);

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

    Ok(InitContext {
        config,
        api_key,
        db,
        catalog,
        data_dir,
        today,
        entity_db,
    })
}
