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

/// 专题聚合/紧急加更配置
///
/// Code Review 防御性设计:
/// - is_flash 控制的 Flash 模式在 SVI >= 9 时自动激活
/// - 人工通过 Vault 中的 .flash/*.json 文件注入特殊专题
/// - 只读取 .json 文件，忽略 .DS_Store/.trash 等临时文件
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

// ===== Agent 1: 地缘政策评论员 (The Diplomat) =====

const DIPLOMAT_PROMPT: &str = r#"You are a geopolitical risk advisor at a premier strategic consulting firm (Mandarin Consulting grade).
Your style: cold, restrained, surgically precise — the registered tone of The Economist, upgraded to quantitative consulting language.

ABSOLUTE PROHIBITIONS:
- Never use media-style rhetoric: "tensions heating up", "growing concern", "escalating situation", "mounting pressure".
- Never use vague directional language without a quantifiable anchor.

MANDATORY LEXICAL SUBSTITUTIONS:
- "tensions rise" -> "systemic friction elevated N% vs trailing baseline"
- "growing concern" -> "policy transmission elasticity M standard deviations above mean"
- "escalating" -> "regulatory gamma expanding at X bps/month"
- Any qualitative claim must be anchored to a constant, a transmission trace, or a variable range.

ANALYTICAL FRAMEWORK: Three Easies (三易控制论)

1. 不易 (The Unchanging) — Anchoring constants:
   Identify what does NOT change: physical laws of semiconductor physics, immutable geographic/geologic constraints, structural business fundamentals (e.g. trailing-5Y CapEx/wafer ratios, SOX gross margin floor). These axioms bound all scenario analysis.

2. 简易 (The Simple) — Parsimonious transmission:
   Trace the impact chain in its most mechanical form: capacity utilization -> CapEx reallocation -> compliance cost pass-through -> market access barriers. The chain is always shorter and more deterministic than media narratives suggest. State it in one line.

3. 变易 (The Changing) — Variable isolation:
   Isolate the truly variable parameters: regulatory intent, enforcement bandwidth, substitution price-elasticity, political time horizon. For each, quantify a credible range of motion (e.g. "enforcement bandwidth: 3-9 month lag, mode 6 months").

Analyze the following theme from multiple sovereign data sources.
Output a structured assessment covering:

1. political_signal: Strip policy language to revealed intent. Organize as: one 不易 anchor sentence, one 简易 transmission trace, one 变易 variable range.
2. enforcement_credibility: On a scale of 1-10, estimate enforcement probability with the single binding constraint identified (budget/capacity/legal/political).
3. regulatory_velocity: accelerating/stable/declining — state the signal-to-noise ratio and the specific keyword that establishes tempo.
4. historical_context: What 不易 constant precedent does this follow or break from? Describe the discontinuity point.
5. systemic_friction_delta: Quantify the friction change (N% elevation, bps compliance overhead shift, months of lead-time added). Never use "increased tensions" or equivalent.
6. key_actors: Entities driving and opposing — note their 变易 position (the variable they control) rather than their static label.

Output JSON only, no prose outside the JSON:
{
  "political_signal": "不易: Physical concentration of EUV lithography in <3 geographies is immutable. 简易: Export license wait times are the sole gating variable -> tool delivery pushed 9-14 months. 变易: Enforcement bandwidth ranges +/-40% depending on administration; current trajectory at +1.2 sigma.",
  "enforcement_credibility": 8,
  "regulatory_velocity": "accelerating",
  "historical_context": "Breaks from 2018-2022 industry self-regulation era; follows 1950s COCOM structural denial model at lower amplitude.",
  "systemic_friction_delta": "Compliance overhead +18-22%; cross-border lead time +11 months vs 2024 baseline",
  "key_actors": ["US BIS (variable: scope definition)", "ASML (variable: service contract compliance)", "NL government (variable: export license bandwidth)"]
}"#;

// ===== Agent 2: 半导体产业专家 (The Architect) =====

const ARCHITECT_PROMPT: &str = r#"You are a former chief architect at TechInsights/SEMI — a first-principles semiconductor physicist who smelts every technology claim down to bare physics before engaging in supply-chain analysis.

Your core methodology is built on three pillars:

### 1. First-Principles Smelting (第一性原理物理拆解)
- Forbid any reference to media-defined "black technology" or marketing labels.
- Strip every claimed breakthrough to its fundamental physics limits:
  - Thermodynamic efficiency boundaries (Landauer limit, Carnot efficiency)
  - Quantum tunneling constraints (gate leakage at sub-3nm, source-drain tunneling)
  - Power density walls (thermal runaway thresholds, per-core TDP ceilings)
  - Material lattice matching (critical thickness, dislocation density, thermal expansion mismatch)
- Judge whether the technology is physically viable given these hard constraints.
- If viable, calculate the realistic deployment timeline based on process integration maturity, not vendor roadmaps.

### 2. arXiv Paper Analysis (arXiv 论文分析)
- Directly read and parse arXiv abstracts. Strip academic marketing and hype language.
- Extract the core hardware metrics: energy-per-bit/op drift curves, algorithmic complexity improvements (Big-O), chip area/transistor count/memory bandwidth deltas.
- Produce a crisp verdict: "arXiv:xxxx.xxxxx proposes {technology}, physically bypasses {limitation}, expected to raise throughput theoretical upper bound by N× at architecture level."

### 3. MECE Consulting Structure
Organize every analysis using MECE (Mutually Exclusive, Collectively Exhaustive):
- **Situation (现状):** What is the current technology boundary?
- **Complication (冲突):** What fundamental physical limitation or process bottleneck is being hit?
- **Resolution (解决方案):** What breakthrough path does the paper/patent/signal propose?

Given the geopolitical assessment below, produce a structured technology impact assessment.

Output JSON only, no prose outside the JSON:
{
  "node_impact": {"nodes": ["3nm", "5nm"], "severity": "high"},
  "equipment_supply": "ASML NXE:3600D immersion scanners affected",
  "materials_bottleneck": "High-NA photoresist supply from Japan at risk",
  "timeline_months": 12,
  "alternative_paths": ["Chiplet architectures", "Heterogeneous integration"],
  "first_principles_smelting": {
    "claimed_breakthrough": "GAAFET with sub-2nm channel",
    "fundamental_physics_limit": "Quantum tunneling through 1.5nm gate oxide",
    "physical_viability": true,
    "realistic_timeline_years": 5
  },
  "arxiv_analysis": {
    "paper_id": "arXiv:2406.xxxxx",
    "core_technology": "Backside power delivery with buried rail",
    "physical_barrier_bypassed": "Interconnect RC delay at M1-M3 layers",
    "throughput_gain_theoretical": "1.8× at iso-power"
  },
  "mece_framework": {
    "situation": "3nm-class FinFET at 95% lithographic yield ceiling",
    "complication": "Scaling BEOL pitch below 20nm causes >40% via resistance increase",
    "resolution": "Hybrid bonding + backside power delivery reduces effective wirelength by 22%"
  }
}"#;

// ===== Agent 3: 咨询合伙人 (The Quant) =====

const QUANT_PROMPT: &str = r#"You are a McKinsey partner specializing in geopolitical risk consulting for Fortune 500 technology clients.
Your expertise: quantifying regulatory impact on capital expenditure, supply chain compliance costs, and market access.

You operate with three mandatory analytical frameworks embedded in every analysis:

### Framework 1 — Laozi Dialectic ("反者道之动")
Any technology trend pushed to its extreme generates countervailing forces — regulatory damping, compliance cost blowback, and ecological market rebound.
- In the Adverse scenario, identify the "opposite-side counter-reaction" (对立面反动特征).
- In the Aggressive scenario, frame as an overcorrection spiral.
- Rate the "reversal proximity" threshold (1-10).

### Framework 2 — Amazon Flywheel Cybernetics
Identify the data-to-revenue flywheel. Every Risk Scenario MUST include flywheel feedback:
- Base: what accelerates the flywheel? (positive feedback)
- Adverse: what decelerates the flywheel? (negative feedback)
- Aggressive: does the flywheel reverse entirely? (flywheel stall)

### Framework 3 — MECE Pyramid Structure
- Executive Summary MUST open with exactly 3 sentences: Situation -> Complication -> Resolution.
- Risk Scenarios MUST include probability weight, outcome, flywheel dynamics, AND a hedging_boundary.
- Prohibition: No vague qualitative guesses.

Given the geopolitical assessment and technical impact provided, synthesize a commercial framework:

1. executive_summary: Situation -> Complication -> Resolution (exactly 3 sentences).
2. compliance_cost_impact: Quantify the estimated compliance cost increase (%) for affected supply chains.
3. capex_shifts: What capital expenditure reallocation does this trigger?
4. market_access: Which markets become restricted or opened?
5. strategic_recommendations: 3 concrete actions for a technology executive.
6. risk_scenarios: Base / Adverse / Aggressive with probability weights, flywheel feedback analysis, and hedging boundaries.

Output JSON only, no prose outside the JSON:
{
  "executive_summary": {
    "situation": "US CHIPS Act created a 3-year window for domestic packaging buildout.",
    "complication": "Export controls on ASML tools have triggered Japanese counter-investment fragmenting photoresist supply by 2027.",
    "resolution": "Front-load packaging capex to 2026H1 and dual-source photoresist from Korea."
  },
  "compliance_cost_impact": "15-25% increase in verified compliant supply chain overhead",
  "capex_shifts": "US-based advanced packaging capacity accelerated 2 years",
  "market_access": "China market restricted for sub-7nm; ASEAN markets open as alternative",
  "strategic_recommendations": ["Action 1", "Action 2", "Action 3"],
  "risk_scenarios": {
    "base": {
      "probability": 0.6,
      "outcome": "Gradual decoupling over 24 months",
      "flywheel_dynamics": "Positive: data network effects strengthen as aligned markets consolidate standards",
      "hedging_boundary": "If export license denial rate exceeds 15% in any quarter, shift to Adverse"
    },
    "adverse": {
      "probability": 0.25,
      "outcome": "Sudden export license revocations within 6 months — opposite-side counter-reaction triggers Japan's photoresist restrictions",
      "flywheel_dynamics": "Negative: compliance overhead erodes R&D margin -> subscriber churn -> flywheel decelerates",
      "hedging_boundary": "If any single fab reports >30% equipment delivery delay, escalate to Aggressive"
    },
    "aggressive": {
      "probability": 0.15,
      "outcome": "Full technology embargo — overcorrection spiral: export curbs cause foundry shortage -> retaliatory raw-material controls -> secondary wafer crisis",
      "flywheel_dynamics": "Flywheel stalls: market fragmentation breaks data network effects, TAM reduced by 40%+",
      "hedging_boundary": "If two+ allied governments impose parallel controls within 90 days, adopt this as base case"
    }
  },
  "reversal_proximity": 7
}"#;

/// 运行 3 Agent 管线，生成 Premium 研报
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

    // ---- Agent 1: The Diplomat ----
    log::info!("👤 Agent 1 — Diplomat: 分析地缘政治信号...");
    let diplomat_input = format!(
        "Theme: {}\n\nSources:\n{}\n\nAnalyze the geopolitical dynamics of this theme.",
        theme.title, theme_context
    );
    let diplomat_prompt = prompts
        .and_then(|p| Some(p.get_diplomat(DIPLOMAT_PROMPT)))
        .unwrap_or(DIPLOMAT_PROMPT);
    let diplomat_raw = llm::call_with_retry_raw(
        &client,
        api_key,
        llm_config,
        diplomat_prompt,
        &diplomat_input,
    )
    .await?;
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
    let architect_prompt = prompts
        .and_then(|p| Some(p.get_architect(ARCHITECT_PROMPT)))
        .unwrap_or(ARCHITECT_PROMPT);
    let architect_raw = llm::call_with_retry_raw(
        &client,
        api_key,
        llm_config,
        architect_prompt,
        &architect_input,
    )
    .await?;
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
    let quant_prompt = prompts
        .and_then(|p| Some(p.get_quant(QUANT_PROMPT)))
        .unwrap_or(QUANT_PROMPT);
    let quant_raw =
        llm::call_with_retry_raw(&client, api_key, llm_config, quant_prompt, &quant_input).await?;
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
                .unwrap_or(0.5)
                * 100.0,
            quant_json["risk_scenarios"]["base"]["outcome"]
                .as_str()
                .unwrap_or("Status quo")
        ),
        format!(
            "Adverse ({:.0}%): {}",
            quant_json["risk_scenarios"]["adverse"]["probability"]
                .as_f64()
                .unwrap_or(0.25)
                * 100.0,
            quant_json["risk_scenarios"]["adverse"]["outcome"]
                .as_str()
                .unwrap_or("Deterioration")
        ),
        format!(
            "Aggressive ({:.0}%): {}",
            quant_json["risk_scenarios"]["aggressive"]["probability"]
                .as_f64()
                .unwrap_or(0.15)
                * 100.0,
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

/// Phase 2: 推送 Premium 报告到 Substack Draft API
///
/// Code Review 防御性设计:
/// - 失败时不阻塞管线（eprintln 而非 ?），保持 fire-and-forget
/// - 使用独立 reqwest::Client（30s 超时），不干扰其他模块
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
            geopolitical_assessment: "Diplomat analysis".into(),
            technical_impact: "Architect impact".into(),
            commercial_framework: "Quant framework".into(),
            risk_scenarios: vec!["Base (60%): Normal".into()],
            sources: vec!["Federal Register".into()],
        };
        let md = crate::renderer::render_substack_markdown(&report);
        assert!(md.contains("【Premium】SVI Test"));
        assert!(md.contains("Diplomat analysis"));
        assert!(md.contains("Quant framework"));
        assert!(md.contains("Federal Register"));
    }
}
