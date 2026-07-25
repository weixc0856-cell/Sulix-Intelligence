//! `impl StoreBackend for D1Store` — pure 1:1 delegation to D1Store domain methods.
//!
//! This is a mechanical anti-corruption layer between the `StoreBackend` trait
//! (port) and `D1Store` (adapter).  Every method delegates to the corresponding
//! `D1Store::method()` directly — no additional logic.

use async_trait::async_trait;

use crate::backend::StoreBackend;
use crate::{
    ArtifactRecord, Decision, DecisionEvaluation, DecisionStats, DiscoveryMethod, EntityActivitySummary, EntityArticle,
    EntityDetail, EntityRef, EntitySignalCandidate, EntitySummary, EventIndexEntry, Feed, NewArticle, NewArtifact,
    NewArtifactRecord, NewDecision, NewDecisionEvaluation, NewOutbox, NewOutcomeEvent, NewReflection, OutcomeEvent,
    OutboxEntry, Reflection, RelatedEntity, RelatedEntityRef, SignalBriefInput, SignalDetail, SignalEvent,
    SignalThreadFilter, SignalUpsertResult, StoreError, UpdateReflection,
};

#[async_trait(?Send)]
impl StoreBackend for crate::D1Store {
    async fn get_feed(&self, id: i64) -> Result<Option<Feed>, StoreError> {
        crate::D1Store::get_feed(self, id).await
    }
    async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError> {
        crate::D1Store::record_fetch_result(self, feed_id, fetched_at, etag, last_modified).await
    }
    async fn active_rule_jsons(&self, audience_tag: &str) -> Result<Vec<String>, StoreError> {
        crate::D1Store::active_rule_jsons(self, audience_tag).await
    }
    async fn insert_article(&self, article: &NewArticle) -> Result<Option<i64>, StoreError> {
        crate::D1Store::insert_article(self, article).await
    }
    async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        tags_json: &str,
        vector_id: &str,
        score: f64,
    ) -> Result<(), StoreError> {
        crate::D1Store::set_ai_summary(self, article_id, summary, tags_json, vector_id, score).await
    }
    async fn set_raw_content_r2_key(&self, article_id: i64, r2_key: Option<&str>) -> Result<(), StoreError> {
        crate::D1Store::set_raw_content_r2_key(self, article_id, r2_key).await
    }
    async fn expire_old_articles(&self, now: i64, days: i64) -> Result<u64, StoreError> {
        crate::D1Store::expire_old_articles(self, now, days).await
    }
    async fn upsert_entity(&self, name: &str, normalized: &str, entity_type: &str) -> Result<i64, StoreError> {
        crate::D1Store::upsert_entity(self, name, normalized, entity_type).await
    }
    async fn link_article_entity(
        &self,
        article_id: i64,
        entity_id: i64,
        relevance: f64,
        context: Option<&str>,
    ) -> Result<(), StoreError> {
        crate::D1Store::link_article_entity(self, article_id, entity_id, relevance, context).await
    }
    async fn link_entity_relation(
        &self,
        source: i64,
        target: i64,
        rtype: &str,
        confidence: f64,
    ) -> Result<(), StoreError> {
        crate::D1Store::link_entity_relation(self, source, target, rtype, confidence).await
    }
    async fn list_entities(&self, limit: u32, offset: u32) -> Result<Vec<EntitySummary>, StoreError> {
        crate::D1Store::list_entities(self, limit, offset).await
    }
    async fn entity_detail(&self, id: i64) -> Result<Option<EntityDetail>, StoreError> {
        crate::D1Store::entity_detail(self, id).await
    }
    async fn entity_relations(&self, entity_id: i64, limit: u32) -> Result<Vec<RelatedEntity>, StoreError> {
        crate::D1Store::entity_relations(self, entity_id, limit).await
    }
    async fn article_entities(&self, article_id: i64) -> Result<Vec<EntityRef>, StoreError> {
        crate::D1Store::article_entities(self, article_id).await
    }
    async fn create_artifact(&self, artifact: &NewArtifact) -> Result<i64, StoreError> {
        crate::D1Store::create_artifact(self, artifact).await
    }
    async fn list_artifacts_by_entity(
        &self,
        entity_id: i64,
        limit: u32,
    ) -> Result<Vec<crate::ArtifactEntry>, StoreError> {
        crate::D1Store::list_artifacts_by_entity(self, entity_id, limit).await
    }
    async fn put_artifact(&self, artifact: &NewArtifactRecord) -> Result<i64, StoreError> {
        crate::D1Store::put_artifact(self, artifact).await
    }
    async fn get_artifact(&self, artifact_type: &str, date: &str) -> Result<Option<ArtifactRecord>, StoreError> {
        crate::D1Store::get_artifact(self, artifact_type, date).await
    }
    async fn list_artifacts(&self, artifact_type: &str, limit: u32) -> Result<Vec<ArtifactRecord>, StoreError> {
        crate::D1Store::list_artifacts(self, artifact_type, limit).await
    }
    async fn entity_articles(&self, entity_id: i64, limit: u32, offset: u32) -> Result<Vec<EntityArticle>, StoreError> {
        crate::D1Store::entity_articles(self, entity_id, limit, offset).await
    }
    async fn entity_activity_summary(
        &self,
        entity_id: i64,
        now: i64,
        days: i64,
    ) -> Result<EntityActivitySummary, StoreError> {
        crate::D1Store::entity_activity_summary(self, entity_id, now, days).await
    }
    async fn entity_signal_candidates(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError> {
        crate::D1Store::entity_signal_candidates(self, now, days, limit).await
    }
    async fn upsert_signal_thread(
        &self,
        signal_key: &str,
        anchor_entity_id: Option<i64>,
        title: &str,
        status: &str,
        discovery_method: &DiscoveryMethod,
        discovery_score: Option<f64>,
    ) -> Result<SignalUpsertResult, StoreError> {
        crate::D1Store::upsert_signal_thread(
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
    async fn update_signal_lifecycle(&self, now: i64) -> Result<(), StoreError> {
        crate::D1Store::update_signal_lifecycle(self, now).await
    }
    async fn get_active_signal_threads(&self, limit: u32) -> Result<Vec<SignalBriefInput>, StoreError> {
        crate::D1Store::get_active_signal_threads(self, limit).await
    }
    async fn list_signal_threads(&self, filter: &SignalThreadFilter) -> Result<Vec<SignalBriefInput>, StoreError> {
        crate::D1Store::list_signal_threads(self, filter).await
    }
    async fn load_signal_detail(&self, thread_id: i64) -> Result<Option<SignalDetail>, StoreError> {
        crate::D1Store::load_signal_detail(self, thread_id).await
    }
    async fn entity_signal_candidates_filtered(
        &self,
        now: i64,
        days: i64,
        limit: u32,
        min_entity_articles: u32,
        min_sources: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError> {
        crate::D1Store::entity_signal_candidates_filtered(self, now, days, limit, min_entity_articles, min_sources)
            .await
    }
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
        crate::D1Store::append_signal_instance_v2(
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
        crate::D1Store::insert_signal_event(self, thread_id, event_type, payload).await
    }
    async fn load_signal_events(&self, thread_id: i64, limit: u32) -> Result<Vec<SignalEvent>, StoreError> {
        crate::D1Store::load_signal_events(self, thread_id, limit).await
    }
    async fn load_thread_related_entities(
        &self,
        thread_id: i64,
        limit: u32,
    ) -> Result<Vec<RelatedEntityRef>, StoreError> {
        crate::D1Store::load_thread_related_entities(self, thread_id, limit).await
    }
    async fn recent_embedded_articles(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<crate::ArticleEmbeddingRef>, StoreError> {
        crate::D1Store::recent_embedded_articles(self, now, days, limit).await
    }
    async fn create_decision(&self, d: &NewDecision) -> Result<i64, StoreError> {
        crate::D1Store::create_decision(self, d).await
    }
    async fn get_decision(&self, id: i64) -> Result<Option<Decision>, StoreError> {
        crate::D1Store::get_decision(self, id).await
    }
    async fn list_decisions(&self, status: Option<&str>, limit: u32) -> Result<Vec<Decision>, StoreError> {
        crate::D1Store::list_decisions(self, status, limit).await
    }
    async fn update_decision_status(&self, id: i64, status: &str) -> Result<(), StoreError> {
        crate::D1Store::update_decision_status(self, id, status).await
    }
    async fn decisions_by_signal(&self, signal_thread_id: i64) -> Result<Vec<Decision>, StoreError> {
        crate::D1Store::decisions_by_signal(self, signal_thread_id).await
    }
    async fn decision_stats(&self) -> Result<DecisionStats, StoreError> {
        crate::D1Store::decision_stats(self).await
    }
    async fn create_outcome(&self, e: &NewOutcomeEvent) -> Result<i64, StoreError> {
        crate::D1Store::create_outcome(self, e).await
    }
    async fn get_decision_outcomes(&self, decision_id: i64) -> Result<Vec<OutcomeEvent>, StoreError> {
        crate::D1Store::get_decision_outcomes(self, decision_id).await
    }
    async fn create_evaluation(&self, e: &NewDecisionEvaluation) -> Result<i64, StoreError> {
        crate::D1Store::create_evaluation(self, e).await
    }
    async fn get_decision_evaluations(&self, decision_id: i64) -> Result<Vec<DecisionEvaluation>, StoreError> {
        crate::D1Store::get_decision_evaluations(self, decision_id).await
    }
    async fn get_latest_evaluation(&self, decision_id: i64) -> Result<Option<DecisionEvaluation>, StoreError> {
        crate::D1Store::get_latest_evaluation(self, decision_id).await
    }

    // ── Object Outbox ──

    async fn insert_outbox(&self, entry: &NewOutbox) -> Result<i64, StoreError> {
        crate::D1Store::insert_outbox(self, entry).await
    }
    async fn drain_outbox(&self, limit: u32) -> Result<Vec<OutboxEntry>, StoreError> {
        crate::D1Store::drain_outbox(self, limit).await
    }
    async fn mark_outbox_archived(&self, id: i64) -> Result<(), StoreError> {
        crate::D1Store::mark_outbox_archived(self, id).await
    }
    async fn mark_outbox_failed(&self, id: i64) -> Result<(), StoreError> {
        crate::D1Store::mark_outbox_failed(self, id).await
    }

    // ── Event Archive Index ──

    async fn insert_event_index(
        &self,
        event_id: &str,
        aggregate_type: &str,
        aggregate_id: &str,
        event_type: &str,
        object_key: &str,
        occurred_at: i64,
    ) -> Result<(), StoreError> {
        crate::D1Store::insert_event_index(self, event_id, aggregate_type, aggregate_id, event_type, object_key, occurred_at).await
    }
    async fn find_event_keys(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        limit: u32,
    ) -> Result<Vec<EventIndexEntry>, StoreError> {
        crate::D1Store::find_event_keys(self, aggregate_type, aggregate_id, limit).await
    }

    // ── Reflection Engine (Sprint 5.4) ──

    async fn create_reflection(&self, req: &NewReflection) -> Result<i64, StoreError> {
        crate::D1Store::create_reflection(self, req).await
    }
    async fn update_reflection(&self, req: &UpdateReflection) -> Result<(), StoreError> {
        crate::D1Store::update_reflection(self, req).await
    }
    async fn get_reflection_by_decision(&self, decision_id: i64) -> Result<Option<Reflection>, StoreError> {
        crate::D1Store::get_reflection_by_decision(self, decision_id).await
    }
    async fn decisions_eligible_for_reflection(&self, now: i64, limit: u32) -> Result<Vec<i64>, StoreError> {
        crate::D1Store::decisions_eligible_for_reflection(self, now, limit).await
    }
    async fn failed_reflections_for_retry(&self, limit: u32) -> Result<Vec<Reflection>, StoreError> {
        crate::D1Store::failed_reflections_for_retry(self, limit).await
    }
    async fn stale_generating_reflections(&self, now: i64) -> Result<Vec<Reflection>, StoreError> {
        crate::D1Store::stale_generating_reflections(self, now).await
    }
}
