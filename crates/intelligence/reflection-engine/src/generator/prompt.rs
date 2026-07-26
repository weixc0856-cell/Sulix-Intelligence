//! Reflection prompt builder — constructs the Decision Context Package for LLM inference.

use model_runtime::reflection_schema;
use model_runtime::{ContextBlock, GenerationParams, ModelRequest, ModelResponse};

use crate::context::ReflectionContext;

/// Build a ModelRequest from a ReflectionContext for reflection generation.
pub fn build_reflection_request(context: &ReflectionContext) -> ModelRequest {
    let system = build_reflection_system_prompt();
    let context_blocks = build_context_blocks(context);

    ModelRequest {
        task: model_runtime::ModelTask::Reflection,
        system_prompt: system,
        context: context_blocks,
        output_schema: Some(reflection_schema()),
        parameters: GenerationParams { temperature: 0.3, max_tokens: 2048 },
    }
}

/// System prompt for the reflection task.
fn build_reflection_system_prompt() -> String {
    r#"You are a decision reflection analyst. Your task is to analyze a decision and its outcome,
then generate structured lessons and decision rules.

Output JSON with this exact structure:
{
  "result": "correct" | "wrong" | "mixed",
  "confidence_calibration": "accurate" | "overconfident" | "underconfident",
  "quality_score": 0.0-1.0,
  "lessons": [
    {
      "category": "methodology" | "evidence" | "timing" | "risk",
      "domain": "the subject area",
      "description": "detailed lesson (at least 20 chars)",
      "severity": "high" | "medium" | "low",
      "confidence": 0.0-1.0,
      "evidence_basis": ["specific evidence references"]
    }
  ],
  "rules": [
    {
      "condition_domain": "the domain this rule applies to",
      "condition_trigger": "what triggers this rule",
      "action_type": "review" | "escalate" | "wait" | "investigate",
      "action_instruction": "what to do when triggered",
      "confidence": 0.0-1.0
    }
  ]
}

Ensure every lesson has evidence_basis populated with specific references.
Ensure at least 1 lesson and 1 rule.
"#
    .into()
}

/// Build context blocks from the ReflectionContext.
fn build_context_blocks(context: &ReflectionContext) -> Vec<ContextBlock> {
    let decision_json = serde_json::json!({
        "id": context.decision.id,
        "title": context.decision.title,
        "type": context.decision.decision_type,
    });

    let thesis_json = serde_json::json!({
        "hypothesis": context.thesis.hypothesis,
        "assumptions": context.thesis.assumptions,
        "initial_confidence": context.thesis.initial_confidence,
    });

    let outcome_json = context.outcome.as_ref().map(|o| {
        serde_json::json!({
            "type": o.outcome_type,
            "observation": o.observation,
        })
    });

    let evaluations_json: Vec<serde_json::Value> = context
        .evaluations
        .iter()
        .map(|e| {
            serde_json::json!({
                "evaluation": e.evaluation,
                "confidence": e.confidence,
                "reasoning": e.reasoning,
            })
        })
        .collect();

    let evidence_json: Vec<serde_json::Value> = context
        .evidence
        .iter()
        .map(|e| {
            serde_json::json!({
                "source": e.source,
                "summary": e.summary,
                "relevance": e.relevance_score,
            })
        })
        .collect();

    let mut blocks = Vec::new();

    blocks.push(ContextBlock { title: "Decision".into(), content: decision_json.to_string(), priority: 1.0 });

    blocks.push(ContextBlock { title: "Thesis".into(), content: thesis_json.to_string(), priority: 0.8 });

    if let Some(ref o) = outcome_json {
        blocks.push(ContextBlock { title: "Outcome".into(), content: o.to_string(), priority: 1.0 });
    }

    if !evaluations_json.is_empty() {
        blocks.push(ContextBlock {
            title: "Evaluations".into(),
            content: serde_json::to_string(&evaluations_json).unwrap_or_default(),
            priority: 0.6,
        });
    }

    if !evidence_json.is_empty() {
        blocks.push(ContextBlock {
            title: "Evidence".into(),
            content: serde_json::to_string(&evidence_json).unwrap_or_default(),
            priority: 0.7,
        });
    }

    blocks.push(ContextBlock {
        title: "Completeness Score".into(),
        content: format!("{:.2}", context.completeness_score),
        priority: 0.3,
    });

    blocks
}

/// Parse a ModelResponse into a ReflectionDraft.
pub fn parse_reflection_response(response: &ModelResponse) -> Result<crate::generator::ReflectionDraft, String> {
    let parsed = response.parsed.as_ref().ok_or_else(|| "No parsed JSON in model response".to_string())?;

    serde_json::from_value(parsed.clone()).map_err(|e| format!("Failed to parse reflection draft: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_expected_sections() {
        let prompt = build_reflection_system_prompt();
        assert!(prompt.contains("result"));
        assert!(prompt.contains("lessons"));
        assert!(prompt.contains("rules"));
        assert!(prompt.contains("evidence_basis"));
    }
}
