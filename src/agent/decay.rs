//! Decay Agent (🪦 记忆墓地守护者)
//!
//! 每天运行：埋葬过期旧文章 + 匹配唤醒信号。
//! 不参与清晨的战斗——仓库管理工作。
//!
//! 埋葬条件：
//! 1. 文章超过 retention_days 未被访问
//! 2. 重要性（原始）≤ burial_threshold
//!
//! 唤醒条件：
//! 今日新文章的 title/category 匹配墓地条目的 title

use anyhow::Result;

use crate::config::{GraveyardConfig, LlmConfig};
use crate::db::{BurialEntry, Database, GraveyardEntry};
use crate::fetcher::Article;
use crate::llm;

/// Decay Agent 执行报告
pub struct DecayReport {
    pub buried: usize,
    pub wakeups: Vec<GraveyardEntry>,
    pub compressed: usize,
}

/// 执行记忆墓地维护
pub async fn run_maintenance(
    db: &Database,
    new_articles: &[Article],
    api_key: &str,
    llm_config: &LlmConfig,
    config: &GraveyardConfig,
) -> Result<DecayReport> {
    if !config.enabled {
        return Ok(DecayReport {
            buried: 0,
            wakeups: vec![],
            compressed: 0,
        });
    }

    let mut report = DecayReport {
        buried: 0,
        wakeups: vec![],
        compressed: 0,
    };

    // 1. 查询过期文章
    let expired_ids = db.get_expired_article_ids(config.retention_days)?;
    if expired_ids.is_empty() {
        log::info!("🪦 无过期文章需要处理");
    } else {
        log::info!("🪦 发现 {} 篇过期文章", expired_ids.len());

        // 2. 获取每篇的详情并构建埋葬条目
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()?;

        let mut entries = Vec::new();

        for id in &expired_ids {
            // 获取文章详情
            if let Some((title, category, source, content)) = db.get_article_by_id(id)? {
                // 简单埋葬策略：所有过期文章直接埋葬
                // 未来可升级为 LLM 重新评估
                let compressed = if config.compression && !content.is_empty() {
                    compress_article(&title, &content, &client, api_key, llm_config).await
                } else {
                    String::new()
                };

                if !compressed.is_empty() {
                    report.compressed += 1;
                }

                entries.push(BurialEntry {
                    id: format!("grave_{}", id),
                    article_id: id.clone(),
                    title,
                    category,
                    source,
                    original_importance: 5, // 默认 — 未来从 LLM 分析结果获取
                    compressed_content: compressed,
                    burial_reason: "age".into(),
                });
            }
        }

        // 3. 批量埋葬
        if !entries.is_empty() {
            report.buried = db.bury_articles(&entries)?;
            log::info!(
                "✅ 埋葬完成: {} 篇 (压缩 {} 篇)",
                report.buried,
                report.compressed
            );
        }
    }

    // 4. 唤醒检查：今日新文章匹配墓地
    for article in new_articles {
        let matches = db.search_graveyard(&article.title, &article.category)?;
        for entry in matches {
            if !report.wakeups.iter().any(|w| w.id == entry.id) {
                // log matched entry
                log::info!(
                    "📢 唤醒信号: '{}' → 匹配墓地条目 '{}' ({})",
                    article.title,
                    entry.title,
                    entry.category,
                );
                report.wakeups.push(entry);
            }
        }
    }

    if !report.wakeups.is_empty() {
        log::info!("📢 当日共有 {} 条唤醒信号", report.wakeups.len());
    } else {
        log::info!("🪦 唤醒检查完成: 无匹配");
    }

    Ok(report)
}

/// 用 LLM 压缩单篇文章内容
///
/// 复用 call_with_retry 做压缩，将冗长正文压缩为 3-5 句话。
/// 压缩内容保存在 judgment 字段中返回。
async fn compress_article(
    title: &str,
    content: &str,
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
) -> String {
    let system_prompt = "你是一个信息压缩师。将以下文章压缩为 3-5 句话，\
        保留关键信号、数字、结论和行动建议。\
        不要加自己的评论，只做压缩。\
        Output json. 输出严格JSON：{\"articles\":[{\"title\":\"原文标题\",\"importance\":5,\"relevance\":\"高\",\"time_horizon\":\"短期\",\"action\":\"观察\",\"confidence\":\"中\",\"judgment\":\"压缩后的 3-5 句话\"}]}";

    let user_prompt = format!(
        "请压缩以下文章：\n\n标题: {}\n\n正文:\n{}",
        title,
        if content.len() > 3000 {
            let end = content.floor_char_boundary(3000);
            format!("{}...", &content[..end])
        } else {
            content.to_string()
        }
    );

    match llm::call_with_retry(client, api_key, llm_config, system_prompt, &user_prompt).await {
        Ok(results) => results
            .first()
            .map(|r| {
                let compressed = r.judgment.clone();
                if compressed.len() > 300 {
                    let end = compressed.floor_char_boundary(300);
                    format!("{}...", &compressed[..end])
                } else {
                    compressed
                }
            })
            .unwrap_or_default(),
        Err(e) => {
            log::warn!("⚠️ 压缩失败 ({})，跳过压缩", e);
            String::new()
        }
    }
}
