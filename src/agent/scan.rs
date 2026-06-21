//! Scan Agent — 快速初筛模块
//!
//! 在完整 LLM 分析之前对文章做轻量扫描，按重要性评分分级路由：
//! - ≤threshold → 跳过（噪音/广告/无关）
//! - ≥threshold → 保留，进入后续完整分析
//!
//! 设计原则:
//! - 只传标题+来源，不传全文（节省 token）
//! - 使用更短的 max_tokens（仅做"轻与重"二元判断）
//! - 失败时所有文章保留（安全降级）

use anyhow::Result;
use std::collections::HashMap;

use crate::config::LlmConfig;
use crate::fetcher::Article;
use crate::llm;

/// 过滤结果
pub struct FilteredArticles {
    /// 保留的文章（importance >= threshold）
    pub keep: Vec<Article>,
    /// 被过滤的文章（importance < threshold）
    pub filtered_out: Vec<Article>,
}

/// 对分组后的文章执行扫描和过滤
pub async fn scan_and_filter(
    grouped: &HashMap<String, Vec<Article>>,
    threshold: u8,
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<FilteredArticles> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let mut keep = Vec::new();
    let mut filtered_out = Vec::new();
    let batch_size = 8usize;

    for (category, articles) in grouped {
        if articles.is_empty() {
            continue;
        }

        log::info!("🔍 Scan Agent 扫描 [{}] — {} 篇", category, articles.len());

        let total_batches = articles.len().div_ceil(batch_size);

        for (batch_idx, batch) in articles.chunks(batch_size).enumerate() {
            if articles.len() > batch_size {
                log::debug!(
                    "  ↳ 第 {}/{} 批 ({} 篇)",
                    batch_idx + 1,
                    total_batches,
                    batch.len()
                );
            }

            let system_prompt = build_scan_prompt(category, batch_idx + 1, total_batches, batch);
            let user_prompt = build_scan_user_prompt(category, batch_idx + 1, batch);

            // 复用 llm.rs 的 call_with_retry（指数退避 + 4xx 不重试）
            let result =
                llm::call_with_retry(&client, api_key, llm_config, &system_prompt, &user_prompt)
                    .await;

            match result {
                Ok(raw_results) => {
                    // 将每篇扫描结果与原文章匹配，按重要性分级
                    for article in batch {
                        let importance = raw_results
                            .iter()
                            .find(|r| r.title == article.title)
                            .map(|r| r.importance.clamp(1, 10))
                            .unwrap_or(5); // 匹配失败时默认"保留"

                        if importance <= threshold {
                            filtered_out.push(article.clone());
                        } else {
                            keep.push(article.clone());
                        }
                    }
                }
                Err(e) => {
                    // 该批扫描失败 → 全部保留（安全降级）
                    log::warn!(
                        "⚠️ Scan Agent [{}] 第{}批扫描失败: {}，全部保留",
                        category,
                        batch_idx + 1,
                        e
                    );
                    keep.extend(batch.iter().cloned());
                }
            }

            // 批间短间隔
            if articles.len() > batch_size {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }

        log::info!(
            "✅ Scan Agent [{}]: {} 篇保留, {} 篇过滤 (阈值≤{})",
            category,
            keep.len(),
            filtered_out.len(),
            threshold,
        );
    }

    Ok(FilteredArticles { keep, filtered_out })
}

/// 构建 scan system prompt
///
/// 比完整分析短得多，只做"轻与重"的二元判断。
/// 不继承 prompts.base 中的深度判断框架。
fn build_scan_prompt(
    category: &str,
    batch_idx: usize,
    total_batches: usize,
    articles: &[Article],
) -> String {
    let n = articles.len();
    format!(
        r#"你是一个快速扫描员。你的任务不是深度分析——而是快速判断每篇文章对个人创业者来说值不值得进一步分析。

当前领域: {category}（第 {batch}/{total} 批，共 {n} 篇）

对每篇文章输出 JSON 字段:
1. title: 原文标题（按原文原样输出）
2. importance (1-10)：
   - 1-3 = 噪音、广告软文、PR稿、无关话题 → 不值得进一步分析
   - 4-6 = 值得关注但非紧急
   - 7-10 = 重要信号、范式级变化、直接影响个人创业决策
3. judgment: 一句话说明为什么重要或不重要（10-30字）

必填字段（可直接给默认值）:
- relevance: "低"（无需分析，填默认值即可）
- time_horizon: "短期"（无需分析，填默认值即可）
- action: "观察"（无需分析，填默认值即可）
- confidence: "低"（无需分析，填默认值即可）"#,
        category = category,
        batch = batch_idx,
        total = total_batches,
        n = n,
    )
}

/// 构建 scan user prompt
///
/// 只传标题和来源，不传全文（Scan Agent 只看标题级信号）
fn build_scan_user_prompt(category: &str, batch_idx: usize, articles: &[Article]) -> String {
    let mut prompt = format!(
        "请快速扫描以下 {category} 领域的 {n} 篇文章（第 {batch} 批）：\n\n",
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
