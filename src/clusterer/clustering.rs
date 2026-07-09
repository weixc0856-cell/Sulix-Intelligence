//! Clustering — 将文章聚类为主题
//!
//! 从 clusterer.rs 拆分，依赖 `crate::clusterer::Theme` 共享类型。

use anyhow::Result;

use crate::domain::theme::Theme;
use crate::config::LlmConfig;
use crate::fetcher::Article;
use crate::llm;

/// 将文章聚类为主题
pub async fn cluster_articles(
    articles: &[Article],
    api_key: &str,
    llm_config: &LlmConfig,
) -> Result<Vec<Theme>> {
    if articles.is_empty() {
        return Ok(Vec::new());
    }

    let client = crate::llm::create_client(120)?;

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
