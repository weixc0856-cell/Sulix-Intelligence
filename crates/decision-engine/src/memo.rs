//! Decision Memo generator — 12-section consulting-grade format.

use std::time::SystemTime;

use crate::domain::{DecisionMemo, MemoSection};

/// A framework trace for inclusion in the memo.
#[derive(Debug, Clone)]
pub struct FrameworkMemoSection {
    pub name: String,
    pub category: String,
    pub reasoning: String,
}

/// Generate a Decision Memo from decision context and optional framework traces.
///
/// Sections 5 (Key Assumptions) and 8 (Risk Assessment) are dynamically
/// populated from framework data when provided. Otherwise they show
/// placeholder text as in v1.
pub fn generate_memo(
    decision_id: i64,
    title: &str,
    context: &Option<String>,
    rationale: &Option<String>,
    confidence: f64,
    _signal_title: Option<&str>,
    frameworks: Option<&[FrameworkMemoSection]>,
) -> DecisionMemo {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let context_text = context.as_deref().unwrap_or("No context recorded");
    let rationale_text = rationale.as_deref().unwrap_or("No rationale recorded");

    let (assumptions_text, risk_text) = match frameworks {
        Some(fws) if !fws.is_empty() => (
            fws.iter()
                .map(|fw| format!("- {} ({}): {}", fw.name, fw.category, fw.reasoning))
                .collect::<Vec<_>>()
                .join("\n"),
            fws.iter()
                .map(|fw| format!("- {} may be incomplete if conditions change", fw.name))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        _ => (
            "To be documented. Each assumption should be falsifiable.".into(),
            "Risks should be documented as outcomes are tracked.".into(),
        ),
    };

    DecisionMemo {
        version: "1".into(),
        generated_at: now,
        sections: vec![
            section(1, "Executive Summary", &format!("Decision {}: {}. Confidence: {:.0}%.", decision_id, title, confidence * 100.0)),
            section(2, "Decision Context", context_text),
            section(3, "Situation Analysis", "Derived from associated signal intelligence."),
            section(4, "Evidence Review", rationale_text),
            section(5, "Frameworks Applied", &assumptions_text),
            section(6, "Strategic Options", "Option A: Proceed as planned.\nOption B: Monitor and wait.\nOption C: Gather additional evidence."),
            section(7, "Recommendation", &format!("Proceed with: {}", title)),
            section(8, "Countervailing Risks", &risk_text),
            section(9, "Expected Outcomes", "Define measurable metrics (users, revenue, adoption rate)."),
            section(10, "Confidence Assessment", &format!("Confidence: {:.0}%. Based on available evidence and source quality.", confidence * 100.0)),
            section(11, "Action Plan", "Define milestones with target dates."),
            section(12, "Review Date", &format!("Schedule review within 30 days. Generated at {} UTC.", now)),
        ],
    }
}

fn section(order: u32, title: &str, content: &str) -> MemoSection {
    MemoSection { order, title: title.to_string(), content: content.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memo_has_12_sections() {
        let memo = generate_memo(1, "Test", &None, &None, 0.8, None, None);
        assert_eq!(memo.sections.len(), 12);
        assert_eq!(memo.version, "1");
    }

    #[test]
    fn memo_contains_confidence() {
        let memo = generate_memo(1, "Test", &None, &None, 0.75, None, None);
        let section10 = &memo.sections[9];
        assert!(section10.content.contains("75%"));
    }

    #[test]
    fn memo_includes_frameworks_when_provided() {
        let frameworks = vec![
            FrameworkMemoSection { name: "Compound Growth".into(), category: "Finance".into(), reasoning: "Growth rate is 20% MoM".into() },
            FrameworkMemoSection { name: "Network Effects".into(), category: "Strategy".into(), reasoning: "Platform dynamics favor winner".into() },
        ];
        let memo = generate_memo(1, "Test", &None, &None, 0.8, None, Some(&frameworks));
        let section5 = &memo.sections[4];
        assert_eq!(section5.title, "Frameworks Applied");
        assert!(section5.content.contains("Compound Growth"));
        assert!(section5.content.contains("Network Effects"));
    }
}
