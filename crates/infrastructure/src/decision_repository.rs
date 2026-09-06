//! D1-backed DecisionRepository — maps between Domain aggregate and D1 rows.
//!
//! Lives in infrastructure (not decision-engine) to keep domain pure.
//!
//! `S` is bound to the **narrow domain decision ports** the adapter actually
//! calls: row upsert (`DecisionUpsertStore`), DTO read (`DecisionRepository`)
//! and query (`DecisionQueryService`). It never depends on a store-wide
//! composite (narrowed since P2, 2026-09-06).

use async_trait::async_trait;
use decision_engine::{
    decode_expected_outcomes, encode_expected_outcomes, DecisionAggregate, DecisionError, DecisionStatus,
    ObservedOutcome, OutcomeVerdict, ReconstructDecision,
};
use shared_kernel::ids::DecisionId;
use store::{Decision, DecisionQueryService, DecisionRepository, DecisionUpsertStore, OutcomeEvent};

/// Maps domain `DecisionAggregate` to/from D1 `decisions` table rows.
pub struct D1DecisionRepository<S> {
    store: S,
}

impl<S: DecisionRepository + DecisionQueryService + DecisionUpsertStore> D1DecisionRepository<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    fn status_to_d1(status: &DecisionStatus) -> &'static str {
        match status {
            DecisionStatus::Draft => "draft",
            DecisionStatus::Proposed => "proposed",
            DecisionStatus::Approved => "approved",
            DecisionStatus::Executing => "active",
            DecisionStatus::Completed => "completed",
            DecisionStatus::Invalidated => "superseded",
        }
    }

    fn status_from_d1(s: &str) -> DecisionStatus {
        match s {
            "draft" => DecisionStatus::Draft,
            "proposed" => DecisionStatus::Proposed,
            "approved" => DecisionStatus::Approved,
            "active" => DecisionStatus::Executing,
            "completed" => DecisionStatus::Completed,
            "superseded" | "invalidated" => DecisionStatus::Invalidated,
            _ => DecisionStatus::Draft,
        }
    }

    fn d1_id(id: &str) -> Result<i64, DecisionError> {
        id.strip_prefix("DEC-")
            .and_then(|s| s.parse::<i64>().ok())
            .ok_or_else(|| DecisionError::NotFound(id.to_string()))
    }

    fn from_store(d: Decision, observed_outcomes: Vec<ObservedOutcome>) -> DecisionAggregate {
        let status = Self::status_from_d1(&d.status);
        DecisionAggregate::reconstruct(ReconstructDecision {
            id: DecisionId::new(d.id),
            title: d.title,
            hypothesis: d.hypothesis,
            confidence: d.confidence,
            status,
            rationale: d.rationale,
            decision_type: d.decision_type,
            priority: d.priority,
            signal_thread_id: d.signal_thread_id,
            actor_id: d.actor_id,
            // expected_outcomes: NULL/absent column (legacy rows) degrades to [].
            expected_outcomes: decode_expected_outcomes(d.expected_outcomes.as_deref()),
            // observed_outcomes is not a column — it is reconstructed from the
            // `outcome_events` fact rows so a hydrated aggregate is a faithful
            // reconstruction (a `Completed`/outcome-bearing decision must not
            // present an empty outcome list, or `complete()` would reject it
            // with MissingOutcome despite persisted outcomes) (①, 2026-09-06).
            observed_outcomes,
            created_at: d.created_at,
            updated_at: d.updated_at,
        })
    }

    /// Map one `outcome_events` fact row onto the aggregate's `ObservedOutcome`.
    ///
    /// The fact layer stores observations, not verdicts (no verdict column, and
    /// the write path maps every completing outcome to
    /// `OutcomeVerdict::Inconclusive`), so hydration is verdict-neutral to stay
    /// consistent with [`crate`]'s write mapping. A durable verdict would need a
    /// new column + caller-supplied verdict input (SD-B backlog item).
    fn from_outcome(e: &OutcomeEvent) -> ObservedOutcome {
        ObservedOutcome {
            metric: e.outcome_type.clone(),
            actual_value: e.observation.clone(),
            outcome_type: OutcomeVerdict::Inconclusive,
            evidence_url: e.evidence_url.clone(),
            observed_at: if e.observed_at > 0 { e.observed_at } else { e.created_at },
        }
    }

    /// Load the `observed_outcomes` for a decision from its `outcome_events`
    /// fact rows (the fact layer is the durable observation store).
    async fn load_outcomes(&self, decision_id: i64) -> Result<Vec<ObservedOutcome>, DecisionError> {
        self.store
            .list_outcomes(decision_id)
            .await
            .map(|rows| rows.iter().map(Self::from_outcome).collect())
            .map_err(|e| DecisionError::Infrastructure(e.to_string()))
    }

    /// Map the aggregate onto a full `decisions` row. Unlike the legacy
    /// `into_new` (INSERT-only), this carries the explicit primary key and
    /// timestamps so `save` is a true idempotent upsert keyed by aggregate id.
    fn into_row(decision: &DecisionAggregate) -> Result<Decision, DecisionError> {
        Ok(Decision {
            id: Self::d1_id(&decision.id().0)?,
            signal_thread_id: decision.signal_thread_id(),
            actor_id: decision.actor_id(),
            decision_type: decision.decision_type().to_string(),
            title: decision.title().to_string(),
            hypothesis: decision.hypothesis().map(String::from),
            rationale: decision.rationale().map(String::from),
            confidence: decision.confidence(),
            status: Self::status_to_d1(decision.status()).to_string(),
            priority: decision.priority().to_string(),
            expected_outcomes: Some(encode_expected_outcomes(decision.expected_outcomes())),
            created_at: decision.created_at(),
            updated_at: decision.updated_at(),
        })
    }
}

#[async_trait(?Send)]
impl<S: DecisionRepository + DecisionQueryService + DecisionUpsertStore> decision_engine::DecisionRepository
    for D1DecisionRepository<S>
{
    async fn save(&self, decision: &DecisionAggregate) -> Result<(), DecisionError> {
        let row = Self::into_row(decision)?;
        self.store.upsert_decision(&row).await.map_err(|e| DecisionError::Infrastructure(e.to_string()))
    }

    async fn save_new(&self, decision: &DecisionAggregate) -> Result<bool, DecisionError> {
        let row = Self::into_row(decision)?;
        self.store.try_insert_decision(&row).await.map_err(|e| DecisionError::Infrastructure(e.to_string()))
    }

    async fn find(&self, id: &str) -> Result<Option<DecisionAggregate>, DecisionError> {
        let d1_id = Self::d1_id(id)?;
        let row = match self.store.find_decision(d1_id).await {
            Ok(Some(r)) => r,
            Ok(None) => return Ok(None),
            Err(e) => return Err(DecisionError::Infrastructure(e.to_string())),
        };
        let observed = self.load_outcomes(row.id).await?;
        Ok(Some(Self::from_store(row, observed)))
    }

    async fn find_by_signal(&self, signal_thread_id: i64) -> Result<Vec<DecisionAggregate>, DecisionError> {
        let rows = self
            .store
            .decisions_by_signal(signal_thread_id)
            .await
            .map_err(|e| DecisionError::Infrastructure(e.to_string()))?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let observed = self.load_outcomes(row.id).await?;
            out.push(Self::from_store(row, observed));
        }
        Ok(out)
    }

    async fn list(&self, status: Option<&str>, limit: u32) -> Result<Vec<DecisionAggregate>, DecisionError> {
        let rows =
            self.store.list_decisions(status, limit).await.map_err(|e| DecisionError::Infrastructure(e.to_string()))?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let observed = self.load_outcomes(row.id).await?;
            out.push(Self::from_store(row, observed));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Bring the aggregate-repository trait into scope so `repo.save/find/…`
    // method calls resolve. `as _` avoids colliding with the store DTO trait of
    // the same name re-exported via `super::*`.
    use decision_engine::DecisionRepository as _;
    use decision_engine::{DecisionAggregate, DecisionStatus, ExpectedOutcome, ReconstructDecision};
    use store::memory::MemoryStore;

    type Repo = D1DecisionRepository<MemoryStore>;

    fn make_aggregate(id: i64, status: DecisionStatus, signal_thread_id: Option<i64>) -> DecisionAggregate {
        make_aggregate_full(id, status, signal_thread_id, vec![])
    }

    /// Hydrate an aggregate exactly as a D1 row would after save+find.
    /// created_at/updated_at flow through `into_row` (now full-row upsert).
    fn make_aggregate_full(
        id: i64,
        status: DecisionStatus,
        signal_thread_id: Option<i64>,
        expected_outcomes: Vec<ExpectedOutcome>,
    ) -> DecisionAggregate {
        DecisionAggregate::reconstruct(ReconstructDecision {
            id: DecisionId::new(id),
            title: format!("Decision {id}"),
            hypothesis: Some("X will cause Y".into()),
            confidence: 0.8,
            status,
            rationale: Some("Based on evidence".into()),
            decision_type: "experiment".into(),
            priority: "high".into(),
            signal_thread_id,
            actor_id: Some(7),
            expected_outcomes,
            observed_outcomes: vec![],
            created_at: 500,
            updated_at: 600,
        })
    }

    fn sample_outcomes() -> Vec<ExpectedOutcome> {
        vec![
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
        ]
    }

    #[test]
    fn status_mapping_round_trips_all_variants() {
        let cases = [
            (DecisionStatus::Draft, "draft"),
            (DecisionStatus::Proposed, "proposed"),
            (DecisionStatus::Approved, "approved"),
            (DecisionStatus::Executing, "active"),
            (DecisionStatus::Completed, "completed"),
            (DecisionStatus::Invalidated, "superseded"),
        ];
        for (status, d1) in cases {
            assert_eq!(Repo::status_to_d1(&status), d1, "status_to_d1({status:?})");
            assert_eq!(Repo::status_from_d1(d1), status, "status_from_d1({d1})");
        }
    }

    #[test]
    fn status_from_d1_accepts_legacy_alias_and_unknowns() {
        // Legacy rows may carry "invalidated"; unknown values degrade to Draft.
        assert_eq!(Repo::status_from_d1("invalidated"), DecisionStatus::Invalidated);
        assert_eq!(Repo::status_from_d1("not-a-status"), DecisionStatus::Draft);
    }

    #[test]
    fn d1_id_parses_decimal_suffix_and_rejects_garbage() {
        assert_eq!(Repo::d1_id("DEC-000042"), Ok(42));
        assert_eq!(Repo::d1_id("DEC-000000"), Ok(0));
        assert!(Repo::d1_id("42").is_err(), "missing DEC- prefix must reject");
        assert!(Repo::d1_id("DEC-abc").is_err(), "non-numeric suffix must reject");
        assert!(Repo::d1_id("").is_err());
    }

    #[test]
    fn save_then_find_round_trips_all_business_fields() {
        let repo = Repo::new(MemoryStore::new());
        let agg = make_aggregate(1, DecisionStatus::Approved, Some(42));
        futures::executor::block_on(repo.save(&agg)).unwrap();

        let found = futures::executor::block_on(repo.find("DEC-000001")).unwrap().expect("row must exist");
        assert_eq!(found.id().0, "DEC-000001");
        assert_eq!(found.title(), "Decision 1");
        assert_eq!(found.hypothesis(), Some("X will cause Y"));
        assert!(f64::abs(found.confidence() - 0.8) < f64::EPSILON);
        assert_eq!(*found.status(), DecisionStatus::Approved);
        assert_eq!(found.rationale(), Some("Based on evidence"));
        assert_eq!(found.decision_type(), "experiment");
        assert_eq!(found.priority(), "high");
        assert_eq!(found.signal_thread_id(), Some(42));
        assert_eq!(found.actor_id(), Some(7));
        assert!(found.expected_outcomes().is_empty());
    }

    #[test]
    fn expected_outcomes_round_trip_via_save_find() {
        let repo = Repo::new(MemoryStore::new());
        let agg = make_aggregate_full(1, DecisionStatus::Approved, Some(42), sample_outcomes());
        futures::executor::block_on(repo.save(&agg)).unwrap();

        let found = futures::executor::block_on(repo.find("DEC-000001")).unwrap().expect("row must exist");
        let eo = found.expected_outcomes();
        assert_eq!(eo.len(), 2);
        assert_eq!(eo[0].metric, "accuracy");
        assert_eq!(eo[0].expected_value, ">= 0.9");
        assert_eq!(eo[0].measurement_method, "eval set");
        assert_eq!(eo[1].metric, "latency");
        assert_eq!(eo[1].expected_value, "< 200ms");
        assert_eq!(eo[1].measurement_method, "p95");
    }

    #[test]
    fn expected_outcomes_empty_encodes_as_empty_array_and_round_trips() {
        let repo = Repo::new(MemoryStore::new());
        let agg = make_aggregate(3, DecisionStatus::Proposed, None);
        futures::executor::block_on(repo.save(&agg)).unwrap();
        let found = futures::executor::block_on(repo.find("DEC-000003")).unwrap().unwrap();
        assert!(found.expected_outcomes().is_empty(), "no outcomes must hydrate as [], not an error");
    }

    #[test]
    fn find_returns_none_for_missing_row_and_err_for_bad_id() {
        let repo = Repo::new(MemoryStore::new());
        // Well-formed id, no row → None (D1 returns no rows, not an error).
        assert!(futures::executor::block_on(repo.find("DEC-000999")).unwrap().is_none());
        // Malformed id → hard error before touching the store.
        assert!(matches!(futures::executor::block_on(repo.find("42")), Err(DecisionError::NotFound(_))));
    }

    #[test]
    fn save_round_trips_every_status_via_update() {
        let repo = Repo::new(MemoryStore::new());
        for (i, status) in [
            DecisionStatus::Draft,
            DecisionStatus::Proposed,
            DecisionStatus::Approved,
            DecisionStatus::Executing,
            DecisionStatus::Completed,
            DecisionStatus::Invalidated,
        ]
        .into_iter()
        .enumerate()
        {
            // Decision ids are written explicitly into the row id, so a save
            // of DEC-{n} always lands on row id n (no auto-increment coupling).
            let agg = make_aggregate(i as i64 + 1, status.clone(), None);
            futures::executor::block_on(repo.save(&agg)).unwrap();
            let found = futures::executor::block_on(repo.find(&agg.id().0)).unwrap().unwrap();
            assert_eq!(*found.status(), status, "round-trip for {status:?} (DEC-{:06})", i + 1);
        }
    }

    #[test]
    fn second_save_updates_in_place_not_duplicate() {
        let repo = Repo::new(MemoryStore::new());
        let v1 = make_aggregate(1, DecisionStatus::Proposed, None);
        futures::executor::block_on(repo.save(&v1)).unwrap();
        futures::executor::block_on(repo.save(&v1)).unwrap();

        // Same id saved twice → exactly one row, not two.
        let all = futures::executor::block_on(repo.list(None, 100)).unwrap();
        assert_eq!(all.len(), 1, "second save of the same aggregate id must update, not insert");

        // Save a second revision of the same id — a post-transition aggregate
        // (status moved on, thread linked, outcomes attached).
        let v2 = DecisionAggregate::reconstruct(ReconstructDecision {
            id: DecisionId::new(1),
            title: "Decision 1 (executing)".into(),
            hypothesis: Some("X will cause Y".into()),
            confidence: 0.8,
            status: DecisionStatus::Executing,
            rationale: Some("Based on evidence".into()),
            decision_type: "experiment".into(),
            priority: "high".into(),
            signal_thread_id: Some(9),
            actor_id: Some(7),
            expected_outcomes: sample_outcomes(),
            observed_outcomes: vec![],
            created_at: 500,
            updated_at: 700,
        });
        futures::executor::block_on(repo.save(&v2)).unwrap();

        let all = futures::executor::block_on(repo.list(None, 100)).unwrap();
        assert_eq!(all.len(), 1, "upsert must keep a single row across revisions");
        let found = futures::executor::block_on(repo.find("DEC-000001")).unwrap().unwrap();
        assert_eq!(found.title(), "Decision 1 (executing)");
        assert_eq!(*found.status(), DecisionStatus::Executing);
        assert_eq!(found.signal_thread_id(), Some(9));
        assert_eq!(found.expected_outcomes().len(), 2);
    }

    #[test]
    fn upsert_preserves_created_at_and_refreshes_updated_at() {
        let repo = Repo::new(MemoryStore::new());
        let first = make_aggregate(1, DecisionStatus::Approved, None); // created_at 500, updated_at 600
        futures::executor::block_on(repo.save(&first)).unwrap();
        assert_eq!(futures::executor::block_on(repo.find("DEC-000001")).unwrap().unwrap().created_at(), 500);

        // A later save carries a drifted created_at (e.g. reconstructed with a
        // wrong stamp) — the row must keep the FIRST insert's value and only
        // refresh updated_at.
        let later = DecisionAggregate::reconstruct(ReconstructDecision {
            id: DecisionId::new(1),
            title: "Decision 1".into(),
            hypothesis: Some("X will cause Y".into()),
            confidence: 0.8,
            status: DecisionStatus::Completed,
            rationale: Some("Based on evidence".into()),
            decision_type: "experiment".into(),
            priority: "high".into(),
            signal_thread_id: None,
            actor_id: Some(7),
            expected_outcomes: vec![],
            observed_outcomes: vec![],
            created_at: 9_999_999, // must NOT clobber the preserved value
            updated_at: 800,
        });
        futures::executor::block_on(repo.save(&later)).unwrap();

        let found = futures::executor::block_on(repo.find("DEC-000001")).unwrap().unwrap();
        assert_eq!(found.created_at(), 500, "created_at is preserved from the first insert");
        assert_eq!(found.updated_at(), 800, "updated_at is refreshed on upsert");
        assert_eq!(*found.status(), DecisionStatus::Completed);
    }

    #[test]
    fn find_by_signal_filters_by_thread_link() {
        let repo = Repo::new(MemoryStore::new());
        for (i, signal) in [(1i64, Some(100i64)), (2, Some(100)), (3, Some(200))] {
            let agg = make_aggregate(i, DecisionStatus::Draft, signal);
            futures::executor::block_on(repo.save(&agg)).unwrap();
        }

        let thread_100 = futures::executor::block_on(repo.find_by_signal(100)).unwrap();
        assert_eq!(thread_100.len(), 2);
        assert!(thread_100.iter().all(|d| d.signal_thread_id() == Some(100)));

        let thread_200 = futures::executor::block_on(repo.find_by_signal(200)).unwrap();
        assert_eq!(thread_200.len(), 1);
        assert_eq!(thread_200[0].id().0, "DEC-000003");

        assert!(futures::executor::block_on(repo.find_by_signal(999)).unwrap().is_empty());
    }

    #[test]
    fn list_filters_by_status_and_returns_all_without_filter() {
        let repo = Repo::new(MemoryStore::new());
        let statuses =
            [DecisionStatus::Draft, DecisionStatus::Approved, DecisionStatus::Completed, DecisionStatus::Completed];
        for (i, s) in statuses.into_iter().enumerate() {
            futures::executor::block_on(repo.save(&make_aggregate(i as i64 + 1, s, None))).unwrap();
        }

        let completed = futures::executor::block_on(repo.list(Some("completed"), 10)).unwrap();
        assert_eq!(completed.len(), 2);
        assert!(completed.iter().all(|d| *d.status() == DecisionStatus::Completed));

        let all = futures::executor::block_on(repo.list(None, 10)).unwrap();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn find_hydrates_observed_outcomes_from_outcome_events() {
        use store::NewOutcomeEvent;
        use store::OutcomeRepository as _;
        let repo = Repo::new(MemoryStore::new());

        // Decision sitting at Executing with an outcome fact already recorded —
        // the state that used to hydrate `observed_outcomes = []` and made
        // `complete()` reject with MissingOutcome despite the persisted outcome.
        futures::executor::block_on(repo.save(&make_aggregate(1, DecisionStatus::Executing, None))).unwrap();
        futures::executor::block_on(repo.store.save_outcome(&NewOutcomeEvent {
            decision_id: 1,
            outcome_type: "accuracy".into(),
            observation: "reached 0.92".into(),
            evidence_url: Some("https://example.com/run".into()),
            observed_at: Some(9_000),
        }))
        .unwrap();

        let mut found = futures::executor::block_on(repo.find("DEC-000001")).unwrap().expect("row must exist");
        assert_eq!(found.observed_outcomes().len(), 1, "observed_outcomes must hydrate from outcome_events");
        assert_eq!(found.observed_outcomes()[0].metric, "accuracy");
        assert_eq!(found.observed_outcomes()[0].actual_value, "reached 0.92");
        assert_eq!(found.observed_outcomes()[0].observed_at, 9_000);

        // Faithful reconstruction: the aggregate can now complete (the invariant
        // sees the persisted outcome) — previously MissingOutcome.
        found.complete().unwrap();
        assert_eq!(*found.status(), DecisionStatus::Completed);
    }

    #[test]
    fn save_new_inserts_a_fresh_id() {
        let repo = Repo::new(MemoryStore::new());
        let fresh = make_aggregate(7, DecisionStatus::Executing, None);

        let inserted = futures::executor::block_on(repo.save_new(&fresh)).unwrap();
        assert!(inserted, "save_new on an unclaimed id must insert");

        let found = futures::executor::block_on(repo.find("DEC-000007")).unwrap().expect("row must exist");
        assert_eq!(*found.status(), DecisionStatus::Executing);
    }

    #[test]
    fn save_new_refuses_an_existing_id_without_overwriting() {
        let repo = Repo::new(MemoryStore::new());
        let first = make_aggregate(1, DecisionStatus::Executing, None); // title "Decision 1", created_at 500
        futures::executor::block_on(repo.save(&first)).unwrap();

        // A second creator racing to the same id must be refused, not upserted
        // over the first row (②: two concurrent creates can compute the same
        // MAX(id)+1 — ADR-005).
        let racer = make_aggregate_full(1, DecisionStatus::Completed, Some(7), sample_outcomes());
        let inserted = futures::executor::block_on(repo.save_new(&racer)).unwrap();
        assert!(!inserted, "save_new must refuse an id that already has a row");

        // The original row is untouched — nothing was silently lost.
        let found = futures::executor::block_on(repo.find("DEC-000001")).unwrap().expect("row must exist");
        assert_eq!(*found.status(), DecisionStatus::Executing, "racer must not have overwritten the row");
        assert_eq!(found.title(), "Decision 1");
        assert_eq!(found.signal_thread_id(), None);
        assert!(found.expected_outcomes().is_empty());
        assert_eq!(futures::executor::block_on(repo.list(None, 100)).unwrap().len(), 1);
    }
}
