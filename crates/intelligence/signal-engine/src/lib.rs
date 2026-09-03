//! Signal Engine — materialise entity-driven signal candidates into
//! persistent signal threads with structured instances and events.
//!
//! ╔══════════════════════════════════════════════════════════════════╗
//! ║  DEPRECATED — Sprint 6.2D                                      ║
//! ║                                                                  ║
//! ║  Domain types have moved to `intelligence-domain` crate.        ║
//! ║  This crate is retained for backward compat:                    ║
//! ║  - SignalEngine::run() orchestration                            ║
//! ║  - SignalSource trait + discovery sources                       ║
//! ║  - Scoring/discovery pipeline                                   ║
//! ║                                                                  ║
//! ║  New code should use `intelligence_domain::*` for domain types. ║
//! ║  TODO (Sprint 6.2E+): migrate consumers then remove this crate. ║
//! ╚══════════════════════════════════════════════════════════════════╝
//!
//! ## Architecture
//!
//! Iterates over [`SignalSource`] providers, gathers candidates,
//! persists them as signal threads with instances and events.

#![deny(clippy::all)]
#![deny(unused)]

pub mod discovery;
pub mod error;
pub mod models;
pub mod ports;
pub mod query;
pub mod scoring;
pub mod source;

mod candidate;

pub use error::SignalError;
pub use scoring::score_to_impact;

use crate::models::SignalMutation;
use crate::ports::{SemanticQuery, SignalDiscovery, SignalEvent, SignalEventLog, SignalPersistence};
use crate::source::{DiscoveryContext, SignalSource};

/// Aggregate report from a single Signal Engine run.
#[derive(Debug, Default, Clone)]
pub struct SignalEngineReport {
    pub threads_created: u64,
    pub instances_appended: u64,
    pub events_written: u64,
    pub lifecycle_transitions: u64,
}

/// Signal Engine — entry point for signal productionisation.
pub struct SignalEngine;

impl SignalEngine {
    /// Run a single cycle of the signal engine.
    ///
    /// Iterates over all [`SignalSource`] providers, gathers candidates,
    /// and persists them as signal threads.
    pub async fn run(
        persistence: &dyn SignalPersistence,
        event_log: Option<&dyn SignalEventLog>,
        discovery: &dyn SignalDiscovery,
        semantic: Option<&dyn SemanticQuery>,
        sources: &[&dyn SignalSource],
        now: i64,
    ) -> Result<SignalEngineReport, SignalError> {
        let mut report = SignalEngineReport::default();

        // Build context shared by all sources
        let ctx = DiscoveryContext { discovery, semantic, now };

        // 1. Gather candidates from all sources
        let mut all_candidates: Vec<crate::source::SignalCandidate> = Vec::new();
        for source in sources {
            match source.candidates(ctx.clone()).await {
                Ok(mut cands) => all_candidates.append(&mut cands),
                Err(_e) => { /* source failed — logged by caller */ }
            }
        }

        // 2. Persist each candidate as a signal thread
        for candidate in &all_candidates {
            let impact = score_to_impact(candidate.score);

            let upsert = persistence
                .upsert_signal_thread(
                    &candidate.signal_key,
                    candidate.anchor_entity_id,
                    &candidate.title,
                    &candidate.status,
                    &candidate.discovery_method,
                    candidate.discovery_score,
                )
                .await?;
            let thread_id = upsert.id;
            if upsert.mutation == SignalMutation::Created {
                report.threads_created += 1;
            }

            // Sprint 5.10: skip instance append if score/trend unchanged
            let should_append = match persistence.latest_instance_fingerprint(thread_id).await {
                Ok(Some((s, t))) => (s - candidate.score).abs() > 0.01 || t != candidate.trend,
                _ => true,
            };

            if should_append {
                let _instance_id = persistence
                    .append_signal_instance(
                        thread_id,
                        candidate.score,
                        impact,
                        &candidate.trend,
                        candidate.article_count,
                        candidate.source_count,
                        candidate.avg_score,
                        candidate.anchor_entity_id.unwrap_or(0),
                    )
                    .await?;
                report.instances_appended += 1;
            }

            let payload = serde_json::json!({
                "score": candidate.score,
                "impact": impact,
                "article_count": candidate.article_count,
                "source_count": candidate.source_count,
                "trend": candidate.trend,
                "signal_key": candidate.signal_key,
            });

            // Sprint 5.10: the event log is the canonical path. Legacy signal_events removed.
            // The per-run `events_written` counter seeds the stored event_id
            // (adapter derives `evt_{occurred_at}_{sequence}`) — unchanged behaviour.
            let sig_id = format!("SIG-{thread_id:06}");
            if let Some(log) = event_log {
                report.events_written += 1;
                let event = SignalEvent {
                    event_type: "SignalScoreChanged".into(),
                    aggregate_id: sig_id.clone(),
                    payload: payload.clone(),
                    occurred_at: now,
                };
                let _ = log.append(&event, report.events_written).await;
            }

            if upsert.mutation == SignalMutation::Created {
                if let Some(log) = event_log {
                    report.events_written += 1;
                    let event = SignalEvent {
                        event_type: "SignalCreated".into(),
                        aggregate_id: sig_id,
                        payload: serde_json::json!({"signal_key": candidate.signal_key}),
                        occurred_at: now,
                    };
                    let _ = log.append(&event, report.events_written).await;
                }
            }
        }

        // 3. Run lifecycle transitions
        persistence.update_signal_lifecycle(now).await?;
        report.lifecycle_transitions = 1;

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    //! Orchestration tests over fake ports. The engine's write loop, dedup and
    //! dual-write event path are verified without any store dependency (the
    //! original wasm-gated integration test exercised MemoryStore, which returns
    //! empty candidates and hardcoded upserts — see `signal_repository` infra
    //! tests for the DTO↔owned mapping coverage that replaces it).

    use std::cell::{Cell, RefCell};

    use async_trait::async_trait;

    use crate::models::{DiscoveryMethod, SignalMutation, SignalUpsertResult};
    use crate::ports::{SignalDiscovery, SignalEvent, SignalEventLog, SignalPersistence};
    use crate::source::{DiscoveryContext, SignalCandidate, SignalSource};
    use crate::{SignalEngine, SignalError};

    /// Emits a fixed candidate set regardless of the discovery context.
    struct StubSource {
        candidates: Vec<SignalCandidate>,
    }

    impl SignalSource for StubSource {
        fn candidates<'a>(
            &'a self,
            _ctx: DiscoveryContext<'a>,
        ) -> futures::future::LocalBoxFuture<'a, Result<Vec<SignalCandidate>, String>> {
            Box::pin(async move { Ok(self.candidates.clone()) })
        }
    }

    /// Returns an error from every source call (source-failure path).
    struct FailingSource;

    impl SignalSource for FailingSource {
        fn candidates<'a>(
            &'a self,
            _ctx: DiscoveryContext<'a>,
        ) -> futures::future::LocalBoxFuture<'a, Result<Vec<SignalCandidate>, String>> {
            Box::pin(async move { Err("boom".into()) })
        }
    }

    #[derive(Default)]
    struct FakePersistence {
        fingerprint: RefCell<Option<(f64, String)>>,
        upserts: Cell<u32>,
        appends: Cell<u32>,
        lifecycle: Cell<u32>,
    }

    impl FakePersistence {
        fn with_fingerprint(score: f64, trend: &str) -> Self {
            Self { fingerprint: RefCell::new(Some((score, trend.into()))), ..Default::default() }
        }
    }

    #[async_trait(?Send)]
    impl SignalPersistence for FakePersistence {
        async fn upsert_signal_thread(
            &self,
            _signal_key: &str,
            _anchor_entity_id: Option<i64>,
            _title: &str,
            _status: &str,
            _discovery_method: &DiscoveryMethod,
            _discovery_score: Option<f64>,
        ) -> Result<SignalUpsertResult, SignalError> {
            self.upserts.set(self.upserts.get() + 1);
            Ok(SignalUpsertResult { id: 1, mutation: SignalMutation::Created })
        }

        async fn latest_instance_fingerprint(&self, _thread_id: i64) -> Result<Option<(f64, String)>, SignalError> {
            Ok(self.fingerprint.borrow().clone())
        }

        async fn append_signal_instance(
            &self,
            _thread_id: i64,
            _score: f64,
            _impact: &str,
            _trend: &str,
            _article_count: i64,
            _source_count: i64,
            _avg_score: f64,
            _entity_id: i64,
        ) -> Result<i64, SignalError> {
            self.appends.set(self.appends.get() + 1);
            Ok(100)
        }

        async fn update_signal_lifecycle(&self, _now: i64) -> Result<(), SignalError> {
            self.lifecycle.set(self.lifecycle.get() + 1);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeDiscovery;

    #[async_trait(?Send)]
    impl SignalDiscovery for FakeDiscovery {
        async fn entity_signal_candidates(
            &self,
            _now: i64,
            _days: i64,
            _limit: u32,
            _min_entity_articles: u32,
            _min_sources: u32,
        ) -> Result<Vec<crate::models::EntityCandidate>, SignalError> {
            Ok(Vec::new())
        }

        async fn recent_embedded_articles(
            &self,
            _now: i64,
            _days: i64,
            _limit: u32,
        ) -> Result<Vec<crate::models::EmbeddedArticle>, SignalError> {
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct FakeEventLog {
        events: RefCell<Vec<(SignalEvent, u64)>>,
    }

    #[async_trait(?Send)]
    impl SignalEventLog for FakeEventLog {
        async fn append(&self, event: &SignalEvent, sequence: u64) -> Result<(), SignalError> {
            self.events.borrow_mut().push((event.clone(), sequence));
            Ok(())
        }

        async fn load(&self, _aggregate_id: &str, _limit: u32) -> Result<Vec<SignalEvent>, SignalError> {
            Ok(Vec::new())
        }
    }

    fn candidate() -> SignalCandidate {
        SignalCandidate {
            signal_key: "entity:1".into(),
            anchor_entity_id: Some(1),
            title: "CUDA".into(),
            status: "active".into(),
            discovery_method: DiscoveryMethod::Entity,
            discovery_score: Some(0.8),
            score: 0.8,
            trend: "rising".into(),
            article_count: 3,
            source_count: 2,
            avg_score: 0.7,
            evidence: vec![],
            related_entities: vec![],
        }
    }

    fn run(
        persistence: &dyn SignalPersistence,
        log: Option<&dyn SignalEventLog>,
        discovery: &dyn SignalDiscovery,
        sources: &[&dyn SignalSource],
        now: i64,
    ) -> crate::SignalEngineReport {
        futures::executor::block_on(SignalEngine::run(persistence, log, discovery, None, sources, now)).unwrap()
    }

    #[test]
    fn creates_thread_appends_instance_and_dual_writes_events() {
        let persistence = FakePersistence::default();
        let log = FakeEventLog::default();
        let discovery = FakeDiscovery;
        let stub = StubSource { candidates: vec![candidate()] };
        let sources: [&dyn SignalSource; 1] = [&stub];

        let report = run(&persistence, Some(&log), &discovery, &sources, 1_700_000_000);

        assert_eq!(report.threads_created, 1);
        assert_eq!(report.instances_appended, 1);
        assert_eq!(report.events_written, 2);
        assert_eq!(report.lifecycle_transitions, 1);
        assert_eq!(persistence.upserts.get(), 1);
        assert_eq!(persistence.appends.get(), 1);
        assert_eq!(persistence.lifecycle.get(), 1);

        // Dual-write: ScoreChanged (seq 1) then Created (seq 2) on the thread.
        let events = log.events.borrow();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0.event_type, "SignalScoreChanged");
        assert_eq!(events[1].0.event_type, "SignalCreated");
        assert_eq!(events[0].1, 1);
        assert_eq!(events[1].1, 2);
        assert_eq!(events[0].0.aggregate_id, "SIG-000001");
    }

    #[test]
    fn skips_instance_append_when_fingerprint_unchanged() {
        // Same (score, trend) as the fingerprint → no instance, events still written.
        let persistence = FakePersistence::with_fingerprint(0.8, "rising");
        let log = FakeEventLog::default();
        let discovery = FakeDiscovery;
        let stub = StubSource { candidates: vec![candidate()] };
        let sources: [&dyn SignalSource; 1] = [&stub];

        let report = run(&persistence, Some(&log), &discovery, &sources, 1_700_000_000);

        assert_eq!(report.threads_created, 1);
        assert_eq!(report.instances_appended, 0);
        assert_eq!(report.events_written, 2);
        assert_eq!(persistence.appends.get(), 0);
        assert_eq!(log.events.borrow().len(), 2);
    }

    #[test]
    fn source_failure_is_contained_and_lifecycle_still_runs() {
        let persistence = FakePersistence::default();
        let log = FakeEventLog::default();
        let discovery = FakeDiscovery;
        let sources: [&dyn SignalSource; 1] = [&FailingSource];

        let report = run(&persistence, Some(&log), &discovery, &sources, 1_700_000_000);

        // No candidates → no threads/instances/events; lifecycle always runs.
        assert_eq!(report.threads_created, 0);
        assert_eq!(report.instances_appended, 0);
        assert_eq!(report.events_written, 0);
        assert_eq!(report.lifecycle_transitions, 1);
        assert_eq!(persistence.upserts.get(), 0);
        assert_eq!(persistence.lifecycle.get(), 1);
    }

    #[test]
    fn run_without_event_log_is_tolerated() {
        let persistence = FakePersistence::default();
        let discovery = FakeDiscovery;
        let stub = StubSource { candidates: vec![candidate()] };
        let sources: [&dyn SignalSource; 1] = [&stub];

        let report = run(&persistence, None, &discovery, &sources, 1_700_000_000);

        assert_eq!(report.threads_created, 1);
        assert_eq!(report.instances_appended, 1);
        assert_eq!(report.events_written, 0);
    }
}
