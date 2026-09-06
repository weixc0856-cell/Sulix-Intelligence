//! DecisionAggregate — the root entity for the Decision bounded context.
//!
//! This is a **deep module**: external code calls
//! `DecisionAggregate::propose(cmd)` and `aggregate.transition(target)`,
//! never `validate()` → `score()` → `persist()` → `notify()` as separate
//! steps. Every behavioral method enforces the aggregate's invariants and
//! produces `DecisionDomainEvent`s that the application layer drains.

use serde::{Deserialize, Serialize};
use shared_kernel::ids::DecisionId;

use crate::commands::{
    ApproveDecision, ExecuteDecision, InvalidateDecision, ProposeDecision, ReconstructDecision, RecordOutcome,
};
use crate::error::DecisionError;
use crate::events::DecisionDomainEvent;
use crate::outcome::{ExpectedOutcome, ObservedOutcome};
use crate::status::DecisionStatus;

/// The Decision aggregate.
///
/// ## Invariants
///
/// 1. `confidence` is always in `[0.0, 1.0]`.
/// 2. Status transitions follow the DAG in [`DecisionStatus`].
/// 3. `Completed` requires at least one observed outcome.
/// 4. Events are accumulated and drained — each event is emitted exactly
///    once by the application service.
/// 5. Status is mutated **only** through the named behavioural methods
///    (`propose` / `approve` / `execute` / `complete` / `invalidate`).
///    There is deliberately **no** public status setter and no generic
///    `change_status` — the application layer must never write `status`
///    directly (P1, 2026-09-06).
///
/// ## Serialization note
///
/// The `Serialize`/`Deserialize` derives are an **internal round-trip /
/// snapshot helper only** — not the D1 persistence contract. `events` is
/// transient and skipped. Production hydration goes through
/// [`DecisionAggregate::reconstruct`]; D1 field mapping is owned by the
/// repository adapter.
///
/// ## Dead-code note
///
/// Several fields (`hypothesis`, `rationale`, etc.) are stored for
/// persistence but not yet exposed via read accessors. They are not dead
/// — the `DecisionRepository::save` implementation maps them to D1
/// columns. `#[allow(dead_code)]` prevents false positives until the
/// full repository cycle (save → find → hydrate) is wired in Phase 6.2C.
#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize)]
pub struct DecisionAggregate {
    /// Domain ID (`DEC-{id:06}`).
    id: DecisionId,
    /// Human-readable title.
    title: String,
    /// Falsifiable hypothesis being tested.
    hypothesis: Option<String>,
    /// Confidence in the hypothesis (0.0 – 1.0).
    confidence: f64,
    /// Current lifecycle status.
    status: DecisionStatus,
    /// Reasoning behind the decision.
    rationale: Option<String>,
    /// Categorisation.
    decision_type: String,
    /// Relative importance.
    priority: String,
    /// Link to the originating signal thread.
    signal_thread_id: Option<i64>,
    /// Who proposed this.
    actor_id: Option<i64>,
    /// Predictions made at proposal time.
    expected_outcomes: Vec<ExpectedOutcome>,
    /// Real-world outcomes observed.
    observed_outcomes: Vec<ObservedOutcome>,
    /// Timestamps.
    created_at: i64,
    updated_at: i64,
    /// Uncommitted domain events — consumed by the application service
    /// after `save()` succeeds. Transient: never serialized/deserialized
    /// (serde `skip` → empty on hydrate).
    #[serde(skip)]
    events: Vec<DecisionDomainEvent>,
}

impl DecisionAggregate {
    // ── Factory ────────────────────────────────────────────────────

    /// Reconstruct an aggregate from persisted state.
    ///
    /// Unlike `propose()`, this does NOT validate business rules or emit
    /// domain events — it trusts the data came from a valid prior state.
    /// Used by repository implementations to hydrate aggregates from D1 rows.
    pub fn reconstruct(cmd: ReconstructDecision) -> Self {
        Self {
            id: cmd.id,
            title: cmd.title,
            hypothesis: cmd.hypothesis,
            confidence: cmd.confidence,
            status: cmd.status,
            rationale: cmd.rationale,
            decision_type: cmd.decision_type,
            priority: cmd.priority,
            signal_thread_id: cmd.signal_thread_id,
            actor_id: cmd.actor_id,
            expected_outcomes: cmd.expected_outcomes,
            observed_outcomes: cmd.observed_outcomes,
            created_at: cmd.created_at,
            updated_at: cmd.updated_at,
            events: Vec::new(),
        }
    }

    /// Propose a new decision.
    ///
    /// Validates invariants, sets status to `Proposed`, and records a
    /// `DecisionDomainEvent::Proposed` event.
    pub fn propose(cmd: ProposeDecision, now: i64) -> Result<Self, DecisionError> {
        if cmd.title.is_empty() {
            return Err(DecisionError::EmptyTitle);
        }
        if !(0.0..=1.0).contains(&cmd.confidence) {
            return Err(DecisionError::InvalidConfidence(cmd.confidence));
        }

        let id = DecisionId::new(cmd.id);

        Ok(Self {
            events: vec![DecisionDomainEvent::Proposed {
                decision_id: format!("DEC-{:06}", cmd.id),
                title: cmd.title.clone(),
                confidence: cmd.confidence,
                decision_type: cmd.decision_type.clone(),
            }],
            id,
            title: cmd.title,
            hypothesis: cmd.hypothesis,
            confidence: cmd.confidence,
            status: DecisionStatus::Proposed,
            rationale: cmd.rationale,
            decision_type: cmd.decision_type,
            priority: cmd.priority,
            signal_thread_id: cmd.signal_thread_id,
            actor_id: cmd.actor_id,
            expected_outcomes: cmd.expected_outcomes,
            observed_outcomes: Vec::new(),
            created_at: now,
            updated_at: now,
        })
    }

    // ── Behavioural methods ────────────────────────────────────────

    /// Approve a proposed decision.
    pub fn approve(&mut self, cmd: ApproveDecision) -> Result<(), DecisionError> {
        let decision_id = self.id.0.clone();
        self.transition_with_event(DecisionStatus::Approved, || DecisionDomainEvent::Approved {
            decision_id,
            approved_by: cmd.approved_by,
        })
    }

    /// Start executing an approved decision.
    pub fn execute(&mut self, _cmd: ExecuteDecision) -> Result<(), DecisionError> {
        self.transition(DecisionStatus::Executing)
    }

    /// Attach a real-world outcome observation.
    /// Does NOT auto-transition — attaching outcomes and transitioning
    /// status are separate concerns.
    pub fn attach_outcome(&mut self, cmd: RecordOutcome) {
        let verdict = format!("{:?}", cmd.outcome.outcome_type).to_lowercase();
        let metric = cmd.outcome.metric.clone();
        self.observed_outcomes.push(cmd.outcome);
        self.updated_at =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        self.events.push(DecisionDomainEvent::OutcomeAttached { decision_id: self.id.0.clone(), metric, verdict });
    }

    /// Complete the decision.
    /// Invariant: requires at least one observed outcome.
    pub fn complete(&mut self) -> Result<(), DecisionError> {
        if self.observed_outcomes.is_empty() {
            return Err(DecisionError::MissingOutcome);
        }
        self.transition(DecisionStatus::Completed)
    }

    /// Invalidate the decision (superseded, withdrawn, irrelevant).
    pub fn invalidate(&mut self, cmd: InvalidateDecision) -> Result<(), DecisionError> {
        let decision_id = self.id.0.clone();
        self.transition_with_event(DecisionStatus::Invalidated, || DecisionDomainEvent::Invalidated {
            decision_id,
            reason: cmd.reason,
        })
    }

    // ── Event drain ────────────────────────────────────────────────

    /// Drain accumulated domain events. Called by the application service
    /// after persisting the aggregate.
    pub fn drain_events(&mut self) -> Vec<DecisionDomainEvent> {
        std::mem::take(&mut self.events)
    }

    // ── Accessors (read-only) ──────────────────────────────────────

    pub fn id(&self) -> &DecisionId {
        &self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn confidence(&self) -> f64 {
        self.confidence
    }

    pub fn status(&self) -> &DecisionStatus {
        &self.status
    }

    pub fn hypothesis(&self) -> Option<&str> {
        self.hypothesis.as_deref()
    }

    pub fn rationale(&self) -> Option<&str> {
        self.rationale.as_deref()
    }

    pub fn decision_type(&self) -> &str {
        &self.decision_type
    }

    pub fn priority(&self) -> &str {
        &self.priority
    }

    pub fn signal_thread_id(&self) -> Option<i64> {
        self.signal_thread_id
    }

    pub fn actor_id(&self) -> Option<i64> {
        self.actor_id
    }

    pub fn observed_outcomes(&self) -> &[ObservedOutcome] {
        &self.observed_outcomes
    }

    pub fn expected_outcomes(&self) -> &[ExpectedOutcome] {
        &self.expected_outcomes
    }

    pub fn created_at(&self) -> i64 {
        self.created_at
    }

    pub fn updated_at(&self) -> i64 {
        self.updated_at
    }

    // ── Internal helpers ───────────────────────────────────────────

    /// Transition status with a generic event.
    fn transition(&mut self, target: DecisionStatus) -> Result<(), DecisionError> {
        let from = self.status.clone();
        if !self.status.can_transition_to(&target) {
            return Err(DecisionError::InvalidTransition { from: from.clone(), to: target.clone() });
        }
        // Invariant: Completed requires outcomes.
        if target == DecisionStatus::Completed && self.observed_outcomes.is_empty() {
            return Err(DecisionError::MissingOutcome);
        }

        let to = target.clone();
        self.status = target;
        self.updated_at =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);

        let event = if to == DecisionStatus::Completed {
            DecisionDomainEvent::Completed {
                decision_id: self.id.0.clone(),
                outcome_count: self.observed_outcomes.len(),
            }
        } else if to == DecisionStatus::Invalidated {
            // Invalidate has a custom event with reason — caller should use
            // transition_with_event or the invalidate method above.
            return Ok(());
        } else {
            DecisionDomainEvent::StatusChanged { decision_id: self.id.0.clone(), from, to }
        };
        self.events.push(event);
        Ok(())
    }

    /// Transition with a caller-supplied event payload.
    fn transition_with_event(
        &mut self,
        target: DecisionStatus,
        event_fn: impl FnOnce() -> DecisionDomainEvent,
    ) -> Result<(), DecisionError> {
        let from = self.status.clone();
        if !self.status.can_transition_to(&target) {
            return Err(DecisionError::InvalidTransition { from, to: target });
        }
        self.status = target;
        self.updated_at =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        self.events.push(event_fn());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_propose(id: i64) -> ProposeDecision {
        ProposeDecision {
            id,
            title: "Test hypothesis".into(),
            hypothesis: Some("X will cause Y".into()),
            confidence: 0.85,
            rationale: Some("Based on evidence".into()),
            decision_type: "experiment".into(),
            priority: "high".into(),
            signal_thread_id: None,
            actor_id: Some(1),
            expected_outcomes: vec![],
        }
    }

    #[test]
    fn propose_creates_aggregate_with_proposed_status() {
        let agg = DecisionAggregate::propose(make_propose(1), 1000).unwrap();
        assert_eq!(*agg.status(), DecisionStatus::Proposed);
        assert_eq!(agg.title(), "Test hypothesis");
        assert!((agg.confidence() - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn propose_rejects_empty_title() {
        let mut cmd = make_propose(1);
        cmd.title = "".into();
        let err = DecisionAggregate::propose(cmd, 1000).unwrap_err();
        assert_eq!(err, DecisionError::EmptyTitle);
    }

    #[test]
    fn propose_rejects_invalid_confidence() {
        let mut cmd = make_propose(1);
        cmd.confidence = 1.5;
        let err = DecisionAggregate::propose(cmd, 1000).unwrap_err();
        assert_eq!(err, DecisionError::InvalidConfidence(1.5));
    }

    #[test]
    fn propose_emits_proposed_event() {
        let mut agg = DecisionAggregate::propose(make_propose(42), 1000).unwrap();
        let events = agg.drain_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DecisionDomainEvent::Proposed { .. }));
    }

    #[test]
    fn approve_transitions_from_proposed() {
        let mut agg = DecisionAggregate::propose(make_propose(1), 1000).unwrap();
        agg.approve(ApproveDecision { decision_id: agg.id().0.clone(), approved_by: "reviewer".into() }).unwrap();
        assert_eq!(*agg.status(), DecisionStatus::Approved);
    }

    #[test]
    fn approve_fails_from_draft() {
        // Currently propose() sets to Proposed, so this tests the
        // transition only works from the right source state.
        let mut agg = DecisionAggregate::propose(make_propose(1), 1000).unwrap();
        agg.approve(ApproveDecision { decision_id: agg.id().0.clone(), approved_by: "reviewer".into() }).unwrap();
        // Now Approved — can't approve again
        let err = agg.approve(ApproveDecision { decision_id: agg.id().0.clone(), approved_by: "reviewer".into() });
        assert!(err.is_err());
    }

    #[test]
    fn complete_requires_outcomes() {
        let mut agg = DecisionAggregate::propose(make_propose(1), 1000).unwrap();
        agg.approve(ApproveDecision { decision_id: agg.id().0.clone(), approved_by: "reviewer".into() }).unwrap();
        agg.execute(ExecuteDecision { decision_id: agg.id().0.clone() }).unwrap();
        let err = agg.complete();
        assert_eq!(err, Err(DecisionError::MissingOutcome));
    }

    #[test]
    fn complete_with_outcomes_succeeds() {
        let mut agg = DecisionAggregate::propose(make_propose(1), 1000).unwrap();
        agg.approve(ApproveDecision { decision_id: agg.id().0.clone(), approved_by: "reviewer".into() }).unwrap();
        agg.execute(ExecuteDecision { decision_id: agg.id().0.clone() }).unwrap();
        agg.attach_outcome(RecordOutcome {
            decision_id: agg.id().0.clone(),
            outcome: ObservedOutcome {
                metric: "accuracy".into(),
                actual_value: "0.92".into(),
                outcome_type: crate::outcome::OutcomeVerdict::Achieved,
                evidence_url: None,
                observed_at: 2000,
            },
        });
        agg.complete().unwrap();
        assert_eq!(*agg.status(), DecisionStatus::Completed);
    }

    #[test]
    fn drain_events_clears_event_buffer() {
        let mut agg = DecisionAggregate::propose(make_propose(1), 1000).unwrap();
        let drained = agg.drain_events();
        assert_eq!(drained.len(), 1);
        let empty = agg.drain_events();
        assert!(empty.is_empty());
    }

    #[test]
    fn invalidate_from_executing() {
        let mut agg = DecisionAggregate::propose(make_propose(1), 1000).unwrap();
        agg.approve(ApproveDecision { decision_id: agg.id().0.clone(), approved_by: "reviewer".into() }).unwrap();
        agg.execute(ExecuteDecision { decision_id: agg.id().0.clone() }).unwrap();
        agg.invalidate(InvalidateDecision {
            decision_id: agg.id().0.clone(),
            reason: "superseded by new evidence".into(),
        })
        .unwrap();
        assert_eq!(*agg.status(), DecisionStatus::Invalidated);
    }

    #[test]
    fn serde_round_trip_preserves_state_and_skips_transient_events() {
        let mut cmd = make_propose(7);
        cmd.expected_outcomes = vec![
            ExpectedOutcome {
                metric: "accuracy".into(),
                expected_value: ">= 0.9".into(),
                measurement_method: "eval set".into(),
            },
            ExpectedOutcome {
                metric: "latency".into(),
                expected_value: "< 200ms".into(),
                measurement_method: "p95".into(),
            },
        ];
        let mut agg = DecisionAggregate::propose(cmd, 1000).unwrap();

        // The aggregate currently holds a pending Proposed event — it must
        // NOT leak into the state snapshot.
        let json = serde_json::to_string(&agg).unwrap();
        assert!(!json.contains("decision.proposed"), "transient events must not serialize");
        assert!(!json.contains("\"events\""), "events field must be skipped");

        let mut copy: DecisionAggregate = serde_json::from_str(&json).unwrap();
        // Transient buffer starts empty on the hydrated copy...
        assert!(copy.drain_events().is_empty());
        // ...while the original still owns its pending event.
        assert_eq!(agg.drain_events().len(), 1);

        // State round-trips exactly (stable re-serialization).
        assert_eq!(serde_json::to_string(&copy).unwrap(), json);
        assert_eq!(copy.id().0, "DEC-000007");
        assert_eq!(*copy.status(), DecisionStatus::Proposed);
        assert!(f64::abs(copy.confidence() - 0.85) < f64::EPSILON);
        assert_eq!(copy.expected_outcomes().len(), 2);
        assert_eq!(copy.expected_outcomes()[0].metric, "accuracy");
        assert_eq!(copy.expected_outcomes()[1].expected_value, "< 200ms");
        assert!(copy.observed_outcomes().is_empty());
        assert_eq!(copy.created_at(), 1000);
        assert_eq!(copy.updated_at(), 1000);
    }
}
