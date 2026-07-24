//! Filter/scoring rules engine. Operates purely on already-parsed article
//! text -- no D1, no HTTP -- so it stays reusable regardless of what the
//! storage backend ends up being, and is trivial to unit test.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    KeywordIncludes { field: Field, keyword: String },
    KeywordExcludes { field: Field, keyword: String },
    SourceIn { feed_urls: Vec<String> },
    All { conditions: Vec<Condition> },
    Any { conditions: Vec<Condition> },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Title,
    Summary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    pub audience_tag: String,
    pub condition: Condition,
    /// Score delta applied when the condition matches; negative values
    /// downrank rather than hard-exclude, positive values boost.
    pub score_delta: f64,
}

pub struct ArticleInput<'a> {
    pub title: &'a str,
    pub summary: &'a str,
    pub feed_url: &'a str,
}

fn eval(condition: &Condition, article: &ArticleInput) -> bool {
    match condition {
        Condition::KeywordIncludes { field, keyword } => {
            field_text(*field, article).to_lowercase().contains(&keyword.to_lowercase())
        }
        Condition::KeywordExcludes { field, keyword } => {
            !field_text(*field, article).to_lowercase().contains(&keyword.to_lowercase())
        }
        Condition::SourceIn { feed_urls } => feed_urls.iter().any(|u| u == article.feed_url),
        Condition::All { conditions } => conditions.iter().all(|c| eval(c, article)),
        Condition::Any { conditions } => conditions.iter().any(|c| eval(c, article)),
    }
}

fn field_text<'a>(field: Field, article: &ArticleInput<'a>) -> &'a str {
    match field {
        Field::Title => article.title,
        Field::Summary => article.summary,
    }
}

/// Apply every enabled rule for the given audience and sum up the score
/// deltas. Callers persist the resulting score via `store::set_ai_summary`.
pub fn score(article: &ArticleInput, rules: &[Rule], audience_tag: &str) -> f64 {
    rules
        .iter()
        .filter(|r| r.audience_tag == audience_tag)
        .filter(|r| eval(&r.condition, article))
        .map(|r| r.score_delta)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_include_matches_case_insensitively() {
        let rule = Rule {
            name: "boost macro".into(),
            audience_tag: "default".into(),
            condition: Condition::KeywordIncludes { field: Field::Title, keyword: "macro".into() },
            score_delta: 2.0,
        };
        let article = ArticleInput { title: "China Macro Outlook", summary: "", feed_url: "https://example.com/feed" };
        assert_eq!(score(&article, &[rule], "default"), 2.0);
    }

    #[test]
    fn keyword_exclude_skips_matches() {
        let rule = Rule {
            name: "no politics".into(),
            audience_tag: "default".into(),
            condition: Condition::KeywordExcludes { field: Field::Title, keyword: "politics".into() },
            score_delta: 1.0,
        };
        let article = ArticleInput { title: "Politics Today", summary: "", feed_url: "https://example.com/feed" };
        assert_eq!(score(&article, &[rule], "default"), 0.0);
    }

    #[test]
    fn source_in_matches_feed_url() {
        let rule = Rule {
            name: "only techcrunch".into(),
            audience_tag: "default".into(),
            condition: Condition::SourceIn { feed_urls: vec!["https://techcrunch.com/feed".into()] },
            score_delta: 3.0,
        };
        let article = ArticleInput { title: "A Startup", summary: "", feed_url: "https://techcrunch.com/feed" };
        assert_eq!(score(&article, &[rule], "default"), 3.0);
    }

    #[test]
    fn all_condition_requires_all() {
        let rule = Rule {
            name: "boost macro ai".into(),
            audience_tag: "default".into(),
            condition: Condition::All {
                conditions: vec![
                    Condition::KeywordIncludes { field: Field::Title, keyword: "AI".into() },
                    Condition::KeywordIncludes { field: Field::Title, keyword: "macro".into() },
                ],
            },
            score_delta: 5.0,
        };
        let match_both = ArticleInput { title: "AI Macro Trends", summary: "", feed_url: "https://example.com/feed" };
        let match_one = ArticleInput { title: "AI Daily", summary: "", feed_url: "https://example.com/feed" };
        assert_eq!(score(&match_both, std::slice::from_ref(&rule), "default"), 5.0);
        assert_eq!(score(&match_one, std::slice::from_ref(&rule), "default"), 0.0);
    }

    #[test]
    fn any_condition_matches_partial() {
        let rule = Rule {
            name: "boost AI or crypto".into(),
            audience_tag: "default".into(),
            condition: Condition::Any {
                conditions: vec![
                    Condition::KeywordIncludes { field: Field::Title, keyword: "AI".into() },
                    Condition::KeywordIncludes { field: Field::Title, keyword: "crypto".into() },
                ],
            },
            score_delta: 2.0,
        };
        let article = ArticleInput { title: "Crypto Winter", summary: "", feed_url: "https://example.com/feed" };
        assert_eq!(score(&article, &[rule], "default"), 2.0);
    }

    #[test]
    fn audience_tag_filters_rules() {
        let rule = Rule {
            name: "only for devs".into(),
            audience_tag: "developer".into(),
            condition: Condition::KeywordIncludes { field: Field::Title, keyword: "rust".into() },
            score_delta: 1.0,
        };
        let article = ArticleInput { title: "Rust is Fast", summary: "", feed_url: "https://example.com/feed" };
        assert_eq!(score(&article, std::slice::from_ref(&rule), "default"), 0.0);
        assert_eq!(score(&article, std::slice::from_ref(&rule), "developer"), 1.0);
    }

    #[test]
    fn scores_accumulate() {
        let rules = vec![
            Rule {
                name: "boost macro".into(),
                audience_tag: "default".into(),
                condition: Condition::KeywordIncludes { field: Field::Title, keyword: "macro".into() },
                score_delta: 2.0,
            },
            Rule {
                name: "boost AI".into(),
                audience_tag: "default".into(),
                condition: Condition::KeywordIncludes { field: Field::Title, keyword: "AI".into() },
                score_delta: 3.0,
            },
        ];
        let article = ArticleInput { title: "AI Macro Outlook", summary: "", feed_url: "https://example.com/feed" };
        assert_eq!(score(&article, &rules, "default"), 5.0);
    }

    #[test]
    fn empty_rules_return_zero() {
        let article = ArticleInput { title: "AI breaks records", summary: "", feed_url: "https://example.com" };
        assert_eq!(score(&article, &[], "default"), 0.0);
    }

    #[test]
    fn wrong_audience_tag_returns_zero() {
        let rule = Rule {
            name: "only for dev".into(),
            audience_tag: "developer".into(),
            condition: Condition::KeywordIncludes { field: Field::Title, keyword: "AI".into() },
            score_delta: 5.0,
        };
        let article = ArticleInput { title: "AI revolution", summary: "", feed_url: "https://example.com" };
        assert_eq!(score(&article, &[rule], "investor"), 0.0);
    }

    #[test]
    fn negative_score_delta_downranks() {
        let rule = Rule {
            name: "penalize crypto".into(),
            audience_tag: "default".into(),
            condition: Condition::KeywordIncludes { field: Field::Title, keyword: "crypto".into() },
            score_delta: -2.0,
        };
        let article = ArticleInput { title: "Crypto crash", summary: "", feed_url: "https://example.com" };
        assert_eq!(score(&article, &[rule], "default"), -2.0);
    }

    #[test]
    fn empty_title_still_matches_summary() {
        let rule = Rule {
            name: "boost safety".into(),
            audience_tag: "default".into(),
            condition: Condition::KeywordIncludes { field: Field::Summary, keyword: "safety".into() },
            score_delta: 3.0,
        };
        let article = ArticleInput { title: "", summary: "AI safety is important", feed_url: "https://example.com" };
        assert_eq!(score(&article, &[rule], "default"), 3.0);
    }

    #[test]
    fn summary_not_used_when_field_is_title() {
        let rule = Rule {
            name: "boost title only".into(),
            audience_tag: "default".into(),
            condition: Condition::KeywordIncludes { field: Field::Title, keyword: "safety".into() },
            score_delta: 3.0,
        };
        let article =
            ArticleInput { title: "Other news", summary: "AI safety is important", feed_url: "https://example.com" };
        assert_eq!(score(&article, &[rule], "default"), 0.0);
    }

    #[test]
    fn deep_nested_all_conditions() {
        let rule = Rule {
            name: "boost AI safety research".into(),
            audience_tag: "default".into(),
            condition: Condition::All {
                conditions: vec![
                    Condition::Any {
                        conditions: vec![
                            Condition::KeywordIncludes { field: Field::Title, keyword: "AI".into() },
                            Condition::KeywordIncludes { field: Field::Title, keyword: "machine learning".into() },
                        ],
                    },
                    Condition::KeywordIncludes { field: Field::Title, keyword: "safety".into() },
                ],
            },
            score_delta: 5.0,
        };
        let match_both = ArticleInput { title: "AI safety breakthrough", summary: "", feed_url: "https://example.com" };
        let match_only_ai =
            ArticleInput { title: "AI performance gains", summary: "", feed_url: "https://example.com" };
        assert_eq!(score(&match_both, std::slice::from_ref(&rule), "default"), 5.0);
        assert_eq!(score(&match_only_ai, std::slice::from_ref(&rule), "default"), 0.0);
    }
}
