//! Intelligence Pipeline 独立运行 CLI
//!
//! 用于独立运行和测试新管线（不依赖旧系统）。
//! 支持分步执行和 debug 输出。
//!
//! 用法:
//!   # 运行完整管线（从 observation.json 开始）
//!   cargo run --bin intel-pipeline -- --input tests/fixtures/observation.json
//!
//!   # 只运行 Signal 分类步骤
//!   cargo run --bin intel-pipeline -- --step signal --input tests/fixtures/observation.json
//!
//!   # Debug 模式（每步写 JSON 文件）
//!   cargo run --bin intel-pipeline -- --input tests/fixtures/observation.json --debug-dir debug/run
//!
//!   # 从 stdin 读取输入
//!   cat observations.json | cargo run --bin intel-pipeline -- --input -

use std::path::PathBuf;

use anyhow::Result;

use sulix_config::LlmConfig;
use sulix_contract as contract;
use sulix_intelligence::*;

/// 简单的参数解析（不用 clap，保持最小依赖）
struct CliArgs {
    input: PathBuf,
    step: StepKind,
    debug_dir: Option<PathBuf>,
    config: PathBuf,
    history: PathBuf,
}

#[derive(Debug, PartialEq)]
enum StepKind {
    Signal,
    Thesis,
    Decision,
    Full,
}

impl CliArgs {
    fn parse() -> Result<Self> {
        let args: Vec<String> = std::env::args().collect();
        let mut input = PathBuf::from("tests/fixtures/observation.json");
        let mut step = StepKind::Full;
        let mut debug_dir: Option<PathBuf> = None;
        let mut config = PathBuf::from("config.toml");
        let mut history = PathBuf::from("data/decision_history.jsonl");

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--input" | "-i" => {
                    i += 1;
                    input = PathBuf::from(&args[i]);
                }
                "--step" | "-s" => {
                    i += 1;
                    step = match args[i].as_str() {
                        "signal" => StepKind::Signal,
                        "thesis" => StepKind::Thesis,
                        "decision" => StepKind::Decision,
                        "full" => StepKind::Full,
                        other => anyhow::bail!("未知步骤: {} (可选: signal|thesis|decision|full)", other),
                    };
                }
                "--debug-dir" => {
                    i += 1;
                    debug_dir = Some(PathBuf::from(&args[i]));
                }
                "--config" | "-c" => {
                    i += 1;
                    config = PathBuf::from(&args[i]);
                }
                "--history" => {
                    i += 1;
                    history = PathBuf::from(&args[i]);
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => anyhow::bail!("未知参数: {}", args[i]),
            }
            i += 1;
        }

        Ok(Self {
            input,
            step,
            debug_dir,
            config,
            history,
        })
    }
}

fn print_help() {
    eprintln!("Intelligence Pipeline 独立运行 CLI");
    eprintln!();
    eprintln!("用法:");
    eprintln!("  intel-pipeline --input <file> [options]");
    eprintln!();
    eprintln!("参数:");
    eprintln!("  --input, -i <path>     输入 JSON 文件（或 \"-\" 读 stdin）");
    eprintln!("  --step, -s <name>      管线步骤: signal|thesis|decision|full (默认 full)");
    eprintln!("  --debug-dir <path>     Debug 输出目录");
    eprintln!("  --config, -c <path>    配置文件 (默认 config.toml)");
    eprintln!("  --history <path>       DecisionHistory 路径 (默认 data/decision_history.jsonl)");
    eprintln!("  --help, -h             显示此帮助");
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = CliArgs::parse()?;

    // 加载配置
    let cfg = sulix_config::Config::from_file(&args.config.to_string_lossy())?;
    let api_key = cfg.get_api_key()?;

    // 读取输入
    let input_json = if args.input.to_string_lossy() == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        std::fs::read_to_string(&args.input)?
    };
    let artifact: Artifact = serde_json::from_str(&input_json)?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    // 构建 StepContext
    let ctx = match &args.debug_dir {
        Some(dir) => {
            std::fs::create_dir_all(dir)?;
            StepContext::new_debug(&today, dir.clone())
        }
        None => StepContext::new(&today),
    };

    // 从 DecisionHistory 加载历史决策（用于 smoothing）
    let last_decisions = load_last_decisions(&args.history);

    // 执行管线（所有步骤共享同一个 api_key）
    match args.step {
        StepKind::Signal => run_signal(artifact, &cfg.llm, &api_key, &ctx).await?,
        StepKind::Thesis => run_thesis(artifact, &cfg.llm, &api_key, &ctx).await?,
        StepKind::Decision => run_decision(artifact, &cfg.llm, &api_key, &ctx, last_decisions).await?,
        StepKind::Full => run_full(artifact, &cfg.llm, &api_key, &ctx, &args.history, last_decisions).await?,
    }

    Ok(())
}

fn print_artifact(artifact: &Artifact) -> Result<()> {
    println!("{}", artifact.to_json()?);
    Ok(())
}

async fn run_signal(
    artifact: Artifact,
    llm_config: &LlmConfig,
    api_key: &str,
    ctx: &StepContext,
) -> Result<()> {
    let observations = artifact.into_observations()?;
    let step = SignalClassificationStepBuilder::new(llm_config.clone(), api_key).build();
    let signals = step.classify(observations, ctx).await?;
    print_artifact(&Artifact::Signals(signals))
}

async fn run_thesis(
    artifact: Artifact,
    llm_config: &LlmConfig,
    api_key: &str,
    ctx: &StepContext,
) -> Result<()> {
    let signals = artifact.into_signals()?;
    let step = ThesisGenerationStepBuilder::new(llm_config.clone(), api_key).build();
    let theses = step.generate(signals, ctx).await?;
    print_artifact(&Artifact::Theses(theses))
}

async fn run_decision(
    artifact: Artifact,
    llm_config: &LlmConfig,
    api_key: &str,
    ctx: &StepContext,
    last_decisions: Vec<contract::Decision>,
) -> Result<()> {
    let theses = artifact.into_theses()?;
    let step = DecisionMappingStepBuilder::new()
        .with_llm_judge(llm_config.clone(), api_key)
        .with_last_decisions(last_decisions).build();
    let decisions = step.map(theses, ctx).await?;
    print_artifact(&Artifact::Decisions(decisions))
}

async fn run_full(
    artifact: Artifact,
    llm_config: &LlmConfig,
    api_key: &str,
    ctx: &StepContext,
    history_path: &PathBuf,
    last_decisions: Vec<contract::Decision>,
) -> Result<()> {
    let observations = artifact.into_observations()?;
    let pipeline = IntelligencePipeline::new(
        SignalClassificationStepBuilder::new(llm_config.clone(), api_key).build(),
        ThesisGenerationStepBuilder::new(llm_config.clone(), api_key).build(),
        DecisionMappingStepBuilder::new()
            .with_llm_judge(llm_config.clone(), api_key)
            .with_last_decisions(last_decisions).build(),
    );

    let output = pipeline.run(observations, ctx).await?;

    // 文本输出（人类可读）
    println!("=== Intelligence Pipeline Output ===");
    println!("Signals:  {}", output.signals.len());
    println!("Theses:   {}", output.theses.len());
    println!("Decisions: {}", output.decisions.len());

    if !output.signals.is_empty() {
        println!("\n--- Signals ---");
        for s in &output.signals {
            println!("  {} | imp={:.2} | domain={} | {:?}", s.id, s.importance, s.domain, s.category);
        }
    }

    if !output.theses.is_empty() {
        println!("\n--- Theses ---");
        for t in &output.theses {
            println!(
                "  {} | conf={:.2} | status={:?} | evidence={} | conditions={:?}",
                t.id, t.confidence, t.status, t.evidence.len(), t.falsification_conditions
            );
            if let Some(theme) = &t.theme {
                println!("       theme={}", theme);
            }
        }
    }

    if !output.decisions.is_empty() {
        println!("\n--- Decisions ---");
        for d in &output.decisions {
            println!(
                "  {} | {:?} | conf={:.2} | horizon={:?} | rule_passed={} | requires_review={}",
                d.id, d.action, d.confidence, d.horizon, d.rule_passed, d.requires_review
            );
            if !d.reasoning.is_empty() {
                println!("       reasoning: {}", d.reasoning);
            }
        }
    }

    // 写入 DecisionHistory
    let mut history = DecisionHistory::open(history_path)?;
    let count = history.append_from_decisions(&output.decisions, &ctx.today)?;
    println!("\n📜 DecisionHistory: {} 条已追加到 {}", count, history_path.display());

    Ok(())
}

