use crate::types::ContextResult;
use async_trait::async_trait;

#[async_trait(?Send)]
pub trait ContextProvider {
    async fn build_context(&self, query: &str) -> Result<ContextResult, String>;
}
