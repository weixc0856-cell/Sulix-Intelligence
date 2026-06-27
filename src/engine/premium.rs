//! Premium Research — 阶段式深度研报管线
//!
//! 核心转变：
//!   旧（角色驱动）：Diplomat → Architect → Quant（角色随业务膨胀，不可扩展）
//!   新（阶段驱动）：WhatChanged → WhyItMatters → WhatToDo（流程稳定，可配置阶段数）
//!
//! 理由：
//!   角色会越来越多（10+ 角色），流程不会。
//!   阶段驱动使管线可扩展——可任意插入 "AdversarialReview" 等新阶段，
//!   而无需每次新增一个 Agent 角色。

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::domain::theme::Theme;
use crate::config::LlmConfig;
use crate::llm;

/// 最终 Premium 研报
#[derive(Debug, Clone, Serialize)]
pub struct PremiumReport {
    pub theme_title: String,
    pub date: String,
    pub executive_summary: String,
    pub geopolitical_assessment: String,
    pub technical_impact: String,
    pub commercial_framework: String,
    pub risk_scenarios: Vec<String>,
    pub sources: Vec<String>,
}

/// 专题聚合/紧急加更配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialTopic {
    pub topic_id: String,
    pub title: String,
    pub is_flash: bool,
    pub perspective: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub locked_sources: Option<Vec<String>>,
}

// ===== Stage 1: WhatChanged =====

const STAGE1_WHAT_CHANGED: &str = r#"You are a senior strategic analyst. Your job is to answer one question:

"What happened and why does it matter?"

Focus on:
1. What specific event or signal triggered this analysis?
2. What is the factual, verifiable change — stripped of interpretation?
3. Why does this change matter right now — what's the time horizon?

Output JSON only:
{
  "bluf": "One-sentence bottom line: the specific change and why it matters.",
  "factual_change": "What actually happened — verifiable facts only, no interpretation (2-3 sentences).",
  "time_horizon": "immediate / short-term / medium-term / long-term",
  "signal_quality": "Established-Fact / First-Principles / Developing-Inference / Assertion-Rumor",
  "key_sources": ["source 1", "source 2"]
}"#;

// ===== Stage 2: WhyItMatters =====

const STAGE2_WHY_IT_MATTERS: &str = r#"You are a strategic analyst. Given the factual change identified in Stage 1, answer:

"Why does this matter — what are the implications and second-order effects?"

Focus on:
1. Direct implications: What changes as a direct result?
2. Second-order effects: What changes as a consequence of the direct changes?
3. Who is affected and how?
4. What assumptions would have to hold for this analysis to be wrong?

Output JSON only:
{
  "geopolitical_impact": "Geopolitical and regulatory implications (2-3 sentences).",
  "industry_impact": "Industry and competitive implications (2-3 sentences).",
  "second_order": "Second-order effects — what happens next as a result of the direct changes (2-3 sentences).",
  "affected_stakeholders": ["stakeholder 1", "stakeholder 2"],
  "key_assumptions": [
    {"assumption": "critical assumption", "confidence": "high/medium/low"}
  ],
  "adverse_scenario": "What if the key assumptions are wrong? (1-2 sentences)"
}"#;

// ===== Stage 3: WhatToDo =====

const STAGE3_WHAT_TO_DO: &str = r#"You are a strategic advisor. Given the analysis from previous stages, answer:

"What actions should be considered?"

Focus on:
1. What should someone DO based on this analysis — not just "monitor" or "watch"
2. What's the recommendation confidence and why?
3. What scenarios should be planned for?
4. What specific actions, with time horizons?

Output JSON only:
{
  "executive_summary": "Situation -> Complication -> Resolution (3 sentences max).",
  "recommendations": [
    {"action": "Specific action 1", "time_horizon": "immediate/30d/90d", "impact": "high/medium/low"}
  ],
  "risk_scenarios": {
    "base": {"description": "Most likely scenario", "probability": 0.6, "mitigation": "What to do"},
    "adverse": {"description": "Downside scenario", "probability": 0.25, "mitigation": "What to do"},
    "aggressive": {"description": "Upside/downside extreme", "probability": 0.15, "mitigation": "What to do"}
  },
  "commercial_framework": "Commercial and strategic implications (2-3 sentences)."
}"#;

/// 运行阶段式研报管线，生成 Premium 研报
///
/// 默认 3 阶段：WhatChanged → WhyItMatters → WhatToDo
/// 可通过 PremiumConfig 自定义阶段数量和 prompt。
pub async fn generate_premium_report(
    theme: &Theme,
    theme_context: &str,
    api_key: &str,
    llm_config: &LlmConfig,
    prompts: Option<&crate::config::PromptsConfig>,
) -> Result<PremiumReport> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    // ---- Stage 1: WhatChanged ----
    log::info!("📋 Stage 1 — WhatChanged: 识别核心变化...");
    let stage1_input = format!(
        "## Theme: {}\n\nContext:\n{}\n\nWhat happened and why does it matter?",
        theme.title, theme_context
    );
    let stage1_prompt = prompts
        .map(|p| p.get_diplomat(STAGE1_WHAT_CHANGED))
        .unwrap_or(STAGE1_WHAT_CHANGED);
    let stage1_raw =
        llm::call_with_retry_raw(&client, api_key, llm_config, stage1_prompt, &stage1_input)
            .await?;
    let stage1_json: serde_json::Value = llm::parse_json_lenient(&stage1_raw)?;
    let stage1_output = stage1_json["bluf"]
        .as_str()
        .unwrap_or("Analysis unavailable")
        .to_string();

    // ---- Stage 2: WhyItMatters ----
    log::info!("📋 Stage 2 — WhyItMatters: 推演影响与二阶效应...");
    let stage2_input = format!(
        "## Theme: {}\n\nWhat Changed:\n{}\n\nWhat are the implications and second-order effects?",
        theme.title, stage1_output
    );
    let stage2_prompt = prompts
        .map(|p| p.get_architect(STAGE2_WHY_IT_MATTERS))
        .unwrap_or(STAGE2_WHY_IT_MATTERS);
    let stage2_raw =
        llm::call_with_retry_raw(&client, api_key, llm_config, stage2_prompt, &stage2_input)
            .await?;
    let stage2_json: serde_json::Value = llm::parse_json_lenient(&stage2_raw)?;
    let geopolitical = stage2_json["geopolitical_impact"]
        .as_str()
        .unwrap_or("Analysis unavailable")
        .to_string();
    let industry = stage2_json["industry_impact"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let stage2_output = format!("{}\n\n{}", geopolitical, industry);

    // ---- Stage 3: WhatToDo ----
    log::info!("📋 Stage 3 — WhatToDo: 合成行动建议...");
    let stage3_input = format!(
        "## Theme: {}\n\nWhat Changed:\n{}\n\nImplications:\n{}\n\nWhat actions should be considered?",
        theme.title, stage1_output, stage2_output
    );
    let stage3_prompt = prompts
        .map(|p| p.get_quant(STAGE3_WHAT_TO_DO))
        .unwrap_or(STAGE3_WHAT_TO_DO);
    let stage3_raw =
        llm::call_with_retry_raw(&client, api_key, llm_config, stage3_prompt, &stage3_input)
            .await?;
    let stage3_json: serde_json::Value = llm::parse_json_lenient(&stage3_raw)?;

    let executive_summary = stage3_json["executive_summary"]
        .as_str()
        .unwrap_or(&stage1_output)
        .to_string();
    let commercial = stage3_json["commercial_framework"]
        .as_str()
        .unwrap_or("")
        .to_string();

    // 解析风险情景
    let risk_scenarios = {
        let scenarios = &stage3_json["risk_scenarios"];
        vec![
            format!(
                "Base ({:.0}%): {}",
                scenarios["base"]["probability"].as_f64().unwrap_or(0.5) * 100.0,
                scenarios["base"]["description"]
                    .as_str()
                    .unwrap_or("Status quo")
            ),
            format!(
                "Adverse ({:.0}%): {}",
                scenarios["adverse"]["probability"].as_f64().unwrap_or(0.25) * 100.0,
                scenarios["adverse"]["description"]
                    .as_str()
                    .unwrap_or("Deterioration")
            ),
            format!(
                "Aggressive ({:.0}%): {}",
                scenarios["aggressive"]["probability"]
                    .as_f64()
                    .unwrap_or(0.15)
                    * 100.0,
                scenarios["aggressive"]["description"]
                    .as_str()
                    .unwrap_or("Crisis")
            ),
        ]
    };

    let sources: Vec<String> = theme.articles.iter().map(|a| a.source.clone()).collect();

    log::info!("✅ Premium 研报生成: {} — 3 阶段管线完成", theme.title);
    Ok(PremiumReport {
        theme_title: theme.title.clone(),
        date,
        executive_summary,
        geopolitical_assessment: geopolitical,
        technical_impact: industry,
        commercial_framework: commercial,
        risk_scenarios,
        sources,
    })
}

/// 推送 Premium 报告到 Substack Draft API
///
/// 失败时不阻塞管线（eprintln 而非 ?），保持 fire-and-forget
pub async fn push_to_substack(
    report: &PremiumReport,
    api_key: &str,
    publication_url: &str,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let md = crate::renderer::render_substack_markdown(report);
    let payload = serde_json::json!({
        "title": format!("【Premium】{}", report.theme_title),
        "content": md,
        "type": "premium",
        "newsletter": true,
    });

    let resp = client
        .post(format!("{}/api/v1/drafts", publication_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Substack API error: {}", resp.status());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_premium_report_fields() {
        let report = PremiumReport {
            theme_title: "Test".into(),
            date: "2026-06-23".into(),
            executive_summary: "Summary".into(),
            geopolitical_assessment: "Geo".into(),
            technical_impact: "Tech".into(),
            commercial_framework: "Commercial".into(),
            risk_scenarios: vec!["Base (60%): Test".into()],
            sources: vec!["Source A".into()],
        };
        assert_eq!(report.theme_title, "Test");
        assert_eq!(report.risk_scenarios.len(), 1);
        assert_eq!(report.sources.len(), 1);
    }

    #[test]
    fn test_substack_markdown_renders_fields() {
        let report = PremiumReport {
            theme_title: "SVI Test".into(),
            date: "2026-06-23".into(),
            executive_summary: "Exec summary".into(),
            geopolitical_assessment: "Stage2 analysis".into(),
            technical_impact: "Industry impact".into(),
            commercial_framework: "Stage3 framework".into(),
            risk_scenarios: vec!["Base (60%): Normal".into()],
            sources: vec!["Federal Register".into()],
        };
        let md = crate::renderer::render_substack_markdown(&report);
        assert!(md.contains("【Premium】SVI Test"));
        assert!(md.contains("Stage2 analysis"));
        assert!(md.contains("Stage3 framework"));
        assert!(md.contains("Federal Register"));
    }
}
