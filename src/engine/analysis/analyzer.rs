//! 主题分析编排 — analyze_theme + challenge_theme

use anyhow::Result;

use crate::clusterer::{AdverseScenario, Assumption, FactBaseEntry, Theme, ThemeAnalysis};
use crate::config::LlmConfig;
use crate::llm;

use super::causal::parse_causal_chain;
use super::svi::map_to_scl;

/// 分析单个主题：综合所有文章，输出影响判断
pub async fn analyze_theme(
    theme: &Theme,
    api_key: &str,
    llm_config: &LlmConfig,
    language: &str,
    prompts: Option<&crate::config::PromptsConfig>,
) -> Result<ThemeAnalysis> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let is_zh = language == "zh";
    let base_prompt = r#"You are a senior strategic analyst writing for a daily briefing read by startup founders and indie developers.

Your job is NOT to summarize the news.
Your job is to answer one question:
"Does this information change what a founder should do in the next 6 months?"

[CORE RULES]
- Every field must contain a JUDGMENT, not a summary.
- If no thesis is changed, say so explicitly ("No change.") — that is a valid and valuable answer.
- Never open with "In a significant development...", "This week...", "As tensions escalate..." — open cold with the judgment.
- Connect events into causal chains: A → B → C → D. Events are not isolated.

Output JSON Schema:
{
  "bluf": "One-sentence bottom line. Start with the judgment, not the news.",
  "impact": "Strategic implications for founders (1 sentence). What does this mean for a startup?",
  "geopolitical_fact": "What happened — concise, factual, verifiable (2-3 sentences). Situation-Complication-Resolution.",
  "supply_chain_impact": "Why it matters — strategic implications for founders (2-3 sentences). Include industry impact.",
  "analysis_paragraph": "What changed — did this confirm or challenge an existing thesis? If nothing changed, say 'No change.'",
  "what_to_do": "What should I do — one specific, actionable recommendation for a startup founder (1 sentence). Can be 'Nothing.'",
  "what_to_watch": "What signal would change this assessment — what to look for next (1 sentence).",
  "causal_chain": "A → B → C → D chain. Example: 'Export controls → GPU受限 → 推理需求上升 → 开源推理框架爆发 → 应用层门槛下降'",
  "signal_strength": 7,
  "evidence_level": "Established-Fact",
  "fact_base": [
    {"evidence": "verifiable fact", "interpretation": "what it means for founders", "confidence": "Established-Fact"}
  ],
  "connections": ["Related theme 1", "Related theme 2"],
  "assumptions": [
    {"text": "承重假设描述（该判断成立的前提条件）", "load_bearing": true, "evidence_strength": "strong"}
  ]
}

signal_strength (founder's framework):
- 9-10: Changes my strategy this quarter
- 7-8: Changes my priorities this month
- 5-6: Good to know, no immediate action
- 1-4: Noise, ignore

Evidence Level (4 levels):
- Established-Fact: Direct, verifiable evidence from authoritative sources.
- First-Principles: No direct evidence required; conclusion flows from physical law or economic necessity.
- Developing-Inference: Emerging but incomplete evidence.
- Assertion-Rumor: Unverified claim, treat as hypothesis.

[OUTPUT RULE] Output json only (valid JSON)."#;
    let base_prompt = prompts
        .map(|p| p.get_analyze_theme(base_prompt))
        .unwrap_or(base_prompt);
    let system_prompt = if is_zh {
        format!("{}\n\n[CRITICAL COMPLIANCE]: All structural JSON values (strings) MUST be translated into high-density, editorial Traditional Chinese (繁體中文). Do NOT translate JSON keys. Ensure JSON structure remains unmodified.\nExport controls → 出口管制, supply chain → 供應鏈, semiconductor → 半導體, chip → 晶片, tariff → 關稅.", base_prompt)
    } else {
        base_prompt.to_string()
    };

    let mut user_prompt = format!("## 主题: {}\n{}\n\n", theme.title, theme.summary);
    user_prompt.push_str(&format!("共 {} 条证据：\n\n", theme.articles.len()));
    for (i, a) in theme.articles.iter().enumerate() {
        let body = a
            .content
            .as_deref()
            .or(a.summary.as_deref())
            .unwrap_or("(无全文)");
        let truncated = if body.len() > 1500 {
            let end = body.floor_char_boundary(1500);
            &body[..end]
        } else {
            body
        };
        let description = if truncated.len() > 10 {
            truncated
        } else {
            &a.title
        };
        user_prompt.push_str(&format!(
            "证据 {}: 「{}」——来自 {}\n\n",
            i + 1,
            description,
            a.source
        ));
    }

    let raw = llm::call_with_retry_raw(&client, api_key, llm_config, &system_prompt, &user_prompt)
        .await?;
    let parsed: serde_json::Value = llm::parse_json_lenient(&raw)?;

    // 带日志的字段提取：LLM 缺失字段时记录 warn
    let es = |field: &str, default: &str| -> String {
        let s = parsed[field].as_str();
        if s.is_none() {
            log::warn!("⚠️ LLM 响应缺少字段 '{}'，使用默认值 '{}'", field, default);
        }
        s.unwrap_or(default).to_string()
    };

    let source_urls: Vec<String> = theme.articles.iter().map(|a| a.url.clone()).collect();
    let connections = parsed["connections"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let fact_base = parsed["fact_base"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    Some(FactBaseEntry {
                        evidence: v["evidence"].as_str()?.to_string(),
                        interpretation: v["interpretation"].as_str()?.to_string(),
                        confidence: v["confidence"]
                            .as_str()
                            .unwrap_or("发展中-推断")
                            .to_string(),
                    })
                })
                .collect::<Vec<FactBaseEntry>>()
        })
        .unwrap_or_default();

    let mut fact_base = fact_base;
    let evidence_level_raw = es("evidence_level", "发展中-推断");
    let evidence_level = map_to_scl(&evidence_level_raw);
    for fb in &mut fact_base {
        fb.confidence = map_to_scl(&fb.confidence);
    }

    // 解析 assumptions（从 LLM JSON 中提取承重假设）
    let assumptions: Vec<Assumption> = parsed["assumptions"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    Some(Assumption {
                        text: v["text"].as_str()?.to_string(),
                        load_bearing: v["load_bearing"].as_bool().unwrap_or(true),
                        evidence_strength: v["evidence_strength"]
                            .as_str()
                            .unwrap_or("medium")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(ThemeAnalysis {
        theme_id: theme.id.clone(),
        theme_title: theme.title.clone(),
        bluf: es("bluf", "待分析"),
        impact: es("impact", "待分析"),
        evidence_level,
        signal_strength: (parsed["signal_strength"].as_u64().unwrap_or(5) as u8).clamp(1, 10),
        geopolitical_fact: es("geopolitical_fact", ""),
        supply_chain_impact: es("supply_chain_impact", ""),
        analysis_paragraph: es("analysis_paragraph", ""),
        fact_base,
        connections,
        source_urls,
        assumptions,
        adverse: None,
        next_tests: vec![],
        open_questions: vec![],
        chains: parse_causal_chain(&parsed["causal_chain"]),
        what_to_do: es("what_to_do", ""),
        what_to_watch: es("what_to_watch", ""),
    })
}

/// 蓝军验证：挑战主题分析
///
/// 使用 LLM 对分析进行对抗性审查，识别：
///   1. 未被识别的承重假设
///   2. 可能的逆境情景
///   3. 分析中的盲点
///   4. 需要注意的早期预警信号
pub async fn challenge_theme(
    analysis: &ThemeAnalysis,
    api_key: &str,
    llm_config: &LlmConfig,
    prompts: Option<&crate::config::PromptsConfig>,
) -> Result<(
    Vec<Assumption>,
    Option<AdverseScenario>,
    Vec<String>,
    Vec<String>,
)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()?;

    let base_prompt = "You are a red-team adversarial analyst. Your job is to STRESS-TEST the following strategic analysis.

Find weaknesses, hidden assumptions, and failure scenarios that the original analyst missed.

Output JSON Schema:
{
  \"hidden_assumptions\": [
    {\"text\": \"隐藏的承重假设描述\", \"load_bearing\": true, \"evidence_strength\": \"weak\"}
  ],
  \"adverse_scenario\": {
    \"scenario\": \"如果假设不成立会发生什么？\",
    \"early_warning\": \"什么信号会表明这个情景正在发生？\",
    \"severity\": \"critical / high / moderate\"
  },
  \"blind_spots\": [\"分析中缺失的角度 1\", \"分析中缺失的角度 2\"],
  \"early_warnings\": [\"需要关注的预警信号 1\"]
}

[OUTPUT RULE] Output json only (valid JSON).";

    let prompt = prompts
        .map(|p| p.get_analyze_theme(base_prompt))
        .unwrap_or(base_prompt);

    let user_prompt = format!(
        "## 主题: {}\nBLUF: {}\n影响: {}\n地缘: {}\n供应链: {}\n分析: {}\n",
        analysis.theme_title,
        analysis.bluf,
        analysis.impact,
        analysis.geopolitical_fact,
        analysis.supply_chain_impact,
        analysis.analysis_paragraph,
    );

    let raw = llm::call_with_retry_raw(&client, api_key, llm_config, prompt, &user_prompt).await?;
    let parsed: serde_json::Value = llm::parse_json_lenient(&raw)?;

    // 解析隐藏假设
    let hidden_assumptions: Vec<Assumption> = parsed["hidden_assumptions"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    Some(Assumption {
                        text: v["text"].as_str()?.to_string(),
                        load_bearing: v["load_bearing"].as_bool().unwrap_or(true),
                        evidence_strength: v["evidence_strength"]
                            .as_str()
                            .unwrap_or("weak")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // 解析逆境情景
    let adverse_scenario = parsed["adverse_scenario"].as_object().and_then(|obj| {
        Some(AdverseScenario {
            scenario: obj.get("scenario")?.as_str()?.to_string(),
            early_warning: obj.get("early_warning")?.as_str()?.to_string(),
            severity: obj.get("severity")?.as_str()?.to_string(),
        })
    });

    // 解析盲点
    let blind_spots: Vec<String> = parsed["blind_spots"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // 解析预警信号
    let early_warnings: Vec<String> = parsed["early_warnings"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok((
        hidden_assumptions,
        adverse_scenario,
        blind_spots,
        early_warnings,
    ))
}
