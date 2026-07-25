use async_trait::async_trait;

use super::{ArtifactData, EntityInternal, MemoryStore, RelationEdge};
use crate::backend::StoreBackend;
use crate::{
    ArtifactEntry, Decision, EntityActivitySummary, EntityArticle, EntityDetail, EntityRef, EntitySignalCandidate,
    EntitySummary, Feed, IntelligenceSignal, NewArticle, NewArtifact, NewDecision, RelatedEntity, RelatedEntityRef,
    SignalBriefInput, SignalDetail, SignalEvent, SignalThreadFilter, StoreError,
};

#[async_trait(?Send)]
impl StoreBackend for MemoryStore {
    // ===== Feed / Article / Rule domain =====

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
        // Dedup by feed_id + guid (same as D1's INSERT OR IGNORE)
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

    // ===== Entity domain =====

    async fn upsert_entity(&self, name: &str, normalized: &str, entity_type: &str) -> Result<i64, StoreError> {
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let mut entities = self.entities.borrow_mut();

        // Check for existing by normalized_name
        if let Some(existing) = entities.values().find(|e| e.normalized_name == normalized) {
            let existing_id = existing.id;
            let updated_at = now;
            let type_str = entity_type.to_string();
            if let Some(e) = entities.get_mut(&existing_id) {
                e.updated_at = updated_at;
                e.entity_type = type_str;
            }
            return Ok(existing_id);
        }

        let id = *self.next_entity_id.borrow();
        *self.next_entity_id.borrow_mut() = id + 1;
        entities.insert(
            id,
            EntityInternal {
                id,
                name: name.to_string(),
                normalized_name: normalized.to_string(),
                entity_type: entity_type.to_string(),
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

        // Check for existing relation (unique constraint equivalent)
        let existing = edges.iter_mut().find(|e| e.source == source && e.target == target && e.rtype == rtype);
        if let Some(existing) = existing {
            existing.last_seen = now;
            existing.confidence = confidence;
        } else {
            edges.push(RelationEdge {
                source,
                target,
                rtype: rtype.to_string(),
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
            .map(|e| {
                let article_count = links.iter().filter(|(_, eid)| *eid == e.id).count() as i64;
                EntitySummary {
                    id: e.id,
                    name: e.name.clone(),
                    normalized_name: e.normalized_name.clone(),
                    entity_type: e.entity_type.clone(),
                    canonical_id: e.canonical_id,
                    article_count,
                    last_seen: e.updated_at,
                }
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

        Ok(entities.get(&id).map(|e| {
            let article_count = links.iter().filter(|(_, eid)| *eid == e.id).count() as i64;
            EntityDetail {
                id: e.id,
                name: e.name.clone(),
                normalized_name: e.normalized_name.clone(),
                entity_type: e.entity_type.clone(),
                canonical_id: e.canonical_id,
                description: e.description.clone(),
                metadata: e.metadata.clone(),
                article_count,
                created_at: e.created_at,
                updated_at: e.updated_at,
            }
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

        // Collect article IDs linked to this entity (dedup by id)
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
        let paged: Vec<EntityArticle> = if start < ids.len() {
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
        };
        Ok(paged)
    }

    async fn entity_activity_summary(
        &self,
        entity_id: i64,
        _now: i64,
        _days: i64,
    ) -> Result<EntityActivitySummary, StoreError> {
        let links = self.article_entity_links.borrow();

        let article_ids: std::collections::HashSet<i64> =
            links.iter().filter(|(_, eid)| *eid == entity_id).map(|(aid, _)| *aid).collect();

        Ok(EntityActivitySummary {
            article_count: article_ids.len() as i64,
            source_count: 0,
            avg_score: 0.0,
            max_score: 0.0,
            first_seen_at: None,
            last_seen_at: None,
            trend: "stable".into(),
        })
    }

    // ===== Signal domain =====

    async fn entity_signal_candidates(
        &self,
        _now: i64,
        _days: i64,
        _limit: u32,
    ) -> Result<Vec<EntitySignalCandidate>, StoreError> {
        Ok(Vec::new())
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_signal(
        &self,
        _entity_id: Option<i64>,
        title: &str,
        _summary: &str,
        _confidence: f64,
        _impact: &str,
        _trend: &str,
        _article_count: i64,
        _source_count: i64,
        _evidence_ids: &[i64],
        _related_ids: &[i64],
    ) -> Result<i64, StoreError> {
        // MemoryStore: just count signals for test assertions
        self.artifacts.borrow_mut().push(ArtifactData {
            id: 0,
            artifact_type: "signal".into(),
            entity_id: _entity_id.unwrap_or(0),
            r2_key: title.to_string(),
            schema_version: String::new(),
            model: None,
            pipeline_version: String::new(),
            metadata: None,
            created_at: 0,
        });
        Ok(self.artifacts.borrow().len() as i64)
    }

    async fn load_recent_signals(&self, _limit: u32, _offset: u32) -> Result<Vec<IntelligenceSignal>, StoreError> {
        Ok(Vec::new())
    }

    async fn load_signal_by_id(&self, _id: i64) -> Result<Option<IntelligenceSignal>, StoreError> {
        Ok(None)
    }

    async fn entity_signals(&self, _entity_id: i64, _limit: u32) -> Result<Vec<IntelligenceSignal>, StoreError> {
        Ok(Vec::new())
    }

    async fn upsert_signal_thread(
        &self,
        _signal_key: &str,
        _anchor_entity_id: Option<i64>,
        _title: &str,
        _status: &str,
    ) -> Result<i64, StoreError> {
        Ok(1)
    }

    async fn append_signal_instance(
        &self,
        _thread_id: i64,
        _confidence: f64,
        _impact: &str,
        _trend: &str,
        _article_count: i64,
        _source_count: i64,
    ) -> Result<i64, StoreError> {
        Ok(1)
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

    // ===== Artifact / Briefing domain =====

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

    async fn load_signal_detail(&self, _thread_id: i64) -> Result<Option<SignalDetail>, StoreError> {
        Ok(None)
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
        let now = 1000000; // fixed timestamp for deterministic tests
        self.signal_events.borrow_mut().push(SignalEvent {
            id,
            thread_id,
            event_type: event_type.to_string(),
            payload: payload.map(|s| s.to_string()),
            created_at: now,
        });
        Ok(())
    }

    async fn load_signal_events(&self, thread_id: i64, _limit: u32) -> Result<Vec<SignalEvent>, StoreError> {
        let events = self.signal_events.borrow();
        let result: Vec<SignalEvent> = events.iter().filter(|e| e.thread_id == thread_id).cloned().collect();
        Ok(result)
    }

    async fn load_thread_related_entities(
        &self,
        _thread_id: i64,
        _limit: u32,
    ) -> Result<Vec<RelatedEntityRef>, StoreError> {
        Ok(Vec::new())
    }

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
        let decisions = self.decisions.borrow();
        Ok(decisions.iter().find(|d| d.id == id).cloned())
    }

    async fn list_decisions(&self, status: Option<&str>, _limit: u32) -> Result<Vec<Decision>, StoreError> {
        let decisions = self.decisions.borrow();
        match status {
            Some(s) => {
                let filtered: Vec<Decision> = decisions.iter().filter(|d| d.status == s).cloned().collect();
                Ok(filtered)
            }
            None => Ok(decisions.clone()),
        }
    }

    async fn update_decision_status(&self, id: i64, status: &str) -> Result<(), StoreError> {
        let mut decisions = self.decisions.borrow_mut();
        if let Some(d) = decisions.iter_mut().find(|d| d.id == id) {
            d.status = status.to_string();
            d.updated_at = 1000000;
        }
        Ok(())
    }

    async fn decisions_by_signal(&self, signal_thread_id: i64) -> Result<Vec<Decision>, StoreError> {
        let decisions = self.decisions.borrow();
        let filtered: Vec<Decision> =
            decisions.iter().filter(|d| d.signal_thread_id == Some(signal_thread_id)).cloned().collect();
        Ok(filtered)
    }
}
