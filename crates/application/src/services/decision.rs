//! Decision Application Service — real use-case orchestration over the
//! `decision-engine` aggregate (D2 vertical, P3).
//!
//! Orchestrates the Decision aggregate and persists its state through the
//! aggregate's own repository. It does **not** write outbox events and does
//! **not** know about `EventEnvelope`: it returns typed results carrying the
//! drained domain events + final state, and the delivery layer (`worker-entry`)
//! maps those to outbox envelopes (decision-vertical plan, SD-C adapter model).
//!
//! Generic over two narrow seams:
//! - `R: decision_engine::DecisionRepository` — aggregate save/find (hydrate)
//! - `S: DecisionIdSource + OutcomeRepository + EvaluationRepository` — the
//!   store that allocates decision ids and persists `outcome_events` /
//!   `decision_evaluations` fact rows (D1Store implements all three; delivery
//!   passes the same store the repository wraps).
//!
//! Lifecycle changes happen **only** through aggregate behavioural methods
//! (`propose` / `approve` / `execute` / `complete` / `invalidate`) — there is no
//! raw status write (aggregate invariant #5, P1). Events are drained **only
//! after** the state save succeeds, so a failed persist never leaks events
//! (SD-D failure-recovery invariant).

use decision_engine::{
    ApproveDecision, DecisionAggregate, DecisionDomainEvent, DecisionRepository, DecisionStatus, ExecuteDecision,
    InvalidateDecision, ObservedOutcome, OutcomeVerdict, ProposeDecision, RecordOutcome,
};
use domain::{DecisionIdSource, EvaluationRepository, NewDecisionEvaluation, NewOutcomeEvent, OutcomeRepository};

/// Re-exported so delivery-layer callers can map failures without depending on
/// `decision-engine` directly.
pub use decision_engine::DecisionError;

/// The Decision application service.
pub struct DecisionService<R, S> {
    repo: R,
    store: S,
}

/// Lifecycle transition intent accepted by [`DecisionService::transition`].
///
/// This is the **command token** form of the status endpoint. The delivery
/// layer maps HTTP `status` strings onto it; the use-case turns it into the
/// matching aggregate behavioural method.
#[derive(Debug, Clone)]
pub enum DecisionLifecycleCommand {
    /// `approve` → Proposed → Approved.
    Approve,
    /// `execute` → Approved → Executing.
    Execute,
    /// `complete` → Executing → Completed (requires an observed outcome).
    Complete,
    /// `invalidate` → any-active → Invalidated.
    Invalidate { reason: String },
}

impl DecisionLifecycleCommand {
    /// The status the command targets, used for idempotent no-op detection
    /// (a decision already at the target status is a success, not an error —
    /// mirrors the legacy idempotent `UPDATE`).
    fn target(&self) -> DecisionStatus {
        match self {
            Self::Approve => DecisionStatus::Approved,
            Self::Execute => DecisionStatus::Executing,
            Self::Complete => DecisionStatus::Completed,
            Self::Invalidate { .. } => DecisionStatus::Invalidated,
        }
    }
}

/// Result of [`DecisionService::create`].
#[derive(Debug)]
pub struct CreatedDecision {
    /// Numeric `decisions.id` — the aggregate id's numeric suffix.
    pub decision_id: i64,
    /// The aggregate after the create walk (events drained).
    pub aggregate: DecisionAggregate,
    /// Domain events drained after a successful save (`Proposed` / `Approved` /
    /// `StatusChanged`). The delivery layer maps these to outbox envelopes.
    pub events: Vec<DecisionDomainEvent>,
}

/// Result of [`DecisionService::transition`].
#[derive(Debug)]
pub struct LifecycleTransition {
    /// Numeric `decisions.id`.
    pub decision_id: i64,
    /// The aggregate after the transition (events drained).
    pub aggregate: DecisionAggregate,
    /// Events drained after the save that persisted the transition.
    pub events: Vec<DecisionDomainEvent>,
    /// `false` when the decision was already at the target status (idempotent
    /// no-op — nothing was saved, `events` is empty).
    pub transitioned: bool,
}

/// Result of [`DecisionService::record_outcome`].
#[derive(Debug)]
pub struct OutcomeRecording {
    /// `outcome_events.id` of the persisted fact row.
    pub outcome_id: i64,
    /// Numeric `decisions.id` the outcome was recorded against.
    pub decision_id: i64,
    /// The aggregate's post-record state (events drained).
    pub aggregate: DecisionAggregate,
    /// `true` when recording the outcome advanced the lifecycle to `Completed`
    /// (SD-A2: only an Executing decision completes on outcome record).
    pub completed: bool,
    /// Events drained after the save (`OutcomeAttached`, and `Completed` when
    /// the decision completed).
    pub events: Vec<DecisionDomainEvent>,
}

/// Result of [`DecisionService::record_evaluation`].
#[derive(Debug)]
pub struct EvaluationRecording {
    /// `decision_evaluations.id` of the persisted judgment row.
    pub evaluation_id: i64,
    /// Numeric `decisions.id` the evaluation was recorded against.
    pub decision_id: i64,
}

impl<R: DecisionRepository, S: DecisionIdSource + OutcomeRepository + EvaluationRepository> DecisionService<R, S> {
    /// Construct the service from its two narrow seams.
    pub fn new(repo: R, store: S) -> Self {
        Self { repo, store }
    }

    /// Create flow — the aggregate id must precede `propose` (its events embed
    /// `DEC-{id}` at proposal time), so the id is allocated from the store's id
    /// space first (the numeric suffix **is** the `decisions` primary key).
    ///
    /// The row write uses `save_new` (insert-or-refuse), **not** `save`
    /// (upsert): two concurrent creates can read the same `MAX(id)+1`
    /// (single-writer allocation, ADR-005), and `save`'s upsert would let the
    /// second silently overwrite the first creator's row. A refused insert means
    /// the id was just claimed by a racing create, so the flow re-allocates and
    /// retries (②, 2026-09-06). Bounded, then a conflict surfaces.
    pub async fn create(&self, cmd: ProposeDecision) -> Result<CreatedDecision, DecisionError> {
        const CREATE_RETRIES: usize = 4;
        for _ in 0..CREATE_RETRIES {
            let decision_id =
                self.store.next_decision_id().await.map_err(|e| DecisionError::Infrastructure(e.to_string()))?;
            let mut proposal = cmd.clone();
            proposal.id = decision_id;

            let mut decision = DecisionAggregate::propose(proposal, Self::now())?;

            // Legacy create landed decisions directly at `'active'` (Executing), and
            // every read side treats `'active'` as "live decision" (dashboard stats,
            // context-engine retrieval, `GET /decisions?status=active`). Reproduce
            // that end-state through the legal DAG steps — propose → approve →
            // execute — rather than a lossy status write (P3 decision, 2026-09-06).
            decision.approve(ApproveDecision { decision_id: decision.id().0.clone(), approved_by: "system".into() })?;
            decision.execute(ExecuteDecision { decision_id: decision.id().0.clone() })?;

            if self.repo.save_new(&decision).await? {
                // Drain only after the save succeeded — on failure the events stay
                // uncommitted with the aggregate (SD-D invariant).
                let events = decision.drain_events();
                return Ok(CreatedDecision { decision_id, aggregate: decision, events });
            }
            // `Ok(false)`: the id raced with a concurrent create — re-allocate
            // and retry. The collided aggregate is dropped, never persisted.
        }
        Err(DecisionError::Conflict("decision id allocation exhausted after repeated create collisions".into()))
    }

    /// Lifecycle transition via the aggregate's named methods. Idempotent: a
    /// decision already at the target status is a success no-op.
    pub async fn transition(
        &self,
        decision_id: i64,
        command: DecisionLifecycleCommand,
    ) -> Result<LifecycleTransition, DecisionError> {
        let domain_id = Self::domain_id(decision_id);
        let mut decision = self.load(&domain_id).await?;

        if decision.status() == &command.target() {
            return Ok(LifecycleTransition {
                decision_id,
                aggregate: decision,
                events: Vec::new(),
                transitioned: false,
            });
        }

        match &command {
            DecisionLifecycleCommand::Approve => {
                decision.approve(ApproveDecision { decision_id: domain_id.clone(), approved_by: "system".into() })?
            }
            DecisionLifecycleCommand::Execute => {
                decision.execute(ExecuteDecision { decision_id: domain_id.clone() })?
            }
            DecisionLifecycleCommand::Complete => decision.complete()?,
            DecisionLifecycleCommand::Invalidate { reason } => {
                decision.invalidate(InvalidateDecision { decision_id: domain_id.clone(), reason: reason.clone() })?
            }
        }

        self.repo.save(&decision).await?;
        let events = decision.drain_events();
        Ok(LifecycleTransition { decision_id, aggregate: decision, events, transitioned: true })
    }

    /// Record a factual outcome observation.
    ///
    /// The `outcome_events` row (the fact layer) is always persisted. Lifecycle
    /// only advances for an **Executing** decision: the observation is attached
    /// to the aggregate and the aggregate completes — a true row flip to
    /// `completed`, fixing the legacy behaviour that stamped "completed" on
    /// every outcome regardless of state (SD-A2). Non-Executing decisions just
    /// record the fact and leave lifecycle untouched.
    pub async fn record_outcome(&self, outcome: &NewOutcomeEvent) -> Result<OutcomeRecording, DecisionError> {
        let decision_id = outcome.decision_id;
        let domain_id = Self::domain_id(decision_id);
        let mut decision = self.load(&domain_id).await?;
        let was_executing = *decision.status() == DecisionStatus::Executing;

        // Fact row first — this is the durable observation.
        let outcome_id =
            self.store.save_outcome(outcome).await.map_err(|e| DecisionError::Infrastructure(e.to_string()))?;

        if !was_executing {
            return Ok(OutcomeRecording {
                outcome_id,
                decision_id,
                aggregate: decision,
                completed: false,
                events: Vec::new(),
            });
        }

        decision
            .attach_outcome(RecordOutcome { decision_id: domain_id.clone(), outcome: Self::observed_outcome(outcome) });
        decision.complete()?;
        self.repo.save(&decision).await?;
        let events = decision.drain_events();
        Ok(OutcomeRecording { outcome_id, decision_id, aggregate: decision, completed: true, events })
    }

    /// Record a judgment about a decision's hypothesis.
    ///
    /// Evaluations are the judgment layer and do not change the decision's
    /// lifecycle. Kept a pure fact insert for byte-parity with the legacy
    /// `create_evaluation` (no existence check, matching prior behaviour).
    pub async fn record_evaluation(
        &self,
        evaluation: &NewDecisionEvaluation,
    ) -> Result<EvaluationRecording, DecisionError> {
        let decision_id = evaluation.decision_id;
        let evaluation_id =
            self.store.save_evaluation(evaluation).await.map_err(|e| DecisionError::Infrastructure(e.to_string()))?;
        Ok(EvaluationRecording { evaluation_id, decision_id })
    }

    // ── Helpers ────────────────────────────────────────────────────

    fn domain_id(decision_id: i64) -> String {
        format!("DEC-{decision_id:06}")
    }

    async fn load(&self, domain_id: &str) -> Result<DecisionAggregate, DecisionError> {
        self.repo.find(domain_id).await?.ok_or_else(|| DecisionError::NotFound(domain_id.to_string()))
    }

    fn now() -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
    }

    /// Transient mapping from the fact-layer `NewOutcomeEvent` onto the
    /// aggregate's `ObservedOutcome` so the completing path satisfies the
    /// aggregate's `Complete` invariant (≥1 observed outcome). The durable
    /// observation is the `outcome_events` row; reconstructing `observed_outcomes`
    /// from that row on `find` is a SD-B backlog item, so this mapping is only
    /// needed to carry the completing request through the aggregate.
    fn observed_outcome(e: &NewOutcomeEvent) -> ObservedOutcome {
        ObservedOutcome {
            metric: e.outcome_type.clone(),
            actual_value: e.observation.clone(),
            outcome_type: OutcomeVerdict::Inconclusive,
            evidence_url: e.evidence_url.clone(),
            observed_at: e.observed_at.unwrap_or_else(Self::now),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use decision_engine::{ExpectedOutcome, ReconstructDecision};
    use futures::executor::block_on;
    use shared_kernel::ids::DecisionId;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    use store::memory::MemoryStore;

    type Svc = DecisionService<MemAggregateRepo, MemoryStore>;

    /// In-memory `decision_engine::DecisionRepository` for use-case tests.
    ///
    /// The plan (§7) said use-case tests would go through the infra
    /// `D1DecisionRepository` + `MemoryStore`; that requires sharing one
    /// `MemoryStore` across the service's repository and fact ports, but
    /// `MemoryStore` is `RefCell`-backed and not `Clone`. This double stores
    /// the aggregate's P1 serde snapshot per id instead, and is `Rc`-shared so
    /// the test can seed / hydrate through the same rows the service writes.
    /// The D1↔aggregate mapping itself stays covered by the infra adapter's own
    /// 12 tests; here we exercise the use-case at the domain seam.
    #[derive(Clone, Default)]
    struct MemAggregateRepo {
        /// aggregate id (`DEC-000001`) → serde snapshot.
        rows: Rc<RefCell<HashMap<String, String>>>,
        /// When set, `save` returns `Err` without writing (failure-recovery).
        fail_saves: bool,
    }

    #[async_trait::async_trait(?Send)]
    impl DecisionRepository for MemAggregateRepo {
        async fn save(&self, decision: &DecisionAggregate) -> Result<(), DecisionError> {
            if self.fail_saves {
                return Err(DecisionError::Infrastructure("injected save failure".into()));
            }
            let snapshot = serde_json::to_string(decision).map_err(|e| DecisionError::Infrastructure(e.to_string()))?;
            self.rows.borrow_mut().insert(decision.id().0.clone(), snapshot);
            Ok(())
        }

        async fn save_new(&self, decision: &DecisionAggregate) -> Result<bool, DecisionError> {
            if self.fail_saves {
                return Err(DecisionError::Infrastructure("injected save failure".into()));
            }
            let snapshot = serde_json::to_string(decision).map_err(|e| DecisionError::Infrastructure(e.to_string()))?;
            let mut rows = self.rows.borrow_mut();
            if rows.contains_key(&decision.id().0) {
                return Ok(false); // id already claimed (racing create) — refuse
            }
            rows.insert(decision.id().0.clone(), snapshot);
            Ok(true)
        }

        async fn find(&self, id: &str) -> Result<Option<DecisionAggregate>, DecisionError> {
            match self.rows.borrow().get(id).cloned() {
                Some(snapshot) => {
                    serde_json::from_str(&snapshot).map(Some).map_err(|e| DecisionError::Infrastructure(e.to_string()))
                }
                None => Ok(None),
            }
        }

        async fn find_by_signal(&self, signal_thread_id: i64) -> Result<Vec<DecisionAggregate>, DecisionError> {
            let rows = self.rows.borrow().clone();
            let mut out = Vec::new();
            for snapshot in rows.into_values() {
                let agg: DecisionAggregate =
                    serde_json::from_str(&snapshot).map_err(|e| DecisionError::Infrastructure(e.to_string()))?;
                if agg.signal_thread_id() == Some(signal_thread_id) {
                    out.push(agg);
                }
            }
            Ok(out)
        }

        async fn list(&self, status: Option<&str>, limit: u32) -> Result<Vec<DecisionAggregate>, DecisionError> {
            let rows = self.rows.borrow().clone();
            let mut out = Vec::new();
            for snapshot in rows.into_values() {
                let agg: DecisionAggregate =
                    serde_json::from_str(&snapshot).map_err(|e| DecisionError::Infrastructure(e.to_string()))?;
                let matches = status.is_none_or(|s| format!("{:?}", agg.status()).to_lowercase() == s);
                if matches {
                    out.push(agg);
                }
            }
            out.truncate(limit as usize);
            Ok(out)
        }
    }

    fn svc(repo: &MemAggregateRepo, store: MemoryStore) -> Svc {
        DecisionService::new(repo.clone(), store)
    }

    fn create_cmd(id: i64) -> ProposeDecision {
        ProposeDecision {
            id,
            title: "Verify the hypothesis".into(),
            hypothesis: Some("X will cause Y".into()),
            confidence: 0.8,
            rationale: Some("Signal evidence".into()),
            decision_type: "experiment".into(),
            priority: "high".into(),
            signal_thread_id: Some(42),
            actor_id: Some(7),
            expected_outcomes: sample_outcomes(),
        }
    }

    fn sample_outcomes() -> Vec<ExpectedOutcome> {
        vec![
            ExpectedOutcome {
                metric: "accuracy".into(),
                expected_value: ">= 0.9".into(),
                measurement_method: "eval".into(),
            },
            ExpectedOutcome {
                metric: "latency".into(),
                expected_value: "< 200ms".into(),
                measurement_method: "p95".into(),
            },
        ]
    }

    /// Seed an aggregate directly at an explicit id/status — the only way to get
    /// a Proposed/Approved row under a create flow that lands at Executing.
    fn seed(repo: &MemAggregateRepo, id: i64, status: DecisionStatus) {
        let agg = DecisionAggregate::reconstruct(ReconstructDecision {
            id: DecisionId::new(id),
            title: format!("Decision {id}"),
            hypothesis: Some("X will cause Y".into()),
            confidence: 0.8,
            status,
            rationale: Some("Based on evidence".into()),
            decision_type: "experiment".into(),
            priority: "high".into(),
            signal_thread_id: None,
            actor_id: Some(7),
            expected_outcomes: vec![],
            observed_outcomes: vec![],
            created_at: 500,
            updated_at: 600,
        });
        block_on(repo.save(&agg)).unwrap();
    }

    fn find(repo: &MemAggregateRepo, decision_id: i64) -> DecisionAggregate {
        block_on(repo.find(&format!("DEC-{decision_id:06}"))).unwrap().expect("row must exist")
    }

    #[test]
    fn create_persists_executing_with_expected_outcomes_and_event_trail() {
        let repo = MemAggregateRepo::default();
        let service = svc(&repo, MemoryStore::new());

        let created = block_on(service.create(create_cmd(0))).unwrap();
        assert!(created.decision_id >= 1, "id must be allocated, got {}", created.decision_id);

        // Aggregate event trail: propose → approve → execute.
        assert!(created.events.iter().any(|e| matches!(e, DecisionDomainEvent::Proposed { .. })));
        assert!(created.events.iter().any(|e| matches!(e, DecisionDomainEvent::Approved { .. })));
        assert!(created
            .events
            .iter()
            .any(|e| { matches!(e, DecisionDomainEvent::StatusChanged { to: DecisionStatus::Executing, .. }) }));

        // Hydrate read-back (gate: request → … → hydrate 回读).
        let hydrated = find(&repo, created.decision_id);
        assert_eq!(*hydrated.status(), DecisionStatus::Executing, "create must land at 'active' (Executing)");
        assert_eq!(hydrated.title(), "Verify the hypothesis");
        assert_eq!(hydrated.hypothesis(), Some("X will cause Y"));
        assert_eq!(hydrated.decision_type(), "experiment");
        assert_eq!(hydrated.priority(), "high");
        assert_eq!(hydrated.signal_thread_id(), Some(42));
        assert_eq!(hydrated.actor_id(), Some(7));
        let eo = hydrated.expected_outcomes();
        assert_eq!(eo.len(), 2, "expected_outcomes must persist through the aggregate row");
        assert_eq!(eo[0].metric, "accuracy");
        assert_eq!(eo[1].metric, "latency");

        // Fresh aggregate holds no pending events after the save (snapshot form).
        let mut fresh = find(&repo, created.decision_id);
        assert!(fresh.drain_events().is_empty());
    }

    #[test]
    fn create_allocates_distinct_sequential_ids() {
        let repo = MemAggregateRepo::default();
        let service = svc(&repo, MemoryStore::new());

        let a = block_on(service.create(create_cmd(0))).unwrap();
        let b = block_on(service.create(create_cmd(0))).unwrap();
        assert!(a.decision_id != b.decision_id, "ids must be distinct");
        assert_eq!(b.decision_id, a.decision_id + 1);
        // Both rows exist and are independently Executing.
        assert_eq!(*find(&repo, a.decision_id).status(), DecisionStatus::Executing);
        assert_eq!(*find(&repo, b.decision_id).status(), DecisionStatus::Executing);
    }

    #[test]
    fn create_retries_when_the_allocated_id_is_already_taken() {
        let repo = MemAggregateRepo::default();
        let service = svc(&repo, MemoryStore::new());

        // Occupy DEC-000001 — the id MemoryStore's counter hands out first — so
        // the first allocation races exactly like two concurrent D1 creates both
        // reading MAX(id)+1 = 1 (②, ADR-005). create must detect the refusal and
        // re-allocate rather than silently overwrite.
        seed(&repo, 1, DecisionStatus::Executing);

        let created = block_on(service.create(create_cmd(0))).unwrap();
        assert_eq!(created.decision_id, 2, "racing create must re-allocate past the taken id");

        // Both rows survive — the original was never clobbered.
        assert_eq!(*find(&repo, 1).status(), DecisionStatus::Executing, "original row must be untouched");
        assert_eq!(*find(&repo, 2).status(), DecisionStatus::Executing, "retried create must land Executing");
    }

    #[test]
    fn create_returns_conflict_when_allocation_keeps_colliding() {
        let repo = MemAggregateRepo::default();
        let service = svc(&repo, MemoryStore::new());

        // Occupy every id the allocator hands out across the bounded retries
        // (counter starts at 1 → ids 1..=4 on the 4 attempts).
        for id in 1..=4 {
            seed(&repo, id, DecisionStatus::Executing);
        }
        let err = block_on(service.create(create_cmd(0))).unwrap_err();
        assert!(matches!(err, DecisionError::Conflict(_)), "exhausted retries must surface a conflict: {err:?}");
    }

    #[test]
    fn create_does_not_persist_or_leak_events_when_save_fails() {
        let repo = MemAggregateRepo { fail_saves: true, ..Default::default() };
        let service = svc(&repo, MemoryStore::new());

        let err = block_on(service.create(create_cmd(0))).unwrap_err();
        assert!(matches!(err, DecisionError::Infrastructure(_)), "save failure must surface: {err:?}");

        // No partial row was written (events were drained only after a
        // successful save — SD-D invariant), so nothing is hydratable.
        let all = block_on(repo.list(None, 100)).unwrap();
        assert!(all.is_empty(), "failed create must not leave a partial decision row");
    }

    #[test]
    fn transition_approves_then_executes_a_proposed_decision() {
        let repo = MemAggregateRepo::default();
        let service = svc(&repo, MemoryStore::new());
        seed(&repo, 5, DecisionStatus::Proposed);

        let approved = block_on(service.transition(5, DecisionLifecycleCommand::Approve)).unwrap();
        assert!(approved.transitioned);
        assert!(approved.events.iter().any(|e| matches!(e, DecisionDomainEvent::Approved { .. })));
        assert_eq!(*find(&repo, 5).status(), DecisionStatus::Approved, "row must persist 'approved'");

        let executed = block_on(service.transition(5, DecisionLifecycleCommand::Execute)).unwrap();
        assert!(executed.transitioned);
        assert_eq!(*find(&repo, 5).status(), DecisionStatus::Executing);
    }

    #[test]
    fn transition_is_noop_at_target_status_and_complete_requires_outcome() {
        let repo = MemAggregateRepo::default();
        let service = svc(&repo, MemoryStore::new());
        let created = block_on(service.create(create_cmd(0))).unwrap(); // Executing

        // execute on an already-Executing decision → idempotent success no-op.
        let noop = block_on(service.transition(created.decision_id, DecisionLifecycleCommand::Execute)).unwrap();
        assert!(!noop.transitioned);
        assert!(noop.events.is_empty());
        assert_eq!(*find(&repo, created.decision_id).status(), DecisionStatus::Executing);

        // complete without any observed outcome → aggregate invariant rejects it.
        let err = block_on(service.transition(created.decision_id, DecisionLifecycleCommand::Complete)).unwrap_err();
        assert_eq!(err, DecisionError::MissingOutcome);

        // approve from Executing is off-DAG.
        let err = block_on(service.transition(created.decision_id, DecisionLifecycleCommand::Approve)).unwrap_err();
        assert!(matches!(err, DecisionError::InvalidTransition { .. }));
    }

    #[test]
    fn transition_missing_decision_returns_notfound() {
        let repo = MemAggregateRepo::default();
        let service = svc(&repo, MemoryStore::new());
        let err = block_on(service.transition(999, DecisionLifecycleCommand::Execute)).unwrap_err();
        assert_eq!(err, DecisionError::NotFound("DEC-000999".into()));
    }

    #[test]
    fn transition_invalidate_from_executing_persists_superseded() {
        let repo = MemAggregateRepo::default();
        let service = svc(&repo, MemoryStore::new());
        let created = block_on(service.create(create_cmd(0))).unwrap();

        let inv = block_on(service.transition(
            created.decision_id,
            DecisionLifecycleCommand::Invalidate { reason: "superseded by new evidence".into() },
        ))
        .unwrap();
        assert!(inv.transitioned);
        assert!(inv.events.iter().any(|e| matches!(e, DecisionDomainEvent::Invalidated { .. })));
        assert_eq!(*find(&repo, created.decision_id).status(), DecisionStatus::Invalidated);
    }

    #[test]
    fn record_outcome_completes_an_executing_decision() {
        let repo = MemAggregateRepo::default();
        let service = svc(&repo, MemoryStore::new());
        let created = block_on(service.create(create_cmd(0))).unwrap();

        let outcome = NewOutcomeEvent {
            decision_id: created.decision_id,
            outcome_type: "accuracy".into(),
            observation: "reached 0.92".into(),
            evidence_url: Some("https://example.com/run".into()),
            observed_at: Some(9_000),
        };
        let recording = block_on(service.record_outcome(&outcome)).unwrap();
        assert!(recording.completed, "Executing decision must complete on outcome record (SD-A2)");
        assert!(recording.outcome_id >= 1);
        assert!(recording.events.iter().any(|e| matches!(e, DecisionDomainEvent::OutcomeAttached { .. })));
        assert!(recording.events.iter().any(|e| matches!(e, DecisionDomainEvent::Completed { .. })));

        // Row flipped to completed.
        assert_eq!(*find(&repo, created.decision_id).status(), DecisionStatus::Completed);
    }

    #[test]
    fn record_outcome_on_non_executing_decision_records_fact_only() {
        let repo = MemAggregateRepo::default();
        let service = svc(&repo, MemoryStore::new());
        seed(&repo, 6, DecisionStatus::Proposed);

        let outcome = NewOutcomeEvent {
            decision_id: 6,
            outcome_type: "latency".into(),
            observation: "spiked".into(),
            evidence_url: None,
            observed_at: None,
        };
        let recording = block_on(service.record_outcome(&outcome)).unwrap();
        assert!(!recording.completed, "non-Executing decision must not advance lifecycle");
        assert!(recording.outcome_id >= 1);
        assert!(recording.events.is_empty());

        // Lifecycle untouched.
        assert_eq!(*find(&repo, 6).status(), DecisionStatus::Proposed);
    }

    #[test]
    fn record_outcome_on_missing_decision_returns_notfound() {
        let repo = MemAggregateRepo::default();
        let service = svc(&repo, MemoryStore::new());
        let outcome = NewOutcomeEvent {
            decision_id: 4242,
            outcome_type: "x".into(),
            observation: "y".into(),
            evidence_url: None,
            observed_at: None,
        };
        let err = block_on(service.record_outcome(&outcome)).unwrap_err();
        assert_eq!(err, DecisionError::NotFound("DEC-004242".into()));
    }

    #[test]
    fn record_evaluation_persists_judgment_fact() {
        let repo = MemAggregateRepo::default();
        let service = svc(&repo, MemoryStore::new());
        let created = block_on(service.create(create_cmd(0))).unwrap();

        let evaluation = NewDecisionEvaluation {
            decision_id: created.decision_id,
            evaluation: domain::EvaluationResult::Confirmed,
            confidence: Some(0.9),
            reasoning: Some("Outcome matched the prediction".into()),
            evaluator: domain::EvaluationSource::AI,
            evaluated_at: Some(9_100),
        };
        let recording = block_on(service.record_evaluation(&evaluation)).unwrap();
        assert!(recording.evaluation_id >= 1);
    }
}
