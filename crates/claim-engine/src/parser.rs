//! Parse LLM claim extraction responses into structured ClaimCandidates.

use crate::domain::{ClaimCandidate, ClaimType, EvidenceRef, LlmClaimOutput, Uncertainty};

/// Parse an LLM response into a list of ClaimCandidates.
pub fn parse_claims_from_response(response: &str) -> Result<Vec<ClaimCandidate>, String> {
    // Strip markdown fences if present
    let cleaned = strip_markdown_fences(response);
    let output: LlmClaimOutput =
        serde_json::from_str(cleaned).map_err(|e| format!("Failed to parse LLM claim output: {e}"))?;

    let mut claims = Vec::new();
    for item in output.claims {
        let claim_type = parse_claim_type(&item.claim_type)?;
        let uncertainty = parse_uncertainty(&item.uncertainty)?;

        let evidence_refs: Vec<EvidenceRef> =
            item.evidence_article_ids.iter().map(|&id| EvidenceRef { article_id: id, relevance: 1.0 }).collect();

        claims.push(ClaimCandidate {
            statement: item.statement,
            claim_type,
            reasoning: item.reasoning,
            falsification: item.falsification,
            evidence_refs,
            counter_arguments: item.counter_arguments,
            uncertainty,
        });
    }

    Ok(claims)
}

fn strip_markdown_fences(input: &str) -> &str {
    let input = input.trim();
    if input.starts_with("```") {
        if let Some(end) = input.rfind("```") {
            let start = input.find('\n').map(|i| i + 1).unwrap_or(3);
            return input[start..end].trim();
        }
    }
    input
}

fn parse_claim_type(s: &str) -> Result<ClaimType, String> {
    match s.trim().to_lowercase().as_str() {
        "fact" => Ok(ClaimType::Fact),
        "trend" => Ok(ClaimType::Trend),
        "prediction" => Ok(ClaimType::Prediction),
        "causal" => Ok(ClaimType::Causal),
        "opinion" => Ok(ClaimType::Opinion),
        other => Err(format!("Unknown claim type: {other}")),
    }
}

fn parse_uncertainty(s: &str) -> Result<Uncertainty, String> {
    match s.trim().to_lowercase().as_str() {
        "low" => Ok(Uncertainty::Low),
        "medium" => Ok(Uncertainty::Medium),
        "high" => Ok(Uncertainty::High),
        other => Err(format!("Unknown uncertainty: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_json() {
        let input = r#"{
            "claims": [{
                "claim_type": "trend",
                "statement": "AI investment is increasing",
                "reasoning": "Multiple sources report rising funding",
                "falsification": "Q3 funding drops below Q2",
                "evidence_article_ids": [1, 2],
                "counter_arguments": ["May be seasonal"],
                "uncertainty": "low"
            }]
        }"#;
        let claims = parse_claims_from_response(input).unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].claim_type, ClaimType::Trend);
        assert_eq!(claims[0].evidence_refs.len(), 2);
    }

    #[test]
    fn strips_markdown_fences() {
        let input = "```json\n{\"claims\": []}\n```";
        let result = parse_claims_from_response(input).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn rejects_unknown_type() {
        let input =
            r#"{"claims": [{"claim_type": "unknown", "statement": "x", "reasoning": "y", "uncertainty": "low"}]}"#;
        assert!(parse_claims_from_response(input).is_err());
    }
}
