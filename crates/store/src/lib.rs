//! D1 access layer.  Every other crate (rules, ai-pipeline, search, api)
//! talks to storage only through this crate, so backend swaps never leak
//! into business logic.
//!
//! Type definitions live in [`models`] and are re-exported from the crate
//! root so callers write `store::Feed` / `store::StoreError` etc.

pub mod models;
pub use models::*;

pub mod backend;
pub mod memory;
pub mod traits;
pub use backend::StoreBackend;
pub use traits::*;

pub mod domain;

pub use domain::decision::record_crud::{NewDecisionRecord, NewOutcome};

mod d1_delegate;

use std::sync::Arc;

use worker::D1Database;

/// Production D1-backed store.
#[derive(Clone)]
pub struct D1Store {
    pub(crate) db: Arc<D1Database>,
}

/// Backward-compatible alias.
pub type Store = D1Store;

// ---- Pure helper functions (extracted for testability) ----

/// Generate `?1,?2,?3` placeholders for SQL `IN` clauses.
pub(crate) fn in_placeholders(count: usize) -> String {
    (1..=count).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",")
}

/// Escape SQL LIKE wildcards (`%`, `_`) and the escape character (`\`) itself.
/// Must be applied in this order: `\` first, then `%`, then `_`.
pub(crate) fn escape_like(input: &str) -> String {
    input.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

/// Build a SQL LIKE pattern that matches a JSON-stringified tag: `%"tag"%`.
/// Uses `ESCAPE '\'` semantics — caller must add the ESCAPE clause to the SQL.
pub(crate) fn tag_like_pattern(tag: &str) -> String {
    format!("%\"{}\"%", escape_like(tag))
}

/// Check whether a cron last-run timestamp is within 3600 seconds of `now`.
pub(crate) fn is_cron_healthy(last_run_at: Option<i64>, now: i64) -> bool {
    last_run_at.is_some_and(|ts| now - ts < 3600)
}

impl D1Store {
    pub fn new(db: D1Database) -> Self {
        Self { db: Arc::new(db) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- in_placeholders --

    #[test]
    fn in_placeholders_three() {
        assert_eq!(in_placeholders(3), "?1,?2,?3");
    }
    #[test]
    fn in_placeholders_one() {
        assert_eq!(in_placeholders(1), "?1");
    }
    #[test]
    fn in_placeholders_zero() {
        assert_eq!(in_placeholders(0), "");
    }

    // -- tag_like_pattern --

    #[test]
    fn tag_like_pattern_simple() {
        assert_eq!(tag_like_pattern("AI"), r#"%"AI"%"#);
    }
    #[test]
    fn tag_like_pattern_empty() {
        assert_eq!(tag_like_pattern(""), r#"%""%"#);
    }

    // -- escape_like --

    #[test]
    fn escape_like_normal() {
        assert_eq!(escape_like("rust"), "rust");
    }

    #[test]
    fn escape_like_percent() {
        assert_eq!(escape_like("100%"), r"100\%");
    }

    #[test]
    fn escape_like_underscore() {
        assert_eq!(escape_like("GPT_5"), r"GPT\_5");
    }

    #[test]
    fn escape_like_all_wildcards() {
        assert_eq!(escape_like(r"a%b_c"), r"a\%b\_c");
    }

    // -- is_cron_healthy --

    #[test]
    fn cron_healthy_recent() {
        assert!(is_cron_healthy(Some(1000), 3599));
    }
    #[test]
    fn cron_healthy_exact_boundary() {
        assert!(!is_cron_healthy(Some(1000), 4600));
    }
    #[test]
    fn cron_healthy_never() {
        assert!(!is_cron_healthy(None, 1000));
    }

    // -- MemoryStore integration tests --

    #[test]
    fn mem_store_loads_active_rules() {
        let store = memory::MemoryStore::new().with_rules(vec!["{\"score\":10}".into()]);
        let rules = futures::executor::block_on(store.active_rule_jsons("default")).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0], "{\"score\":10}");
    }

    #[test]
    fn mem_store_active_rules_empty_when_none() {
        let store = memory::MemoryStore::new();
        let rules = futures::executor::block_on(store.active_rule_jsons("default")).unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn mem_store_inserts_article() {
        let store = memory::MemoryStore::new();
        let article = NewArticle {
            feed_id: 1,
            guid: "guid-1".into(),
            title: "Test".into(),
            url: None,
            published_at: None,
            raw_content_r2_key: None,
        };
        let id = futures::executor::block_on(store.save_article(&article)).unwrap();
        assert!(id.is_some());
    }

    #[test]
    fn mem_store_dedup_article() {
        let store = memory::MemoryStore::new();
        let article = NewArticle {
            feed_id: 1,
            guid: "dup-guid".into(),
            title: "Original".into(),
            url: None,
            published_at: None,
            raw_content_r2_key: None,
        };
        let id1 = futures::executor::block_on(store.save_article(&article)).unwrap();
        assert!(id1.is_some());
        let id2 = futures::executor::block_on(store.save_article(&article)).unwrap();
        assert!(id2.is_none(), "duplicate should return None");
    }

    #[test]
    fn mem_store_set_ai_summary() {
        let store = memory::MemoryStore::new();
        let result = futures::executor::block_on(store.set_ai_summary(42, "summary", "[\"tag1\"]", "vec-42", 8.5));
        assert!(result.is_ok());
    }

    #[test]
    fn mem_store_record_fetch_result() {
        let store = memory::MemoryStore::new();
        let result =
            futures::executor::block_on(store.record_fetch_result(1, 1000, Some("etag-x"), Some("modified-y")));
        assert!(result.is_ok());
        assert_eq!(store.fetch_results.borrow().len(), 1);
        let (fid, _, e, lm) = store.fetch_results.borrow().first().unwrap().clone();
        assert_eq!(fid, 1);
        assert_eq!(e, Some("etag-x".into()));
        assert_eq!(lm, Some("modified-y".into()));
    }

    #[test]
    fn mem_store_returns_err_on_fail_insert() {
        let mut store = memory::MemoryStore::new();
        store.fail_insert = true;
        let article = NewArticle {
            feed_id: 1,
            guid: "err-test".into(),
            title: "Err".into(),
            url: None,
            published_at: None,
            raw_content_r2_key: None,
        };
        assert!(futures::executor::block_on(store.save_article(&article)).is_err());
    }

    #[test]
    fn mem_store_returns_err_on_fail_rules() {
        let mut store = memory::MemoryStore::new();
        store.fail_rules = true;
        assert!(futures::executor::block_on(store.active_rule_jsons("default")).is_err());
    }

    #[test]
    fn mem_store_returns_err_on_fail_fetch_result() {
        let mut store = memory::MemoryStore::new();
        store.fail_fetch_result = true;
        assert!(futures::executor::block_on(store.record_fetch_result(1, 0, None, None)).is_err());
    }

    #[test]
    fn mem_store_returns_err_on_fail_summary() {
        let mut store = memory::MemoryStore::new();
        store.fail_summary = true;
        assert!(futures::executor::block_on(store.set_ai_summary(1, "summary", "[]", "vec-1", 0.0)).is_err());
    }

    // -- Confidence Event (Sprint 5.4B) --

    #[test]
    fn confidence_history_append_and_list() {
        let store = memory::MemoryStore::new();
        let e = NewConfidenceEvent {
            entity_type: "decision".into(),
            entity_id: "DEC-001".into(),
            confidence: 0.85,
            reason: Some("initial assessment".into()),
            trigger_event: None,
            factors_json: None,
        };
        let id = futures::executor::block_on(ConfidenceRepository::append_confidence(&store, &e)).unwrap();
        assert_eq!(id, 1);

        let history =
            futures::executor::block_on(ConfidenceRepository::list_confidence_history(&store, "decision", "DEC-001"))
                .unwrap();
        assert_eq!(history.len(), 1);
        assert!((history[0].confidence - 0.85).abs() < f64::EPSILON);
        assert_eq!(history[0].previous_confidence, None);
    }
}
