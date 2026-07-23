//! Signal Strategies preview endpoint.
//! Evaluates a proposed strategy against recent articles and returns
//! matched results so users can see impact before saving.

use crate::{json_err, json_ok};
use rules::{score, ArticleInput, Condition};
use store::{PreviewMatch, PreviewRequest, PreviewResult, Store};
use worker::*;

/// POST /api/strategies/preview
///
/// Accepts a strategy condition + score_delta, evaluates against recent
/// articles, and returns matched items with human-readable match reasons.
pub async fn preview(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let store = Store::new(ctx.env.d1("DB")?);

    let body: PreviewRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return json_err(400, "invalid JSON body"),
    };

    // Parse condition from the incoming JSON
    let condition: Condition = match serde_json::from_value(body.condition.clone()) {
        Ok(c) => c,
        Err(e) => return json_err(400, &format!("invalid condition: {e}")),
    };

    // Build a temporary rule for scoring
    let rule = rules::Rule {
        name: "preview".into(),
        audience_tag: "default".into(),
        condition: condition.clone(),
        score_delta: body.score_delta,
    };

    // Fetch recent articles (max 500, default 100)
    let articles = match store.recent_articles_for_preview(100).await {
        Ok(a) => a,
        Err(e) => return json_err(500, &e.to_string()),
    };

    let total = articles.len() as i64;

    // Build a human-readable match reason from the condition
    let match_reason = describe_condition(&condition);

    let mut matched_items: Vec<PreviewMatch> = Vec::new();
    let mut matched_count: i64 = 0;

    for article in &articles {
        let input = ArticleInput {
            title: &article.title,
            summary: &article.ai_summary,
            feed_url: "", // preview doesn't need feed_url matching
        };
        let result = score(&input, std::slice::from_ref(&rule), "default");
        if result != 0.0 {
            matched_count += 1;
            matched_items.push(PreviewMatch {
                id: article.id,
                title: article.title.clone(),
                url: article.url.clone(),
                published_at: article.published_at,
                feed_name: article.feed_name.clone(),
                score_change: result,
                matched_reason: match_reason.clone(),
            });
        }
    }

    json_ok(serde_json::json!(PreviewResult {
        total,
        matched: matched_count,
        signal_type: body.signal_type,
        items: matched_items,
    }))
}

/// Produce a human-readable description of the condition.
fn describe_condition(condition: &Condition) -> String {
    match condition {
        Condition::KeywordIncludes { field, keyword } => {
            format!("{} contains \"{}\"", field_name(*field), keyword)
        }
        Condition::KeywordExcludes { field, keyword } => {
            format!("{} excludes \"{}\"", field_name(*field), keyword)
        }
        Condition::SourceIn { feed_urls } => {
            if feed_urls.len() == 1 {
                format!("source is {}", feed_urls[0])
            } else {
                format!("source is one of {} feeds", feed_urls.len())
            }
        }
        Condition::All { .. } => "all conditions match".into(),
        Condition::Any { .. } => "any condition matches".into(),
    }
}

fn field_name(f: rules::Field) -> &'static str {
    match f {
        rules::Field::Title => "Title",
        rules::Field::Summary => "Summary",
    }
}
