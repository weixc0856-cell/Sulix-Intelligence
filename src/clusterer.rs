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
    pub sources: Vec<String>, // 来源列表，用于溯源
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
    pub bluf: String,                  // 一句话结论
    pub impact: String,                // 战略影响
    pub geopolitical_fact: String,     // Layer 2: 客观事实复述（海外版）
    pub supply_chain_impact: String,   // Layer 2: 供应链传导分析
    pub analysis_paragraph: String,    // 分析与背景（用于聚合输出）
    pub evidence_level: String,        // SCL: 确立-事实
    pub signal_strength: u8,           // 1-10 信号强度
    pub fact_base: Vec<FactBaseEntry>, // 抄 McKinsey: 事实-解读-置信度表格
    pub connections: Vec<String>,      // 关联的其他主题
    pub source_urls: Vec<String>,      // 原文链接
    // Phase 1: 蓝军输出
    pub assumptions: Vec<Assumption>,
    pub adverse: Option<AdverseScenario>,
    pub next_tests: Vec<String>,
    pub open_questions: Vec<String>,
    pub chains: Vec<CausalChain>,
    /// 创业者行动建议（创始人五段）
    pub what_to_do: String,
    /// 后续关注信号（创始人五段）
    pub what_to_watch: String,
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

Output json. 输出严格 JSON：
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

    let raw =
        llm::call_with_retry_raw(&client, api_key, llm_config, system_prompt, &user_prompt).await?;
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
                themes.push(Theme {
                    id,
                    title,
                    summary,
                    articles: theme_articles,
                    sources,
                });
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

    log::info!(
        "📊 聚类完成: {} 篇文章 → {} 个主题",
        articles.len(),
        themes.len()
    );
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
  "connections": ["Related theme 1", "Related theme 2"]
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
        .and_then(|p| Some(p.get_analyze_theme(base_prompt)))
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
        // 只传干净的描述，不传内部字段名
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
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // 强制映射 L1-L5 → SCL
    let mut fact_base = fact_base; // make mutable for SCL mapping
    let evidence_level_raw = parsed["evidence_level"].as_str().unwrap_or("发展中-推断");
    let evidence_level = map_to_scl(evidence_level_raw);
    for fb in &mut fact_base {
        fb.confidence = map_to_scl(&fb.confidence);
    }
    let theme_id_str = &theme.id;
    let analysis_paragraph = parsed["analysis_paragraph"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            log::warn!(
                "analysis_paragraph missing in LLM output for theme {}",
                theme_id_str
            );
            String::new()
        });
    let geopolitical_fact = parsed["geopolitical_fact"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            log::warn!(
                "geopolitical_fact missing in LLM output for theme {}",
                theme_id_str
            );
            String::new()
        });
    let supply_chain_impact = parsed["supply_chain_impact"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            log::warn!(
                "supply_chain_impact missing in LLM output for theme {}",
                theme_id_str
            );
            String::new()
        });

    Ok(ThemeAnalysis {
        theme_id: theme.id.clone(),
        theme_title: theme.title.clone(),
        bluf: parsed["bluf"].as_str().unwrap_or("待分析").to_string(),
        impact: parsed["impact"].as_str().unwrap_or("待分析").to_string(),
        evidence_level,
        signal_strength: parsed["signal_strength"].as_u64().unwrap_or_else(|| {
            log::warn!(
                "signal_strength missing in LLM output for theme {}",
                theme_id_str
            );
            5
        }) as u8,
        fact_base,
        connections,
        source_urls,
        assumptions: vec![],
        adverse: None,
        next_tests: vec![],
        open_questions: vec![],
        chains: parse_causal_chain(&parsed["causal_chain"]),
        analysis_paragraph,
        geopolitical_fact,
        supply_chain_impact,
        what_to_do: parsed["what_to_do"].as_str().unwrap_or("").to_string(),
        what_to_watch: parsed["what_to_watch"].as_str().unwrap_or("").to_string(),
    })
}

/// 蓝军验证：挑战主题分析，输出承重假设、逆境情景、待验证项
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
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let system_prompt = r#"你是一个多疑的审查员（蓝军）。你的任务是挑战给定的判断，输出分析。

你运用以下三个强制框架进行审查：

### 框架1 — 老子《道德经》"反者道之动"辩证法
任何技术趋势推至极端必然产生反向阻尼力：合规代价攀升、生态反弹力倍增。
- 在逆境情景中必须识别"对立面反动特征"：某项技术普及 -> 同时导致另一材料供应链断裂
- 评分当前趋势的"物极必反临界度"(1-10)，10分表示反向力已可观测

### 框架2 — 亚马逊飞轮控制论
识别该领域数据->收入->数据的正反馈飞轮结构：
- 正反馈加速：什么因素在推动飞轮加速？
- 负反馈减速：什么因素可能使飞轮减速或逆转？
- 临界点：正反馈转为负反馈的转折点在哪？

### 框架3 — MECE 金字塔结构强制
- 承重假设必须 MECE，不得重叠或遗漏逻辑分支
- 逆境情景必须包含半定量概率区间(Probability Range)和对冲边界(Hedging Boundary)
- 待验证项必须包含具体数据源或指标及建议时间窗口

Output json. 输出严格 JSON，必须包含以下全部字段：
{
  "assumptions": [
    {"text": "假设内容", "load_bearing": true, "evidence_strength": "weak|moderate|strong", "category": "技术|政策|市场|供应链|金融"}
  ],
  "adverse": {
    "scenario": "如果...则...（必须包含对立面反动特征描述）",
    "opposite_reaction": "该趋势推向极致的反向反作用力是什么？",
    "flywheel_risk": "飞轮在此情景下如何减速或逆转？",
    "early_warning": "可观测的早期预警信号",
    "severity": "high|med|low",
    "probability_range": "例如 10-25%",
    "hedging_boundary": "什么可观测阈值触发重新评估？"
  },
  "next_tests": [
    {"test": "要证伪/证实的具体测试", "data_source": "建议数据源或指标", "time_window": "建议观察时间窗口"}
  ],
  "open_questions": ["当前无法回答但影响判断的关键问题"],
  "flywheel_analysis": {
    "positive_loop": "当前正反馈加速因素",
    "negative_loop": "潜在负反馈减速因素",
    "tipping_point": "正反馈转为负反馈的临界阈值"
  },
  "reversal_proximity": 7
}

证据强度标准：
- strong: 多方确认的事实
- moderate: 有依据但非确凿
- weak: 推测或无证据

MECE 原则：各假设之间不得有逻辑重叠或遗漏。禁止输出"待进一步分析"等模糊表述。"#;
    let system_prompt = prompts
        .and_then(|p| Some(p.get_challenge_theme(&system_prompt)))
        .unwrap_or(system_prompt);

    let user_prompt = format!(
        "请挑战以下判断：\n\n标题: {}\n\n结论: {}\n\n影响: {}\n\n证据等级: {}\n\n信号强度: {}/10",
        analysis.theme_title,
        analysis.bluf,
        analysis.impact,
        analysis.evidence_level,
        analysis.signal_strength,
    );

    match llm::call_with_retry_raw(&client, api_key, llm_config, system_prompt, &user_prompt).await
    {
        Ok(raw) => {
            if let Ok(parsed) = llm::parse_json_lenient(&raw) {
                let assumptions = parsed["assumptions"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| {
                                Some(Assumption {
                                    text: v["text"].as_str()?.to_string(),
                                    load_bearing: v["load_bearing"].as_bool().unwrap_or(false),
                                    evidence_strength: v["evidence_strength"]
                                        .as_str()
                                        .unwrap_or("weak")
                                        .to_string(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let adverse = parsed["adverse"].as_object().map(|_| AdverseScenario {
                    scenario: parsed["adverse"]["scenario"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    early_warning: parsed["adverse"]["early_warning"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    severity: parsed["adverse"]["severity"]
                        .as_str()
                        .unwrap_or("med")
                        .to_string(),
                });

                let next_tests = parsed["next_tests"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let open_questions = parsed["open_questions"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
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
pub fn synthesize(themes: &[Theme], analyses: &[ThemeAnalysis]) -> Summary {
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

/// 战略异动指数 (SVI) 权重
const SVI_ENTITY_SURGE: f64 = 0.20;
const SVI_SANCTION_SENSITIVITY: f64 = 0.25;
const SVI_PATENT_MUTATION: f64 = 0.15;
const SVI_SOURCE_CREDIBILITY: f64 = 0.15;
const SVI_TEMPORAL_URGENCY: f64 = 0.10;
const SVI_RECENCY: f64 = 0.15;

/// 计算战略异动指数 (SVI)
///
/// 五维综合评分 0-10，取代粗糙的 single_strength >= 5 单维度触发。
/// SVI >= 7 → 标准 Premium 触发，SVI >= 9 → Flash 紧急加更模式。
///
/// 当前实现使用 `signal_strength` 作为 SanctionSensitivity 和 PatentMutation 的代理。
/// Phase 2 将接入 `EntitySanctionDb` 和 USPTO 专利突变检测以精确化这两个维度。
pub fn calculate_svi(
    analysis: &ThemeAnalysis,
    theme: &Theme,
    sources: &[crate::config::SourceConfig],
) -> u8 {
    // 1. EntitySurge: 同一 source 在主题 articles 中的出现密度
    let article_count = theme.articles.len() as f64;
    let mut source_counts: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    for art in &theme.articles {
        *source_counts.entry(art.source.as_str()).or_insert(0) += 1;
    }
    let max_repeats = source_counts.values().copied().max().unwrap_or(1) as f64;
    let entity_surge = if article_count >= 3.0 {
        (max_repeats / article_count).min(1.0)
    } else {
        0.3
    };

    // 2. SourceScore: 取 articles 中最高的 source score（归一化到 0.1-1.0）
    // score: 10=OpenAI Blog/BIS, 5=默认, 1=社交噪音
    let best_score = theme
        .articles
        .iter()
        .map(|a| {
            sources
                .iter()
                .find(|s| s.name == a.source)
                .map(|s| s.score as f64 / 10.0)
                .unwrap_or(0.5)
        })
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0.5);

    // 3. Recency: 基于文章发布时间（无时间戳则用 signal_strength 代理）
    let recency = if let Some(pub_date) = theme.articles.iter().filter_map(|a| a.published_at).max()
    {
        let days_old = (chrono::Utc::now() - pub_date.with_timezone(&chrono::Utc))
            .num_days()
            .max(0);
        if days_old <= 1 {
            1.0
        } else if days_old <= 3 {
            0.8
        } else if days_old <= 7 {
            0.5
        } else {
            0.2
        }
    } else {
        0.5
    };

    // 4. TemporalUrgency: signal_strength 作为 LLM 对时效性的评估
    let temporal_urgency = (analysis.signal_strength as f64) / 10.0;

    // 5. SanctionSensitivity: Phase 1 用 signal_strength 代理，Phase 2 接入 EntitySanctionDb
    let sanction_sensitivity = temporal_urgency;

    // 6. PatentMutation: Phase 1 用 signal_strength 代理，Phase 2 接入 USPTO 突变检测
    let patent_mutation = (analysis.signal_strength as f64) / 10.0;

    let score = entity_surge * SVI_ENTITY_SURGE
        + sanction_sensitivity * SVI_SANCTION_SENSITIVITY
        + patent_mutation * SVI_PATENT_MUTATION
        + best_score * SVI_SOURCE_CREDIBILITY
        + temporal_urgency * SVI_TEMPORAL_URGENCY
        + recency * SVI_RECENCY;

    ((score * 10.0).round() as u8).min(10).max(0)
}

/// 解析 LLM 输出的因果链字符串为 Vec<CausalChain>
/// 输入: "出口管制 → GPU受限 → 推理需求上升 → 开源推理框架爆发 → 应用层门槛下降"
/// 输出: [CausalChain { trigger: "出口管制", direct_effect: "GPU受限", chain_reaction: [...], ... }]
fn parse_causal_chain(value: &serde_json::Value) -> Vec<CausalChain> {
    let text = match value.as_str() {
        Some(s) if !s.is_empty() && s != "null" => s.to_string(),
        _ => return vec![],
    };
    // 按 → 或 -> 拆分
    let parts: Vec<&str> = text
        .split(|c| c == '→' || (c == '-' && text.contains("->")))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() < 2 {
        return vec![];
    }
    let trigger = parts[0].to_string();
    let direct_effect = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
    let chain_reaction: Vec<String> = parts.iter().skip(2).map(|s| s.to_string()).collect();
    vec![CausalChain {
        trigger,
        direct_effect,
        chain_reaction,
        second_order: vec![],
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    // (change detection tests moved to src/change_detection.rs)
}

// Re-export change_detection module types for backward compatibility
pub use crate::change_detection::{detect_changes_llm, detect_changes_rule, ChangeSummary};

// ===== News Layer: LLM 预去重 =====

/// LLM 预去重 prompt 结构
const PREDEDUP_SYSTEM_PROMPT: &str = r#"你是新闻去重专家。判断哪些文章在报道同一事件。
输出JSON: {"keep": [保留的文章序号], "merge_groups": [[同一事件的文章序号组]]}
只返回JSON，不要解释。"#;

/// 在聚类前对文章做 LLM 语义去重
/// 按 category 分批，batch_size 建议 15-20
pub async fn llm_prededup(
    articles: &[Article],
    api_key: &str,
    llm_config: &crate::config::LlmConfig,
    prompts: Option<&crate::config::PromptsConfig>,
    batch_size: usize,
) -> Result<Vec<Article>> {
    if articles.len() <= 1 {
        return Ok(articles.to_vec());
    }

    // 按 category 分组
    let mut by_cat: std::collections::HashMap<String, Vec<Article>> =
        std::collections::HashMap::new();
    for art in articles.iter() {
        by_cat
            .entry(art.category.clone())
            .or_default()
            .push(art.clone());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let system_prompt = prompts
        .and_then(|p| Some(p.get_cluster_articles(PREDEDUP_SYSTEM_PROMPT)))
        .unwrap_or(PREDEDUP_SYSTEM_PROMPT);

    let mut result = Vec::new();

    for (_cat, batch) in by_cat {
        if batch.len() <= 1 {
            result.extend(batch);
            continue;
        }

        for chunk in batch.chunks(batch_size) {
            let article_list: String = chunk
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    format!(
                        "[{}] {}: {}",
                        i,
                        a.title,
                        a.summary.as_deref().unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            let user_prompt = format!(
                "Category: {}\n\nArticles:\n{}",
                chunk[0].category, article_list
            );

            match crate::llm::call_with_retry_raw(
                &client,
                api_key,
                llm_config,
                system_prompt,
                &user_prompt,
            )
            .await
            {
                Ok(raw) => {
                    let clean = raw
                        .trim()
                        .trim_start_matches("```json")
                        .trim_start_matches("```")
                        .trim_end_matches("```")
                        .trim();

                    #[derive(serde::Deserialize, Default)]
                    struct DedupOutput {
                        keep: Vec<usize>,
                        merge_groups: Vec<Vec<usize>>,
                    }

                    let dedup: DedupOutput = serde_json::from_str(clean).unwrap_or_default();

                    let mut keep_indices: std::collections::HashSet<usize> =
                        dedup.keep.into_iter().collect();
                    for group in &dedup.merge_groups {
                        // 每组保留第一篇
                        if let Some(&first) = group.first() {
                            keep_indices.insert(first);
                        }
                    }

                    if keep_indices.is_empty() {
                        result.extend(chunk.iter().cloned());
                    } else {
                        for (i, article) in chunk.iter().enumerate() {
                            if keep_indices.contains(&i) {
                                result.push(article.clone());
                            }
                        }
                    }
                }
                Err(_) => {
                    // LLM 失败，回退保留全部
                    result.extend(chunk.iter().cloned());
                }
            }
        }
    }

    Ok(result)
}
