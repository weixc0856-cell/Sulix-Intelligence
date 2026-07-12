//! Summary — 跨信号/判断的综合摘要
//!
//! 从 IntelligenceOutput 生成人类可读的今日摘要。
//! 规则驱动（非 LLM），确保快速、可预测、零成本。
//!
//! 旧版: clusterer/synthesis.rs（接收旧 Theme + ThemeAnalysis）
//! 新版: 接收 contract::Signal + contract::Thesis + contract::Decision

use sulix_contract as contract;

/// 摘要文本
#[derive(Debug, Clone)]
pub struct SummaryText {
    /// 标题行（如 "3 个信号指向 AI 基础设施加速"）
    pub headline: String,
    /// 叙述体
    pub narrative: String,
    /// 信号数量
    pub signal_count: usize,
    /// 判断数量
    pub thesis_count: usize,
    /// 决策数量
    pub decision_count: usize,
}

/// 从 pipeline 输出生成综合摘要
///
/// 规则（非 LLM）:
/// - 高重要性信号 > 1 个 → "N 个信号指向同一方向"
/// - 有决策 → 附决策摘要
pub fn synthesize(
    signals: &[contract::Signal],
    theses: &[contract::Thesis],
    decisions: &[contract::Decision],
) -> SummaryText {
    let narrative = build_narrative(signals, theses, decisions);
    let headline = build_headline(signals, theses, decisions);

    SummaryText {
        headline,
        narrative,
        signal_count: signals.len(),
        thesis_count: theses.len(),
        decision_count: decisions.len(),
    }
}

fn build_headline(
    signals: &[contract::Signal],
    _theses: &[contract::Thesis],
    decisions: &[contract::Decision],
) -> String {
    let high_importance = signals.iter().filter(|s| s.importance >= 0.7).count();
    let high_domains: std::collections::HashSet<&str> = signals
        .iter()
        .filter(|s| s.importance >= 0.7)
        .map(|s| s.domain.as_str())
        .collect();

    if !decisions.is_empty() {
        let invest_count = decisions.iter().filter(|d| matches!(d.action, contract::DecisionType::Invest | contract::DecisionType::Build)).count();
        if invest_count > 0 {
            return format!("{} 个高价值信号 → {} 个行动决策", high_importance, invest_count);
        }
    }

    if high_importance >= 2 && !high_domains.is_empty() {
        let domains = high_domains.into_iter().collect::<Vec<_>>().join(" / ");
        return format!("{} 个信号聚焦 {}", high_importance, domains);
    }

    if high_importance == 1 {
        if let Some(top) = signals.iter().max_by(|a, b| a.importance.partial_cmp(&b.importance).unwrap_or(std::cmp::Ordering::Equal)) {
            return format!("关注 {}（重要性 {:.2})", top.domain, top.importance);
        }
    }

    format!("{} 条信号分析", signals.len())
}

fn build_narrative(
    signals: &[contract::Signal],
    theses: &[contract::Thesis],
    decisions: &[contract::Decision],
) -> String {
    let mut narrative = String::new();

    // 高重要性信号
    let important: Vec<&contract::Signal> = signals.iter().filter(|s| s.importance >= 0.7).collect();
    if !important.is_empty() {
        narrative.push_str(&format!("今日 {} 条信号达到高重要性（≥0.7）：\n", important.len()));
        for s in &important {
            narrative.push_str(&format!("  - [{}] {} (imp={:.2})\n", s.domain, s.why, s.importance));
        }
    }

    // 判断摘要
    if !theses.is_empty() {
        narrative.push_str(&format!("\n活跃判断: {} 条\n", theses.len()));
        for t in theses.iter().take(3) {
            narrative.push_str(&format!("  - [{}] conf={:.2}, status={:?}\n", t.claim, t.confidence, t.status));
        }
    }

    // 决策摘要
    if !decisions.is_empty() {
        narrative.push_str(&format!("\n今日决策: {} 条\n", decisions.len()));
        for d in decisions {
            narrative.push_str(&format!("  - {:?}: {} (conf={:.2})\n", d.action, d.thesis_id, d.confidence));
        }
    }

    narrative
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_signal(domain: &str, importance: f64) -> contract::Signal {
        contract::Signal {
            id: "sig".into(),
            observation_id: "obs".into(),
            importance,
            domain: domain.into(),
            category: contract::SignalCategory::ContextUpdate,
            why: format!("Signal about {}", domain),
        }
    }

    fn sample_thesis() -> contract::Thesis {
        contract::Thesis {
            id: "thesis_001".into(),
            claim: "AI Infrastructure".into(),
            confidence: 0.65,
            evidence: vec![],
            status: contract::ThesisStatus::Active,
            falsification_conditions: vec![],
            time_horizon: "12_months".into(),
            theme: None,
            belief_statement: None,
        }
    }

    #[test]
    fn test_synthesize_empty() {
        let result = synthesize(&[], &[], &[]);
        assert_eq!(result.signal_count, 0);
        assert!(!result.headline.is_empty());
    }

    #[test]
    fn test_synthesize_with_signals() {
        let signals = vec![sample_signal("AI", 0.8)];
        let result = synthesize(&signals, &[], &[]);
        assert_eq!(result.signal_count, 1);
        assert!(result.headline.contains("AI"));
    }

    #[test]
    fn test_synthesize_with_decisions() {
        let signals = vec![sample_signal("AI", 0.85)];
        let decisions = vec![contract::Decision {
            id: "dec".into(),
            thesis_id: "t1".into(),
            action: contract::DecisionType::Invest,
            confidence: 0.7,
            horizon: contract::DecisionHorizon::Days90,
            reasoning: "test".into(),
            made_at: "2026-07-12".into(),
            rule_passed: true,
            requires_review: false,
            review_reason: None,
        }];
        let theses = vec![sample_thesis()];
        let result = synthesize(&signals, &theses, &decisions);
        assert_eq!(result.decision_count, 1);
        assert!(result.narrative.contains("Invest"));
    }
}


