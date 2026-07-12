//! Sulix Intelligence — 个人创业者的 AI 战略情报助手
//!
//! 新管线架构：
//!   init()            → 配置/DB/EntityDb
//!   agent::signal()   → 源抓取/去重/丰富 (Observation 输入)
//!   intelligence::    → Signal → Thesis → Decision → MDX
//!

use std::path::PathBuf;

use anyhow::Result;

// CLI crate modules
mod agent;
mod db;
mod entity;

use sulix_config as config;
use sulix_contract as contract;
use sulix_intelligence as intelligence;

// ===== 入口 =====

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let start = std::time::Instant::now();
    let cfg = config::Config::from_file("config.toml")?;

    if let Ok(vault_path) = std::env::var("VAULT_PATH") {
        log::info!("⚙️ CI 覆盖 vault_path: {}", vault_path);
        // Note: config is immutable after load; for CI we rely on env override in config loading
    }

    let api_key = cfg.get_api_key()?;
    let data_dir = cfg.storage.as_ref()
        .and_then(|s| s.data_dir.as_deref())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    log::info!(
        "🚀 Sulix Intelligence | {} | 配置: {} 个源, LLM: {}",
        today,
        cfg.sources.len(),
        cfg.llm.model
    );

    // ===== Signal Agent: 抓取 → 去重 → 丰富 → 实体提取 =====
    let signal_result = agent::signal::agent_signal(
        &cfg,
        &crate::db::Database::open(&crate::db::get_db_path(&cfg))?,
        &today,
        crate::entity::EntitySanctionDb::new(),
    )
    .await;

    let Some((new_articles, _source_statuses, _entity_db)) = signal_result? else {
        log::info!("✅ 今日无新文章，结束");
        return Ok(());
    };

    if new_articles.is_empty() {
        log::info!("✅ 今日无新文章，结束");
        return Ok(());
    }

    log::info!("📥 新文章: {} 篇", new_articles.len());

    // ===== Intelligence Pipeline: Observation → Signal → Thesis → Decision =====

    // 1. Article → Observation
    let observations: Vec<contract::Observation> =
        new_articles.iter().map(|a| contract::Observation::from(a.clone())).collect();
    log::info!("  ➡️ {} articles → {} observations", new_articles.len(), observations.len());

    // 2. 从 MemoryEngine 加载已有 Thesis
    let memory_path = data_dir.join("memory_db.json");
    let existing_theses = intelligence::load_theses_from_memory_db(&memory_path);

    // 3. 从 DecisionHistory 加载上次决策
    let history_path = data_dir.join("decision_history.jsonl");
    let last_decisions = intelligence::loader::load_last_decisions(&history_path);

    // 4. 构建管线
    let pipeline = intelligence::IntelligencePipeline::new(
        intelligence::SignalClassificationStepBuilder::new(cfg.llm.clone(), &api_key).build(),
        intelligence::ThesisGenerationStepBuilder::new(cfg.llm.clone(), &api_key)
            .with_existing_theses(existing_theses).build(),
        intelligence::DecisionMappingStepBuilder::new()
            .with_llm_judge(cfg.llm.clone(), &api_key)
            .with_last_decisions(last_decisions).build(),
    );

    let ctx = intelligence::StepContext::new_debug(&today, data_dir.join("debug/pipeline"));

    // 5. 运行管线
    match pipeline.run(observations, &ctx).await {
        Ok(output) => {
            log::info!(
                "  ✅ 管线: {} signals → {} theses → {} decisions ({}ms)",
                output.signals.len(),
                output.theses.len(),
                output.decisions.len(),
                output.stats.elapsed_ms()
            );
            log::info!("  📊 {}", sulix_llm::audit::llm_audit_summary());

            // 后处理: Calibration + Summary
            let calibration = intelligence::postprocessing::calibrate(
                &output.signals, &output.theses, &output.decisions,
                &cfg.llm, &api_key, "zh",
            ).await;
            if !calibration.is_empty() {
                log::info!("  🤖 Calibration: {}", calibration);
            }

            let _summary = intelligence::postprocessing::synthesize(
                &output.signals, &output.theses, &output.decisions,
            );

            // 回流: Thesis → MemoryEngine
            if !output.theses.is_empty() {
                intelligence::save_theses_to_memory_db(&memory_path, &output.theses);
            }

            // 写入 DecisionHistory
            let _ = intelligence::DecisionHistory::open(&history_path)
                .and_then(|mut h| h.append_from_decisions(&output.decisions, &today));

            // MDX 输出
            let intel_mdx_dir = data_dir.join("intelligence_mdx");
            let mdx_cfg = intelligence::output::IntelligenceOutputConfig {
                mdx_dir: intel_mdx_dir,
                locale: "en".into(),
            };
            let _ = intelligence::output::render_to_mdx(&output, &mdx_cfg, &today);
        }
        Err(e) => {
            log::error!("⚠️ 管线运行失败: {}", e);
        }
    }

    log::info!("✅ Sulix Intelligence 执行完成 ({:.1}s)", start.elapsed().as_secs_f64());
    log::info!("  📊 {}", sulix_llm::audit::llm_audit_summary());
    Ok(())
}


