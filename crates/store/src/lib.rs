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
pub use backend::StoreBackend;

pub mod domain;

use async_trait::async_trait;
use worker::D1Database;

/// Production D1-backed store.
pub struct D1Store {
    db: D1Database,
}

/// Backward-compatible alias.
pub type Store = D1Store;

// ---- Pure helper functions (extracted for testability) ----

/// Generate `?1,?2,?3` placeholders for SQL `IN` clauses.
pub(crate) fn in_placeholders(count: usize) -> String {
    (1..=count).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",")
}

/// Build a SQL LIKE pattern that matches a JSON-stringified tag: `%"tag"%`.
pub(crate) fn tag_like_pattern(tag: &str) -> String {
    format!("%\"{}\"%", tag)
}

/// Check whether a cron last-run timestamp is within 3600 seconds of `now`.
pub(crate) fn is_cron_healthy(last_run_at: Option<i64>, now: i64) -> bool {
    last_run_at.is_some_and(|ts| now - ts < 3600)
}

impl D1Store {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }
}

// ---- StoreBackend impl (delegates to D1Store methods) ----

#[async_trait(?Send)]
impl StoreBackend for D1Store {
    async fn get_feed(&self, id: i64) -> Result<Option<Feed>, StoreError> {
        D1Store::get_feed(self, id).await
    }

    async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError> {
        D1Store::record_fetch_result(self, feed_id, fetched_at, etag, last_modified).await
    }

    async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError> {
        D1Store::active_rule_jsons(self, audience_tag).await
    }

    async fn insert_article(&self, article: &NewArticle) -> Result<Option<i64>, StoreError> {
        D1Store::insert_article(self, article).await
    }

    async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        tags_json: &str,
        vector_id: &str,
        score: f64,
    ) -> Result<(), StoreError> {
        D1Store::set_ai_summary(self, article_id, summary, tags_json, vector_id, score).await
    }

    async fn set_raw_content_r2_key(&self, article_id: i64, r2_key: Option<&str>) -> Result<(), StoreError> {
        D1Store::set_raw_content_r2_key(self, article_id, r2_key).await
    }

    async fn expire_old_articles(&self, now: i64, days: i64) -> Result<u64, StoreError> {
        D1Store::expire_old_articles(self, now, days).await
    }

    async fn upsert_entity(&self, name: &str, normalized: &str, entity_type: &str) -> Result<i64, StoreError> {
        D1Store::upsert_entity(self, name, normalized, entity_type).await
    }

    async fn link_article_entity(
        &self,
        article_id: i64,
        entity_id: i64,
        relevance: f64,
        context: Option<&str>,
    ) -> Result<(), StoreError> {
        D1Store::link_article_entity(self, article_id, entity_id, relevance, context).await
    }

    async fn link_entity_relation(
        &self,
        source: i64,
        target: i64,
        rtype: &str,
        confidence: f64,
    ) -> Result<(), StoreError> {
        D1Store::link_entity_relation(self, source, target, rtype, confidence).await
    }

    async fn list_entities(&self, limit: u32, offset: u32) -> Result<Vec<EntitySummary>, StoreError> {
        D1Store::list_entities(self, limit, offset).await
    }

    async fn entity_detail(&self, id: i64) -> Result<Option<EntityDetail>, StoreError> {
        D1Store::entity_detail(self, id).await
    }

    async fn entity_relations(&self, entity_id: i64, limit: u32) -> Result<Vec<RelatedEntity>, StoreError> {
        D1Store::entity_relations(self, entity_id, limit).await
    }

    async fn article_entities(&self, article_id: i64) -> Result<Vec<EntityRef>, StoreError> {
        D1Store::article_entities(self, article_id).await
    }

    async fn create_artifact(&self, artifact: &NewArtifact) -> Result<i64, StoreError> {
        D1Store::create_artifact(self, artifact).await
    }

    async fn list_artifacts_by_entity(&self, entity_id: i64, limit: u32) -> Result<Vec<ArtifactEntry>, StoreError> {
        D1Store::list_artifacts_by_entity(self, entity_id, limit).await
    }

    async fn entity_articles(&self, entity_id: i64, limit: u32, offset: u32) -> Result<Vec<EntityArticle>, StoreError> {
        D1Store::entity_articles(self, entity_id, limit, offset).await
    }

    async fn entity_activity_summary(
        &self,
        entity_id: i64,
        now: i64,
        days: i64,
    ) -> Result<EntityActivitySummary, StoreError> {
        D1Store::entity_activity_summary(self, entity_id, now, days).await
    }

    async fn entity_signal_candidates(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError> {
        D1Store::entity_signal_candidates(self, now, days, limit).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_signal(
        &self,
        entity_id: Option<i64>,
        title: &str,
        summary: &str,
        confidence: f64,
        impact: &str,
        trend: &str,
        article_count: i64,
        source_count: i64,
        evidence_ids: &[i64],
        related_ids: &[i64],
    ) -> Result<i64, StoreError> {
        D1Store::save_signal(
            self,
            entity_id,
            title,
            summary,
            confidence,
            impact,
            trend,
            article_count,
            source_count,
            evidence_ids,
            related_ids,
        )
        .await
    }

    async fn load_recent_signals(&self, limit: u32, offset: u32) -> Result<Vec<IntelligenceSignal>, StoreError> {
        D1Store::load_recent_signals(self, limit, offset).await
    }

    async fn load_signal_by_id(&self, id: i64) -> Result<Option<IntelligenceSignal>, StoreError> {
        D1Store::load_signal_by_id(self, id).await
    }

    async fn entity_signals(&self, entity_id: i64, limit: u32) -> Result<Vec<IntelligenceSignal>, StoreError> {
        D1Store::entity_signals(self, entity_id, limit).await
    }

    async fn upsert_signal_thread(
        &self,
        signal_key: &str,
        anchor_entity_id: Option<i64>,
        title: &str,
        status: &str,
        discovery_method: &DiscoveryMethod,
        discovery_score: Option<f64>,
    ) -> Result<i64, StoreError> {
        D1Store::upsert_signal_thread(
            self,
            signal_key,
            anchor_entity_id,
            title,
            status,
            discovery_method,
            discovery_score,
        )
        .await
    }

    async fn append_signal_instance(
        &self,
        thread_id: i64,
        confidence: f64,
        impact: &str,
        trend: &str,
        article_count: i64,
        source_count: i64,
    ) -> Result<i64, StoreError> {
        D1Store::append_signal_instance(self, thread_id, confidence, impact, trend, article_count, source_count).await
    }

    async fn update_signal_lifecycle(&self, now: i64) -> Result<(), StoreError> {
        D1Store::update_signal_lifecycle(self, now).await
    }

    async fn get_active_signal_threads(&self, limit: u32) -> Result<Vec<SignalBriefInput>, StoreError> {
        D1Store::get_active_signal_threads(self, limit).await
    }

    async fn list_signal_threads(&self, filter: &SignalThreadFilter) -> Result<Vec<SignalBriefInput>, StoreError> {
        D1Store::list_signal_threads(self, filter).await
    }

    async fn load_signal_detail(&self, thread_id: i64) -> Result<Option<SignalDetail>, StoreError> {
        D1Store::load_signal_detail(self, thread_id).await
    }

    async fn entity_signal_candidates_filtered(
        &self,
        now: i64,
        days: i64,
        limit: u32,
        min_entity_articles: u32,
        min_sources: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError> {
        D1Store::entity_signal_candidates_filtered(self, now, days, limit, min_entity_articles, min_sources).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn append_signal_instance_v2(
        &self,
        thread_id: i64,
        score: f64,
        impact: &str,
        trend: &str,
        article_count: i64,
        source_count: i64,
        avg_score: f64,
        entity_id: i64,
    ) -> Result<i64, StoreError> {
        D1Store::append_signal_instance_v2(
            self,
            thread_id,
            score,
            impact,
            trend,
            article_count,
            source_count,
            avg_score,
            entity_id,
        )
        .await
    }

    async fn insert_signal_event(
        &self,
        thread_id: i64,
        event_type: &str,
        payload: Option<&str>,
    ) -> Result<(), StoreError> {
        D1Store::insert_signal_event(self, thread_id, event_type, payload).await
    }

    async fn load_signal_events(&self, thread_id: i64, limit: u32) -> Result<Vec<SignalEvent>, StoreError> {
        D1Store::load_signal_events(self, thread_id, limit).await
    }

    async fn load_thread_related_entities(
        &self,
        thread_id: i64,
        limit: u32,
    ) -> Result<Vec<RelatedEntityRef>, StoreError> {
        D1Store::load_thread_related_entities(self, thread_id, limit).await
    }

    async fn create_decision(&self, d: &NewDecision) -> Result<i64, StoreError> {
        D1Store::create_decision(self, d).await
    }

    async fn get_decision(&self, id: i64) -> Result<Option<Decision>, StoreError> {
        D1Store::get_decision(self, id).await
    }

    async fn list_decisions(&self, status: Option<&str>, limit: u32) -> Result<Vec<Decision>, StoreError> {
        D1Store::list_decisions(self, status, limit).await
    }

    async fn update_decision_status(&self, id: i64, status: &str) -> Result<(), StoreError> {
        D1Store::update_decision_status(self, id, status).await
    }

    async fn decisions_by_signal(&self, signal_thread_id: i64) -> Result<Vec<Decision>, StoreError> {
        D1Store::decisions_by_signal(self, signal_thread_id).await
    }

    async fn create_outcome(&self, e: &NewOutcomeEvent) -> Result<i64, StoreError> {
        D1Store::create_outcome(self, e).await
    }

    async fn get_decision_outcomes(&self, decision_id: i64) -> Result<Vec<OutcomeEvent>, StoreError> {
        D1Store::get_decision_outcomes(self, decision_id).await
    }

    async fn create_evaluation(&self, e: &NewDecisionEvaluation) -> Result<i64, StoreError> {
        D1Store::create_evaluation(self, e).await
    }

    async fn get_decision_evaluations(&self, decision_id: i64) -> Result<Vec<DecisionEvaluation>, StoreError> {
        D1Store::get_decision_evaluations(self, decision_id).await
    }

    async fn get_latest_evaluation(&self, decision_id: i64) -> Result<Option<DecisionEvaluation>, StoreError> {
        D1Store::get_latest_evaluation(self, decision_id).await
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
        let id = futures::executor::block_on(store.insert_article(&article)).unwrap();
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
        let id1 = futures::executor::block_on(store.insert_article(&article)).unwrap();
        assert!(id1.is_some());
        let id2 = futures::executor::block_on(store.insert_article(&article)).unwrap();
        assert!(id2.is_none(), "duplicate should return None");
    }

    #[test]
    fn mem_store_set_ai_summary() {
        let store = memory::MemoryStore::new();
        let result =
            futures::executor::block_on(store.set_ai_summary(42, "AI summary text", "[\"tag1\"]", "vec-42", 8.5));
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
        let result = futures::executor::block_on(store.insert_article(&article));
        assert!(result.is_err());
    }

    #[test]
    fn mem_store_returns_err_on_fail_rules() {
        let mut store = memory::MemoryStore::new();
        store.fail_rules = true;
        let result = futures::executor::block_on(store.active_rule_jsons("default"));
        assert!(result.is_err());
    }

    #[test]
    fn mem_store_returns_err_on_fail_fetch_result() {
        let mut store = memory::MemoryStore::new();
        store.fail_fetch_result = true;
        let result = futures::executor::block_on(store.record_fetch_result(1, 0, None, None));
        assert!(result.is_err());
    }

    #[test]
    fn mem_store_returns_err_on_fail_summary() {
        let mut store = memory::MemoryStore::new();
        store.fail_summary = true;
        let result = futures::executor::block_on(store.set_ai_summary(1, "summary", "[]", "vec-1", 0.0));
        assert!(result.is_err());
    }
}
