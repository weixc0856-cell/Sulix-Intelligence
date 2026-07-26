//! RealReflectionGenerator — generates reflection drafts via ModelProvider.

use async_trait::async_trait;

use crate::context::ReflectionContext;
use crate::generator::prompt::{build_reflection_request, parse_reflection_response};
use crate::generator::r#trait::{ReflectionDraft, ReflectionGenerator};

/// Generates reflection drafts using a ModelProvider for real LLM inference.
pub struct RealReflectionGenerator {
    provider: Box<dyn model_runtime::ModelProvider>,
}

impl RealReflectionGenerator {
    pub fn new(provider: Box<dyn model_runtime::ModelProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait(?Send)]
impl ReflectionGenerator for RealReflectionGenerator {
    async fn generate(&self, context: &ReflectionContext) -> Result<ReflectionDraft, String> {
        let request = build_reflection_request(context);
        let response =
            self.provider.generate(request).await.map_err(|e| format!("Reflection generation failed: {e}"))?;
        parse_reflection_response(&response)
    }
}
