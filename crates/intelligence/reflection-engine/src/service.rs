//! ReflectionEngine — Decision Learning Loop's feedback node.
//!
//! Orchestrates the pipeline:
//!   ContextBuilder → completeness check → Generator (LLM) → Validation → Persister
//!
//! Design principle: domain service never writes artifact storage directly.
//! All durable projections flow through the repository port (D1 state), the
//! injected event log, and the artifact registry.
//!
//! The event log is the shared-kernel [`EventLog`] port — the engine appends a
//! minimal [`DomainEvent`]; an infrastructure adapter supplies envelope
//! metadata and the storage backend.

use shared_kernel::artifact_registry::{ArtifactRef, ArtifactRegistry, NewArtifact};
use shared_kernel::event_log::{DomainEvent, EventLog};

use crate::context::ReflectionContextBuilder;
use crate::generator::ReflectionGenerator;
use crate::repository::{ReflectionRepository, ReflectionUpdate};
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
/// Generic over:
/// - `R`: Reflection persistence ([`ReflectionRepository`])
/// - `E`: Event log ([`EventLog`])
/// - `G`: LLM generator
/// - `A`: Artifact registry for large-object storage (R2)
pub struct ReflectionEngine<R, E, G, A>
where
    R: ReflectionRepository,
    E: EventLog,
    G: ReflectionGenerator,
    A: ArtifactRegistry,
{
    repository: R,
    event_log: E,
    generator: G,
    artifact_registry: A,
}

impl<R, E, G, A> ReflectionEngine<R, E, G, A>
where
    R: ReflectionRepository,
    E: EventLog,
    G: ReflectionGenerator,
    A: ArtifactRegistry,
{
    pub fn new(repository: R, event_log: E, generator: G, artifact_registry: A) -> Self {
        Self { repository, event_log, generator, artifact_registry }
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
        self.execute_at(job, now).await
    }

    /// [`execute`] with the wall clock injected, so the engine is testable
    /// natively (the real `now()` calls into JS and panics off-wasm).
    #[allow(clippy::let_underscore_future)]
    async fn execute_at(&self, job: &ReflectionJob, now: i64) -> Result<ReflectionResult, String> {
        let correlation_id = job.correlation_id.clone();
        let decision_id = job.decision_id;

        // 1. Create reflection row (status=pending→generating)
        let reflection_id = self
            .repository
            .create(decision_id, &Self::job_id(decision_id, now))
            .await
            .map_err(|e| format!("create_reflection failed: {e}"))?;

        // Start lease
        let _ = self
            .repository
            .update(&ReflectionUpdate {
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
            })
            .await;

        // 2. Build context
        let builder = ReflectionContextBuilder::new(&self.repository);
        let context = builder.build(decision_id).await.map_err(|e| {
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
        let draft = self.generator.generate(&context).await.map_err(|e| {
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

        // 6. Store full reflection artifact via ArtifactRegistry → R2
        let draft_json = serde_json::to_string(&draft).unwrap_or_default();
        let artifact_result = self
            .artifact_registry
            .store(NewArtifact {
                artifact_type: "reflection_result".into(),
                owner_type: "reflection".into(),
                owner_id: format!("REF-{reflection_id:06}"),
                content: draft_json.as_bytes().to_vec(),
                content_type: "application/json".into(),
            })
            .await;

        // Non-fatal: if ArtifactRegistry is unavailable, fall back to legacy
        // artifact_key-based storage (outbox → R2 archive via cron).
        let artifact_ref: Option<ArtifactRef> = artifact_result.ok();

        let artifact_key = artifact_ref
            .as_ref()
            .map(|r| r.object_key.clone())
            .unwrap_or_else(|| format!("memory/reflections/REF-{reflection_id:06}.json"));

        // 7. Persist reflection result in D1 (dual-write: artifact_key + result for backward compat)
        let _ = self
            .repository
            .update(&ReflectionUpdate {
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
            })
            .await;

        // 8. Event outbox (ReflectionGenerated — lightweight)
        let event_payload = serde_json::json!({
            "reflection_id": format!("REF-{reflection_id:06}"),
            "decision_id": format!("DEC-{decision_id:06}"),
            "artifact_key": artifact_key,
            "artifact_id": artifact_ref.as_ref().map(|r| r.artifact_id),
            "quality_score": v.quality_score,
            "lesson_count": draft.lessons.len(),
            "rule_count": draft.rules.len(),
        });
        let _ = self
            .repository
            .enqueue_event(
                "event:reflection",
                &format!("memory/events/reflection/{}/{}", now, correlation_id),
                &event_payload,
            )
            .await;

        // 9. Event log append (same payload, second sink — dual write)
        let _ = self
            .event_log
            .append(&DomainEvent {
                event_type: "ReflectionGenerated".into(),
                aggregate_type: "reflection".into(),
                aggregate_id: format!("REF-{reflection_id:06}"),
                payload: event_payload,
                occurred_at: now,
                correlation_id: correlation_id.clone(),
            })
            .await;

        Ok(ReflectionResult { reflection_id, decision_id, status: "generated".into() })
    }

    /// Mark a reflection as failed with error message.
    ///
    /// The retry-count lookup mirrors the pre-decoupling behaviour exactly: it
    /// reads whatever row [`ReflectionRepository::find_latest_for_decision`]
    /// resolves for the id passed in. Switching that lookup to a true by-id read
    /// is a behavioural fix intentionally left out of the porting change.
    async fn mark_failed(&self, id: i64, error: &str) {
        let ref_lookup = self.repository.find_latest_for_decision(id).await.ok().flatten();
        let retry_count = ref_lookup.map(|r| r.retry_count + 1).unwrap_or(0);
        let _ = self
            .repository
            .update(&ReflectionUpdate {
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
            })
            .await;
    }

    /// Mark failed and set retry_count (used for completeness failures).
    async fn mark_failed_with_retry(&self, id: i64, error: &str, retry_count: i64) {
        let _ = self
            .repository
            .update(&ReflectionUpdate {
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
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use futures::executor::LocalPool;
    use shared_kernel::artifact_registry::{ArtifactRef, ArtifactRegistry, NewArtifact, RegistryError};
    use shared_kernel::event_log::{DomainEvent, EventLog, EventLogError};

    use super::*;
    use crate::context::{OutcomeSnapshot, ReflectionContext};
    use crate::error::ReflectionError;
    use crate::generator::{LessonDraft, ReflectionDraft, RuleDraft};
    use crate::repository::{DecisionFacts, ReflectionRecord};

    const DECISION_ID: i64 = 42;
    const NOW: i64 = 1_700_000_000;

    #[derive(Default)]
    struct RepoState {
        created_id: i64,
        enqueued: Vec<(String, String, serde_json::Value)>,
    }

    struct FakeRepo {
        state: Rc<RefCell<RepoState>>,
    }

    impl FakeRepo {
        fn new() -> (Self, Rc<RefCell<RepoState>>) {
            let state = Rc::new(RefCell::new(RepoState::default()));
            (Self { state: state.clone() }, state)
        }
    }

    #[async_trait::async_trait(?Send)]
    impl ReflectionRepository for FakeRepo {
        async fn create(&self, _decision_id: i64, _job_id: &str) -> Result<i64, ReflectionError> {
            let mut state = self.state.borrow_mut();
            state.created_id += 1;
            Ok(state.created_id)
        }

        async fn update(&self, _update: &ReflectionUpdate) -> Result<(), ReflectionError> {
            Ok(())
        }

        async fn find_latest_for_decision(
            &self,
            decision_id: i64,
        ) -> Result<Option<ReflectionRecord>, ReflectionError> {
            let id = self.state.borrow().created_id;
            Ok(Some(ReflectionRecord { id, decision_id, retry_count: 0 }))
        }

        async fn load_decision_context(&self, decision_id: i64) -> Result<Option<DecisionFacts>, ReflectionError> {
            Ok(Some(DecisionFacts {
                decision_id,
                title: "Test decision".into(),
                decision_type: "investment".into(),
                hypothesis: Some("adoption follows capability".into()),
                confidence: 0.8,
                outcome: Some(OutcomeSnapshot { id: 1, outcome_type: "success".into(), observation: "adopted".into() }),
                evaluations: Vec::new(),
            }))
        }

        async fn enqueue_event(
            &self,
            object_type: &str,
            object_key: &str,
            payload: &serde_json::Value,
        ) -> Result<(), ReflectionError> {
            self.state.borrow_mut().enqueued.push((object_type.into(), object_key.into(), payload.clone()));
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeLog {
        state: Rc<RefCell<Vec<DomainEvent>>>,
    }

    impl FakeLog {
        fn new() -> (Self, Rc<RefCell<Vec<DomainEvent>>>) {
            let state = Rc::new(RefCell::new(Vec::new()));
            (Self { state: state.clone() }, state)
        }
    }

    #[async_trait::async_trait(?Send)]
    impl EventLog for FakeLog {
        async fn append(&self, event: &DomainEvent) -> Result<(), EventLogError> {
            self.state.borrow_mut().push(event.clone());
            Ok(())
        }
    }

    struct FakeArtifacts;

    #[async_trait::async_trait(?Send)]
    impl ArtifactRegistry for FakeArtifacts {
        async fn store(&self, artifact: NewArtifact) -> Result<ArtifactRef, RegistryError> {
            Ok(ArtifactRef {
                artifact_id: 7,
                artifact_type: artifact.artifact_type,
                storage: "r2".into(),
                object_key: format!("artifacts/{}", artifact.owner_id),
                size_bytes: artifact.content.len() as i64,
                created_at: NOW,
            })
        }
        async fn read(&self, _artifact_id: i64) -> Result<Option<Vec<u8>>, RegistryError> {
            Ok(None)
        }
        async fn find_by_owner(
            &self,
            _artifact_type: &str,
            _owner_type: &str,
            _owner_id: &str,
        ) -> Result<Option<ArtifactRef>, RegistryError> {
            Ok(None)
        }
    }

    struct FakeGenerator;

    #[async_trait::async_trait(?Send)]
    impl ReflectionGenerator for FakeGenerator {
        async fn generate(&self, _context: &ReflectionContext) -> Result<ReflectionDraft, String> {
            Ok(ReflectionDraft {
                result: "wrong".into(),
                confidence_calibration: "overestimated".into(),
                quality_score: 0.8,
                lessons: vec![LessonDraft {
                    category: "assumption_error".into(),
                    domain: "investment".into(),
                    description: "The adoption-timeline assumption proved materially wrong.".into(),
                    severity: "high".into(),
                    confidence: 0.9,
                    evidence_basis: vec!["OUT-1".into()],
                }],
                rules: vec![RuleDraft {
                    condition_domain: "investment".into(),
                    condition_trigger: "new capability claim".into(),
                    action_type: "require_validation".into(),
                    action_instruction: "verify adoption before acting".into(),
                    confidence: 0.85,
                }],
            })
        }
    }

    /// Hard regression for the step-8/step-9 dual write: a successful execute
    /// must enqueue the outbox payload AND append the same payload to the
    /// EventLog, with consistent event type / aggregate / correlation.
    #[test]
    fn step8_outbox_and_step9_event_log_dual_write() {
        let (repo, repo_state) = FakeRepo::new();
        let (log, log_state) = FakeLog::new();
        let engine = ReflectionEngine::new(repo, log, FakeGenerator, FakeArtifacts);

        let mut pool = LocalPool::new();
        let job = ReflectionJob {
            decision_id: DECISION_ID,
            trigger: ReflectionTrigger::Api,
            correlation_id: "corr-abc".into(),
        };
        let result = pool.run_until(engine.execute_at(&job, NOW)).expect("execute should succeed");

        assert_eq!(result.reflection_id, 1, "first reflection gets id 1");
        assert_eq!(result.decision_id, DECISION_ID);
        assert_eq!(result.status, "generated");

        // (a) step 8 — outbox enqueue happened once. Read the record, then
        // release the RefCell borrow before touching the event log below.
        let (object_type, object_key, outbox_payload) = {
            let repo = repo_state.borrow();
            assert_eq!(repo.enqueued.len(), 1, "outbox enqueue must fire exactly once");
            let (t, k, p) = &repo.enqueued[0];
            (t.clone(), k.clone(), p.clone())
        };
        assert_eq!(object_type, "event:reflection");
        assert!(object_key.contains("memory/events/reflection/"), "outbox object key: {object_key}");
        assert_eq!(outbox_payload["reflection_id"], "REF-000001");
        assert_eq!(outbox_payload["decision_id"], "DEC-000042");

        // (b) step 9 — EventLog.append received the mapped DomainEvent, and its
        // payload matches the outbox payload byte-for-byte (dual write).
        let log = log_state.borrow();
        assert_eq!(log.len(), 1, "event-log append must fire exactly once");
        let ev = &log[0];
        assert_eq!(ev.event_type, "ReflectionGenerated");
        assert_eq!(ev.aggregate_type, "reflection");
        assert_eq!(ev.aggregate_id, "REF-000001");
        assert_eq!(ev.occurred_at, NOW);
        assert_eq!(ev.correlation_id, "corr-abc");
        assert_eq!(ev.payload, outbox_payload, "outbox and event-log payloads must agree");
    }

    /// The dual write must be skipped entirely on failure (validation error):
    /// neither sink records anything.
    #[test]
    fn failed_execute_writes_no_event() {
        let (repo, repo_state) = FakeRepo::new();
        let (log, log_state) = FakeLog::new();

        struct FailingGenerator;
        #[async_trait::async_trait(?Send)]
        impl ReflectionGenerator for FailingGenerator {
            async fn generate(&self, _context: &ReflectionContext) -> Result<ReflectionDraft, String> {
                // Invalid: empty lessons → validation failure before step 8/9.
                Ok(ReflectionDraft {
                    result: "wrong".into(),
                    confidence_calibration: "accurate".into(),
                    quality_score: 0.5,
                    lessons: Vec::new(),
                    rules: Vec::new(),
                })
            }
        }

        let engine = ReflectionEngine::new(repo, log, FailingGenerator, FakeArtifacts);
        let mut pool = LocalPool::new();
        let job = ReflectionJob {
            decision_id: DECISION_ID,
            trigger: ReflectionTrigger::Api,
            correlation_id: "corr-abc".into(),
        };
        let result = pool.run_until(engine.execute_at(&job, NOW));
        assert!(result.is_err(), "validation failure must fail execute");

        assert!(repo_state.borrow().enqueued.is_empty(), "no outbox write on failure");
        assert!(log_state.borrow().is_empty(), "no event-log write on failure");
    }
}
