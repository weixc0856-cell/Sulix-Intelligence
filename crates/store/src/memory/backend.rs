//! Single `impl StoreBackend for MemoryStore` block.
//!
//! Rust does not allow splitting trait impls across files in the same crate.
//! However, the methods are thin wrappers around MemoryStore fields.
//!
//! Section headers follow the domain aggregate boundaries for readability.

use async_trait::async_trait;

use super::{ArtifactData, EntityInternal, MemoryStore, RelationEdge};
use crate::backend::StoreBackend;
use crate::{
    ArtifactEntry, ArtifactRecord, ContextSnapshot, Decision, DecisionEvaluation, DecisionStats, DiscoveryMethod,
    EntityActivitySummary, EntityArticle, EntityDetail, EntityRef, EntitySignalCandidate, EntitySummary, EvalSummary,
    EventIndexEntry, Feed, Memory, NewArticle, NewArtifact, NewArtifactRecord, NewContextSnapshot, NewDecision,
    NewDecisionEvaluation, NewMemory, NewOutbox, NewOutcomeEvent, NewReflection, OutcomeEvent, OutboxEntry, Reflection,
    RelatedEntity, RelatedEntityRef, SignalBriefInput, SignalDetail, SignalEvent, SignalThreadFilter, SignalUpsertResult,
    StoreError, UpdateReflection,
};

#[async_trait(?Send)]
impl StoreBackend for MemoryStore {
    // ── Feed / Article / Rules ──

    async fn get_feed(&self, id: i64) -> Result<Option<Feed>, StoreError> {
        Ok(self.feeds.get(&id).cloned())
    }

    async fn record_fetch_result(
        &self,
        feed_id: i64,
        fetched_at: i64,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<(), StoreError> {
        if self.fail_fetch_result {
            return Err(StoreError::D1("injected fetch result failure".into()));
        }
        self.fetch_results.borrow_mut().push((
            feed_id,
            fetched_at,
            etag.map(|s| s.to_string()),
            last_modified.map(|s| s.to_string()),
        ));
        Ok(())
    }

    async fn active_rule_jsons(&self, _audience_tag: &str) -> Result<Vec<String>, StoreError> {
        if self.fail_rules {
            return Err(StoreError::D1("injected rules failure".into()));
        }
        Ok(self.rules.clone())
    }

    async fn insert_article(&self, article: &NewArticle) -> Result<Option<i64>, StoreError> {
        if self.fail_insert {
            return Err(StoreError::D1("injected insert failure".into()));
        }
        let dup = self.articles.borrow().iter().any(|a| a.feed_id == article.feed_id && a.guid == article.guid);
        if dup {
            return Ok(None);
        }
        let id = *self.next_article_id.borrow();
        *self.next_article_id.borrow_mut() = id + 1;
        self.articles.borrow_mut().push(NewArticle {
            feed_id: article.feed_id,
            guid: article.guid.clone(),
            title: article.title.clone(),
            url: article.url.clone(),
            published_at: article.published_at,
            raw_content_r2_key: article.raw_content_r2_key.clone(),
        });
        Ok(Some(id))
    }

    async fn set_ai_summary(
        &self,
        article_id: i64,
        summary: &str,
        _tags_json: &str,
        _vector_id: &str,
        _score: f64,
    ) -> Result<(), StoreError> {
        if self.fail_summary {
            return Err(StoreError::D1("injected summary failure".into()));
        }
        self.summaries.borrow_mut().insert(article_id, summary.to_string());
        Ok(())
    }

    async fn set_raw_content_r2_key(&self, article_id: i64, r2_key: Option<&str>) -> Result<(), StoreError> {
        if self.fail_r2_key {
            return Err(StoreError::D1("injected r2 key failure".into()));
        }
        self.r2_keys.borrow_mut().insert(article_id, r2_key.map(|s| s.to_string()));
        Ok(())
    }

    async fn expire_old_articles(&self, _now: i64, _days: i64) -> Result<u64, StoreError> {
        Ok(0)
    }

    // ── Entity ──

    async fn upsert_entity(&self, name: &str, normalized: &str, entity_type: &str) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let mut entities = self.entities.borrow_mut();
        // Find existing by normalized_name — must clone id to satisfy borrow checker
        let existing_id = entities.values().find(|e| e.normalized_name == normalized).map(|e| e.id);
        if let Some(eid) = existing_id {
            if let Some(e) = entities.get_mut(&eid) {
                e.updated_at = now;
                e.entity_type = entity_type.to_string();
            }
            return Ok(eid);
        }
        let id = *self.next_entity_id.borrow();
        *self.next_entity_id.borrow_mut() = id + 1;
        entities.insert(
            id,
            EntityInternal {
                id,
                name: name.into(),
                normalized_name: normalized.into(),
                entity_type: entity_type.into(),
                canonical_id: None,
                description: None,
                metadata: None,
                created_at: now,
                updated_at: now,
            },
        );
        Ok(id)
    }

    async fn link_article_entity(
        &self,
        article_id: i64,
        entity_id: i64,
        _relevance: f64,
        _context: Option<&str>,
    ) -> Result<(), StoreError> {
        self.article_entity_links.borrow_mut().push((article_id, entity_id));
        Ok(())
    }

    async fn link_entity_relation(
        &self,
        source: i64,
        target: i64,
        rtype: &str,
        confidence: f64,
    ) -> Result<(), StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let mut edges = self.entity_relation_edges.borrow_mut();
        let existing = edges.iter_mut().find(|e| e.source == source && e.target == target && e.rtype == rtype);
        if let Some(e) = existing {
            e.last_seen = now;
            e.confidence = confidence;
        } else {
            edges.push(RelationEdge {
                source,
                target,
                rtype: rtype.into(),
                confidence,
                first_seen: now,
                last_seen: now,
            });
        }
        Ok(())
    }

    async fn list_entities(&self, limit: u32, offset: u32) -> Result<Vec<EntitySummary>, StoreError> {
        let entities = self.entities.borrow();
        let links = self.article_entity_links.borrow();
        let mut result: Vec<EntitySummary> = entities
            .values()
            .map(|e| EntitySummary {
                id: e.id,
                name: e.name.clone(),
                normalized_name: e.normalized_name.clone(),
                entity_type: e.entity_type.clone(),
                canonical_id: e.canonical_id,
                article_count: links.iter().filter(|(_, eid)| *eid == e.id).count() as i64,
                last_seen: e.updated_at,
            })
            .collect();
        result.sort_by_key(|b| std::cmp::Reverse(b.article_count));
        let start = offset as usize;
        let end = (start + limit as usize).min(result.len());
        Ok(if start < result.len() { result[start..end].to_vec() } else { vec![] })
    }

    async fn entity_detail(&self, id: i64) -> Result<Option<EntityDetail>, StoreError> {
        let entities = self.entities.borrow();
        let links = self.article_entity_links.borrow();
        Ok(entities.get(&id).map(|e| EntityDetail {
            id: e.id,
            name: e.name.clone(),
            normalized_name: e.normalized_name.clone(),
            entity_type: e.entity_type.clone(),
            canonical_id: e.canonical_id,
            description: e.description.clone(),
            metadata: e.metadata.clone(),
            article_count: links.iter().filter(|(_, eid)| *eid == e.id).count() as i64,
            created_at: e.created_at,
            updated_at: e.updated_at,
        }))
    }

    async fn entity_relations(&self, entity_id: i64, limit: u32) -> Result<Vec<RelatedEntity>, StoreError> {
        let entities = self.entities.borrow();
        let edges = self.entity_relation_edges.borrow();
        let mut related: Vec<RelatedEntity> = edges
            .iter()
            .filter(|e| e.source == entity_id || e.target == entity_id)
            .map(|e| {
                let other_id = if e.source == entity_id { e.target } else { e.source };
                let other = entities.get(&other_id);
                RelatedEntity {
                    id: other_id,
                    name: other.map(|o| o.name.clone()).unwrap_or_default(),
                    entity_type: other.map(|o| o.entity_type.clone()).unwrap_or_default(),
                    relation_type: e.rtype.clone(),
                    confidence: e.confidence,
                    last_seen_at: e.last_seen,
                }
            })
            .collect();
        related.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        let limit = limit as usize;
        related.truncate(limit);
        Ok(related)
    }

    async fn article_entities(&self, article_id: i64) -> Result<Vec<EntityRef>, StoreError> {
        let entities = self.entities.borrow();
        let links = self.article_entity_links.borrow();
        Ok(links
            .iter()
            .filter(|(aid, _)| *aid == article_id)
            .filter_map(|(_, eid)| {
                entities.get(eid).map(|e| EntityRef {
                    id: e.id,
                    name: e.name.clone(),
                    normalized_name: e.normalized_name.clone(),
                    entity_type: e.entity_type.clone(),
                    relevance: 1.0,
                    context: None,
                })
            })
            .collect())
    }

    async fn entity_articles(&self, entity_id: i64, limit: u32, offset: u32) -> Result<Vec<EntityArticle>, StoreError> {
        let links = self.article_entity_links.borrow();
        let mut ids: Vec<i64> = links
            .iter()
            .filter(|(_, eid)| *eid == entity_id)
            .map(|(aid, _)| *aid)
            .collect::<std::collections::HashSet<i64>>()
            .into_iter()
            .collect();
        ids.sort();
        ids.reverse();
        let start = offset as usize;
        let end = (start + limit as usize).min(ids.len());
        Ok(if start < ids.len() {
            ids[start..end]
                .iter()
                .map(|&id| EntityArticle {
                    id,
                    title: String::new(),
                    url: None,
                    feed_name: None,
                    published_at: None,
                    ai_summary: String::new(),
                    score: 0.0,
                })
                .collect()
        } else {
            vec![]
        })
    }

    async fn entity_activity_summary(
        &self,
        entity_id: i64,
        _now: i64,
        _days: i64,
    ) -> Result<EntityActivitySummary, StoreError> {
        let links = self.article_entity_links.borrow();
        let ids: std::collections::HashSet<i64> =
            links.iter().filter(|(_, eid)| *eid == entity_id).map(|(aid, _)| *aid).collect();
        Ok(EntityActivitySummary {
            article_count: ids.len() as i64,
            source_count: 0,
            avg_score: 0.0,
            max_score: 0.0,
            first_seen_at: None,
            last_seen_at: None,
            trend: "stable".into(),
        })
    }

    async fn entity_signal_candidates(
        &self,
        _now: i64,
        _days: i64,
        _limit: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError> {
        Ok(Vec::new())
    }
    async fn entity_signal_candidates_filtered(
        &self,
        _now: i64,
        _days: i64,
        _limit: u32,
        _min_entity_articles: u32,
        _min_sources: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError> {
        Ok(Vec::new())
    }
    async fn recent_embedded_articles(
        &self,
        _now: i64,
        _days: i64,
        _limit: u32,
    ) -> Result<Vec<crate::ArticleEmbeddingRef>, StoreError> {
        Ok(Vec::new())
    }

    // ── Signal Threads ──

    async fn upsert_signal_thread(
        &self,
        _signal_key: &str,
        _anchor_entity_id: Option<i64>,
        _title: &str,
        _status: &str,
        _discovery_method: &DiscoveryMethod,
        _discovery_score: Option<f64>,
    ) -> Result<SignalUpsertResult, StoreError> {
        Ok(SignalUpsertResult { id: 1, mutation: crate::SignalMutation::Created })
    }
    async fn update_signal_lifecycle(&self, _now: i64) -> Result<(), StoreError> {
        Ok(())
    }
    async fn get_active_signal_threads(&self, _limit: u32) -> Result<Vec<SignalBriefInput>, StoreError> {
        Ok(Vec::new())
    }
    async fn list_signal_threads(&self, _filter: &SignalThreadFilter) -> Result<Vec<SignalBriefInput>, StoreError> {
        Ok(Vec::new())
    }
    async fn load_signal_detail(&self, _thread_id: i64) -> Result<Option<SignalDetail>, StoreError> {
        Ok(None)
    }

    async fn get_latest_instance_fingerprint(&self, _thread_id: i64) -> Result<Option<(f64, String)>, StoreError> {
        Ok(None)  // MemoryStore: no persisted instances to compare
    }
    async fn append_signal_instance_v2(
        &self,
        _thread_id: i64,
        _score: f64,
        _impact: &str,
        _trend: &str,
        _article_count: i64,
        _source_count: i64,
        _avg_score: f64,
        _entity_id: i64,
    ) -> Result<i64, StoreError> {
        let id = *self.next_signal_event_id.borrow();
        *self.next_signal_event_id.borrow_mut() = id + 1;
        Ok(id)
    }

    async fn insert_signal_event(
        &self,
        thread_id: i64,
        event_type: &str,
        payload: Option<&str>,
    ) -> Result<(), StoreError> {
        let id = *self.next_signal_event_id.borrow();
        *self.next_signal_event_id.borrow_mut() = id + 1;
        self.signal_events.borrow_mut().push(SignalEvent {
            id,
            thread_id,
            event_type: event_type.to_string(),
            payload: payload.map(|s| s.to_string()),
            created_at: 1000000,
        });
        Ok(())
    }

    async fn load_signal_events(&self, thread_id: i64, _limit: u32) -> Result<Vec<SignalEvent>, StoreError> {
        Ok(self.signal_events.borrow().iter().filter(|e| e.thread_id == thread_id).cloned().collect())
    }

    async fn load_thread_related_entities(
        &self,
        _thread_id: i64,
        _limit: u32,
    ) -> Result<Vec<RelatedEntityRef>, StoreError> {
        Ok(Vec::new())
    }

    // ── Artifact / Briefing ──

    async fn create_artifact(&self, artifact: &NewArtifact) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let id = *self.next_artifact_id.borrow();
        *self.next_artifact_id.borrow_mut() = id + 1;
        self.artifacts.borrow_mut().push(ArtifactData {
            id,
            artifact_type: artifact.artifact_type.clone(),
            entity_id: artifact.entity_id,
            r2_key: artifact.r2_key.clone(),
            schema_version: artifact.schema_version.clone(),
            model: artifact.model.clone(),
            pipeline_version: artifact.pipeline_version.clone(),
            metadata: artifact.metadata.clone(),
            created_at: now,
        });
        Ok(id)
    }

    async fn list_artifacts_by_entity(&self, entity_id: i64, limit: u32) -> Result<Vec<ArtifactEntry>, StoreError> {
        let artifacts = self.artifacts.borrow();
        let mut result: Vec<ArtifactEntry> = artifacts
            .iter()
            .filter(|a| a.entity_id == entity_id)
            .map(|a| ArtifactEntry {
                id: a.id,
                artifact_type: a.artifact_type.clone(),
                entity_id: a.entity_id,
                r2_key: a.r2_key.clone(),
                schema_version: a.schema_version.clone(),
                model: a.model.clone(),
                pipeline_version: a.pipeline_version.clone(),
                metadata: a.metadata.clone(),
                created_at: a.created_at,
            })
            .collect();
        result.reverse();
        let limit = limit as usize;
        result.truncate(limit);
        Ok(result)
    }

    // ── Decision ──

    async fn create_decision(&self, d: &NewDecision) -> Result<i64, StoreError> {
        let now = 1000000;
        let id = *self.next_decision_id.borrow();
        *self.next_decision_id.borrow_mut() = id + 1;
        self.decisions.borrow_mut().push(Decision {
            id,
            signal_thread_id: d.signal_thread_id,
            actor_id: d.actor_id,
            decision_type: d.decision_type.clone(),
            title: d.title.clone(),
            hypothesis: d.hypothesis.clone(),
            rationale: d.rationale.clone(),
            confidence: d.confidence,
            status: "active".into(),
            priority: d.priority.clone(),
            created_at: now,
            updated_at: now,
        });
        Ok(id)
    }
    async fn get_decision(&self, id: i64) -> Result<Option<Decision>, StoreError> {
        Ok(self.decisions.borrow().iter().find(|d| d.id == id).cloned())
    }
    async fn list_decisions(&self, status: Option<&str>, _limit: u32) -> Result<Vec<Decision>, StoreError> {
        let decisions = self.decisions.borrow();
        match status {
            Some(s) => Ok(decisions.iter().filter(|d| d.status == s).cloned().collect()),
            None => Ok(decisions.clone()),
        }
    }
    async fn update_decision_status(&self, id: i64, status: &str) -> Result<(), StoreError> {
        if let Some(d) = self.decisions.borrow_mut().iter_mut().find(|d| d.id == id) {
            d.status = status.to_string();
            d.updated_at = 1000000;
        }
        Ok(())
    }
    async fn decisions_by_signal(&self, signal_thread_id: i64) -> Result<Vec<Decision>, StoreError> {
        Ok(self.decisions.borrow().iter().filter(|d| d.signal_thread_id == Some(signal_thread_id)).cloned().collect())
    }
    async fn decision_stats(&self) -> Result<DecisionStats, StoreError> {
        let decisions = self.decisions.borrow();
        let evals = self.evaluations.borrow();
        Ok(DecisionStats {
            total_decisions: decisions.len() as i64,
            active: decisions.iter().filter(|d| d.status == "active").count() as i64,
            completed: decisions.iter().filter(|d| d.status == "completed").count() as i64,
            superseded: decisions.iter().filter(|d| d.status == "superseded").count() as i64,
            by_type: vec![],
            by_priority: vec![],
            evaluation_summary: EvalSummary {
                total_evaluated: evals.len() as i64,
                confirmed: evals.iter().filter(|e| matches!(e.evaluation, crate::EvaluationResult::Confirmed)).count()
                    as i64,
                partially_confirmed: evals
                    .iter()
                    .filter(|e| matches!(e.evaluation, crate::EvaluationResult::PartiallyConfirmed))
                    .count() as i64,
                contradicted: evals
                    .iter()
                    .filter(|e| matches!(e.evaluation, crate::EvaluationResult::Contradicted))
                    .count() as i64,
                inconclusive: evals
                    .iter()
                    .filter(|e| matches!(e.evaluation, crate::EvaluationResult::Inconclusive))
                    .count() as i64,
                accuracy_rate: 0.0,
            },
            top_signals: vec![],
        })
    }

    // ── Outcome ──

    async fn create_outcome(&self, e: &NewOutcomeEvent) -> Result<i64, StoreError> {
        let now = 1000000;
        let id = *self.next_outcome_id.borrow();
        *self.next_outcome_id.borrow_mut() = id + 1;
        let observed_at = e.observed_at.unwrap_or(now);
        self.outcomes.borrow_mut().push(OutcomeEvent {
            id,
            decision_id: e.decision_id,
            outcome_type: e.outcome_type.clone(),
            observation: e.observation.clone(),
            evidence_url: e.evidence_url.clone(),
            observed_at,
            created_at: now,
        });
        Ok(id)
    }
    async fn get_decision_outcomes(&self, decision_id: i64) -> Result<Vec<OutcomeEvent>, StoreError> {
        Ok(self.outcomes.borrow().iter().filter(|o| o.decision_id == decision_id).cloned().collect())
    }

    // ── Evaluation ──

    async fn create_evaluation(&self, e: &NewDecisionEvaluation) -> Result<i64, StoreError> {
        let now = 1000000;
        let id = *self.next_decision_id.borrow();
        *self.next_decision_id.borrow_mut() = id + 1;
        let evaluated_at = e.evaluated_at.unwrap_or(now);
        self.evaluations.borrow_mut().push(DecisionEvaluation {
            id,
            decision_id: e.decision_id,
            evaluation: e.evaluation.clone(),
            confidence: e.confidence,
            reasoning: e.reasoning.clone(),
            evaluator: e.evaluator.clone(),
            evaluated_at,
            created_at: now,
        });
        Ok(id)
    }
    async fn get_decision_evaluations(&self, decision_id: i64) -> Result<Vec<DecisionEvaluation>, StoreError> {
        Ok(self.evaluations.borrow().iter().filter(|e| e.decision_id == decision_id).cloned().collect())
    }
    async fn get_latest_evaluation(&self, decision_id: i64) -> Result<Option<DecisionEvaluation>, StoreError> {
        let result: Vec<DecisionEvaluation> =
            self.evaluations.borrow().iter().filter(|e| e.decision_id == decision_id).cloned().collect();
        Ok(result.into_iter().last())
    }

    // ── Object Outbox ──

    async fn insert_outbox(&self, entry: &NewOutbox) -> Result<i64, StoreError> {
        let now = 1000000;
        let id = *self.next_outbox_id.borrow();
        *self.next_outbox_id.borrow_mut() = id + 1;
        self.outbox.borrow_mut().push(OutboxEntry {
            id,
            object_type: entry.object_type.clone(),
            object_key: entry.object_key.clone(),
            payload: entry.payload.clone(),
            status: "pending".into(),
            created_at: now,
            retry_count: 0,
        });
        Ok(id)
    }
    async fn drain_outbox(&self, limit: u32) -> Result<Vec<OutboxEntry>, StoreError> {
        let outbox = self.outbox.borrow();
        let pending: Vec<OutboxEntry> = outbox
            .iter()
            .filter(|e| e.status == "pending")
            .take(limit as usize)
            .cloned()
            .collect();
        Ok(pending)
    }
    async fn mark_outbox_archived(&self, id: i64) -> Result<(), StoreError> {
        if let Some(e) = self.outbox.borrow_mut().iter_mut().find(|e| e.id == id) {
            e.status = "archived".into();
        }
        Ok(())
    }
    async fn mark_outbox_failed(&self, id: i64) -> Result<(), StoreError> {
        if let Some(e) = self.outbox.borrow_mut().iter_mut().find(|e| e.id == id && e.retry_count >= 3) {
            e.status = "failed".into();
            e.retry_count += 1;
        }
        Ok(())
    }

    // ── Memory Artifacts ──

    async fn put_artifact(&self, artifact: &NewArtifactRecord) -> Result<i64, StoreError> {
        let now = 1000000;
        let id = *self.next_memory_artifact_id.borrow();
        *self.next_memory_artifact_id.borrow_mut() = id + 1;
        self.memory_artifacts.borrow_mut().push(ArtifactRecord {
            id,
            artifact_type: artifact.artifact_type.clone(),
            artifact_date: artifact.artifact_date.clone(),
            object_key: artifact.object_key.clone(),
            schema_version: artifact.schema_version,
            content_hash: artifact.content_hash.clone(),
            size_bytes: artifact.size_bytes,
            metadata: artifact.metadata.clone(),
            created_at: now,
        });
        Ok(id)
    }
    async fn get_artifact(&self, artifact_type: &str, date: &str) -> Result<Option<ArtifactRecord>, StoreError> {
        Ok(self
            .memory_artifacts
            .borrow()
            .iter()
            .find(|a| a.artifact_type == artifact_type && a.artifact_date == date)
            .cloned())
    }
    async fn list_artifacts(&self, artifact_type: &str, limit: u32) -> Result<Vec<ArtifactRecord>, StoreError> {
        let mut results: Vec<ArtifactRecord> = self
            .memory_artifacts
            .borrow()
            .iter()
            .filter(|a| a.artifact_type == artifact_type)
            .cloned()
            .collect();
        results.reverse();
        results.truncate(limit as usize);
        Ok(results)
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
        let now = 1000000;
        let id = *self.next_event_archive_id.borrow();
        *self.next_event_archive_id.borrow_mut() = id + 1;
        self.event_archive.borrow_mut().push(EventIndexEntry {
            id,
            event_id: event_id.to_string(),
            aggregate_type: aggregate_type.to_string(),
            aggregate_id: aggregate_id.to_string(),
            event_type: event_type.to_string(),
            object_key: object_key.to_string(),
            occurred_at,
            created_at: now,
        });
        Ok(())
    }
    async fn find_event_keys(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        limit: u32,
    ) -> Result<Vec<EventIndexEntry>, StoreError> {
        let mut results: Vec<EventIndexEntry> = self
            .event_archive
            .borrow()
            .iter()
            .filter(|e| e.aggregate_type == aggregate_type && e.aggregate_id == aggregate_id)
            .cloned()
            .collect();
        results.reverse();
        results.truncate(limit as usize);
        Ok(results)
    }

    // ── Reflection Engine (Sprint 5.4) ──

    async fn create_reflection(&self, req: &NewReflection) -> Result<i64, StoreError> {
        let now = 1000000;
        let id = *self.next_reflection_id.borrow();
        *self.next_reflection_id.borrow_mut() = id + 1;
        self.reflections.borrow_mut().insert(req.decision_id, Reflection {
            id,
            decision_id: req.decision_id,
            outcome_id: req.outcome_id,
            job_id: req.job_id.clone(),
            status: req.status.clone(),
            artifact_key: None,
            result: None,
            quality_score: None,
            generator_version: Some("reflection-v1".into()),
            lessons_count: 0,
            rules_count: 0,
            generated_by: "system".into(),
            retry_count: 0,
            last_error: None,
            started_at: None,
            lease_until: None,
            created_at: now,
            updated_at: now,
        });
        Ok(id)
    }

    async fn update_reflection(&self, req: &UpdateReflection) -> Result<(), StoreError> {
        let mut map = self.reflections.borrow_mut();
        let r = map.values_mut().find(|r| r.id == req.id);
        if let Some(r) = r {
            r.status = req.status.clone();
            if let Some(v) = &req.result { r.result = Some(v.clone()); }
            if let Some(v) = req.quality_score { r.quality_score = Some(v); }
            if let Some(v) = &req.artifact_key { r.artifact_key = Some(v.clone()); }
            if let Some(v) = req.lessons_count { r.lessons_count = v; }
            if let Some(v) = req.rules_count { r.rules_count = v; }
            if let Some(v) = req.retry_count { r.retry_count = v; }
            if let Some(v) = &req.last_error { r.last_error = Some(v.clone()); }
            if let Some(v) = req.started_at { r.started_at = Some(v); }
            if let Some(v) = req.lease_until { r.lease_until = Some(v); }
            r.updated_at = 1000000;
        }
        Ok(())
    }

    async fn get_reflection_by_decision(&self, decision_id: i64) -> Result<Option<Reflection>, StoreError> {
        Ok(self.reflections.borrow().get(&decision_id).cloned())
    }

    async fn decisions_eligible_for_reflection(&self, _now: i64, _limit: u32) -> Result<Vec<i64>, StoreError> {
        // MemoryStore has no completed decisions; return empty
        Ok(Vec::new())
    }

    async fn failed_reflections_for_retry(&self, _limit: u32) -> Result<Vec<Reflection>, StoreError> {
        Ok(self.reflections.borrow().values().filter(|r| r.status == "failed" && r.retry_count < 3).cloned().collect())
    }

    async fn stale_generating_reflections(&self, _now: i64) -> Result<Vec<Reflection>, StoreError> {
        Ok(Vec::new())
    }

    // ===== Memory Engine (Sprint 5.5) =====

    async fn create_memory(&self, entry: &NewMemory) -> Result<i64, StoreError> {
        let now = 1000000;
        let id = *self.next_memory_id.borrow();
        *self.next_memory_id.borrow_mut() = id + 1;
        self.memories.borrow_mut().insert(id, Memory {
            id,
            memory_type: entry.memory_type.clone(),
            memory_origin: entry.memory_origin.clone(),
            statement: entry.statement.clone(),
            confidence: entry.confidence,
            stability_score: entry.stability_score,
            confidence_updated_at: None,
            memory_sources: entry.memory_sources.clone(),
            artifact_key: entry.artifact_key.clone(),
            status: entry.status.clone(),
            usage_count: 0,
            validation_count: 0,
            promoted_at: now,
            deprecated_at: None,
            last_used_at: None,
            created_at: now,
        });
        Ok(id)
    }

    async fn get_memory(&self, id: i64) -> Result<Option<Memory>, StoreError> {
        Ok(self.memories.borrow().get(&id).cloned())
    }

    async fn list_memories(&self, memory_type: Option<&str>, status: Option<&str>, _limit: u32) -> Result<Vec<Memory>, StoreError> {
        let memories = self.memories.borrow();
        let result: Vec<Memory> = memories
            .values()
            .filter(|m| {
                let type_match = memory_type.map_or(true, |t| m.memory_type == t);
                let status_match = status.map_or(true, |s| m.status == s);
                type_match && status_match
            })
            .cloned()
            .collect();
        // Note: limit not applied for MemoryStore simplicity
        Ok(result)
    }

    async fn touch_memory(&self, id: i64, now: i64) -> Result<(), StoreError> {
        if let Some(m) = self.memories.borrow_mut().get_mut(&id) {
            m.usage_count += 1;
            m.last_used_at = Some(now);
        }
        Ok(())
    }

    async fn count_candidate_memories(&self) -> Result<i64, StoreError> {
        let count = self.memories.borrow().values().filter(|m| m.status == "candidate").count() as i64;
        Ok(count)
    }

    // ===== Context Engine (Sprint 5.6) =====

    async fn save_context_snapshot(&self, snap: &NewContextSnapshot) -> Result<(), StoreError> {
        self.snapshots.borrow_mut().insert(
            snap.id.clone(),
            ContextSnapshot {
                id: snap.id.clone(),
                query: snap.query.clone(),
                intent: snap.intent.clone(),
                domain: snap.domain.clone(),
                engine_version: "context-engine-v1".into(),
                context_json: snap.context_json.clone(),
                object_key: snap.object_key.clone(),
                object_size: snap.object_size,
                evidence_refs: snap.evidence_refs.clone(),
                confidence: snap.confidence,
                user_scope: snap.user_scope.clone(),
                created_at: 1000000,
            },
        );
        Ok(())
    }

    async fn get_context_snapshot(&self, id: &str) -> Result<Option<ContextSnapshot>, StoreError> {
        Ok(self.snapshots.borrow().get(id).cloned())
    }
}
