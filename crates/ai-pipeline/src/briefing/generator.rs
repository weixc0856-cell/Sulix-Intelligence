//! Daily Briefing generator — orchestrates signal loading, prompt building,
//! LLM call, parsing, evidence binding, and final assembly.

use super::parser::parse_briefing_json;
use super::prompt::build_briefing_prompt;
use super::types::{Briefing, EvidenceArticle, Insight, SignalCandidate};
use crate::{Summarizer, SummaryResult};

/// Rank a signal for selection: higher confidence × sqrt(article_count) wins.
fn rank_signal(n_articles: usize, confidence: f64) -> f64 {
    if n_articles == 0 {
        return 0.0;
    }
    let n = n_articles as f64;
    confidence * n.sqrt()
}

/// Select the top N signals by rank.
fn select_signals(candidates: Vec<SignalCandidate>, max: usize) -> Vec<SignalCandidate> {
    let mut sorted = candidates;
    sorted.sort_by(|a, b| {
        let ra = rank_signal(a.article_count, a.avg_score / 10.0);
        let rb = rank_signal(b.article_count, b.avg_score / 10.0);
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
    });
    sorted.truncate(max);
    sorted
}

/// Generate a Daily Intelligence Brief from today's signals.
///
/// # Arguments
/// * `signals` — today's signal candidates (already ranked/selected upstream)
/// * `summarizer` — LLM interface for synthesis
/// * `date` — YYYY-MM-DD string for the briefing date
/// * `now` — current unix timestamp
///
/// # Returns
/// * `Ok(Briefing)` — full structured briefing
/// * `Err(String)` — generation failed (logged, not fatal)
pub async fn generate_daily_brief(
    signals: Vec<SignalCandidate>,
    summarizer: &dyn Summarizer,
    date: &str,
    now: i64,
) -> Result<Briefing, String> {
    // 1. Select top signals
    let candidates = select_signals(signals, 20);

    if candidates.is_empty() {
        return Err("no signals available for briefing".into());
    }

    // 2. Build prompt
    let prompt = build_briefing_prompt(&candidates);

    // 3. Call LLM
    let result: SummaryResult = summarizer
        .summarize("Daily Intelligence Brief", &prompt)
        .await
        .map_err(|e| format!("LLM call failed: {e}"))?;

    // 4. Parse + validate
    let parsed = parse_briefing_json(&result.summary)?;

    // 5. Build evidence lookup by signal id
    let signal_map: std::collections::HashMap<&str, &SignalCandidate> =
        candidates.iter().map(|s| (s.id.as_str(), s)).collect();

    // 6. Assemble Briefing
    let mut insights = Vec::with_capacity(parsed.len());
    let mut all_trend = "stable".to_string();

    for p in parsed {
        // Resolve evidence signal ids → articles
        let mut articles: Vec<EvidenceArticle> = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();
        for sig_id in &p.evidence_signal_ids {
            if let Some(candidate) = signal_map.get(sig_id.as_str()) {
                for &aid in &candidate.article_ids {
                    if seen_ids.insert(aid) {
                        articles.push(EvidenceArticle {
                            id: aid,
                            title: String::new(),
                            url: None,
                            feed_name: None,
                            score: 0.0,
                        });
                    }
                }
            }
        }
        // Track most common trend
        if let Some(first_sig) = p.evidence_signal_ids.first() {
            if let Some(candidate) = signal_map.get(first_sig.as_str()) {
                all_trend = candidate.trend.clone();
            }
        }

        let evidence_count = articles.len() as u32;
        let source_count = seen_ids.len() as u32;

        insights.push(Insight {
            title: p.title,
            category: p.category,
            summary: p.summary,
            why_it_matters: p.why_it_matters,
            recommendation: p.recommendation,
            impact: p.impact,
            confidence: p.confidence,
            evidence_count,
            source_count,
            trend: all_trend.clone(),
            articles,
        });
    }

    Ok(Briefing {
        date: date.to_string(),
        generated_at: now,
        signal_count: candidates.len() as u32,
        insights,
    })
}
