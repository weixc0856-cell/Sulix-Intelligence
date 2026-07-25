//! LLM prompt builder for Briefing generation.
//!
//! The Signal Engine discovers and names signals. The LLM only explains:
//! why_it_matters, recommendation, impact. This prevents hallucinated
//! signal names and keeps the LLM focused on interpretation.

use super::types::SignalCandidate;

const SYSTEM_PROMPT: &str = "\
You are a strategic intelligence analyst.

Your task is to interpret the signals below. Each signal is an entity-anchored
intelligence observation discovered by the signal detection engine.

For each insight you produce:
- Reference evidence_signal_ids that support the insight
- Base your analysis on the signal's context (entities, prior decisions) as well as its metrics
- Do NOT invent new signal categories or rename signals

Each signal includes a `## Context` section with:
- **Entities**: entities frequently mentioned alongside this signal with type and relevance
- **Decisions**: your prior decisions tracking this signal with status and latest evaluation

Use this context to:
- Identify cross-signal connections visible through shared entities
- Consider whether prior decisions (active, confirmed, or contradicted) affect your recommendation
- If a prior decision was contradicted, treat it as a learning signal and adjust confidence accordingly
- Prioritize signals that are actively being decided upon

Output exactly 3-5 intelligence insights as JSON. No markdown, no code fences.

{
  \"schema_version\": 1,
  \"insights\": [
    {
      \"title\": \"Short, direct headline (max 10 words)\",
      \"category\": \"Security | AI | Market | Policy | Product\",
      \"summary\": \"2-3 sentence synthesis of what is happening\",
      \"why_it_matters\": \"1-2 sentences on why this changes the landscape\",
      \"recommendation\": \"1 sentence on what a decision-maker should do\",
      \"impact\": \"High | Medium | Low\",
      \"confidence\": 0.0-1.0,
      \"evidence_signal_ids\": [\"entity_123\", ...]
    }
  ]
}

For evidence_signal_ids, reference the signal IDs listed below that support each insight.
Use the signal's entity name and evidence as the basis for your interpretation.
Do NOT suggest that the signals themselves are wrong or incomplete — they are data-driven.
";

pub fn build_briefing_prompt(candidates: &[SignalCandidate]) -> String {
    let mut signals_section = String::new();
    signals_section.push_str("Current Intelligence Signals:\n\n");

    for sig in candidates.iter() {
        // Top article titles (max 3) as evidence context
        let top_titles: Vec<&str> = sig.articles.iter().take(3).map(|a| a.title.as_str()).collect();
        let titles_str = if top_titles.is_empty() { "none".to_string() } else { top_titles.join(" | ") };

        // Format context section
        let mut ctx_str = String::new();
        if !sig.context.entities.is_empty() {
            ctx_str.push_str("  ## Context\n");
            ctx_str.push_str("  Entities:\n");
            for e in &sig.context.entities {
                ctx_str.push_str(&format!("    - {} ({}, relevance:{:.2})\n", e.name, e.entity_type, e.relevance));
            }
            if !sig.context.decisions.is_empty() {
                ctx_str.push_str("  Decisions:\n");
                for d in &sig.context.decisions {
                    let eval_str = d.latest_evaluation.as_deref().unwrap_or("no evaluation");
                    ctx_str.push_str(&format!(
                        "    - [{status}] {title} | Latest: {eval}\n",
                        status = d.status,
                        title = d.title,
                        eval = eval_str
                    ));
                }
            }
        }

        signals_section.push_str(&format!(
            "[{id}] {title}
  Articles: {n} | Sources: {src} | Avg Score: {score:.1} | Trend: {trend}
  {context}  Key Evidence: {titles}
\n",
            id = sig.id,
            title = sig.title,
            n = sig.article_count,
            src = sig.source_count,
            score = sig.avg_score,
            trend = sig.trend,
            context = ctx_str,
            titles = titles_str,
        ));
    }

    format!(
        "{system}\n\n---\n\nAnalyze these {n} intelligence signals:\n\n{signals}",
        system = SYSTEM_PROMPT,
        n = candidates.len(),
        signals = signals_section,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_entity_signal_fields() {
        let candidates = vec![SignalCandidate {
            id: "entity_42".into(),
            title: "NVIDIA".into(),
            category: String::new(),
            signal_summary: String::new(),
            article_count: 32,
            source_count: 14,
            avg_score: 8.2,
            trend: "rising".into(),
            articles: vec![],
            context: Default::default(),
        }];
        let prompt = build_briefing_prompt(&candidates);
        assert!(prompt.contains("NVIDIA"));
        assert!(prompt.contains("Articles: 32"));
        assert!(prompt.contains("Sources: 14"));
        assert!(prompt.contains("entity_42"));
        assert!(!prompt.contains("Technology & AI Infrastructure"));
    }
}
