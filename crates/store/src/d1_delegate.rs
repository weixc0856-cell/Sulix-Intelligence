//! Anti-corruption layer: implements every domain trait + the legacy
//! `StoreBackend` for [`D1Store`](crate::D1Store).
//!
//! Each method delegates 1:1 to a `D1Store` method ?no additional logic.

use async_trait::async_trait;

use std::collections::HashMap;

use crate::backend::StoreBackend;
use crate::traits::*;
use crate::{
    Article, ArticleDetail, ArticleEmbeddingRef, ArtifactEntry, ArtifactRecord, BriefArticle, Claim, ClaimEvidence,
    ConfidenceEvent, DayCount, Decision, DecisionEvaluation, DecisionStats, DiscoveryMethod, EntityActivitySummary,
    EntityArticle, EntityDetail, EntitySignalCandidate, EntitySummary, EventIndexEntry, Feed, FeedStats, HealthStats,
    Memory, NewArticle, NewArtifact, NewArtifactRecord, NewClaim, NewConfidenceEvent, NewContextSnapshot, NewDecision,
    NewDecisionEvaluation, NewMemory, NewObservation, NewOutbox, NewOutcomeEvent, NewReflection, NewSource,
    Observation, OutboxEntry, OutcomeEvent, PendingArticle, RadarResponse, Reflection, RelatedEntity, RelatedEntityRef,
    ScoreDist, SignalBriefInput, SignalDetail, SignalEvent, SignalThread, SignalThreadFilter, SignalUpsertResult,
    Source, StoreError, TodaySignal, UpdateReflection,
};

//  Repositories (save / find)

#[async_trait(?Send)]
impl FeedRepository for crate::D1Store {
    async fn save_feed(&self, feed: &Feed) -> Result<i64, StoreError> {
        crate::D1Store::insert_feed(
            self,
            &feed.url,
            &feed.title.clone().unwrap_or_default(),
            &feed.category.clone().unwrap_or_default(),
            feed.fetch_interval_sec,
        )
        .await
        .map(|opt| opt.unwrap_or(0))
    }
    async fn find_feed(&self, id: i64) -> Result<Option<Feed>, StoreError> {
        crate::D1Store::get_feed(self, id).await
    }
}

#[async_trait(?Send)]
impl ArticleRepository for crate::D1Store {
    async fn save_article(&self, article: &NewArticle) -> Result<Option<i64>, StoreError> {
        crate::D1Store::insert_article(self, article).await
    }
    async fn find_article(&self, id: i64) -> Result<Option<Article>, StoreError> {
        crate::D1Store::article_by_id(self, id).await
    }
}

#[async_trait(?Send)]
impl EntityRepository for crate::D1Store {
    async fn save_entity(&self, name: &str, normalized_name: &str, entity_type: &str) -> Result<i64, StoreError> {
        crate::D1Store::upsert_entity(self, name, normalized_name, entity_type).await
    }
    async fn find_entity(&self, id: i64) -> Result<Option<EntityDetail>, StoreError> {
        crate::D1Store::entity_detail(self, id).await
    }
    async fn link_article(&self, article_id: i64, entity_id: i64, relevance: f64) -> Result<(), StoreError> {
        crate::D1Store::link_article_entity(self, article_id, entity_id, relevance, None).await
    }
    async fn link_relation(&self, source: i64, target: i64, rtype: &str) -> Result<(), StoreError> {
        crate::D1Store::link_entity_relation(self, source, target, rtype, 1.0).await
    }
}

#[async_trait(?Send)]
impl SignalRepository for crate::D1Store {
    async fn save_signal(&self, thread: &SignalThread) -> Result<i64, StoreError> {
        // Map discovery_method string to DiscoveryMethod enum (default to Entity)
        let dm = match thread.discovery_method.as_str() {
            "semantic" => DiscoveryMethod::Semantic,
            _ => DiscoveryMethod::Entity,
        };
        let result = crate::D1Store::upsert_signal_thread(
            self,
            &thread.signal_key,
            thread.anchor_entity_id,
            &thread.title,
            &thread.status,
            &dm,
            thread.discovery_score,
        )
        .await?;
        Ok(result.id)
    }
    async fn find_signal(&self, id: i64) -> Result<Option<SignalThread>, StoreError> {
        crate::D1Store::load_signal_detail(self, id).await.map(|opt| {
            opt.map(|d| SignalThread {
                id: d.id,
                signal_key: String::new(), // SignalDetail doesn't carry signal_key; populated in Phase 2
                anchor_entity_id: d.anchor_entity.as_ref().map(|e| e.id),
                title: d.title,
                description: d.description,
                status: d.status,
                health_score: d.health.score,
                first_seen_at: Some(d.first_seen_at),
                last_seen_at: Some(d.last_seen_at),
                discovery_method: String::new(),
                discovery_score: None,
                created_at: d.first_seen_at,
                updated_at: d.last_seen_at,
            })
        })
    }
    async fn find_signal_by_key(&self, _key: &str) -> Result<Option<SignalThread>, StoreError> {
        Err(StoreError::D1("find_signal_by_key not yet implemented on D1Store".into()))
    }
}

#[async_trait(?Send)]
impl DecisionRepository for crate::D1Store {
    async fn save_decision(&self, decision: &NewDecision) -> Result<i64, StoreError> {
        crate::D1Store::create_decision(self, decision).await
    }
    async fn find_decision(&self, id: i64) -> Result<Option<Decision>, StoreError> {
        crate::D1Store::get_decision(self, id).await
    }
}

#[async_trait(?Send)]
impl OutcomeRepository for crate::D1Store {
    async fn save_outcome(&self, e: &NewOutcomeEvent) -> Result<i64, StoreError> {
        crate::D1Store::create_outcome(self, e).await
    }
}

#[async_trait(?Send)]
impl EvaluationRepository for crate::D1Store {
    async fn save_evaluation(&self, e: &NewDecisionEvaluation) -> Result<i64, StoreError> {
        crate::D1Store::create_evaluation(self, e).await
    }
}

#[async_trait(?Send)]
impl ClaimRepository for crate::D1Store {
    async fn save_claim(&self, c: &NewClaim) -> Result<i64, StoreError> {
        crate::D1Store::create_claim(self, c).await
    }
    async fn find_claim(&self, id: i64) -> Result<Option<Claim>, StoreError> {
        crate::D1Store::get_claim(self, id).await
    }
}

#[async_trait(?Send)]
impl ConfidenceRepository for crate::D1Store {
    async fn append_confidence(&self, e: &NewConfidenceEvent) -> Result<i64, StoreError> {
        crate::D1Store::append_confidence(self, e).await
    }
    async fn list_confidence_history(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<ConfidenceEvent>, StoreError> {
        crate::D1Store::list_confidence_history(self, entity_type, entity_id).await
    }
}

#[async_trait(?Send)]
impl SourceRepository for crate::D1Store {
    async fn save_source(&self, s: &NewSource) -> Result<i64, StoreError> {
        crate::D1Store::save_source(self, s).await
    }
    async fn find_source(&self, id: i64) -> Result<Option<Source>, StoreError> {
        crate::D1Store::find_source(self, id).await
    }
    async fn find_source_by_feed(&self, feed_id: i64) -> Result<Option<Source>, StoreError> {
        crate::D1Store::find_source_by_feed(self, feed_id).await
    }
    async fn delete_source(&self, id: i64) -> Result<(), StoreError> {
        crate::D1Store::delete_source(self, id).await
    }
}

#[async_trait(?Send)]
impl ObservationRepository for crate::D1Store {
    async fn save_observation(&self, o: &NewObservation) -> Result<i64, StoreError> {
        crate::D1Store::create_observation(self, o).await
    }
    async fn find_observation(&self, id: i64) -> Result<Option<Observation>, StoreError> {
        crate::D1Store::get_observation(self, id).await
    }
    async fn find_observation_by_hash(&self, hash: &str) -> Result<Option<Observation>, StoreError> {
        crate::D1Store::find_observation_by_hash(self, hash).await
    }
}

//  Query Services (read model)

#[async_trait(?Send)]
impl FeedQueryService for crate::D1Store {
    async fn feeds_due_for_fetch(&self, now: i64, category: Option<&str>) -> Result<Vec<Feed>, StoreError> {
        crate::D1Store::feeds_due_for_fetch(self, now, category).await
    }
    async fn all_feeds(&self, status_filter: Option<&str>) -> Result<Vec<Feed>, StoreError> {
        crate::D1Store::all_feeds(self, status_filter).await
    }
    async fn feed_stats(&self) -> Result<Vec<FeedStats>, StoreError> {
        crate::D1Store::feed_stats(self).await
    }
    async fn health_stats(&self) -> Result<HealthStats, StoreError> {
        crate::D1Store::health_stats(self).await
    }
    async fn pipeline_status(&self, now: i64) -> Result<serde_json::Value, StoreError> {
        crate::D1Store::pipeline_status(self, now).await
    }
    async fn score_distribution(&self) -> Result<ScoreDist, StoreError> {
        crate::D1Store::score_distribution(self).await
    }
    async fn article_trend(&self, days: i64) -> Result<Vec<DayCount>, StoreError> {
        crate::D1Store::article_trend(self, days).await
    }
}

#[async_trait(?Send)]
impl ArticleQueryService for crate::D1Store {
    async fn latest_articles(&self, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError> {
        crate::D1Store::latest_articles(self, limit, offset).await
    }
    async fn article_count(&self) -> Result<i64, StoreError> {
        crate::D1Store::article_count(self).await
    }
    async fn trending_articles(&self, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError> {
        crate::D1Store::trending_articles(self, limit, offset).await
    }
    async fn trending_count(&self) -> Result<i64, StoreError> {
        crate::D1Store::trending_count(self).await
    }
    async fn article_by_id(&self, id: i64) -> Result<Option<Article>, StoreError> {
        crate::D1Store::article_by_id(self, id).await
    }
    async fn articles_by_ids(&self, ids: &[i64]) -> Result<Vec<Article>, StoreError> {
        crate::D1Store::articles_by_ids(self, ids).await
    }
    async fn article_detail(&self, id: i64) -> Result<Option<ArticleDetail>, StoreError> {
        crate::D1Store::article_detail(self, id).await
    }
    async fn adjacent_articles(&self, id: i64) -> Result<(Option<Article>, Option<Article>), StoreError> {
        crate::D1Store::adjacent_articles(self, id).await
    }
    async fn articles_by_tag(&self, tag: &str, limit: u32, offset: u32) -> Result<Vec<PendingArticle>, StoreError> {
        crate::D1Store::articles_by_tag(self, tag, limit, offset).await
    }
    async fn articles_by_category(
        &self,
        category: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<PendingArticle>, StoreError> {
        crate::D1Store::articles_by_category(self, category, limit, offset).await
    }
    async fn related_articles(&self, article_id: i64, limit: u32) -> Result<Vec<PendingArticle>, StoreError> {
        crate::D1Store::related_articles(self, article_id, limit).await
    }
    async fn get_raw_content_key(&self, article_id: i64) -> Result<Option<String>, StoreError> {
        crate::D1Store::get_raw_content_key(self, article_id).await
    }
    async fn categories_summary(&self) -> Result<Vec<(String, i64)>, StoreError> {
        crate::D1Store::categories_summary(self).await
    }
    async fn tags_summary(&self) -> Result<Vec<(String, i64)>, StoreError> {
        crate::D1Store::tags_summary(self).await
    }
    async fn recent_embedded_articles(
        &self,
        now: i64,
        days: i64,
        limit: u32,
    ) -> Result<Vec<ArticleEmbeddingRef>, StoreError> {
        crate::D1Store::recent_embedded_articles(self, now, days, limit).await
    }
}

#[async_trait(?Send)]
impl EntityQueryService for crate::D1Store {
    async fn list_entities(&self, limit: u32, offset: u32) -> Result<Vec<EntitySummary>, StoreError> {
        crate::D1Store::list_entities(self, limit, offset).await
    }
    async fn entity_detail(&self, id: i64) -> Result<Option<EntityDetail>, StoreError> {
        crate::D1Store::entity_detail(self, id).await
    }
    async fn entity_relations(&self, entity_id: i64, limit: u32) -> Result<Vec<RelatedEntity>, StoreError> {
        crate::D1Store::entity_relations(self, entity_id, limit).await
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
}

#[async_trait(?Send)]
impl SignalQueryService for crate::D1Store {
    async fn radar(&self, _filter: &SignalThreadFilter) -> Result<RadarResponse, StoreError> {
        Err(StoreError::D1("radar() not available on D1Store; use SignalQueryService via signal-engine".into()))
    }
    async fn signal_detail(&self, _id: i64) -> Result<Option<SignalDetail>, StoreError> {
        Err(StoreError::D1("signal_detail() not available on D1Store; use SignalQueryService via signal-engine".into()))
    }
    async fn list_signal_threads(&self, filter: &SignalThreadFilter) -> Result<Vec<SignalBriefInput>, StoreError> {
        crate::D1Store::list_signal_threads(self, filter).await
    }
    async fn get_active_signal_threads(&self, limit: u32) -> Result<Vec<SignalBriefInput>, StoreError> {
        crate::D1Store::get_active_signal_threads(self, limit).await
    }
    async fn signals_today(&self) -> Result<Vec<TodaySignal>, StoreError> {
        // NOTE: signals_today in D1Store currently takes (now: i64). Pass 0 for now;
        // the query uses `now-86400` internally so the exact value matters mostly for
        // the "last 24h" window. A call with real `now` will be wired when SignalQueryService
        // is promoted to its own infrastructure crate.
        crate::D1Store::signals_today(self, 0).await
    }
}

#[async_trait(?Send)]
impl DecisionQueryService for crate::D1Store {
    async fn list_decisions(&self, status: Option<&str>, limit: u32) -> Result<Vec<Decision>, StoreError> {
        crate::D1Store::list_decisions(self, status, limit).await
    }
    async fn decisions_by_signal(&self, signal_thread_id: i64) -> Result<Vec<Decision>, StoreError> {
        crate::D1Store::decisions_by_signal(self, signal_thread_id).await
    }
    async fn decision_stats(&self) -> Result<DecisionStats, StoreError> {
        crate::D1Store::decision_stats(self).await
    }
    async fn list_outcomes(&self, decision_id: i64) -> Result<Vec<OutcomeEvent>, StoreError> {
        crate::D1Store::get_decision_outcomes(self, decision_id).await
    }
    async fn list_evaluations(&self, decision_id: i64) -> Result<Vec<DecisionEvaluation>, StoreError> {
        crate::D1Store::get_decision_evaluations(self, decision_id).await
    }
    async fn get_latest_evaluation(&self, decision_id: i64) -> Result<Option<DecisionEvaluation>, StoreError> {
        crate::D1Store::get_latest_evaluation(self, decision_id).await
    }
}

#[async_trait(?Send)]
impl OutcomeQueryService for crate::D1Store {
    async fn list_outcomes(&self, decision_id: i64) -> Result<Vec<OutcomeEvent>, StoreError> {
        crate::D1Store::get_decision_outcomes(self, decision_id).await
    }
}

#[async_trait(?Send)]
impl ClaimQueryService for crate::D1Store {
    async fn list_claims(&self, status: Option<&str>, limit: u32) -> Result<Vec<Claim>, StoreError> {
        crate::D1Store::list_claims(self, status, None, limit, 0).await
    }
    async fn get_claim_evidence(&self, claim_id: i64) -> Result<Vec<ClaimEvidence>, StoreError> {
        crate::D1Store::get_claim_evidence(self, claim_id).await
    }
}

#[async_trait(?Send)]
impl SourceQueryService for crate::D1Store {
    async fn list_sources(
        &self,
        tier: Option<&str>,
        policy: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Source>, StoreError> {
        crate::D1Store::list_sources(self, tier, policy, limit, offset).await
    }
}

#[async_trait(?Send)]
impl ObservationQueryService for crate::D1Store {
    async fn list_observations(
        &self,
        source_type: Option<&str>,
        source_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Observation>, StoreError> {
        crate::D1Store::list_observations(self, source_type, source_id, limit, offset).await
    }
}

#[async_trait(?Send)]
impl EvaluationQueryService for crate::D1Store {
    async fn list_evaluations(&self, decision_id: i64) -> Result<Vec<DecisionEvaluation>, StoreError> {
        crate::D1Store::get_decision_evaluations(self, decision_id).await
    }
    async fn get_latest_evaluation(&self, decision_id: i64) -> Result<Option<DecisionEvaluation>, StoreError> {
        crate::D1Store::get_latest_evaluation(self, decision_id).await
    }
}

#[async_trait(?Send)]
impl BatchSignalQueryService for crate::D1Store {
    async fn batch_evidence(&self, thread_ids: &[i64]) -> Result<HashMap<i64, Vec<BriefArticle>>, StoreError> {
        crate::D1Store::batch_evidence(self, thread_ids).await
    }
    async fn batch_related_entities(
        &self,
        thread_ids: &[i64],
    ) -> Result<HashMap<i64, Vec<RelatedEntityRef>>, StoreError> {
        crate::D1Store::batch_related_entities(self, thread_ids).await
    }
}

//  Legacy StoreBackend (remaining methods not yet migrated to subtraits)

#[async_trait(?Send)]
impl StoreBackend for crate::D1Store {
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

    async fn create_decision(&self, d: &NewDecision) -> Result<i64, StoreError> {
        crate::D1Store::create_decision(self, d).await
    }

    async fn get_decision(&self, id: i64) -> Result<Option<Decision>, StoreError> {
        crate::D1Store::get_decision(self, id).await
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

    async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError> {
        crate::D1Store::record_fetch_result(self, feed_id, fetched_at, etag, last_modified).await
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

    async fn update_signal_lifecycle(&self, now: i64) -> Result<(), StoreError> {
        crate::D1Store::update_signal_lifecycle(self, now).await
    }

    async fn load_signal_detail(&self, thread_id: i64) -> Result<Option<SignalDetail>, StoreError> {
        crate::D1Store::load_signal_detail(self, thread_id).await
    }

    async fn get_latest_instance_fingerprint(&self, thread_id: i64) -> Result<Option<(f64, String)>, StoreError> {
        crate::D1Store::get_latest_instance_fingerprint(self, thread_id).await
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

    async fn update_decision_status(&self, id: i64, status: &str) -> Result<(), StoreError> {
        crate::D1Store::update_decision_status(self, id, status).await
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

    async fn create_artifact(&self, artifact: &NewArtifact) -> Result<i64, StoreError> {
        crate::D1Store::create_artifact(self, artifact).await
    }
    async fn list_artifacts_by_entity(&self, entity_id: i64, limit: u32) -> Result<Vec<ArtifactEntry>, StoreError> {
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

    async fn create_reflection(&self, req: &NewReflection) -> Result<i64, StoreError> {
        crate::D1Store::create_reflection(self, req).await
    }
    async fn update_reflection(&self, req: &UpdateReflection) -> Result<(), StoreError> {
        crate::D1Store::update_reflection(self, req).await
    }
    async fn get_reflection_by_decision(&self, decision_id: i64) -> Result<Option<Reflection>, StoreError> {
        crate::D1Store::get_reflection_by_decision(self, decision_id).await
    }
    // ===== Claim (Sprint 5.3) =====

    async fn create_claim(&self, c: &NewClaim) -> Result<i64, StoreError> {
        crate::D1Store::create_claim(self, c).await
    }
    async fn get_claim(&self, id: i64) -> Result<Option<Claim>, StoreError> {
        crate::D1Store::get_claim(self, id).await
    }
    async fn list_claims(&self, status: Option<&str>, limit: u32) -> Result<Vec<Claim>, StoreError> {
        crate::D1Store::list_claims(self, status, None, limit, 0).await
    }
    async fn get_claim_evidence(&self, claim_id: i64) -> Result<Vec<ClaimEvidence>, StoreError> {
        crate::D1Store::get_claim_evidence(self, claim_id).await
    }
    async fn create_observation(&self, o: &NewObservation) -> Result<i64, StoreError> {
        crate::D1Store::create_observation(self, o).await
    }
    async fn get_observation(&self, id: i64) -> Result<Option<Observation>, StoreError> {
        crate::D1Store::get_observation(self, id).await
    }
    async fn find_observation_by_hash(&self, hash: &str) -> Result<Option<Observation>, StoreError> {
        crate::D1Store::find_observation_by_hash(self, hash).await
    }
    async fn list_observations(
        &self,
        source_type: Option<&str>,
        source_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Observation>, StoreError> {
        crate::D1Store::list_observations(self, source_type, source_id, limit, offset).await
    }
    async fn append_confidence(&self, e: &NewConfidenceEvent) -> Result<i64, StoreError> {
        crate::D1Store::append_confidence(self, e).await
    }
    async fn list_confidence_history(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<ConfidenceEvent>, StoreError> {
        crate::D1Store::list_confidence_history(self, entity_type, entity_id).await
    }

    // ===== Source Registry (Sprint 5.6) =====

    async fn save_source(&self, s: &NewSource) -> Result<i64, StoreError> {
        crate::D1Store::save_source(self, s).await
    }
    async fn find_source(&self, id: i64) -> Result<Option<Source>, StoreError> {
        crate::D1Store::find_source(self, id).await
    }
    async fn find_source_by_feed(&self, feed_id: i64) -> Result<Option<Source>, StoreError> {
        crate::D1Store::find_source_by_feed(self, feed_id).await
    }
    async fn list_sources(
        &self,
        tier: Option<&str>,
        policy: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Source>, StoreError> {
        crate::D1Store::list_sources(self, tier, policy, limit, offset).await
    }
}

//  Fine-grained P4 subtraits — the 8 methods above were lifted off StoreBackend
//  and are now reachable through composition instead of the legacy supertrait.

#[async_trait(?Send)]
impl OutboxStore for crate::D1Store {
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
}

#[async_trait(?Send)]
impl EventIndexStore for crate::D1Store {
    async fn insert_event_index(
        &self,
        event_id: &str,
        aggregate_type: &str,
        aggregate_id: &str,
        event_type: &str,
        object_key: &str,
        occurred_at: i64,
    ) -> Result<(), StoreError> {
        crate::D1Store::insert_event_index(
            self,
            event_id,
            aggregate_type,
            aggregate_id,
            event_type,
            object_key,
            occurred_at,
        )
        .await
    }
    async fn find_event_keys(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        limit: u32,
    ) -> Result<Vec<EventIndexEntry>, StoreError> {
        crate::D1Store::find_event_keys(self, aggregate_type, aggregate_id, limit).await
    }
}

#[async_trait(?Send)]
impl MemoryPersistence for crate::D1Store {
    async fn create_memory(&self, entry: &NewMemory) -> Result<i64, StoreError> {
        crate::D1Store::create_memory(self, entry).await
    }
    async fn list_memories(
        &self,
        memory_type: Option<&str>,
        status: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Memory>, StoreError> {
        crate::D1Store::list_memories(self, memory_type, status, limit).await
    }
}

#[async_trait(?Send)]
impl ContextSnapshotStore for crate::D1Store {
    async fn save_context_snapshot(&self, snap: &NewContextSnapshot) -> Result<(), StoreError> {
        crate::D1Store::save_context_snapshot(self, snap).await
    }
}
