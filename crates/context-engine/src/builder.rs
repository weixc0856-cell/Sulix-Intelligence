use crate::models::NewContextSnapshot;
use crate::repository::ContextRepository;

use crate::assembler::assemble;
use crate::intent::parse;
use crate::pattern::{DefaultPatternDetector, PatternDetector};
use crate::planner::plan;
use crate::ranking::{apply_ranking_strategy, DefaultRanking};
use crate::retriever::{retrieve_decisions, retrieve_memories, retrieve_reflections};
use crate::types::{AgentContext, ContextRequestOptions};

/// ContextBuilder — facade that orchestrates the full pipeline.
///
/// Generic over [`ContextRepository`]; the composition root supplies a concrete
/// adapter (e.g. `D1ContextRepository`). The engine never names a concrete
/// store type.
pub struct ContextBuilder<R: ContextRepository> {
    repo: R,
}

impl<R: ContextRepository> ContextBuilder<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// Build context for a query. Full pipeline: parse → plan → retrieve → rank → assemble → snapshot.
    ///
    /// The snapshot is written to D1 via [`ContextRepository::save_context_snapshot`].
    /// (The legacy R2 artifact path was removed — every caller passed no object store.)
    pub async fn build(
        &self,
        query: &str,
        options: Option<ContextRequestOptions>,
        user_scope: Option<String>,
    ) -> Result<AgentContext, String> {
        let opts = options.unwrap_or_default();
        let _max_results = opts.max_results.unwrap_or(10);

        // 1. Parse intent
        let intent = parse(query);

        // 2. Plan retrieval
        let retrieval_plan = plan(&intent);

        // 3. Retrieve
        let decisions = if let Some(ref dq) = retrieval_plan.decision_query {
            retrieve_decisions(&self.repo, dq).await?
        } else {
            Vec::new()
        };
        let reflections = if let Some(ref rq) = retrieval_plan.reflection_query {
            retrieve_reflections(&self.repo, rq).await?
        } else {
            Vec::new()
        };
        let memories = if let Some(ref mq) = retrieval_plan.memory_query {
            retrieve_memories(&self.repo, mq).await?
        } else {
            Vec::new()
        };

        // 4. Rank
        let ranker = DefaultRanking;
        let decisions = apply_ranking_strategy(&ranker, decisions);
        let reflections = apply_ranking_strategy(&ranker, reflections);
        let memories = apply_ranking_strategy(&ranker, memories);

        // 5. Detect patterns
        let patterns = if retrieval_plan.pattern_enabled {
            let detector = DefaultPatternDetector;
            detector.detect(&decisions, &reflections)
        } else {
            Vec::new()
        };

        // 6. Assemble
        let now = (js_sys::Date::now() / 1000.0) as i64;
        let snapshot_id = format!("CTX-{}", now);
        let context = assemble(&snapshot_id, query, &intent, decisions, reflections, memories, patterns);

        // 7. Persist snapshot (D1 metadata; non-fatal on failure)
        let context_json = serde_json::to_string(&context).unwrap_or_default();
        let evidence_refs: Vec<String> = context.evidence.iter().map(|e| e.source_id.clone()).collect();

        let _ = self
            .repo
            .save_context_snapshot(&NewContextSnapshot {
                id: snapshot_id.clone(),
                query: query.into(),
                intent: serde_json::to_string(&intent).unwrap_or_default(),
                domain: intent.domain.clone(),
                context_json,
                evidence_refs: Some(serde_json::to_string(&evidence_refs).unwrap_or_default()),
                confidence: context.confidence.overall,
                user_scope,
            })
            .await;

        Ok(context)
    }
}
