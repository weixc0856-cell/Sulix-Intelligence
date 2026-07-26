use async_trait::async_trait;

use crate::{Claim, StoreError};

#[async_trait(?Send)]
pub trait ClaimQueryService {
    async fn list_claims(&self, status: Option<&str>, limit: u32) -> Result<Vec<Claim>, StoreError>;
}
