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
