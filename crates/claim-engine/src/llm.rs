//! LlmClaimExtractor — uses ModelProvider for LLM-based claim extraction.

use async_trait::async_trait;

use crate::domain::ClaimCandidate;
use crate::extractor::ClaimExtractor;
use crate::parser::parse_claims_from_response;
use crate::prompt::build_claim_extraction_prompt;

/// Extracts claims from articles using a ModelProvider.
pub struct LlmClaimExtractor {
    provider: Box<dyn model_runtime::ModelProvider>,
}

impl LlmClaimExtractor {
    pub fn new(provider: Box<dyn model_runtime::ModelProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait(?Send)]
impl ClaimExtractor for LlmClaimExtractor {
    async fn extract(
        &self,
        title: &str,
        body: &str,
        _article_id: i64,
        frameworks_context: Option<&str>,
    ) -> Result<Vec<ClaimCandidate>, String> {
        let prompt = build_claim_extraction_prompt(title, body, frameworks_context);

        let request = model_runtime::ModelRequest {
            task: model_runtime::ModelTask::ClaimExtraction,
            system_prompt: prompt,
            context: vec![],
            output_schema: Some(serde_json::json!({"type": "json_object"})),
            parameters: model_runtime::GenerationParams { temperature: 0.2, max_tokens: 2048 },
        };

        let response = self.provider.generate(request).await.map_err(|e| format!("Claim extraction failed: {e}"))?;

        let content = response.parsed.map(|v| v.to_string()).unwrap_or(response.text);

        parse_claims_from_response(&content)
    }
}
