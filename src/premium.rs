//! Premium Research — 多 Agent 虚拟智库深度研报
//!
//! 商业模型: 免费看板引流 + 多 Agent 深度研报付费墙
//!
//! Agent 管线（3 层对抗博弈）:
//!   1. The Diplomat (地缘政策评论员) — 政治定性
//!   2. The Architect (半导体产业专家) — 技术影响
//!   3. The Quant (咨询合伙人) — 商业框架 + 决策建议
//!
//! 每个 Agent 的输出作为下一个 Agent 的输入上下文，形成链式推演。

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::clusterer::Theme;
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

// ===== Agent 1: 地缘政策评论员 (The Diplomat) =====

const DIPLOMAT_PROMPT: &str = r#"You are a former senior correspondent for The Economist specializing in international technology policy.
Your style: cold, restrained, surgically precise. You strip away emotional language and cut directly to the political博弈.

Analyze the following theme from multiple sovereign data sources.
Output a structured assessment covering:

1. political_signal: What is the core political intent behind this signal? (2-3 sentences)
2. enforcement_credibility: On a scale of 1-10, how likely is this to be enforced? What determines this?
3. regulatory_velocity: Is this accelerating, stable, or declining? What keywords indicate the tempo?
4. historical_context: What precedent does this follow or break from?
5. key_actors: Who are the primary entities driving and opposing this?

Output JSON only, no prose outside the JSON:
{
  "political_signal": "...",
  "enforcement_credibility": 7,
  "regulatory_velocity": "accelerating",
  "historical_context": "...",
  "key_actors": ["Entity A", "Entity B"]
}"#;

// ===== Agent 2: 半导体产业专家 (The Architect) =====

const ARCHITECT_PROMPT: &str = r#"You are a former chief architect at a top semiconductor research institute (TechInsights/SEMI).
Your expertise: wafer fabrication, advanced packaging, lithography, supply chain bill-of-materials.

Given the geopolitical assessment below, analyze the PHYSICAL technology impact:

1. node_impact: Which process nodes (3nm/5nm/7nm/etc.) are directly affected?
2. equipment_supply: What specific equipment/tools face supply constraints?
3. materials_bottleneck: Are there photoresist/chemical/wafer-level material choke points?
4. timeline: How quickly does this translate to physical supply disruption? (months)
5. alternative_paths: What technical workarounds exist (chiplet/heterogeneous/custom)?

Output JSON only:
{
  "node_impact": {"nodes": ["3nm", "5nm"], "severity": "high"},
  "equipment_supply": "ASML NXE:3600D immersion scanners affected",
  "materials_bottleneck": "High-NA photoresist supply from Japan at risk",
  "timeline_months": 12,
  "alternative_paths": ["Chiplet architectures", "Heterogeneous integration"]
}"#;

// ===== Agent 3: 咨询合伙人 (The Quant) =====

const QUANT_PROMPT: &str = r#"You are a McKinsey partner specializing in geopolitical risk consulting for Fortune 500 technology clients.
Your expertise: quantifying regulatory impact on capital expenditure, supply chain compliance costs, and market access.

Given the geopolitical assessment and technical impact provided, synthesize a commercial framework:

1. compliance_cost_impact: Quantify the estimated compliance cost increase (%) for affected supply chains.
2. capex_shifts: What capital expenditure reallocation does this trigger? (e.g., "fab diversification to Arizona")
3. market_access: Which markets become restricted or opened?
4. strategic_recommendations: 3 concrete actions for a technology executive (not general advice).
5. risk_scenarios: Base / Adverse / Aggressive scenarios with probability weights.

Output JSON only:
{
  "compliance_cost_impact": "15-25% increase in verified compliant supply chain overhead",
  "capex_shifts": "US-based advanced packaging capacity accelerated 2 years",
  "market_access": "China market restricted for sub-7nm; ASEAN markets open as alternative",
  "strategic_recommendations": ["Action 1", "Action 2", "Action 3"],
  "risk_scenarios": {
    "base": {"probability": 0.6, "outcome": "Gradual decoupling over 24 months"},
    "adverse": {"probability": 0.25, "outcome": "Sudden export license revocations within 6 months"},
    "aggressive": {"probability": 0.15, "outcome": "Full technology embargo triggering industry-wide supply crisis"}
  }
}"#;

/// 运行 3 Agent 管线，生成 Premium 研报
pub async fn generate_premium_report(
    theme: &Theme,
    theme_context: &str,
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<PremiumReport> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    // ---- Agent 1: The Diplomat ----
    log::info!("👤 Agent 1 — Diplomat: 分析地缘政治信号...");
    let diplomat_input = format!(
        "Theme: {}\n\nSources:\n{}\n\nAnalyze the geopolitical dynamics of this theme.",
        theme.title, theme_context
    );
    let diplomat_raw = llm::call_with_retry_raw(
        &client, api_key, llm_config, DIPLOMAT_PROMPT, &diplomat_input,
    ).await?;
    let diplomat_json: serde_json::Value = llm::parse_json_lenient(&diplomat_raw)?;
    let political_signal = diplomat_json["political_signal"]
        .as_str()
        .unwrap_or("Analysis unavailable")
        .to_string();

    // ---- Agent 2: The Architect ----
    log::info!("👤 Agent 2 — Architect: 分析产业技术影响...");
    let architect_input = format!(
        "Theme: {}\n\nGeopolitical Assessment:\n{}\n\nAnalyze the physical technology/supply chain impact.",
        theme.title, political_signal
    );
    let architect_raw = llm::call_with_retry_raw(
        &client, api_key, llm_config, ARCHITECT_PROMPT, &architect_input,
    ).await?;
    let architect_json: serde_json::Value = llm::parse_json_lenient(&architect_raw)?;
    let technical_impact = architect_json["equipment_supply"]
        .as_str()
        .unwrap_or("Impact analysis unavailable")
        .to_string();

    // ---- Agent 3: The Quant ----
    log::info!("👤 Agent 3 — Quant: 合成商业框架...");
    let quant_input = format!(
        "Theme: {}\n\nGeopolitical Assessment:\n{}\n\nTechnical Impact:\n{}\n\nSynthesize commercial framework.",
        theme.title, political_signal, technical_impact
    );
    let quant_raw = llm::call_with_retry_raw(
        &client, api_key, llm_config, QUANT_PROMPT, &quant_input,
    ).await?;
    let quant_json: serde_json::Value = llm::parse_json_lenient(&quant_raw)?;

    let commercial_framework = quant_json["compliance_cost_impact"]
        .as_str()
        .unwrap_or("Commercial analysis unavailable")
        .to_string();
    let strategic_recs = quant_json["strategic_recommendations"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default();

    let risk_scenarios = vec![
        format!(
            "Base ({:.0}%): {}",
            quant_json["risk_scenarios"]["base"]["probability"]
                .as_f64()
                .unwrap_or(0.5) * 100.0,
            quant_json["risk_scenarios"]["base"]["outcome"]
                .as_str()
                .unwrap_or("Status quo")
        ),
        format!(
            "Adverse ({:.0}%): {}",
            quant_json["risk_scenarios"]["adverse"]["probability"]
                .as_f64()
                .unwrap_or(0.25) * 100.0,
            quant_json["risk_scenarios"]["adverse"]["outcome"]
                .as_str()
                .unwrap_or("Deterioration")
        ),
        format!(
            "Aggressive ({:.0}%): {}",
            quant_json["risk_scenarios"]["aggressive"]["probability"]
                .as_f64()
                .unwrap_or(0.15) * 100.0,
            quant_json["risk_scenarios"]["aggressive"]["outcome"]
                .as_str()
                .unwrap_or("Crisis")
        ),
    ];

    let executive_summary = format!(
        "{}\n\nStrategic recommendations: {}",
        political_signal, strategic_recs
    );

    let sources: Vec<String> = theme.articles.iter().map(|a| a.source.clone()).collect();

    log::info!("✅ Premium 研报生成: {} — 3 Agent 管线完成", theme.title);
    Ok(PremiumReport {
        theme_title: theme.title.clone(),
        date,
        executive_summary,
        geopolitical_assessment: political_signal,
        technical_impact,
        commercial_framework,
        risk_scenarios,
        sources,
    })
}
