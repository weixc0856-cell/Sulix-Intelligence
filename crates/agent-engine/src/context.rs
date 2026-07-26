use async_trait::async_trait;
use crate::types::ContextResult;

#[async_trait(?Send)]
pub trait ContextProvider {
    async fn build_context(&self, query: &str) -> Result<ContextResult, String>;
}
