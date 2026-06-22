//! Clusterer — 主题聚类
//!
//! 将 N 篇文章聚类为 ≤5 个主题，每个主题包含关联的文章和综合影响分析。
//! 参考 McKinsey Tech Trends 的分类分层结构。

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::LlmConfig;
use crate::fetcher::Article;
use crate::llm;

/// 一个主题
#[derive(Debug, Clone, Serialize)]
pub struct Theme {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub articles: Vec<Article>,
    pub sources: Vec<String>,  // 来源列表，用于溯源
}

/// Fact Base 条目（抄 situation-assessment: Evidence | Interpretation | Confidence）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactBaseEntry {
    pub evidence: String,
    pub interpretation: String,
    pub confidence: String,
}

/// 承重假设（蓝军输出）
#[derive(Debug, Clone, Serialize)]
pub struct Assumption {
    pub text: String,
    pub load_bearing: bool,
    pub evidence_strength: String,
}

/// 逆境情景（蓝军输出）
#[derive(Debug, Clone, Serialize)]
pub struct AdverseScenario {
    pub scenario: String,
    pub early_warning: String,
    pub severity: String,
}

/// 因果链（跨域分析框架）
#[derive(Debug, Clone, Serialize)]
pub struct CausalChain {
    pub trigger: String,
    pub direct_effect: String,
    pub chain_reaction: Vec<String>,
    pub second_order: Vec<String>,
}

/// 主题分析结果
#[derive(Debug, Clone, Serialize)]
pub struct ThemeAnalysis {
    pub theme_id: String,
    pub theme_title: String,
    pub bluf: String,              // 一句话结论
    pub impact: String,            // 战略影响
    pub geopolitical_fact: String,  // Layer 2: 客观事实复述（海外版）
    pub supply_chain_impact: String,// Layer 2: 供应链传导分析
    pub analysis_paragraph: String, // 分析与背景（用于聚合输出）
    pub evidence_level: String,    // SCL: 确立-事实
    pub signal_strength: u8,       // 1-10 信号强度
    pub fact_base: Vec<FactBaseEntry>,  // 抄 McKinsey: 事实-解读-置信度表格
    pub connections: Vec<String>,  // 关联的其他主题
    pub source_urls: Vec<String>,  // 原文链接
    // Phase 1: 蓝军输出
    pub assumptions: Vec<Assumption>,
    pub adverse: Option<AdverseScenario>,
    pub next_tests: Vec<String>,
    pub open_questions: Vec<String>,
    pub chains: Vec<CausalChain>,
}

/// 将文章聚类为主题
pub async fn cluster_articles(
    articles: &[Article],
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<Vec<Theme>> {
    if articles.is_empty() {
        return Ok(Vec::new());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let system_prompt = r#"你是一个情报分析师。你的任务是将以下文章聚类为不超过 5 个主题。

规则：
1. 仔细阅读每篇文章的标题和摘要，找出它们共同指向的主题
2. 同一主题必须包含 ≥2 篇文章（单篇文章不成主题）
3. 主题标题要简洁有力（10 字以内），如"模型商品化""Agent可靠性""政策风险"
4. 给每个主题写一句话摘要（30 字以内）

输出严格 JSON：
{"themes": [
  {"id": "t1", "title": "模型商品化", "summary": "开源模型能力接近闭锁", "article_indices": [0, 2, 5]},
  {"id": "t2", "title": "Agent可靠性", "summary": "可靠性成为竞争焦点", "article_indices": [1, 3]}
]}

article_indices 是文章在输入列表中的序号（从 0 开始）。
未归入任何主题的文章直接忽略。"#;

    // 构建用户 prompt：精简版，只传标题+来源+摘要
    let mut user_prompt = format!("请将以下 {} 篇文章聚类为主题：\n\n", articles.len());
    for (i, a) in articles.iter().enumerate() {
        let summary = a.summary.as_deref().unwrap_or("");
        let snippet = if summary.len() > 200 {
            let end = summary.floor_char_boundary(200);
            &summary[..end]
        } else {
            summary
        };
        user_prompt.push_str(&format!(
            "[{}] 标题: {} | 来源: {} | 摘要: {}\n",
            i, a.title, a.source, snippet
        ));
    }

    let raw = llm::call_with_retry_raw(&client, api_key, llm_config, &system_prompt, &user_prompt).await?;
    let parsed: serde_json::Value = llm::parse_json_lenient(&raw)?;

    let mut themes = Vec::new();
    if let Some(theme_list) = parsed["themes"].as_array() {
        for t in theme_list {
            let id = t["id"].as_str().unwrap_or("tx").to_string();
            let title = t["title"].as_str().unwrap_or("未命名").to_string();
            let summary = t["summary"].as_str().unwrap_or("").to_string();

            let mut theme_articles = Vec::new();
            let mut sources = Vec::new();
            if let Some(indices) = t["article_indices"].as_array() {
                for idx in indices {
                    if let Some(i) = idx.as_u64() {
                        if let Some(a) = articles.get(i as usize) {
                            if !sources.contains(&a.source) {
                                sources.push(a.source.clone());
                            }
                            theme_articles.push(a.clone());
                        }
                    }
                }
            }
            // 只保留有 ≥2 篇文章的主题
            if theme_articles.len() >= 2 {
                themes.push(Theme { id, title, summary, articles: theme_articles, sources });
            }
        }
    }

    // 如果没有生成任何主题（LLM 输出格式问题），回退：全部归入"其他"
    if themes.is_empty() && !articles.is_empty() {
        let all_sources: Vec<String> = articles.iter().map(|a| a.source.clone()).collect();
        themes.push(Theme {
            id: "t_other".into(),
            title: "今日要闻".into(),
            summary: "未能自动聚类，以下为今日全部信号".into(),
            articles: articles.to_vec(),
            sources: all_sources,
        });
    }

    log::info!("📊 聚类完成: {} 篇文章 → {} 个主题", articles.len(), themes.len());
    Ok(themes)
}

/// 将 L1-L5 旧置信等级映射为 SCL（Sulix Confidence Level）
fn map_to_scl(value: &str) -> String {
    match value.trim() {
        "L1" | "L2" | "确立" => "确立-事实".into(),
        "L3" | "发展中" => "发展中-推断".into(),
        "L4" | "建立" => "建立-传闻".into(),
        "L5" | "噪音" => "噪音".into(),
        other => other.to_string(), // 已经是 SCL 格式则原样返回
    }
}

/// 分析单个主题：综合所有文章，输出影响判断
pub async fn analyze_theme(
    theme: &Theme,
    api_key: &str,
    llm_config: &LlmConfig,
    language: &str,
) -> Result<ThemeAnalysis> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let is_zh = language == "zh";
    let base_prompt = r#"You are a senior geopolitical technology strategist for a publication like The Economist.

Analyze the provided inputs and generate a structured JSON report.
Maintain an objective, authoritative, analytical tone.
Never censor, omit, or sugarcoat hard political facts (e.g. sanctions, tariffs, elections).
Contextualize them into tangible industry metrics.

Output JSON Schema:
{
  "fact_base": [
    {"evidence": "GLM-5.2 scores close to GPT-4o", "interpretation": "Open-source catching up to closed-source", "confidence": "Established-Fact"},
    {"evidence": "Export controls restrict ASML shipments", "interpretation": "Semiconductor supply chain faces new constraints", "confidence": "Established-Fact"}
  ],
  "signal_strength": 7,
  "bluf": "One-sentence conclusion (15 words max)",
  "geopolitical_fact": "Objective summary of the political/regulatory event, including specific actors and countries involved.",
  "supply_chain_impact": "Data-driven synthesis: how this event affects physical supply, compliance costs, or logistics damping.",
  "analysis_paragraph": "This week multiple signals point in the same direction...",
  "connections": ["Related theme 1", "Related theme 2"]
}

signal_strength (GS three-scenario framework):
- Base Scenario: 5-6
- Adverse Scenario: 7-8
- Aggressive Scenario: 9-10
- 1-4: noise or single-point event"#;
    let system_prompt = if is_zh {
        format!("{}\n\n[CRITICAL COMPLIANCE]: All structural JSON values (strings) MUST be translated into high-density, editorial Traditional Chinese (繁體中文). Do NOT translate JSON keys. Ensure JSON structure remains unmodified.\nExport controls → 出口管制, supply chain → 供應鏈, semiconductor → 半導體, chip → 晶片, tariff → 關稅.", base_prompt)
    } else {
        base_prompt.to_string()
    };

    let mut user_prompt = format!("## 主题: {}\n{}\n\n", theme.title, theme.summary);
    user_prompt.push_str(&format!("共 {} 条证据：\n\n", theme.articles.len()));
    for (i, a) in theme.articles.iter().enumerate() {
        let body = a.content.as_deref()
            .or(a.summary.as_deref())
            .unwrap_or("(无全文)");
        let truncated = if body.len() > 1500 {
            let end = body.floor_char_boundary(1500);
            &body[..end]
        } else { body };
        // 只传干净的描述，不传内部字段名
        let description = if truncated.len() > 10 { truncated } else { &a.title };
        user_prompt.push_str(&format!("证据 {}: 「{}」——来自 {}\n\n", i + 1, description, a.source));
    }

    let raw = llm::call_with_retry_raw(&client, api_key, llm_config, &system_prompt, &user_prompt).await?;
    let parsed: serde_json::Value = llm::parse_json_lenient(&raw)?;

    let source_urls: Vec<String> = theme.articles.iter().map(|a| a.url.clone()).collect();
    let connections = parsed["connections"].as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let fact_base = parsed["fact_base"].as_array()
        .map(|arr| {
            arr.iter().filter_map(|v| {
                Some(FactBaseEntry {
                    evidence: v["evidence"].as_str()?.to_string(),
                    interpretation: v["interpretation"].as_str()?.to_string(),
                    confidence: v["confidence"].as_str().unwrap_or("发展中-推断").to_string(),
                })
            }).collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // 强制映射 L1-L5 → SCL
    let mut fact_base = fact_base; // make mutable for SCL mapping
    let evidence_level_raw = parsed["evidence_level"].as_str().unwrap_or("发展中-推断");
    let evidence_level = map_to_scl(evidence_level_raw);
    for fb in &mut fact_base {
        fb.confidence = map_to_scl(&fb.confidence);
    }
    let analysis_paragraph = parsed["analysis_paragraph"].as_str().unwrap_or("").to_string();
    let geopolitical_fact = parsed["geopolitical_fact"].as_str().unwrap_or("").to_string();
    let supply_chain_impact = parsed["supply_chain_impact"].as_str().unwrap_or("").to_string();

    Ok(ThemeAnalysis {
        theme_id: theme.id.clone(),
        theme_title: theme.title.clone(),
        bluf: parsed["bluf"].as_str().unwrap_or("待分析").to_string(),
        impact: parsed["impact"].as_str().unwrap_or("待分析").to_string(),
        evidence_level,
        signal_strength: parsed["signal_strength"].as_u64().unwrap_or(5) as u8,
        fact_base,
        connections,
        source_urls,
        assumptions: vec![],
        adverse: None,
        next_tests: vec![],
        open_questions: vec![],
        chains: vec![],
        analysis_paragraph,
        geopolitical_fact,
        supply_chain_impact,
    })
}

/// 蓝军验证：挑战主题分析，输出承重假设、逆境情景、待验证项
pub async fn challenge_theme(
    analysis: &ThemeAnalysis,
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<(Vec<Assumption>, Option<AdverseScenario>, Vec<String>, Vec<String>)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let system_prompt = r#"你是一个多疑的审查员（蓝军）。你的任务是挑战给定的判断，输出：

1. 承重假设（load-bearing assumptions）：该判断依赖哪些前提？标注是否承重、证据强度
2. 逆境情景（adverse scenario）：如果前提不成立，最坏的合理情景是什么？可观测的早期预警信号是什么？
3. 待验证项（next tests）：要证伪/证实这个判断，需要看到什么具体数据？

输出严格 JSON：
{
  "assumptions": [
    {"text": "假设内容", "load_bearing": true, "evidence_strength": "weak|moderate|strong"}
  ],
  "adverse": {"scenario": "如果...则...", "early_warning": "可观测信号", "severity": "high|med|low"},
  "next_tests": ["测试1", "测试2"]
}

证据强度标准：
- strong: 多方确认的事实
- moderate: 有依据但非确凿
- weak: 推测或无证据"#;

    let user_prompt = format!(
        "请挑战以下判断：\n\n标题: {}\n\n结论: {}\n\n影响: {}\n\n证据等级: {}\n\n信号强度: {}/10",
        analysis.theme_title, analysis.bluf, analysis.impact, analysis.evidence_level, analysis.signal_strength,
    );

    match llm::call_with_retry_raw(&client, api_key, llm_config, system_prompt, &user_prompt).await {
        Ok(raw) => {
            if let Ok(parsed) = llm::parse_json_lenient(&raw) {
                let assumptions = parsed["assumptions"].as_array()
                    .map(|arr| arr.iter().filter_map(|v| {
                        Some(Assumption {
                            text: v["text"].as_str()?.to_string(),
                            load_bearing: v["load_bearing"].as_bool().unwrap_or(false),
                            evidence_strength: v["evidence_strength"].as_str().unwrap_or("weak").to_string(),
                        })
                    }).collect())
                    .unwrap_or_default();

                let adverse = parsed["adverse"].as_object().map(|_| AdverseScenario {
                    scenario: parsed["adverse"]["scenario"].as_str().unwrap_or("").to_string(),
                    early_warning: parsed["adverse"]["early_warning"].as_str().unwrap_or("").to_string(),
                    severity: parsed["adverse"]["severity"].as_str().unwrap_or("med").to_string(),
                });

                let next_tests = parsed["next_tests"].as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();

                let open_questions = parsed["open_questions"].as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();

                Ok((assumptions, adverse, next_tests, open_questions))
            } else {
                Ok((vec![], None, vec![], vec![]))
            }
        }
        Err(e) => {
            log::warn!("⚠️ 蓝军挑战失败: {}", e);
            Ok((vec![], None, vec![], vec![]))
        }
    }
}

/// 整合所有主题分析，输出综合判断
pub fn synthesize(
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
) -> Summary {
    let mut narrative = String::new();

    // 找主题之间的关联
    let mut all_connections: Vec<&str> = Vec::new();
    for a in analyses {
        for c in &a.connections {
            if !all_connections.contains(&c.as_str()) {
                all_connections.push(c);
            }
        }
    }

    // 构建叙事
    if analyses.len() >= 2 {
        narrative.push_str("多个主题指向同一方向：");
        // 用第一个作为起点
        if let Some(first) = analyses.first() {
            narrative.push_str(&format!("\n- {} → {}", first.theme_title, first.bluf));
        }
        for a in analyses.iter().skip(1) {
            narrative.push_str(&format!("\n- {} → {}", a.theme_title, a.bluf));
        }
    } else if let Some(first) = analyses.first() {
        narrative = first.bluf.clone();
    }

    Summary {
        headline: if analyses.len() >= 2 {
            format!("{} 个主题指向同一趋势", analyses.len())
        } else {
            "单主题深度分析".into()
        },
        narrative,
        total_articles: themes.iter().map(|t| t.articles.len()).sum(),
        theme_count: themes.len(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub headline: String,
    pub narrative: String,
    pub total_articles: usize,
    pub theme_count: usize,
}
