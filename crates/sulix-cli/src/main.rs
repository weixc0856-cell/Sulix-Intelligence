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
use sulix_store as store;
use sulix_store::{DecisionRepository, SignalRepository, ThesisRepository};

// ===== CLI 参数 =====

struct CliArgs {
    config_path: PathBuf,
    debug: bool,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut config_path = PathBuf::from("config.toml");
    let mut debug = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" | "-c" if i + 1 < args.len() => {
                config_path = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--debug" | "-d" => {
                debug = true;
                i += 1;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {
                eprintln!("未知参数: {}", args[i]);
                eprintln!("使用 --help 查看用法");
                std::process::exit(2);
            }
        }
    }
    CliArgs { config_path, debug }
}

fn print_help() {
    eprintln!("Sulix Intelligence — 个人创业者的 AI 战略情报助手");
    eprintln!();
    eprintln!("用法: cargo run -p sulix-cli [选项]");
    eprintln!();
    eprintln!("选项:");
    eprintln!("  --config, -c <path>  配置文件路径 (默认 config.toml)");
    eprintln!("  --debug, -d          调试模式 (每步写 JSON 输出到 data/debug/pipeline/)");
    eprintln!("  --help, -h           显示此帮助");
    eprintln!();
    eprintln!("环境变量:");
    eprintln!("  DEEPSEEK_API_KEY     LLM API 密钥");
    eprintln!("  VAULT_PATH           覆盖输出路径 (CI 使用)");
}

// ===== 入口 =====

#[tokio::main]
async fn main() -> Result<()> {
    let cli = parse_args();
    env_logger::init();
    let start = std::time::Instant::now();
    let cfg = config::Config::from_file(&cli.config_path.to_string_lossy())?;

    if let Ok(vault_path) = std::env::var("VAULT_PATH") {
        log::info!("⚙️ CI 覆盖 vault_path: {}", vault_path);
        // Note: config is immutable after load; for CI we rely on env override in config loading
    }

    let api_key = cfg.get_api_key()?;
    let data_dir = cfg
        .storage
        .as_ref()
        .and_then(|s| s.data_dir.as_deref())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));
    let store_path = data_dir.join("store.db");
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    log::info!(
        "🚀 Sulix Intelligence | {} | 配置: {} 个源, LLM: {}",
        today,
        cfg.sources.len(),
        cfg.llm.model
    );

    // ===== 预加载已有 Thesis/Decision（无论是否有新文章都输出 MDX） =====
    let memory_path = data_dir.join("memory_db.json");
    let history_path = data_dir.join("decision_history.jsonl");
    let existing_theses = intelligence::load_theses_from_memory_db(&memory_path);
    let last_decisions = intelligence::loader::load_last_decisions(&history_path);

    // ===== Signal Agent: 抓取 → 去重 → 丰富 → 实体提取 =====
    let signal_result = agent::signal::agent_signal(
        &cfg,
        &crate::db::Database::open(&crate::db::get_db_path(&cfg))?,
        &today,
    )
    .await;

    let new_articles = signal_result?.unwrap_or_default();

    let output = if new_articles.is_empty() {
        log::info!("✅ 今日无新文章，输出已有 Thesis/Decision 的 MDX");

        // 无新文章时，用已有数据构造 IntelligenceOutput
        let decisions: Vec<contract::Decision> = last_decisions.clone();

        intelligence::IntelligenceOutput {
            signals: vec![],
            theses: existing_theses,
            decisions,
            stats: intelligence::PipelineStats::new(),
        }
    } else {
        log::info!("📥 新文章: {} 篇", new_articles.len());

        // 1. Article → Observation, with entity extraction
        let mut observations: Vec<contract::Observation> =
            new_articles.iter().map(|a| a.clone().into()).collect();
        for obs in &mut observations {
            let text = format!("{} {}", obs.title, obs.raw_content);
            obs.entities = crate::entity::extract_entities_from_text(&text);
        }
        log::info!(
            "  ➡️ {} articles → {} observations",
            new_articles.len(),
            observations.len()
        );

        // 2. 构建管线
        let pipeline = intelligence::IntelligencePipeline::new(
            intelligence::SignalClassificationStepBuilder::new(cfg.llm.clone(), &api_key).build(),
            intelligence::ThesisGenerationStepBuilder::new(cfg.llm.clone(), &api_key)
                .with_existing_theses(existing_theses.clone())
                .build(),
            intelligence::DecisionMappingStepBuilder::new()
                .with_llm_judge(cfg.llm.clone(), &api_key)
                .with_last_decisions(last_decisions.clone())
                .build(),
        );

        let ctx = if cli.debug {
            intelligence::StepContext::new_debug(&today, data_dir.join("debug/pipeline"))
        } else {
            intelligence::StepContext::new(&today)
        };

        // 3. 运行管线
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
                    &output.signals,
                    &output.theses,
                    &output.decisions,
                    &cfg.llm,
                    &api_key,
                    "zh",
                )
                .await;
                if !calibration.is_empty() {
                    log::info!("  🤖 Calibration: {}", calibration);
                }

                let _summary = intelligence::postprocessing::synthesize(
                    &output.signals,
                    &output.theses,
                    &output.decisions,
                );

                // 个人影响分析
                let editor_notes = intelligence::postprocessing::analyze_personal_impact(
                    &output.theses,
                    &output.decisions,
                );
                if !editor_notes.is_empty() {
                    log::info!("  📝 Editor Note: {} 条个人影响分析", editor_notes.len());
                    for note in &editor_notes {
                        log::info!(
                            "    [{:?}] {} (magnitude: {})",
                            note.impact_type,
                            note.description,
                            note.magnitude
                        );
                    }
                }

                // 回流: Thesis → MemoryEngine
                if !output.theses.is_empty() {
                    intelligence::save_theses_to_memory_db(&memory_path, &output.theses);
                }

                // 写入 DecisionHistory
                if let Err(e) = intelligence::DecisionHistory::open(&history_path)
                    .and_then(|mut h| h.append_from_decisions(&output.decisions, &today))
                {
                    log::warn!("⚠️ DecisionHistory 写入失败: {}", e);
                }

                output
            }
            Err(e) => {
                log::error!("⚠️ 管线运行失败: {}", e);
                // 管线失败时，仍用已有数据输出 MDX
                intelligence::IntelligenceOutput {
                    signals: vec![],
                    theses: existing_theses,
                    decisions: last_decisions,
                    stats: intelligence::PipelineStats::new(),
                }
            }
        }
    };

    // ===== 双写: Repository (SQLite) + 现有持久化 =====
    if !output.theses.is_empty() || !output.decisions.is_empty() || !output.signals.is_empty() {
        match store::SqliteStore::open(&store_path) {
            Ok(s) => {
                if let Err(e) = s.theses().save_many(&output.theses) {
                    log::warn!("⚠️ Thesis 仓储写入失败: {}", e);
                } else if !output.theses.is_empty() {
                    log::info!("  💾 Thesis 仓储: {} 条已保存", output.theses.len());
                }
                if let Err(e) = s.decisions().save_many(&output.decisions) {
                    log::warn!("⚠️ Decision 仓储写入失败: {}", e);
                } else if !output.decisions.is_empty() {
                    log::info!("  💾 Decision 仓储: {} 条已保存", output.decisions.len());
                }
                if let Err(e) = s.signals().save_many(&output.signals) {
                    log::warn!("⚠️ Signal 仓储写入失败: {}", e);
                } else if !output.signals.is_empty() {
                    log::info!("  💾 Signal 仓储: {} 条已保存", output.signals.len());
                }
            }
            Err(e) => {
                log::warn!("⚠️ 无法打开 SQLite 存储 ({}), 跳过仓储写入", e);
            }
        }
    }

    // ===== MDX 输出（无论有无新文章，都输出已有数据） =====
    if output.theses.is_empty() && output.decisions.is_empty() {
        log::info!("⚠️ 没有 Thesis 或 Decision 数据，跳过 MDX 输出");
    } else {
        let intel_mdx_dir = data_dir.join("intelligence_mdx");
        let mdx_cfg = intelligence::output::IntelligenceOutputConfig {
            mdx_dir: intel_mdx_dir,
            locale: "en".into(),
        };
        let _ = intelligence::output::render_to_mdx(&output, &mdx_cfg, &today);
    }

    // ===== JSON 导出（供 Worker API 消费） =====
    if !output.theses.is_empty() || !output.decisions.is_empty() || !output.signals.is_empty() {
        match intelligence::output::export_to_json(&output, &data_dir) {
            Ok(path) => log::info!("  📦 JSON export: {}", path.display()),
            Err(e) => log::warn!("⚠️ JSON 导出失败: {}", e),
        }
    }

    log::info!(
        "✅ Sulix Intelligence 执行完成 ({:.1}s)",
        start.elapsed().as_secs_f64()
    );
    log::info!("  📊 {}", sulix_llm::audit::llm_audit_summary());
    Ok(())
}
