//! ReflectionEngine — Decision Learning Loop's feedback node.
//!
//! Orchestrates the pipeline:
//!   ContextBuilder → completeness check → Generator (LLM) → Validation → Persister
//!
//! Design principle: domain service never writes artifact storage directly.
//! All durable projections flow through D1 state + outbox.

use event_store::{AggregateRef, EventEnvelope, EventMetadata, EventStore, keys as event_keys};
use store::{NewOutbox, NewReflection, StoreBackend, UpdateReflection};

use crate::context::ReflectionContextBuilder;
use crate::generator::ReflectionGenerator;
use crate::validation;

/// Trigger source for a reflection job.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReflectionTrigger {
    Api,
    Cron,
}

/// A reflection job — the unit of work for the engine.
#[derive(Debug, Clone)]
pub struct ReflectionJob {
    pub decision_id: i64,
    pub trigger: ReflectionTrigger,
    pub correlation_id: String,
}

/// The result of executing a reflection job.
#[derive(Debug)]
pub struct ReflectionResult {
    pub reflection_id: i64,
    pub decision_id: i64,
    pub status: String,
}

/// ReflectionEngine — domain service.
///
/// Generic over repository (StoreBackend), event store, and LLM generator.
pub struct ReflectionEngine<R, E, G>
where
    R: StoreBackend,
    E: EventStore,
    G: ReflectionGenerator,
{
    repository: R,
    event_store: E,
    generator: G,
}

impl<R, E, G> ReflectionEngine<R, E, G>
where
    R: StoreBackend,
    E: EventStore,
    G: ReflectionGenerator,
{
    pub fn new(repository: R, event_store: E, generator: G) -> Self {
        Self { repository, event_store, generator }
    }

    /// Access the underlying repository (for cron housekeeping queries).
    pub fn repository(&self) -> &R {
        &self.repository
    }

    fn now() -> i64 {
        (js_sys::Date::now() / 1000.0) as i64
    }

    fn job_id(decision_id: i64, now: i64) -> String {
        format!("job_reflect_DEC{decision_id:06}_{now}")
    }

    /// Execute a reflection job: load context → check completeness → LLM → validate → persist.
    pub async fn execute(&self, job: &ReflectionJob) -> Result<ReflectionResult, String> {
        let now = Self::now();
        let correlation_id = job.correlation_id.clone();
        let decision_id = job.decision_id;

        // 1. Create reflection row (status=pending→generating)
        let new_reflection = NewReflection {
            decision_id,
            outcome_id: None,
            job_id: Some(Self::job_id(decision_id, now)),
            status: "generating".into(),
        };
        let reflection_id = self.repository.create_reflection(&new_reflection).await
            .map_err(|e| format!("create_reflection failed: {e}"))?;

        // Start lease
        let _ = self.repository.update_reflection(&UpdateReflection {
            id: reflection_id,
            status: "generating".into(),
            result: None,
            quality_score: None,
            artifact_key: None,
            lessons_count: None,
            rules_count: None,
            retry_count: None,
            last_error: None,
            started_at: Some(now),
            lease_until: Some(now + 900),
        }).await;

        // 2. Build context
        let builder = ReflectionContextBuilder::new(&self.repository);
        let context = builder.build(decision_id).await
            .map_err(|e| {
                let _ = self.mark_failed(reflection_id, &format!("context_error: {e}"));
                format!("context build failed: {e}")
            })?;

        // 3. Completeness check
        if context.completeness_score < 0.4 {
            let msg = format!("insufficient_context (score={:.2})", context.completeness_score);
            let _ = self.mark_failed_with_retry(reflection_id, &msg, 3).await;
            return Err(msg);
        }

        // 4. Generate reflection (LLM)
        let draft = self.generator.generate(&context).await
            .map_err(|e| {
                let _ = self.mark_failed(reflection_id, &format!("llm_error: {e}"));
                format!("LLM generation failed: {e}")
            })?;

        // 5. Validate
        let v = validation::validate(&draft);
        if !v.valid {
            let msg = format!("validation_failed: {}", v.errors.join("; "));
            let _ = self.mark_failed(reflection_id, &msg).await;
            return Err(msg);
        }

        // 6. Success — persist + emit events
        let artifact_key = format!("memory/reflections/REF-{reflection_id:06}.json");
        let _ = self.repository.update_reflection(&UpdateReflection {
            id: reflection_id,
            status: "generated".into(),
            result: Some(draft.result.clone()),
            quality_score: Some(v.quality_score),
            artifact_key: Some(artifact_key.clone()),
            lessons_count: Some(draft.lessons.len() as i64),
            rules_count: Some(draft.rules.len() as i64),
            retry_count: None,
            last_error: None,
            started_at: None,
            lease_until: None,
        }).await;

        // 7. Event outbox (ReflectionGenerated — lightweight)
        let event_payload = serde_json::json!({
            "reflection_id": format!("REF-{reflection_id:06}"),
            "decision_id": format!("DEC-{decision_id:06}"),
            "artifact_key": artifact_key,
            "quality_score": v.quality_score,
            "lesson_count": draft.lessons.len(),
            "rule_count": draft.rules.len(),
        });
        let _ = self.repository.insert_outbox(&NewOutbox {
            object_type: "event:reflection".into(),
            object_key: format!("memory/events/reflection/{}/{}", now, correlation_id),
            payload: event_payload.to_string(),
        }).await;

        // 8. Archive outbox (artifact content — R2 worker will pick up)
        let _ = self.repository.insert_outbox(&NewOutbox {
            object_type: "archive:reflection".into(),
            object_key: artifact_key,
            payload: serde_json::to_string(&draft).unwrap_or_default(),
        }).await;

        // 9. EventStore append
        let _ = self.event_store.append_event(&EventEnvelope {
            schema_version: 1,
            event_version: 1,
            event_id: event_keys::format_id(now, reflection_id as u64),
            correlation_id: correlation_id.clone(),
            causation_id: String::new(),
            aggregate: AggregateRef {
                aggregate_type: "reflection".into(),
                aggregate_id: format!("REF-{reflection_id:06}"),
            },
            event_type: "ReflectionGenerated".into(),
            payload: event_payload,
            metadata: EventMetadata {
                actor: "system".into(),
                source: "reflection_engine".into(),
            },
            occurred_at: now,
            created_at: now,
        }).await;

        Ok(ReflectionResult {
            reflection_id,
            decision_id,
            status: "generated".into(),
        })
    }

    /// Mark a reflection as failed with error message.
    async fn mark_failed(&self, id: i64, error: &str) {
        let ref_lookup = self.repository.get_reflection_by_decision(id).await.ok().flatten();
        let retry_count = ref_lookup.map(|r| r.retry_count + 1).unwrap_or(0);
        let _ = self.repository.update_reflection(&UpdateReflection {
            id,
            status: "failed".into(),
            result: None,
            quality_score: None,
            artifact_key: None,
            lessons_count: None,
            rules_count: None,
            retry_count: Some(retry_count),
            last_error: Some(error.to_string()),
            started_at: None,
            lease_until: None,
        }).await;
    }

    /// Mark failed and set retry_count (used for completeness failures).
    async fn mark_failed_with_retry(&self, id: i64, error: &str, retry_count: i64) {
        let _ = self.repository.update_reflection(&UpdateReflection {
            id,
            status: "failed".into(),
            result: None,
            quality_score: None,
            artifact_key: None,
            lessons_count: None,
            rules_count: None,
            retry_count: Some(retry_count),
            last_error: Some(error.to_string()),
            started_at: None,
            lease_until: None,
        }).await;
    }
}
