use async_trait::async_trait;

use crate::{Claim, NewClaim, StoreError};

#[async_trait(?Send)]
pub trait ClaimRepository {
    async fn save_claim(&self, claim: &NewClaim) -> Result<i64, StoreError>;
    async fn find_claim(&self, id: i64) -> Result<Option<Claim>, StoreError>;
}
