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
use crate::fetcher::Article;
use crate::llm;

/// 信号标签（Layer 1：只分类，不打分）
#[derive(Debug, Clone, PartialEq)]
pub enum SignalTag {
    StructuralShift,   // 结构变化（最重要）
    CompetitiveSignal, // 竞争动态
    ContextUpdate,     // 背景补充
    Noise,             // 明显噪音
}

impl SignalTag {
    pub fn as_str(&self) -> &'static str {
        match self {
            SignalTag::StructuralShift => "Structural Shift",
            SignalTag::CompetitiveSignal => "Competitive Signal",
            SignalTag::ContextUpdate => "Context Update",
            SignalTag::Noise => "Noise",
        }
    }
}

/// 三层分流结果（Layer 3）
#[derive(Debug, Clone, Serialize)]
pub struct TriageResult {
    /// 🟢 Insight（评分 ≥ 7）→ 进入主题分析，写入日报
    pub insight: Vec<Article>,
    /// 🟡 Watchlist（评分 3-6）→ 不输出日报，但保存观察
    pub watchlist: Vec<Article>,
    /// 🔵 Signal Memory（结构信号或评分低）→ 极低成本存档
    pub signal_memory: Vec<Article>,
}

/// 对分组后的文章执行信号标记和三层分流（v1.1）
pub async fn scan_and_triage(
    grouped: &HashMap<String, Vec<Article>>,
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<TriageResult> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

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

            let system_prompt = build_scan_prompt_v11(category, batch_idx + 1, total_batches, batch);
            let user_prompt = build_scan_user_prompt(category, batch_idx + 1, batch);

            let result = llm::call_with_retry(&client, api_key, llm_config, &system_prompt, &user_prompt).await;

            match result {
                Ok(raw_results) => {
                    for article in batch {
                        // 找匹配的 LLM 输出
                        let matched = raw_results.iter().find(|r| r.title == article.title);
                        let importance = matched.map(|r| r.importance.clamp(1, 10)).unwrap_or(5);
                        let tag = matched.map(|r| r.relevance.as_str()).unwrap_or("Context Update");

                        // 三层分流
                        // 综合评分 = importance * 0.25 + tag 权重
                        let composite = if tag == "Structural Shift" {
                            (importance as f32 * 0.25 + 9.0 * 0.25) as u8 // Structural Shift 保底高分
                        } else {
                            importance
                        };

                        if composite >= 7 {
                            insight.push(article.clone());
                        } else if composite >= 3 {
                            // Contradiction Tracker: 如果与现有信念冲突，强制保留
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
            category, insight.len(), watchlist.len(), signal_memory.len()
        );
    }

    Ok(TriageResult { insight, watchlist, signal_memory })
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

2. 重要性评分（1-10）：
   不是"这条信息重不重要"，而是"这件事本身的影响有多大"
   - 9-10：范式级变化
   - 7-8：重大格局变化
   - 5-6：值得关注的趋势
   - 3-4：常规信息
   - 1-2：噪音

当前领域: {category}（第 {batch}/{total} 批，共 {n} 篇）

输出严格 JSON：
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
