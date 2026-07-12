//! Intelligence Output — 新管线的 MDX 知识资产产出
//!
//! 当新管线跑完 Observation→Signal→Thesis→Decision 后，
//! 此模块将 PipelineOutput 渲染为 MDX 文件，直接写入 Astro Content Collections。
//!
//! 与旧 publishing::emit 的差异：
//!   - 输入是 contract::* 类型（不是 domain::* 类型）
//!   - 不依赖 old publishing/ 模块
//!   - 不产生 Premium 研报（Phase 3 再补充）
//!
//! MDX 格式：
//!   output/thesis/{slug}.md
//!   output/decision/{slug}.md

use std::path::PathBuf;

use anyhow::Result;

use crate::IntelligenceOutput;
use sulix_contract as contract;

/// 新管线 MDX 输出配置
pub struct IntelligenceOutputConfig {
    /// MDX 输出根目录（对应 old config.output.mdx_dir）
    pub mdx_dir: PathBuf,
    /// 输出语言（"en" / "zh-cn" / "zh-tw"）
    pub locale: String,
}

/// 从新管线输出生成 MDX 文件
///
/// 产出文件:
///   {mdx_dir}/thesis/{slug}.md       — 每个 Thesis 一个文件
///   {mdx_dir}/decision/{slug}.md     — 每个 Decision 一个文件
pub fn render_to_mdx(
    output: &IntelligenceOutput,
    config: &IntelligenceOutputConfig,
    today: &str,
) -> Result<usize> {
    let mut file_count = 0;

    // 输出 Thesis MDX
    for thesis in &output.theses {
        let slug = slugify(&thesis.claim);
        let dir = config.mdx_dir.join("thesis");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.md", slug));

        let mdx = render_thesis_mdx(thesis, today, &config.locale);
        std::fs::write(&path, mdx)?;
        file_count += 1;
    }

    // 输出 Decision MDX
    for decision in &output.decisions {
        let dir = config.mdx_dir.join("decision");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.md", decision.id));

        let mdx = render_decision_mdx(decision, &output.theses, today, &config.locale);
        std::fs::write(&path, mdx)?;
        file_count += 1;
    }

    log::info!("📝 Intelligence Output: {} 个 MDX 文件已写入", file_count);
    Ok(file_count)
}

/// 渲染单个 Thesis 的 MDX 文件
///
/// 输出格式对齐 Astro Content Collections schema (baseAssessmentFields + thesisSpecificFields):
///   title, date, locale, status, confidence [0,1], evidences, challenges, primary_domain
fn render_thesis_mdx(thesis: &contract::Thesis, today: &str, locale: &str) -> String {
    let _slug = slugify(&thesis.claim);
    let status_frontend = thesis.status.to_frontend_string();
    let falsifications_yaml: String = thesis
        .falsification_conditions
        .iter()
        .map(|c| format!("  - \"{}\"", yaml_escape(c)))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"---
title: "{}"
date: "{}"
locale: "{}"
status: "{}"
primary_domain: "{}"
confidence: {}
evidences: {}
challenges: 0
summary: "{}"
falsification_conditions:
{}
---
# {}

**置信度**: {:.0}% | **状态**: {} | **证据**: {} 条

## 判断陈述

{}

## 证伪条件

{}
"#,
        yaml_escape(&thesis.claim),
        today,
        locale,
        yaml_escape(status_frontend),
        yaml_escape(thesis.theme.as_deref().unwrap_or("Other")),
        thesis.confidence,
        thesis.evidence.len(),
        yaml_escape(thesis.summary.as_deref().unwrap_or(&thesis.claim)),
        falsifications_yaml,
        thesis.claim,
        thesis.confidence * 100.0,
        status_frontend,
        thesis.evidence.len(),
        thesis.claim,
        if thesis.falsification_conditions.is_empty() {
            "无".to_string()
        } else {
            thesis
                .falsification_conditions
                .iter()
                .enumerate()
                .map(|(i, c)| format!("{}. {}", i + 1, c))
                .collect::<Vec<_>>()
                .join("\n")
        },
    )
}

/// 渲染单个 Decision 的 MDX 文件
/// 渲染单个 Decision 的 MDX 文件
///
/// 输出格式对齐 Astro Content Collections schema:
///   title, dec_id, asm_id, decision, horizon, confidence,
///   stability (volatile/stable/final), state, created, updated
fn render_decision_mdx(
    decision: &contract::Decision,
    theses: &[contract::Thesis],
    today: &str,
    locale: &str,
) -> String {
    let thesis = theses.iter().find(|t| t.id == decision.thesis_id);
    let thesis_title = thesis.map(|t| t.claim.as_str()).unwrap_or("unknown");
    let action_lower = format!("{:?}", decision.action).to_lowercase();
    let horizon_str = format!("{:?}", decision.horizon);
    let domain = thesis.and_then(|t| t.theme.as_deref()).unwrap_or("Other");

    // stability: Exit→final, >=3 evidence→stable, else volatile
    let stability = if matches!(decision.action, contract::DecisionType::Exit) {
        "final"
    } else if thesis.is_some_and(|t| t.evidence.len() >= 3) {
        "stable"
    } else {
        "volatile"
    };

    format!(
        r#"---
title: "{}"
locale: "{}"
dec_id: "{}"
asm_id: "{}"
decision: "{}"
primary_domain: "{}"
horizon: "{}"
confidence: {:.2}
stability: "{}"
state: "{}"
created: "{}"
updated: "{}"
rationale: "{}"
---
# 决策: {}

**决策**: {} | **置信度**: {:.0}% | **时间范围**: {} | **稳定性**: {}
"#,
        yaml_escape(thesis_title),
        locale,
        decision.id,
        decision.thesis_id,
        yaml_escape(&action_lower),
        yaml_escape(domain),
        yaml_escape(&horizon_str.to_lowercase()),
        decision.confidence,
        stability,
        if decision.rule_passed { "active" } else { "pending" },
        today,
        today,
        yaml_escape(&decision.reasoning),
        action_lower.to_uppercase(),
        action_lower.to_uppercase(),
        decision.confidence * 100.0,
        horizon_str,
        stability,
    )
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>()
        .replace(' ', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string()
}

fn yaml_escape(s: &str) -> String {
    if s.contains('"') || s.contains('\\') || s.contains('\n') {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else if s.contains(':') || s.contains('#') || s.starts_with('-') {
        format!("\"{}\"", s)
    } else {
        s.to_string()
    }
}

/// 将管线输出导出为 JSON 格式（供 Worker API 消费）
///
/// 产出文件:
///   {data_dir}/export.json — 包含 theses、decisions、signals 的完整 JSON
///
/// 架构定位（ADR-012）:
///   JSON 是中间传输格式，不是永久存储。
///   最终状态存储在 Repository（SQLite/D1）中。
pub fn export_to_json(
    output: &IntelligenceOutput,
    data_dir: &std::path::Path,
) -> Result<std::path::PathBuf> {
    use std::io::Write;

    let path = data_dir.join("export.json");
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    // 构建可序列化的 JSON 结构
    let json = serde_json::json!({
        "contract_version": "2",
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "date": today,
        "theses": output.theses,
        "decisions": output.decisions,
        "signals": output.signals,
        "stats": {
            "decision_count": output.decisions.len(),
            "thesis_count": output.theses.len(),
            "signal_count": output.signals.len(),
            "elapsed_ms": output.stats.elapsed_ms(),
        }
    });

    let mut file = std::fs::File::create(&path)?;
    file.write_all(serde_json::to_string_pretty(&json)?.as_bytes())?;
    log::info!("📦 JSON export: {} ({} bytes)", path.display(), json.to_string().len());
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_thesis() -> contract::Thesis {
        contract::Thesis {
            id: "thesis_001".into(),
            claim: "AI Agent adoption will accelerate in enterprise".into(),
            confidence: 0.72,
            evidence: vec!["sig_001".into(), "sig_002".into()],
            status: contract::ThesisStatus::Active,
            falsification_conditions: vec!["Enterprise adoption flat for 12 months".into()],
            time_horizon: "12_months".into(),
            theme: Some("AI Enterprise".into()),
            belief_statement: None,
            summary: None,
        }
    }

    fn sample_decision() -> contract::Decision {
        contract::Decision {
            id: "dec_001".into(),
            thesis_id: "thesis_001".into(),
            action: contract::DecisionType::Invest,
            confidence: 0.72,
            horizon: contract::DecisionHorizon::Days90,
            reasoning: "Strong adoption signals from multiple enterprise pilots".into(),
            made_at: "2026-07-12".into(),
            rule_passed: true,
            requires_review: false,
            review_reason: None,
        }
    }

    #[test]
    fn test_render_thesis_mdx_basic() {
        let thesis = sample_thesis();
        let mdx = render_thesis_mdx(&thesis, "2026-07-12", "en");
        assert!(mdx.contains("title:"));
        assert!(mdx.contains("confidence: 0.72"));
        assert!(mdx.contains("evidences: 2"));
        assert!(mdx.contains("falsification_conditions:"));
        assert!(mdx.contains("Enterprise adoption flat"));
    }

    #[test]
    fn test_render_decision_mdx_basic() {
        let thesis = sample_thesis();
        let decision = sample_decision();
        let mdx = render_decision_mdx(&decision, &[thesis], "2026-07-12", "en");
        assert!(mdx.contains("decision: \"invest\""));
        assert!(mdx.contains("stability: \"volatile\""));
        assert!(mdx.contains("INVEST"));
    }

    #[test]
    fn test_render_to_mdx_creates_files() {
        let dir = std::env::temp_dir().join(format!("test_mdx_output_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let output = IntelligenceOutput {
            signals: vec![],
            theses: vec![sample_thesis()],
            decisions: vec![sample_decision()],
            stats: Default::default(),
        };

        let config = IntelligenceOutputConfig {
            mdx_dir: dir.clone(),
            locale: "en".into(),
        };

        let count = render_to_mdx(&output, &config, "2026-07-12").unwrap();
        assert_eq!(count, 2);

        // thesis file exists
        assert!(dir
            .join("thesis")
            .join("ai-agent-adoption-will-accelerate-in-enterprise.md")
            .exists());
        // decision file exists
        assert!(dir.join("decision").join("dec_001.md").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("AI Agent Growth"), "ai-agent-growth");
        assert_eq!(slugify("  hello--world-- "), "hello-world");
    }

    #[test]
    fn test_yaml_escape_plain() {
        assert_eq!(yaml_escape("hello"), "hello");
    }

    #[test]
    fn test_yaml_escape_with_quotes() {
        let escaped = yaml_escape(r#"say "hello""#);
        assert!(escaped.contains("\\\""));
    }
}
