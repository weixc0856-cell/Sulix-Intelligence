//! Sulix Outcome CLI — 记录和复盘决策结果
//!
//! 认知链闭环：Decision → Outcome → Reflection
//! 两步分离：record 只录事实，reflect 另行触发复盘。
//!
//! Usage:
//!   sulix-outcome record DEC-001 --verdict partial --evidence "..." --impact medium
//!   sulix-outcome reflect OUT-001
//!   sulix-outcome list
//!   sulix-outcome status

use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: sulix-outcome <command> [options]");
        eprintln!("Commands:");
        eprintln!("  record DEC-ID --verdict <v> --evidence <text> --impact <low|medium|high>");
        eprintln!("  reflect OUT-ID");
        eprintln!("  list");
        eprintln!("  status");
        std::process::exit(1);
    }

    let result = match args[1].as_str() {
        "record" => cmd_record(&args[1..]),
        "reflect" => cmd_reflect(&args[1..]),
        "list" => cmd_list(&args[1..]),
        "status" => cmd_status(&args[1..]),
        other => {
            eprintln!("Unknown command: {other}");
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

// ===== Config loading =====

fn load_config() -> anyhow::Result<(sulix_intel::config::Config, PathBuf)> {
    let config = sulix_intel::config::Config::from_file("config.toml")?;
    let vault_path = PathBuf::from(&config.output.vault_path);
    Ok((config, vault_path))
}

fn load_memory(vault_path: &Path) -> anyhow::Result<sulix_intel::engine::memory::MemoryEngine> {
    let memory_path = vault_path.join("memory_db.json");
    let mut memory = sulix_intel::engine::memory::MemoryEngine::new(memory_path);
    memory.load()?;
    Ok(memory)
}

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// 生成 OUT-YYYYMMDD-SEQ 格式 ID
fn generate_outcome_id(vault_path: &Path) -> anyhow::Result<String> {
    let memory = load_memory(vault_path)?;
    let existing = memory.all_outcomes();
    let date_prefix = today();
    let max_seq = existing.iter()
        .filter_map(|o| o.id.strip_prefix(&format!("OUT-{}", date_prefix)))
        .filter_map(|s| s.strip_prefix('-'))
        .filter_map(|s| s.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    Ok(format!("OUT-{}-{:03}", date_prefix, max_seq + 1))
}

/// 双写者防护：检查管线是否在运行
fn check_pipeline_lock(data_dir: &Path) -> anyhow::Result<()> {
    let pid_path = data_dir.join("pipeline.pid");
    if pid_path.exists() {
        let content = std::fs::read_to_string(&pid_path)?;
        anyhow::bail!("Pipeline in progress (pid={}). Try later.", content.trim());
    }
    Ok(())
}

/// 写前检查 mtime，防并发覆盖
fn check_mtime(path: &Path) -> anyhow::Result<SystemTime> {
    let meta = std::fs::metadata(path)?;
    Ok(meta.modified()?)
}

// ===== Commands =====

fn cmd_record(args: &[String]) -> anyhow::Result<()> {
    // Parse: record DEC-ID --verdict <v> --evidence <text> --impact <level>
    if args.len() < 2 {
        anyhow::bail!("Usage: sulix-outcome record DEC-ID --verdict <v> --evidence <text> --impact <level>");
    }
    let dec_id = &args[1];

    let mut verdict_str = "";
    let mut evidence = "";
    let mut impact_str = "";
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--verdict" => { i += 1; verdict_str = args.get(i).map_or("", |s| s); }
            "--evidence" => { i += 1; evidence = args.get(i).map_or("", |s| s); }
            "--impact" => { i += 1; impact_str = args.get(i).map_or("", |s| s); }
            _ => { anyhow::bail!("Unknown option: {}", args[i]); }
        }
        i += 1;
    }

    if verdict_str.is_empty() { anyhow::bail!("--verdict is required (confirmed|partial|invalidated|unknown)"); }
    if evidence.is_empty() { anyhow::bail!("--evidence is required"); }

    let verdict = match verdict_str {
        "confirmed" => sulix_intel::domain::outcome::OutcomeVerdict::Confirmed,
        "partial" => sulix_intel::domain::outcome::OutcomeVerdict::PartiallyConfirmed,
        "invalidated" => sulix_intel::domain::outcome::OutcomeVerdict::Invalidated,
        "unknown" => sulix_intel::domain::outcome::OutcomeVerdict::Unknown,
        other => anyhow::bail!("Invalid verdict: {other} (confirmed|partial|invalidated|unknown)"),
    };
    let impact = match impact_str {
        "low" => sulix_intel::domain::outcome::ImpactLevel::Low,
        "medium" => sulix_intel::domain::outcome::ImpactLevel::Medium,
        "high" => sulix_intel::domain::outcome::ImpactLevel::High,
        "" => sulix_intel::domain::outcome::ImpactLevel::Medium,
        other => anyhow::bail!("Invalid impact: {other} (low|medium|high)"),
    };

    let (config, vault_path) = load_config()?;
    let data_dir = config.storage.as_ref()
        .and_then(|s| s.data_dir.as_deref())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./data"));

    // 双写者防护
    check_pipeline_lock(&data_dir)?;

    // 加载 memory
    let mut memory = load_memory(&vault_path)?;

    // 验证 decision 存在
    let decision = memory.all_decisions().iter()
        .find(|d| d.id == *dec_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Decision '{dec_id}' not found"))?;

    // 生成 ID 和创建 Outcome
    let id = generate_outcome_id(&vault_path)?;
    let outcome = sulix_intel::domain::outcome::Outcome::new(
        id.clone(),
        dec_id.clone(),
        decision.thesis_id.clone(),
        evidence.to_string(),
        verdict,
        impact,
        today(),
    ).0; // Take just the Outcome, events are added by add_outcome()

    // 写前 mtime 检查
    let mem_path = vault_path.join("memory_db.json");
    let before = if mem_path.exists() { Some(check_mtime(&mem_path)?) } else { None };

    // 写入
    let events = memory.add_outcome(outcome)?;
    memory.save()?;

    // 写后 mtime 检查
    if let Some(before_mtime) = before {
        let after = check_mtime(&mem_path)?;
        if after != before_mtime {
            // File was modified by another process between read and write
            // Our save still succeeded, but we should warn
            eprintln!("Warning: memory_db.json was modified concurrently");
        }
    }

    // 追加事件到 JSONL
    let events_dir = data_dir.join("events");
    std::fs::create_dir_all(&events_dir)?;
    let events_path = events_dir.join(format!("{}.jsonl", today()));
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&events_path)?;
    use std::io::Write;
    for event in &events {
        let line = serde_json::to_string(event)?;
        writeln!(file, "{}", line)?;
    }

    println!("✅ Outcome {id} recorded for {dec_id} ({verdict_str}, {impact_str})");
    println!("   Events written to data/events/{}.jsonl", today());
    Ok(())
}

fn cmd_reflect(args: &[String]) -> anyhow::Result<()> {
    if args.len() < 2 {
        anyhow::bail!("Usage: sulix-outcome reflect OUT-ID");
    }
    let outcome_id = &args[1];

    let (_, vault_path) = load_config()?;
    let mut memory = load_memory(&vault_path)?;

    // Find outcome
    let outcome = memory.all_outcomes().iter()
        .find(|o| o.id == *outcome_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Outcome '{outcome_id}' not found"))?;

    // Generate reflection (doesn't auto-store — returns it)
    let reflection = memory.generate_reflection(&outcome.thesis_id)?;
    memory.add_reflection(reflection);
    memory.save()?;

    println!("✅ Reflection generated for {outcome_id}");
    Ok(())
}

fn cmd_list(_args: &[String]) -> anyhow::Result<()> {
    let (_, vault_path) = load_config()?;
    let memory = load_memory(&vault_path)?;
    let outcomes = memory.all_outcomes();

    if outcomes.is_empty() {
        println!("No outcomes recorded yet.");
        return Ok(());
    }

    println!("{:<20} {:<10} {:<14} {:<8} {}", "ID", "Decision", "Verdict", "Impact", "Evidence");
    println!("{}", "-".repeat(80));
    for o in outcomes.iter().rev().take(20) {
        let v = format!("{:?}", o.verdict);
        let ev = if o.description.len() > 40 {
            format!("{}...", &o.description[..40])
        } else {
            o.description.clone()
        };
        println!("{:<20} {:<10} {:<14} {:<8} {}",
            o.id, o.decision_id, v, o.impact.as_str(), ev);
    }
    Ok(())
}

fn cmd_status(_args: &[String]) -> anyhow::Result<()> {
    let (_, vault_path) = load_config()?;
    let memory = load_memory(&vault_path)?;
    let outcomes = memory.all_outcomes();

    let total = outcomes.len();
    let confirmed = outcomes.iter().filter(|o| o.verdict == sulix_intel::domain::outcome::OutcomeVerdict::Confirmed).count();
    let partial = outcomes.iter().filter(|o| o.verdict == sulix_intel::domain::outcome::OutcomeVerdict::PartiallyConfirmed).count();
    let invalidated = outcomes.iter().filter(|o| o.verdict == sulix_intel::domain::outcome::OutcomeVerdict::Invalidated).count();
    let unknown = outcomes.iter().filter(|o| o.verdict == sulix_intel::domain::outcome::OutcomeVerdict::Unknown).count();
    let _reflected_count = memory.all_outcomes().iter()
        .filter(|_o| {
            // Placeholder — Day 2: link reflections to outcome_ids
            false
        })
        .count();

    let accuracy = if total > 0 {
        (confirmed as f64 + partial as f64 * 0.5) / total as f64
    } else {
        0.0
    };

    println!("📊 Outcome Summary");
    println!("   Total: {total}");
    println!("   Confirmed: {confirmed}");
    println!("   Partial: {partial}");
    println!("   Invalidated: {invalidated}");
    println!("   Unknown: {unknown}");
    println!("   Accuracy: {:.1}%", accuracy * 100.0);
    Ok(())
}
