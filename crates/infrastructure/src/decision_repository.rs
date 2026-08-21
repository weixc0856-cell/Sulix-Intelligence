//! D1-backed DecisionRepository — maps between Domain aggregate and D1 rows.
//!
//! Lives in infrastructure (not decision-engine) to keep domain pure.

use async_trait::async_trait;
use decision_engine::{DecisionAggregate, DecisionError, DecisionRepository, DecisionStatus, ReconstructDecision};
use shared_kernel::ids::DecisionId;
use store::StoreBackend;

/// Maps domain `DecisionAggregate` to/from D1 `decisions` table rows.
pub struct D1DecisionRepository<S> {
    store: S,
}

impl<S: StoreBackend> D1DecisionRepository<S> {
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

    fn from_store(d: store::Decision) -> DecisionAggregate {
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
            expected_outcomes: vec![], // expected_outcomes not in legacy decisions table
            observed_outcomes: vec![], // observed_outcomes not in legacy decisions table
            created_at: d.created_at,
            updated_at: d.updated_at,
        })
    }

    fn into_new(decision: &DecisionAggregate) -> store::NewDecision {
        store::NewDecision {
            signal_thread_id: decision.signal_thread_id(),
            actor_id: decision.actor_id(),
            decision_type: decision.decision_type().to_string(),
            title: decision.title().to_string(),
            hypothesis: decision.hypothesis().map(String::from),
            rationale: decision.rationale().map(String::from),
            confidence: decision.confidence(),
            priority: decision.priority().to_string(),
        }
    }
}

#[async_trait(?Send)]
impl<S: StoreBackend> DecisionRepository for D1DecisionRepository<S> {
    async fn save(&self, decision: &DecisionAggregate) -> Result<(), DecisionError> {
        let new = Self::into_new(decision);
        self.store.create_decision(&new).await.map_err(|e| DecisionError::Infrastructure(e.to_string()))?;

        if let Ok(d1_id) = Self::d1_id(&decision.id().0) {
            let status = Self::status_to_d1(decision.status());
            self.store
                .update_decision_status(d1_id, status)
                .await
                .map_err(|e| DecisionError::Infrastructure(e.to_string()))?;
        }
        Ok(())
    }

    async fn find(&self, id: &str) -> Result<Option<DecisionAggregate>, DecisionError> {
        let d1_id = Self::d1_id(id)?;
        self.store
            .get_decision(d1_id)
            .await
            .map_err(|e| DecisionError::Infrastructure(e.to_string()))
            .map(|opt| opt.map(Self::from_store))
    }

    async fn find_by_signal(&self, signal_thread_id: i64) -> Result<Vec<DecisionAggregate>, DecisionError> {
        self.store
            .decisions_by_signal(signal_thread_id)
            .await
            .map_err(|e| DecisionError::Infrastructure(e.to_string()))
            .map(|vec| vec.into_iter().map(Self::from_store).collect())
    }

    async fn list(&self, status: Option<&str>, limit: u32) -> Result<Vec<DecisionAggregate>, DecisionError> {
        self.store
            .list_decisions(status, limit)
            .await
            .map_err(|e| DecisionError::Infrastructure(e.to_string()))
            .map(|vec| vec.into_iter().map(Self::from_store).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use decision_engine::{DecisionAggregate, DecisionStatus, ReconstructDecision};
    use store::memory::MemoryStore;

    type Repo = D1DecisionRepository<MemoryStore>;

    /// Hydrate an aggregate exactly as a D1 row would after save+find.
    /// created_at/updated_at are NOT round-tripped through `into_new`
    /// (NewDecision has no timestamp fields — the store stamps its own).
    fn make_aggregate(id: i64, status: DecisionStatus, signal_thread_id: Option<i64>) -> DecisionAggregate {
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
            expected_outcomes: vec![],
            observed_outcomes: vec![],
            created_at: 500,
            updated_at: 600,
        })
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
            // MemoryStore ids start at 1 — start domain ids at 1 so d1_id maps to a real row.
            let agg = make_aggregate(i as i64 + 1, status.clone(), None);
            futures::executor::block_on(repo.save(&agg)).unwrap();
            let found = futures::executor::block_on(repo.find(&agg.id().0)).unwrap().unwrap();
            assert_eq!(*found.status(), status, "round-trip for {status:?} (DEC-{:06})", i + 1);
        }
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
}
