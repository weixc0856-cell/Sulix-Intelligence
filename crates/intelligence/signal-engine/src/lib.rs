//! Signal Engine — materialise entity-driven signal candidates into
//! persistent signal threads with structured instances and events.
//!
//! This is the intelligence core that bridges the gap between
//! "entity activity detected" and "actionable intelligence signal".
//!
//! ## Architecture
//!
//! ```text
//! Entity Candidates (store::entity_signal_candidates_filtered)
//!     │
//!     ▼
//! SignalEngine::run()
//!     │
//!     ├── 1. fetch & filter candidates
//!     ├── 2. upsert signal thread per candidate
//!     ├── 3. append signal instance snapshot
//!     ├── 4. write signal_events for timeline
//!     └── 5. update lifecycle transitions
//!     │
//!     ▼
//! SignalThreads ready for Radar / Briefing / Detail
//! ```

#![deny(clippy::all)]
#![deny(unused)]

pub mod discovery;
pub mod pipeline;
pub mod query;
pub mod scoring;
pub mod source;

mod candidate;

pub use scoring::score_to_impact;

use store::StoreBackend;

use crate::source::SignalSource;

/// Aggregate report from a single Signal Engine run.
#[derive(Debug, Default, Clone)]
pub struct SignalEngineReport {
    /// Number of entity candidates that became signal threads.
    pub threads_created: u64,
    /// Total signal instances appended across all threads.
    pub instances_appended: u64,
    /// Total signal events written.
    pub events_written: u64,
    /// Number of threads whose lifecycle status changed.
    pub lifecycle_transitions: u64,
}

/// Signal Engine — the pure-logic entry point for signal productionisation.
pub struct SignalEngine;

impl SignalEngine {
    /// Run a single cycle of the signal engine.
    ///
    /// Iterates over all [`SignalSource`] providers, gathers candidates,
    /// merges overlapping ones, and persists them as signal threads.
    ///
    /// # Arguments
    ///
    /// * `store` — Any implementation of [`StoreBackend`].
    /// * `sources` — List of signal candidate providers.
    /// * `now` — Unix timestamp (seconds) representing "now".
    pub async fn run(
        store: &impl StoreBackend,
        sources: &[&dyn SignalSource],
        now: i64,
    ) -> Result<SignalEngineReport, store::StoreError> {
        let mut report = SignalEngineReport::default();

        // 1. Gather candidates from all sources
        let mut all_candidates: Vec<crate::source::SignalCandidate> = Vec::new();
        for source in sources {
            match source.candidates(store, now).await {
                Ok(mut cands) => all_candidates.append(&mut cands),
                Err(_e) => { /* source failed — logged by caller */ }
            }
        }

        // 2. For each candidate, upsert thread + append instance + write event
        for candidate in &all_candidates {
            let impact = score_to_impact(candidate.score);

            let upsert = store
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
            if upsert.mutation == store::SignalMutation::Created {
                report.threads_created += 1;
            }

            let _instance_id = store
                .append_signal_instance_v2(
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

            let payload = serde_json::json!({
                "score": candidate.score,
                "impact": impact,
                "article_count": candidate.article_count,
                "source_count": candidate.source_count,
                "trend": candidate.trend,
                "signal_key": candidate.signal_key,
            });
            store.insert_signal_event(thread_id, "score_changed", Some(&payload.to_string())).await?;
            report.events_written += 1;

            if upsert.mutation == store::SignalMutation::Created {
                store.insert_signal_event(thread_id, "created", None).await?;
                report.events_written += 1;
            }
        }

        // 3. Run lifecycle transitions
        store.update_signal_lifecycle(now).await?;
        report.lifecycle_transitions = 1;

        Ok(report)
    }
}

/// Integration test for the full entity-to-signal-thread pipeline.
/// NOTE: Only runs on wasm32 target because the store crate's D1Store impls
/// depend on `js_sys::Date::now()`.  MemoryStore is used here but the
/// worker-rs runtime (transitive dep) requires a wasm environment.
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use store::{memory::MemoryStore, NewArticle, StoreBackend};

    fn insert_article(store: &MemoryStore, title: &str) -> i64 {
        futures::executor::block_on(store.insert_article(&NewArticle {
            feed_id: 1,
            guid: title.into(),
            title: title.into(),
            url: Some("https://example.com/article".into()),
            published_at: Some(999000),
            raw_content_r2_key: None,
        }))
        .unwrap()
        .unwrap()
    }

    #[test]
    fn test_entity_to_signal_thread_pipeline() {
        let store = MemoryStore::new();

        // 1. Insert test articles (synchronous wrappers)
        let a1 = insert_article(&store, "NVIDIA Blackwell GPU");
        let a2 = insert_article(&store, "NVIDIA CUDA updates");
        let a3 = insert_article(&store, "NVIDIA data center");
        let a4 = insert_article(&store, "CUDA adoption grows");
        let a5 = insert_article(&store, "AI GPU demand rises");

        // 2. Create entities and link articles
        let nvidia_id = futures::executor::block_on(store.upsert_entity("NVIDIA", "nvidia", "organization")).unwrap();
        let cuda_id = futures::executor::block_on(store.upsert_entity("CUDA", "cuda", "product")).unwrap();

        for &aid in &[a1, a2, a3] {
            futures::executor::block_on(store.link_article_entity(aid, nvidia_id, 0.9, None)).unwrap();
        }
        for &aid in &[a2, a4, a5] {
            futures::executor::block_on(store.link_article_entity(aid, cuda_id, 0.7, None)).unwrap();
        }

        let now = 1000000;

        // 3. Verify filtered candidates
        let candidates =
            futures::executor::block_on(store.entity_signal_candidates_filtered(now, 7, 50, 2, 1)).unwrap();
        assert_eq!(candidates.len(), 2, "should have 2 candidates (NVIDIA, CUDA)");

        // 4. Run signal engine
        let source = crate::source::EntitySignalSource;
        let sources = [&source as &dyn crate::source::SignalSource];
        let report = futures::executor::block_on(crate::SignalEngine::run(&store, &sources, now)).unwrap();
        assert!(report.threads_created >= 2, "should create at least 2 threads");
        assert!(report.instances_appended >= 2, "should append at least 2 instances");

        // 5. Verify threads were created
        let threads = futures::executor::block_on(store.get_active_signal_threads(10)).unwrap();
        assert!(!threads.is_empty(), "should have active signal threads");
        let nvidia_thread = threads.iter().find(|t| t.title == "NVIDIA").unwrap();
        assert!(nvidia_thread.instances.len() >= 1, "should have at least 1 instance");

        // 6. Verify signal events
        let events = futures::executor::block_on(store.load_signal_events(nvidia_thread.thread_id, 10)).unwrap();
        assert!(!events.is_empty(), "should have signal events");
        assert_eq!(events[0].event_type, "created", "first event should be 'created'");
    }
}
