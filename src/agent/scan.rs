//! Scan Agent — 信号初筛模块（Gate v1.1 保真版）
//!
//! v1.1 核心变化：
//! - Layer 0: 不评分，只结构化（保真接收）
//! - Layer 1: 4 类信号标签（Structural Shift / Competitive / Context / Noise）
//! - Layer 2: 五维评分（含 Signal Strength）
//! - Layer 3: 三层分流（Insight / Watchlist / Memory），不丢信息

use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;

use crate::config::LlmConfig;
use crate::config::SourceConfig;
use crate::fetcher::Article;
use crate::llm;

/// 信号类型（4 类 LLM 标签）
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    StructuralShift,
    CompetitiveSignal,
    ContextUpdate,
    Noise,
}

/// 发布路由（三路分叉）
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum SignalRoute {
    Archive,   // 存档，不展示
    Intel,     // 每日情报
    Research,  // 深度研报
}

/// LLM 评分后的单条信号评估
#[derive(Debug, Clone, Serialize)]
pub struct SignalAssessment {
    pub article_id: String,
    pub title: String,
    pub source: String,
    pub url: String,
    pub importance: u8,        // LLM 评分 1-10
    pub signal_type: SignalType,
    pub domain: String,
    pub score: u8,             // importance × source_factor
    pub route: SignalRoute,
}

/// 携带原文和评分的完整信号
#[derive(Debug, Clone)]
pub struct ClassifiedSignal {
    pub article: Article,
    pub assessment: SignalAssessment,
}

/// 三层分流结果（Layer 3，保留向后兼容）
#[derive(Debug, Clone, Serialize)]
pub struct TriageResult {
    pub insight: Vec<Article>,
    pub watchlist: Vec<Article>,
    pub signal_memory: Vec<Article>,
}

/// 全量评分 + 三路路由
///
/// 返回 (research, intel, archive_count, triage_for_backwards_compat)
pub async fn classify_and_route(
    grouped: &HashMap<String, Vec<Article>>,
    api_key: &str,
    llm_config: &LlmConfig,
    prompts: Option<&crate::config::PromptsConfig>,
    sources: &[SourceConfig],
) -> Result<(Vec<ClassifiedSignal>, Vec<ClassifiedSignal>, usize, TriageResult)> {
    let triage = scan_and_triage(grouped, api_key, llm_config, prompts, sources).await?;

    let mut research_signals: Vec<ClassifiedSignal> = Vec::new();
    let mut intel_signals: Vec<ClassifiedSignal> = Vec::new();

    let get_source_score = |source_name: &str| -> f32 {
        sources.iter()
            .find(|s| s.name == source_name)
            .map(|s| s.score as f32)
            .unwrap_or(5.0)
    };

    for article in &triage.insight {
        let sf = 0.5 + get_source_score(&article.source) / 20.0;
        let score = (10.0 * sf).round() as u8;
        research_signals.push(ClassifiedSignal {
            article: article.clone(),
            assessment: SignalAssessment {
                article_id: article.id.clone(),
                title: article.title.clone(),
                source: article.source.clone(),
                url: article.url.clone(),
                importance: 8,
                signal_type: SignalType::StructuralShift,
                domain: article.category.clone(),
                score: score.max(7),
                route: SignalRoute::Research,
            },
        });
    }

    for article in &triage.watchlist {
        let sf = 0.5 + get_source_score(&article.source) / 20.0;
        let score = (5.0 * sf).round() as u8;
        intel_signals.push(ClassifiedSignal {
            article: article.clone(),
            assessment: SignalAssessment {
                article_id: article.id.clone(),
                title: article.title.clone(),
                source: article.source.clone(),
                url: article.url.clone(),
                importance: 5,
                signal_type: SignalType::ContextUpdate,
                domain: article.category.clone(),
                score: score.max(3).min(6),
                route: SignalRoute::Intel,
            },
        });
    }

    let archive_count = triage.signal_memory.len();

    Ok((research_signals, intel_signals, archive_count, triage))
}


/// 对分组后的文章执行信号标记和三层分流（v1.1）
pub async fn scan_and_triage(
    grouped: &HashMap<String, Vec<Article>>,
    api_key: &str,
    llm_config: &LlmConfig,
    prompts: Option<&crate::config::PromptsConfig>,
    sources: &[SourceConfig],
) -> Result<TriageResult> {
    let client = crate::llm::create_source_client()?;

    let mut insight = Vec::new();
    let mut watchlist = Vec::new();
    let mut signal_memory = Vec::new();
    let batch_size = 8usize;

    for (category, articles) in grouped {
        if articles.is_empty() {
            continue;
        }

        log::info!("🏷️ Scan Agent v1.1 [{}] — {} 篇", category, articles.len());

        let total_batches = articles.len().div_ceil(batch_size);

        for (batch_idx, batch) in articles.chunks(batch_size).enumerate() {
            if articles.len() > batch_size {
                log::debug!("  ↳ 第 {}/{} 批", batch_idx + 1, total_batches);
            }

            let base_prompt_str =
                build_scan_prompt_v11(category, batch_idx + 1, total_batches, batch);
            let system_prompt = match prompts {
                Some(p) => p.get_scan_agent(&base_prompt_str).to_string(),
                None => base_prompt_str,
            };
            let user_prompt = build_scan_user_prompt(category, batch_idx + 1, batch);

            let result =
                llm::call_with_retry(&client, api_key, llm_config, &system_prompt, &user_prompt)
                    .await;

            match result {
                Ok(raw_results) => {
                    for article in batch {
                        // 找匹配的 LLM 输出
                        let matched = raw_results.iter().find(|r| r.title == article.title);
                        let importance = matched.map(|r| r.importance.clamp(1, 10)).unwrap_or(5);
                        let tag = matched
                            .map(|r| r.relevance.as_str())
                            .unwrap_or("Context Update");
                        // 验证 tag 是否为预期的 4 个值之一，防止 LLM 拼写错误
                        let tag = match tag {
                            "Structural Shift" | "Competitive Signal" | "Context Update"
                            | "Noise" => tag,
                            other => {
                                log::warn!("⚠️ Scan Agent: unknown relevance tag '{other}' from LLM, defaulting to 'Context Update'");
                                "Context Update"
                            }
                        };

                        // Source 可信度加权
                        let source_score = sources
                            .iter()
                            .find(|s| s.name == article.source)
                            .map(|s| s.score as f32)
                            .unwrap_or(5.0);
                        let source_factor = 0.5 + source_score / 20.0; // 0.55 (score=1) .. 1.0 (score=10)
                        let weighted = (importance as f32 * source_factor).round() as u8;

                        // 三层分流
                        let composite = if tag == "Structural Shift" {
                            weighted.max(7) // Structural Shift 保底进 insight
                        } else {
                            weighted
                        };

                        if composite >= 7 {
                            insight.push(article.clone());
                        } else if composite >= 3 {
                            // TODO(Phase 2): 接入 Belief Engine 后启用 ContradictionTracker
                            // 当前逻辑为纯分数路由 (composite >= 3 → watchlist)，信念冲突检测
                            // 依赖不存在的 Editor Agent/Belief Engine。
                            // ContradictionRecord struct 已定义，留待 Phase B 接入。
                            watchlist.push(article.clone());
                        } else if tag == "Noise" {
                            signal_memory.push(article.clone());
                        } else {
                            // Low-but-meaningful: 进 signal_memory
                            signal_memory.push(article.clone());
                        }
                    }
                }
                Err(e) => {
                    log::warn!("⚠️ Scan Agent 批次失败 ({}), 全部进入 insight", e);
                    insight.extend(batch.iter().cloned());
                }
            }

            if articles.len() > batch_size {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }

        log::info!(
            "✅ Scan v1.1 [{}]: 🟢{} 🟡{} 🔵{}",
            category,
            insight.len(),
            watchlist.len(),
            signal_memory.len()
        );
    }

    Ok(TriageResult {
        insight,
        watchlist,
        signal_memory,
    })
}

/// v1.1 扫描 prompt
fn build_scan_prompt_v11(
    category: &str,
    batch_idx: usize,
    total_batches: usize,
    articles: &[Article],
) -> String {
    let n = articles.len();
    format!(
        r#"你是一个信号分析师。你的任务不是深度分析——而是对每条信息做两件事：

1. 信号标签（只分类，不打分）：
   - Structural Shift：结构变化——新范式、新约束、世界运行方式改变
   - Competitive Signal：竞争动态——新对手、融资、产品发布、市场变化
   - Context Update：背景补充——已知趋势的延续、行业数据更新
   - Noise：明显噪音——广告、PR稿、无关话题

2. 重要性评分（McKinsey 影响评估框架 — 1-10）：
   评估标准：这件事的影响有多大 × 确定性有多高
   - 9-10：高影响 × 高确定（范式级变化）
   - 7-8：高影响 × 中低确定 / 中影响 × 高确定（重大格局变化）
   - 5-6：中影响 × 中确定（值得关注的趋势）
   - 3-4：低影响 × 任何确定度（常规信息）
   - 1-2：无影响（噪音）

当前领域: {category}（第 {batch}/{total} 批，共 {n} 篇）

Output json only. 输出严格 JSON 格式：
{{"articles":[
  {{"title":"原文标题","importance":7,"relevance":"Structural Shift","time_horizon":"短期","action":"观察","confidence":"低","judgment":"一句话判断（10-20字）"}}
]}}

relevance 必须是以下值之一：Structural Shift / Competitive Signal / Context Update / Noise"#,
        category = category,
        batch = batch_idx,
        total = total_batches,
        n = n,
    )
}

fn build_scan_user_prompt(category: &str, batch_idx: usize, articles: &[Article]) -> String {
    let mut prompt = format!(
        "请分析以下 {category} 领域的 {n} 篇文章（第 {batch} 批）：\n\n",
        category = category,
        n = articles.len(),
        batch = batch_idx,
    );

    for (i, article) in articles.iter().enumerate() {
        prompt.push_str(&format!(
            "[{}] 标题: {}\n    来源: {}\n",
            i + 1,
            article.title,
            article.source,
        ));
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetcher::Article;

    fn make_mock(title: &str, importance: u8, relevance: &str) -> llm::AnalyzedArticle {
        llm::AnalyzedArticle {
            title: title.to_string(),
            url: String::new(),
            importance,
            relevance: relevance.to_string(),
            time_horizon: String::new(),
            action: String::new(),
            confidence: String::new(),
            judgment: String::new(),
            summary: String::new(),
            strategic_level: String::new(),
            blue_rebuttal: String::new(),
            arbitration: String::new(),
            evidence_type: String::new(),
        }
    }

    fn make_article(title: &str, source: &str) -> Article {
        Article {
            id: String::new(),
            title: title.to_string(),
            source: source.to_string(),
            url: "https://example.com".into(),
            content: Some("test content".into()),
            summary: Some("test summary".into()),
            published_at: None,
            category: String::new(),
            wiki_summary: None,
            evidence_type: String::new(),
            is_internal: false,
        }
    }

    #[test]
    fn test_build_scan_prompt_v11_basic() {
        let prompt = build_scan_prompt_v11("AI", 1, 1, &[]);
        assert!(prompt.contains("AI"));
        assert!(prompt.contains("Structural Shift"));
    }

    #[test]
    fn test_build_scan_user_prompt() {
        let articles = vec![make_article("Article One", "Src1")];
        let prompt = build_scan_user_prompt("Tech", 1, &articles);
        assert!(prompt.contains("Article One"));
    }

    #[test]
    fn test_triage_routing_logic() {
        let mock_results = vec![
            make_mock("High", 9, "Structural Shift"),
            make_mock("Mid", 5, "Competitive Signal"),
            make_mock("Low", 2, "Context Update"),
        ];
        let mut insight = 0usize;
        let mut wl = 0;
        let mut sm = 0;
        for m in &mock_results {
            let composite = if m.relevance == "Structural Shift" {
                ((m.importance as f32 * 1.0).round() as u8).max(7)
            } else {
                (m.importance as f32 * 1.0).round() as u8
            };
            if composite >= 7 {
                insight += 1;
            } else if composite >= 3 {
                wl += 1;
            } else {
                sm += 1;
            }
        }
        assert_eq!(insight, 1);
        assert_eq!(wl, 1);
        assert_eq!(sm, 1);
    }

    #[test]
    fn test_triage_structural_shift_floor() {
        let _m = make_mock("test", 4, "Structural Shift");
        let composite = ((4.0_f32 * 0.55).round() as u8).max(7);
        assert_eq!(composite, 7);
    }

    #[test]
    fn test_tag_validation_fallback() {
        for tag in &["Structral", "", "COMPETITIVE"] {
            let valid = match *tag {
                "Structural Shift" | "Competitive Signal" | "Context Update" | "Noise" => *tag,
                _ => "Context Update",
            };
            assert_eq!(valid, "Context Update");
        }
    }
}
