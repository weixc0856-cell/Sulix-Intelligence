//! LLM 预去重 — 聚类前语义去重
//!
//! 从 clusterer.rs 拆分。

use anyhow::Result;

use crate::fetcher::Article;

/// LLM 预去重 prompt 结构
const PREDEDUP_SYSTEM_PROMPT: &str = r#"你是新闻去重专家。判断哪些文章在报道同一事件。
输出JSON: {"keep": [保留的文章序号], "merge_groups": [[同一事件的文章序号组]]}
只返回JSON，不要解释。"#;

/// 在聚类前对文章做 LLM 语义去重
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
        .map(|p| p.get_cluster_articles(PREDEDUP_SYSTEM_PROMPT))
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

                    let dedup: DedupOutput = match serde_json::from_str(clean) {
                        Ok(d) => d,
                        Err(e) => {
                            log::warn!(
                                "LLM 去重 JSON 解析失败 (chunk of {} articles): {}",
                                chunk.len(),
                                e
                            );
                            DedupOutput::default()
                        }
                    };

                    let mut keep_indices: std::collections::HashSet<usize> =
                        dedup.keep.into_iter().collect();
                    for group in &dedup.merge_groups {
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
                    result.extend(chunk.iter().cloned());
                }
            }
        }
    }
    Ok(result)
}
