//! Signal Engine — materialise entity-driven signal candidates into
//! persistent signal threads with structured instances and events.
//!
//! ## Architecture
//!
//! Iterates over [`SignalSource`] providers, gathers candidates,
//! persists them as signal threads with instances and events.

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
use vectorize::VectorizeIndex;

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
        store: &impl StoreBackend,
        vectorize: Option<&VectorizeIndex>,
        sources: &[&dyn SignalSource],
        now: i64,
    ) -> Result<SignalEngineReport, store::StoreError> {
        let mut report = SignalEngineReport::default();

        // Build context shared by all sources
        let ctx = DiscoveryContext { store: store as &dyn StoreBackend, vectorize, now };

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
        let a1 = insert_article(&store, "NVIDIA Blackwell GPU");
        let a2 = insert_article(&store, "NVIDIA CUDA updates");
        let a3 = insert_article(&store, "NVIDIA data center");
        let a4 = insert_article(&store, "CUDA adoption grows");
        let a5 = insert_article(&store, "AI GPU demand rises");

        let nvidia_id = futures::executor::block_on(store.upsert_entity("NVIDIA", "nvidia", "organization")).unwrap();
        let cuda_id = futures::executor::block_on(store.upsert_entity("CUDA", "cuda", "product")).unwrap();

        for &aid in &[a1, a2, a3] {
            futures::executor::block_on(store.link_article_entity(aid, nvidia_id, 0.9, None)).unwrap();
        }
        for &aid in &[a2, a4, a5] {
            futures::executor::block_on(store.link_article_entity(aid, cuda_id, 0.7, None)).unwrap();
        }

        let now = 1000000;
        let candidates =
            futures::executor::block_on(store.entity_signal_candidates_filtered(now, 7, 50, 2, 1)).unwrap();
        assert_eq!(candidates.len(), 2, "should have 2 candidates (NVIDIA, CUDA)");

        let source = crate::source::EntitySignalSource;
        let sources = [&source as &dyn crate::source::SignalSource];
        let report = futures::executor::block_on(crate::SignalEngine::run(&store, None, &sources, now)).unwrap();
        assert!(report.threads_created >= 2, "should create at least 2 threads");
        assert!(report.instances_appended >= 2, "should append at least 2 instances");

        let threads = futures::executor::block_on(store.get_active_signal_threads(10)).unwrap();
        assert!(!threads.is_empty(), "should have active signal threads");
        let nvidia_thread = threads.iter().find(|t| t.title == "NVIDIA").unwrap();
        assert!(nvidia_thread.instances.len() >= 1, "should have at least 1 instance");

        let events = futures::executor::block_on(store.load_signal_events(nvidia_thread.thread_id, 10)).unwrap();
        assert!(!events.is_empty(), "should have signal events");
        assert_eq!(events[0].event_type, "created", "first event should be 'created'");
    }
}
