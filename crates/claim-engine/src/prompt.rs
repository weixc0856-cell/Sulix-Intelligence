//! Prompt construction for claim extraction.

/// Build the system prompt for claim extraction.
///
/// `frameworks_context` is an optional list of reasoning frameworks to
/// apply during analysis. When provided, the LLM will attempt to map
/// each claim to applicable frameworks.
pub fn build_claim_extraction_prompt(title: &str, body: &str, frameworks_context: Option<&str>) -> String {
    let frameworks_section = match frameworks_context {
        Some(ctx) if !ctx.is_empty() => format!(
            "\n\nApplicable Reasoning Frameworks:\n{ctx}\n\
             For each claim, identify which frameworks apply and how:\n\
             \"frameworks_applied\": [{{\"framework_id\": \"...\", \"relevance\": 0.8, \"reasoning\": \"...\"}}]"
        ),
        _ => String::new(),
    };

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
- opinion: value judgment or interpretation{}

Output ONLY valid JSON:
{{"claims": [{{"claim_type": "fact|trend|prediction|causal|opinion", "statement": "...", "reasoning": "...", "falsification": "...", "evidence_article_ids": [1,2], "counter_arguments": [], "frameworks_applied": [{{"framework_id": "...", "relevance": 0.8, "reasoning": "..."}}], "uncertainty": "low|medium|high"}}]}}

Title: {title}

Body: {body}"#,
        frameworks_section,
        title = title,
        body = body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_input() {
        let prompt = build_claim_extraction_prompt("Test Title", "Test body content", None);
        assert!(prompt.contains("Test Title"));
        assert!(prompt.contains("Test body content"));
        assert!(prompt.contains("claim_type"));
    }

    #[test]
    fn prompt_with_frameworks_includes_them() {
        let ctx = "- Compound Growth: small continuous growth leads to exponential results";
        let prompt = build_claim_extraction_prompt("Test Title", "Test body", Some(ctx));
        assert!(prompt.contains("frameworks_applied"));
        assert!(prompt.contains("Compound Growth"));
    }
}
