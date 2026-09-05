use async_trait::async_trait;

use crate::{Claim, ClaimEvidence, StoreError};

#[async_trait(?Send)]
pub trait ClaimQueryService {
    async fn list_claims(&self, status: Option<&str>, limit: u32) -> Result<Vec<Claim>, StoreError>;
    async fn get_claim_evidence(&self, claim_id: i64) -> Result<Vec<ClaimEvidence>, StoreError>;
}
