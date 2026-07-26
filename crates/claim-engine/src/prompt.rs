//! Prompt construction for claim extraction.

/// Build the system prompt for claim extraction.
pub fn build_claim_extraction_prompt(title: &str, body: &str) -> String {
    format!(
        r#"Extract atomic, falsifiable claims from the following article.

RULES:
1. Each claim must be independently testable — it should be possible to verify or falsify.
2. Separate facts from predictions from opinions.
3. For each claim, identify the specific evidence that supports it.
4. Identify what would prove the claim wrong (falsification condition).
5. Note counter-arguments present in the article.

Claim types:
- fact: verifiable, specific, about past or present
- trend: directional change over time
- prediction: future outcome or forecast
- causal: X causes or influences Y
- opinion: value judgment or interpretation

Output ONLY valid JSON:
{{"claims": [{{"claim_type": "fact|trend|prediction|causal|opinion", "statement": "...", "reasoning": "...", "falsification": "...", "evidence_article_ids": [1,2], "counter_arguments": [], "uncertainty": "low|medium|high"}}]}}

Title: {title}

Body: {body}"#,
        title = title,
        body = body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_input() {
        let prompt = build_claim_extraction_prompt("Test Title", "Test body content");
        assert!(prompt.contains("Test Title"));
        assert!(prompt.contains("Test body content"));
        assert!(prompt.contains("claim_type"));
    }
}
