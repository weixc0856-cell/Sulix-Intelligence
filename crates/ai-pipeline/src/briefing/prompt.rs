//! LLM prompt builder for Briefing generation.

use super::types::SignalCandidate;

const SYSTEM_PROMPT: &str = "\
You are a strategic intelligence analyst.

Your task is NOT summarization.

You must identify:
- emerging trends
- structural changes
- risks
- opportunities

Do NOT repeat article titles.
Do NOT create generic statements.

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
      \"evidence_signal_ids\": [\"sig_001\", ...]
    }
  ]
}

For evidence_signal_ids, reference the signal IDs listed below that support each insight.
";

pub fn build_briefing_prompt(candidates: &[SignalCandidate]) -> String {
    let mut signals_section = String::new();
    signals_section.push_str("Signals:\n\n");
    for sig in candidates.iter() {
        signals_section.push_str(&format!(
            "[{id}] Title: {title}
  Category: {cat}
  Articles: {n}
  Avg Score: {score:.1}
  Trend: {trend}
  Evidence IDs: [{ids}]
\n",
            id = sig.id,
            title = sig.title,
            cat = sig.category,
            n = sig.article_count,
            score = sig.avg_score,
            trend = sig.trend,
            ids = sig.article_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(", "),
        ));
    }

    format!(
        "{}\n\n---\n\nAnalyze these {} signals:\n\n{}",
        SYSTEM_PROMPT,
        candidates.len(),
        signals_section,
    )
}
