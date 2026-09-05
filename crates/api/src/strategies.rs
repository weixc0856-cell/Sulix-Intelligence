//! Signal Strategies preview endpoint.
//! Evaluates a proposed strategy against recent articles and returns
//! matched results so users can see impact before saving.

use crate::{json_err, json_ok};
use application::StrategyPreviewService;
use rules::{score, ArticleInput, Condition};
use store::{PreviewRequest, Store};
use worker::*;

/// POST /api/strategies/preview
///
/// Accepts a strategy condition + score_delta, evaluates against recent
/// articles, and returns matched items with human-readable match reasons.
///
/// The temporary `rules::Rule` construction and the per-article `rules::score`
/// invocation stay here (this layer owns the `rules` dependency); the
/// candidate fetch and the match/filter assembly run in
/// [`StrategyPreviewService`].
pub async fn preview(mut req: Request, ctx: RouteContext<Store>) -> Result<Response> {
    let service = StrategyPreviewService::new(ctx.data.clone());

    let body: PreviewRequest = match req.json().await {
        Ok(b) => b,
        Err(_) => return json_err(400, "invalid JSON body"),
    };

    // Parse condition from the incoming JSON
    let condition: Condition = match serde_json::from_value(body.condition.clone()) {
        Ok(c) => c,
        Err(e) => return json_err(400, &format!("invalid condition: {e}")),
    };

    // Build a temporary rule for scoring (preview doesn't persist a rule)
    let rule = rules::Rule {
        name: "preview".into(),
        audience_tag: "default".into(),
        condition: condition.clone(),
        score_delta: body.score_delta,
    };

    // Build a human-readable match reason from the condition
    let match_reason = describe_condition(&condition);

    // Fetch recent articles and score them (max 500, default 100)
    let result = service
        .preview(100, body.signal_type, match_reason, |article| {
            let input = ArticleInput {
                title: &article.title,
                summary: &article.ai_summary,
                feed_url: "", // preview doesn't need feed_url matching
            };
            score(&input, std::slice::from_ref(&rule), "default")
        })
        .await;

    match result {
        Ok(preview) => json_ok(serde_json::json!(preview)),
        Err(e) => crate::json_err_internal(&e.to_string()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_name_title() {
        assert_eq!(field_name(rules::Field::Title), "Title");
    }
    #[test]
    fn field_name_summary() {
        assert_eq!(field_name(rules::Field::Summary), "Summary");
    }

    #[test]
    fn describe_keyword_includes() {
        let c = Condition::KeywordIncludes { field: rules::Field::Title, keyword: "AI".into() };
        let desc = describe_condition(&c);
        assert_eq!(desc, r#"Title contains "AI""#);
    }

    #[test]
    fn describe_keyword_excludes() {
        let c = Condition::KeywordExcludes { field: rules::Field::Summary, keyword: "crypto".into() };
        let desc = describe_condition(&c);
        assert_eq!(desc, r#"Summary excludes "crypto""#);
    }

    #[test]
    fn describe_source_in_single() {
        let c = Condition::SourceIn { feed_urls: vec!["https://example.com/feed".into()] };
        let desc = describe_condition(&c);
        assert_eq!(desc, "source is https://example.com/feed");
    }

    #[test]
    fn describe_source_in_multiple() {
        let c = Condition::SourceIn { feed_urls: vec!["a".into(), "b".into()] };
        let desc = describe_condition(&c);
        assert_eq!(desc, "source is one of 2 feeds");
    }

    #[test]
    fn describe_all() {
        let c = Condition::All { conditions: vec![] };
        assert_eq!(describe_condition(&c), "all conditions match");
    }

    #[test]
    fn describe_any() {
        let c = Condition::Any { conditions: vec![] };
        assert_eq!(describe_condition(&c), "any condition matches");
    }
}
