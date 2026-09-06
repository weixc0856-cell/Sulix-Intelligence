use crate::repository::ContextRepository;
use crate::types::{DecisionQuery, MemoryQuery, ReflectionQuery, ScoredDecision, ScoredMemory, ScoredReflection};

/// Retrieve decisions matching a query.
pub async fn retrieve_decisions<R: ContextRepository>(
    repo: &R,
    query: &DecisionQuery,
) -> Result<Vec<ScoredDecision>, String> {
    if query.statuses.is_empty() {
        return Ok(Vec::new());
    }
    let statuses: Vec<&str> = query.statuses.iter().map(String::as_str).collect();
    let decisions = repo.list_decisions(&statuses, query.limit).await.map_err(|e| format!("list_decisions: {e}"))?;
    let scored: Vec<ScoredDecision> = decisions
        .into_iter()
        .filter_map(|d| {
            // Client-side domain filter (MVP; D1 doesn't do complex WHERE easily)
            if let Some(ref domain) = query.domain {
                if !d.decision_type.contains(domain) && !d.title.to_lowercase().contains(&domain.to_lowercase()) {
                    return None;
                }
            }
            Some(ScoredDecision {
                id: format!("DEC-{:06}", d.id),
                title: d.title,
                decision_type: d.decision_type,
                status: d.status,
                confidence: d.confidence,
                relevance_score: 0.0,
                rank_components: crate::types::RankComponents {
                    query_alignment: 0.0,
                    confidence: d.confidence,
                    recency: 0.0,
                    usage_frequency: 0.0,
                    user_specificity: 1.0,
                },
            })
        })
        .collect();
    Ok(scored)
}

/// Retrieve reflections matching a query.
#[allow(unused_variables)]
pub async fn retrieve_reflections<R: ContextRepository>(
    repo: &R,
    query: &ReflectionQuery,
) -> Result<Vec<ScoredReflection>, String> {
    // Reflections queried by status via list_decisions then look up. For MVP: return empty vector.
    // Full impl when reflection retrieval is added to the ContextRepository port.
    Ok(Vec::new())
}

/// Retrieve memories matching a query.
pub async fn retrieve_memories<R: ContextRepository>(
    repo: &R,
    query: &MemoryQuery,
) -> Result<Vec<ScoredMemory>, String> {
    let memories =
        repo.list_memories(query.status.as_deref(), query.limit).await.map_err(|e| format!("list_memories: {e}"))?;
    let scored: Vec<ScoredMemory> = memories
        .into_iter()
        .map(|m| ScoredMemory {
            id: format!("MEM-{:06}", m.id),
            statement: m.statement,
            memory_type: m.memory_type,
            confidence: m.confidence,
            relevance_score: 0.0,
            rank_components: crate::types::RankComponents {
                query_alignment: 0.0,
                confidence: m.confidence,
                recency: 0.0,
                usage_frequency: m.usage_count as f64,
                user_specificity: 1.0,
            },
        })
        .collect();
    Ok(scored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ContextError;
    use crate::models::{DecisionRecord, MemoryRecord, NewContextSnapshot};
    use crate::repository::ContextRepository;
    use async_trait::async_trait;
    use std::cell::RefCell;

    /// Fake repo: returns fixed rows and records the `statuses` slices it was
    /// called with (proving the port forwards the evidence allowlist verbatim).
    struct FakeRepo {
        rows: Vec<DecisionRecord>,
        seen: RefCell<Vec<Vec<String>>>,
    }

    impl FakeRepo {
        fn new(rows: Vec<DecisionRecord>) -> Self {
            Self { rows, seen: RefCell::new(Vec::new()) }
        }
    }

    #[async_trait(?Send)]
    impl ContextRepository for FakeRepo {
        async fn list_decisions(&self, statuses: &[&str], _limit: u32) -> Result<Vec<DecisionRecord>, ContextError> {
            self.seen.borrow_mut().push(statuses.iter().map(|s| s.to_string()).collect());
            Ok(self.rows.clone())
        }
        async fn list_memories(&self, _status: Option<&str>, _limit: u32) -> Result<Vec<MemoryRecord>, ContextError> {
            Ok(Vec::new())
        }
        async fn save_context_snapshot(&self, _snap: &NewContextSnapshot) -> Result<(), ContextError> {
            Ok(())
        }
    }

    fn record(id: i64, decision_type: &str, status: &str, title: &str) -> DecisionRecord {
        DecisionRecord {
            id,
            title: title.into(),
            decision_type: decision_type.into(),
            status: status.into(),
            confidence: 0.6,
        }
    }

    #[test]
    fn forwards_statuses_verbatim() {
        let repo = FakeRepo::new(vec![
            record(1, "investment", "active", "Buy index"),
            record(2, "investment", "completed", "Sold holding"),
        ]);
        let query = DecisionQuery { domain: None, statuses: vec!["active".into(), "completed".into()], limit: 10 };
        let scored = futures::executor::block_on(retrieve_decisions(&repo, &query)).unwrap();
        assert_eq!(scored.len(), 2);
        assert_eq!(scored[0].id, "DEC-000001");
        assert_eq!(scored[1].status, "completed");
        // the evidence allowlist reached the port unchanged
        assert_eq!(*repo.seen.borrow(), vec![vec!["active".to_string(), "completed".to_string()]]);
    }

    #[test]
    fn empty_statuses_short_circuits_without_repo_call() {
        let repo = FakeRepo::new(vec![record(1, "investment", "active", "X")]);
        let query = DecisionQuery { domain: None, statuses: vec![], limit: 10 };
        let scored = futures::executor::block_on(retrieve_decisions(&repo, &query)).unwrap();
        assert!(scored.is_empty());
        assert!(repo.seen.borrow().is_empty(), "port must not be called for an empty allowlist");
    }

    #[test]
    fn client_side_domain_filter_still_applies() {
        let repo = FakeRepo::new(vec![
            record(1, "investment", "completed", "Buy index fund"),
            record(2, "investment", "active", "Evaluate REIT"),
            record(3, "brand", "active", "Marketing push"),
        ]);
        let query = DecisionQuery {
            domain: Some("investment".into()),
            statuses: vec!["active".into(), "completed".into()],
            limit: 10,
        };
        let scored = futures::executor::block_on(retrieve_decisions(&repo, &query)).unwrap();
        assert_eq!(scored.len(), 2);
        assert!(scored.iter().all(|d| d.decision_type == "investment"));
        assert_eq!(*repo.seen.borrow(), vec![vec!["active".to_string(), "completed".to_string()]]);
    }
}
