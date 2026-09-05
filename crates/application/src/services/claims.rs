//! Claim application service — read-only Claim use-cases. Claims are written by
//! the Pipeline Agent, never through a public API, so this service is
//! query-only.
//!
//! Generic over the narrowest store surface — [`store::ClaimRepository`] for the
//! claim row and [`store::ClaimQueryService`] for the article-evidence links.

use store::{Claim, ClaimEvidence, StoreError};

/// Application service for the Claim detail use-case.
pub struct ClaimService<S> {
    store: S,
}

impl<S> ClaimService<S>
where
    S: store::ClaimRepository + store::ClaimQueryService,
{
    /// Wrap a store (or store-backed repository/query pair) in the service.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Load a claim together with its article-evidence links.
    ///
    /// Evidence lookup is enrichment, not the primary payload: a lookup failure
    /// degrades to an empty evidence list (matches the historical handler
    /// behaviour, which used `unwrap_or_default`).
    pub async fn detail(&self, id: i64) -> Result<Option<(Claim, Vec<ClaimEvidence>)>, StoreError> {
        let claim = match self.store.find_claim(id).await? {
            Some(c) => c,
            None => return Ok(None),
        };
        let evidence = self.store.get_claim_evidence(id).await.unwrap_or_default();
        Ok(Some((claim, evidence)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::memory::MemoryStore;
    use store::{ClaimRepository, NewClaim};

    fn seed(store: &MemoryStore, statement: &str) -> i64 {
        futures::executor::block_on(store.save_claim(&NewClaim {
            statement: statement.into(),
            claim_type: "fact".into(),
            reasoning: None,
            falsification: None,
            status: None,
            article_id: None,
            observation_id: None,
        }))
        .expect("save_claim should succeed")
    }

    #[test]
    fn detail_returns_claim_with_evidence() {
        let store = MemoryStore::new();
        let id = seed(&store, "the sky is blue");
        let svc = ClaimService::new(store);

        let (claim, evidence) =
            futures::executor::block_on(svc.detail(id)).expect("detail should succeed").expect("claim should exist");
        assert_eq!(claim.id, id);
        assert_eq!(claim.statement, "the sky is blue");
        assert_eq!(claim.claim_type, "fact");
        // MemoryStore records no claim_evidence rows → empty enrichment.
        assert!(evidence.is_empty());
    }

    #[test]
    fn detail_missing_returns_none() {
        let svc = ClaimService::new(MemoryStore::new());
        assert!(futures::executor::block_on(svc.detail(999)).expect("detail should succeed").is_none());
    }
}
