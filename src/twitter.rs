//! Twitter/X 推文管线
//!
//! 每日简报 → 自动格式化推文 (≤280 chars) → Twitter API v2 推送
//! 失败时不阻塞管线（fire-and-forget）

use crate::domain::theme::{Theme, ThemeAnalysis};
use crate::config::TwitterConfig;

/// 从每日简报生成推文（多条，每条 ≤280 字符）
pub fn format_tweets(themes: &[Theme], analyses: &[ThemeAnalysis]) -> Vec<String> {
    let mut tweets = Vec::new();
    let mut indexed: Vec<(&Theme, &ThemeAnalysis)> = themes.iter().zip(analyses.iter()).collect();
    indexed.sort_by_key(|(_, a)| std::cmp::Reverse(a.signal_strength));

    // 第一条：综合摘要
    if let Some((_, analysis)) = indexed.first() {
        let signal_emoji = if analysis.signal_strength >= 9 {
            "🔴"
        } else if analysis.signal_strength >= 7 {
            "🟠"
        } else {
            "📡"
        };
        let mut tweet = format!("{} {} ", signal_emoji, analysis.bluf);
        let source_count: usize = themes.iter().map(|t| t.articles.len()).sum();
        tweet.push_str(&format!("{} themes · {}", analyses.len(), source_count));
        if tweet.len() > 260 {
            tweet.truncate(257);
            tweet.push_str("...");
        }
        tweets.push(tweet);
    }

    // 后续：每个重要主题一条（最多 5 条）
    for (i, (theme, analysis)) in indexed.iter().enumerate() {
        if i == 0 || analysis.signal_strength < 5 {
            continue;
        }
        if tweets.len() >= 5 {
            break;
        }
        let mut t = format!("{} {}", theme.title, analysis.bluf);
        if t.len() > 260 {
            t.truncate(257);
            t.push_str("...");
        }
        tweets.push(t);
    }

    tweets
}

/// 推送单条推文到 Twitter API v2
pub async fn push_tweet(text: &str, config: &TwitterConfig) {
    if !config.enabled || text.is_empty() {
        return;
    }
    let client = match crate::llm::create_client(15) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("⚠️ Twitter: 无法创建 HTTP client: {}", e);
            return;
        }
    };
    let payload = serde_json::json!({"text": text});
    match client
        .post("https://api.twitter.com/2/tweets")
        .header("Authorization", format!("Bearer {}", config.bearer_token))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => {
            if r.status().is_success() {
                log::info!("🐦 推文发送成功: {} chars", text.len());
            } else {
                log::warn!(
                    "⚠️ Twitter API 错误: {} — {}",
                    r.status(),
                    r.text().await.unwrap_or_default()
                );
            }
        }
        Err(e) => log::warn!("⚠️ Twitter 请求失败: {}", e),
    }
}

/// 发布推文管线
pub async fn publish_tweets(themes: &[Theme], analyses: &[ThemeAnalysis], config: &TwitterConfig) {
    if !config.enabled {
        return;
    }
    let tweets = format_tweets(themes, analyses);
    if tweets.is_empty() {
        log::info!("🐦 今日无推文内容");
        return;
    }
    log::info!("🐦 准备发送 {} 条推文", tweets.len());
    for (i, tweet) in tweets.iter().enumerate() {
        push_tweet(tweet, config).await;
        if i < tweets.len() - 1 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetcher::Article;

    fn make_theme(title: &str) -> Theme {
        Theme {
            id: format!("t-{}", title),
            title: title.to_string(),
            summary: "".into(),
            articles: vec![Article {
                id: String::new(),
                title: "t".into(),
                source: "s".into(),
                url: "".into(),
                content: None,
                summary: None,
                published_at: None,
                category: String::new(),
                wiki_summary: None,
                evidence_type: String::new(),
                is_internal: false,
            }],
            sources: vec!["s".into()],
        }
    }

    fn make_analysis(title: &str, bluf: &str, strength: u8) -> ThemeAnalysis {
        ThemeAnalysis {
            theme_id: format!("t-{}", title),
            theme_title: title.to_string(),
            bluf: bluf.to_string(),
            impact: String::new(),
            geopolitical_fact: String::new(),
            supply_chain_impact: String::new(),
            analysis_paragraph: String::new(),
            evidence_level: String::new(),
            signal_strength: strength,
            fact_base: vec![],
            connections: vec![],
            source_urls: vec![],
            assumptions: vec![],
            adverse: None,
            next_tests: vec![],
            open_questions: vec![],
            chains: vec![],
            what_to_do: String::new(),
            what_to_watch: String::new(),
            falsification_conditions: vec![],
        }
    }

    #[test]
    fn test_empty() {
        assert!(format_tweets(&[], &[]).is_empty());
    }

    #[test]
    fn test_single_tweet() {
        let t = format_tweets(&[make_theme("AI")], &[make_analysis("AI", "important", 7)]);
        assert!(t[0].contains("important"));
    }

    #[test]
    fn test_max_five() {
        let themes: Vec<_> = (0..10).map(|i| make_theme(&format!("T{}", i))).collect();
        let analyses: Vec<_> = (0..10)
            .map(|i| make_analysis(&format!("T{}", i), "b", 7))
            .collect();
        assert!(format_tweets(&themes, &analyses).len() <= 5);
    }
}
