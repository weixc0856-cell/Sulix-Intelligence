//! Reddit 数据源适配器
//!
//! 通过 Reddit JSON API 抓取热门帖子。
//! 使用 `https://www.reddit.com/r/{subreddit}/hot.json` 端点。
//! 不需要 API Key（公共 API 免费）。

use anyhow::Result;
use chrono::{DateTime, FixedOffset, Utc};

use crate::config::SourceConfig;
use crate::source::RawSignal;

/// 从 Reddit JSON API 抓取热门帖子
pub async fn fetch_reddit(config: &SourceConfig, _date_range: &str) -> Result<Vec<RawSignal>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("SulixIntel/3.0 (Reddit Source Adapter)")
        .build()?;

    // 从 URL 解析 subreddit（config.url 格式: https://www.reddit.com/r/{subreddit}/）
    let url = config.url.trim_end_matches('/');
    let api_url = format!("{}/hot.json?limit=25", url);

    let resp = client.get(&api_url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Reddit API HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.json().await?;
    let children = body["data"]["children"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Reddit API: 无法解析 children"))?;

    let mut signals = Vec::new();
    for child in children {
        let data = &child["data"];
        let title = data["title"].as_str().unwrap_or("").to_string();
        if title.is_empty() {
            continue;
        }

        let url = data["url"].as_str().unwrap_or("").to_string();
        let permalink = data["permalink"].as_str().unwrap_or("");
        let reddit_url = format!("https://www.reddit.com{}", permalink);

        let selftext = data["selftext"].as_str().unwrap_or("").to_string();
        let score = data["score"].as_i64().unwrap_or(0);
        let num_comments = data["num_comments"].as_i64().unwrap_or(0);
        let created_utc = data["created_utc"].as_f64().unwrap_or(0.0);
        let created = DateTime::from_timestamp(created_utc as i64, 0)
            .unwrap_or_else(Utc::now)
            .with_timezone(&FixedOffset::east_opt(0).unwrap());

        // 跳过分数过低的帖子
        if score < 5 {
            continue;
        }

        // 构建摘要：取 selftext 前 200 字符
        let summary = if selftext.len() > 200 {
            format!(
                "{}... (score: {}, comments: {})",
                &selftext[..200],
                score,
                num_comments
            )
        } else if !selftext.is_empty() {
            format!(
                "{} (score: {}, comments: {})",
                selftext, score, num_comments
            )
        } else {
            format!("Score: {}, Comments: {}", score, num_comments)
        };

        let id = format!("reddit-{}", data["id"].as_str().unwrap_or("unknown"));

        signals.push(RawSignal {
            id,
            title,
            url: if url.starts_with("http") {
                url
            } else {
                reddit_url
            },
            content: Some(selftext),
            summary: Some(summary),
            published_at: Some(created),
            source: config.name.clone(),
            source_id: format!("reddit-{}", config.name),
            category: config.category.clone(),
            metrics: None,
            requires_sanitization: false,
            is_internal: false,
        });
    }

    Ok(signals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reddit_source_config() {
        let config = SourceConfig {
            name: "r/MachineLearning".into(),
            id: None,
            url: "https://www.reddit.com/r/MachineLearning/".into(),
            score: 5,
            enabled: true,
            layer: 3,
            source_type: "reddit".into(),
            category: "AI".into(),
            keywords: None,
            exclude_keywords: None,
            public: true,
        };
        assert_eq!(config.source_type, "reddit");
        assert_eq!(config.category, "AI");
    }
}
