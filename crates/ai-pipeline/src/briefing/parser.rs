//! LLM output parser + schema validator for Briefing generation.

use super::types::{GeneratedInsight, LlmOutput};

#[derive(Debug)]
pub struct ParsedInsight {
    pub title: String,
    pub category: String,
    pub summary: String,
    pub why_it_matters: String,
    pub recommendation: String,
    pub impact: String,
    pub confidence: f64,
    pub evidence_signal_ids: Vec<String>,
}

/// Parse and validate the LLM's JSON response.
/// Returns a list of validated insights or an error explaining what failed.
pub fn parse_briefing_json(text: &str) -> Result<Vec<ParsedInsight>, String> {
    // Strip markdown code fences if present
    let cleaned = text
        .trim()
        .strip_prefix("```json")
        .or_else(|| text.trim().strip_prefix("```"))
        .map(|s| s.trim_end().strip_suffix("```").unwrap_or(s.trim_end()))
        .unwrap_or(text.trim());

    let output: LlmOutput = serde_json::from_str(cleaned).map_err(|e| {
        format!("failed to parse LLM output as JSON: {e}")
    })?;

    if output.insights.is_empty() {
        return Err("LLM returned zero insights".into());
    }
    if output.insights.len() > 5 {
        return Err(format!(
            "LLM returned {} insights (max 5)",
            output.insights.len()
        ));
    }

    let mut validated = Vec::new();
    for (i, ins) in output.insights.into_iter().enumerate() {
        validate_insight(&ins, i)?;
        validated.push(ParsedInsight {
            title: ins.title,
            category: ins.category,
            summary: ins.summary,
            why_it_matters: ins.why_it_matters,
            recommendation: ins.recommendation,
            impact: ins.impact,
            confidence: ins.confidence,
            evidence_signal_ids: ins.evidence_signal_ids,
        });
    }

    Ok(validated)
}

fn validate_insight(ins: &GeneratedInsight, idx: usize) -> Result<(), String> {
    if ins.title.len() < 5 {
        return Err(format!("insight[{idx}] title too short ({})", ins.title.len()));
    }
    if ins.summary.len() < 80 {
        return Err(format!(
            "insight[{idx}] summary too short ({} chars, need >= 80)",
            ins.summary.len()
        ));
    }
    if ins.recommendation.len() < 20 {
        return Err(format!(
            "insight[{idx}] recommendation too short ({} chars, need >= 20)",
            ins.recommendation.len()
        ));
    }
    if !(0.0..=1.0).contains(&ins.confidence) {
        return Err(format!(
            "insight[{idx}] confidence {:.2} out of range [0.0, 1.0]",
            ins.confidence
        ));
    }
    match ins.impact.as_str() {
        "High" | "Medium" | "Low" => {}
        _ => return Err(format!("insight[{idx}] invalid impact: {}", ins.impact)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_json() {
        let json = r#"{
            "schema_version": 1,
            "insights": [
                {
                    "title": "AI Agent Security Shift",
                    "category": "Security",
                    "summary": "This is a reasonably long summary that exceeds eighty characters in total length so that the validation test will pass without any problems at all.",
                    "why_it_matters": "This changes how enterprises approach AI deployment.",
                    "recommendation": "Organizations should review their agent security postures immediately.",
                    "impact": "High",
                    "confidence": 0.87,
                    "evidence_signal_ids": ["sig_001", "sig_003"]
                }
            ]
        }"#;
        let result = parse_briefing_json(json);
        assert!(result.is_ok(), "{:?}", result.err());
        let insights = result.unwrap();
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].title, "AI Agent Security Shift");
        assert_eq!(insights[0].impact, "High");
    }

    #[test]
    fn invalid_impact() {
        let json = r#"{
            "schema_version": 1,
            "insights": [{
                "title": "Test Title Here",
                "category": "AI",
                "summary": "This summary is long enough to pass the minimum character requirement for sure it definitely works now.",
                "why_it_matters": "This matters because of reasons.",
                "recommendation": "Do something about this.",
                "impact": "Critical",
                "confidence": 0.5,
                "evidence_signal_ids": []
            }]
        }"#;
        let result = parse_briefing_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn empty_insights() {
        let json = r#"{"schema_version":1,"insights":[]}"#;
        let result = parse_briefing_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn strips_code_fences() {
        let json = "```json\n{\"schema_version\":1,\"insights\":[{\"title\":\"AI Safety Progress\",\"category\":\"AI\",\"summary\":\"This is a sufficiently long summary that goes way past the eighty character minimum threshold so it should pass the validator.\",\"why_it_matters\":\"Safety alignment advances are accelerating.\",\"recommendation\":\"Teams should evaluate new techniques.\",\"impact\":\"Medium\",\"confidence\":0.65,\"evidence_signal_ids\":[]}]}\n```";
        let result = parse_briefing_json(json);
        assert!(result.is_ok(), "{:?}", result.err());
    }
}
