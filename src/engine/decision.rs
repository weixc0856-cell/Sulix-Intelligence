//! Decision Engine — Thesis → Decision 映射
//!
//! 将 Memory Engine 中的 Thesis 状态映射为可操作的决策建议。
//! 这是确定性映射规则（不是 LLM 调用），确保可审计、可预测。
//!
//! Core mapping:
//!   Proposed   → Learn
//!   Active     → Monitor
//!   Strengthening → Build / Invest
//!   Weakening  → Learn / Monitor
//!   Dormant    → Ignore
//!   Retired (Invalidated) → Exit
//!   Retired (no outcome)  → Ignore

use crate::domain::action::{DecisionHorizon, DecisionStability, DecisionType};
use crate::domain::outcome::OutcomeVerdict;
use crate::domain::thesis::ThesisStatus;
use crate::domain::ThesisDecision;
use crate::engine::memory::MemoryEngine;

/// 将 Memory Engine 中的所有活跃 Thesis 映射为决策建议
pub fn map_theses_to_decisions(memory: &MemoryEngine) -> Vec<ThesisDecision> {
    let mut decisions: Vec<ThesisDecision> = Vec::new();

    for thesis in memory.theses() {
        let decision = map_thesis_to_decision(thesis, memory);
        decisions.push(decision);
    }

    // 按优先级排序（Exit 最优先）
    decisions.sort_by_key(|d| d.priority);
    decisions
}

/// 将单个 Thesis 映射为决策建议（含 Decision Smoothing）
///
/// Decision Smoothing 原则：
/// - EXIT 永远立即生效（风险管理优先）
/// - 其他决策类型：若与历史不同，需连续 2 天出现才允许切换
/// - 连续 3 天相同决策 → Stable；否则 Volatile
fn map_thesis_to_decision(
    thesis: &crate::domain::thesis::Thesis,
    memory: &MemoryEngine,
) -> ThesisDecision {
    let title = &thesis.title;
    let status = &thesis.status;

    // 检查是否有 Invalidated outcome
    let has_invalidated_outcome = memory
        .all_outcomes()
        .iter()
        .any(|o| o.thesis_id == thesis.id && o.verdict == OutcomeVerdict::Invalidated);

    let (raw_type, horizon, rationale) = match status {
        ThesisStatus::Proposed => (
            DecisionType::Learn,
            DecisionHorizon::OneEightyDays,
            format!("新提案 '{}' — 需要更多证据来判断方向", title),
        ),
        ThesisStatus::Active => {
            let support = thesis
                .evidences
                .iter()
                .filter(|e| e.stance == crate::domain::evidence::Stance::Supports)
                .count();
            let challenge = thesis
                .evidences
                .iter()
                .filter(|e| e.stance == crate::domain::evidence::Stance::Challenges)
                .count();
            if support > challenge {
                (
                    DecisionType::Monitor,
                    DecisionHorizon::NinetyDays,
                    format!(
                        "'{}' 支持证据较多 (S:{}/C:{}) — 值得关注，暂不行动",
                        title, support, challenge
                    ),
                )
            } else {
                (
                    DecisionType::Monitor,
                    DecisionHorizon::ThirtyDays,
                    format!(
                        "'{}' 处于平衡状态 (S:{}/C:{}) — 继续监控",
                        title, support, challenge
                    ),
                )
            }
        }
        ThesisStatus::Strengthening => (
            DecisionType::Build,
            DecisionHorizon::NinetyDays,
            format!("'{}' 正在强化 — 连续支持信号，建议投入资源", title),
        ),
        ThesisStatus::Weakening => (
            DecisionType::Learn,
            DecisionHorizon::ThirtyDays,
            format!("'{}' 正在弱化 — 需要更多信息来判断是否调整", title),
        ),
        ThesisStatus::Dormant => (
            DecisionType::Ignore,
            DecisionHorizon::OneEightyDays,
            format!("'{}' 已 30 天无新证据 — 暂时搁置", title),
        ),
        ThesisStatus::Retired => {
            if has_invalidated_outcome {
                (
                    DecisionType::Exit,
                    DecisionHorizon::Immediate,
                    format!("'{}' 已被证伪 — 建议退出该判断", title),
                )
            } else {
                (
                    DecisionType::Ignore,
                    DecisionHorizon::OneEightyDays,
                    format!("'{}' 已自然衰退 — 归档处理", title),
                )
            }
        }
    };

    // Compute confidence from evidence ratio (canonical: engine/memory.rs)
    let confidence = crate::domain::compute_confidence(&thesis.evidences);

    // ── Decision Smoothing ──────────────────────────────────────────────
    // EXIT 永远立即生效，不受 smoothing 影响
    let decision_type = if raw_type == DecisionType::Exit {
        raw_type
    } else {
        let raw_label = raw_type.as_key();
        let history = &thesis.decision_history;
        match history.last() {
            // 没有历史 → 新 thesis，直接用原始决策
            None => raw_type,
            // 与昨天相同 → 继续
            Some(last) if last.decision_type == raw_label => raw_type,
            // 与昨天不同 → 检查是否连续 2 天出现新决策（2-day hysteresis）
            Some(_) => {
                let consistent_new = history
                    .iter()
                    .rev()
                    .take(2)
                    .filter(|s| s.decision_type == raw_label)
                    .count()
                    >= 2;
                if consistent_new {
                    raw_type // 连续 2 天 → 允许切换
                } else {
                    // 只有 1 天 → 抑制翻转，用历史中最近的决策
                    crate::domain::action::DecisionType::from_key(
                        &history.last().unwrap().decision_type,
                    )
                    .unwrap_or(raw_type)
                }
            }
        }
    };
    // ─────────────────────────────────────────────────────────────────

    let priority = decision_type.priority();

    // ── Stability — 结合决策连续性 + outcome history ──────────────────
    let final_label = decision_type.as_key();
    let stability = if decision_type == DecisionType::Exit {
        DecisionStability::Final
    } else {
        // 决策连续性：连续 3 天相同 → Stable
        let consecutive_days = thesis
            .decision_history
            .iter()
            .rev()
            .take_while(|s| s.decision_type == final_label)
            .count();
        if consecutive_days >= 3 {
            DecisionStability::Stable
        } else {
            // 次级：outcome 历史确认也可以 Stable
            let thesis_outcomes: Vec<&crate::domain::outcome::Outcome> = memory
                .all_outcomes()
                .iter()
                .filter(|o| o.thesis_id == thesis.id)
                .collect();
            if !thesis_outcomes.is_empty() {
                let confirmed = thesis_outcomes
                    .iter()
                    .filter(|o| {
                        matches!(
                            o.verdict,
                            OutcomeVerdict::Confirmed | OutcomeVerdict::PartiallyConfirmed
                        )
                    })
                    .count();
                if confirmed as f64 / thesis_outcomes.len() as f64 >= 0.5 {
                    DecisionStability::Stable
                } else {
                    DecisionStability::Volatile
                }
            } else {
                DecisionStability::Volatile
            }
        }
    };
    // ─────────────────────────────────────────────────────────────────

    ThesisDecision {
        thesis_id: thesis.id.clone(),
        thesis_title: thesis.title.clone(),
        decision_type,
        confidence,
        rationale,
        horizon,
        priority,
        stability,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::evidence::{Evidence, Stance};
    use crate::domain::outcome::{Outcome, OutcomeVerdict};
    use crate::domain::thesis::Thesis;

    fn make_thesis(id: &str, title: &str, status: ThesisStatus, evidence_count: usize) -> Thesis {
        Thesis {
            id: id.to_string(),
            title: title.to_string(),
            created: "2026-06-01".to_string(),
            updated: "2026-06-25".to_string(),
            evidences: (0..evidence_count)
                .map(|i| Evidence {
                    date: "2026-06-25".to_string(),
                    title: format!("evidence {}", i),
                    source: "test".to_string(),
                    summary: "test".to_string(),
                    stance: Stance::Supports,
                    signal_strength: 5,
                })
                .collect(),
            assumptions: vec![],
            status,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
            primary_domain: crate::domain::StrategicDomain::default(),
            secondary_domains: vec![],
            lifecycle_events: vec![],
        }
    }

    fn make_memory(theses: Vec<Thesis>, outcomes: Vec<Outcome>) -> MemoryEngine {
        let tmp = std::env::temp_dir().join("test_decision_engine.json");
        let _ = std::fs::remove_file(&tmp);
        let mut mem = MemoryEngine::new(tmp);
        for t in theses {
            mem.test_add_thesis(t);
        }
        for o in outcomes {
            mem.test_add_outcome(o);
        }
        mem
    }

    #[test]
    fn test_strengthening_maps_to_build() {
        let mem = make_memory(
            vec![make_thesis(
                "t1",
                "AI Market",
                ThesisStatus::Strengthening,
                5,
            )],
            vec![],
        );
        let decisions = map_theses_to_decisions(&mem);
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].decision_type, DecisionType::Build);
        assert_eq!(decisions[0].horizon, DecisionHorizon::NinetyDays);
    }

    #[test]
    fn test_retired_with_invalidated_maps_to_exit() {
        let mem = make_memory(
            vec![make_thesis("t1", "Failed Thesis", ThesisStatus::Retired, 3)],
            vec![Outcome {
                id: "o1".into(),
                decision_id: "DEC-001".into(),
                thesis_id: "t1".into(),
                description: "was wrong".into(),
                verdict: OutcomeVerdict::Invalidated,
                impact: crate::domain::outcome::ImpactLevel::High,
                date: "2026-06-25".into(),
                supporting_evidence: vec![],
                expected_signal: String::new(),
                actual_signal: String::new(),
                delta: String::new(),
            }],
        );
        let decisions = map_theses_to_decisions(&mem);
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].decision_type, DecisionType::Exit);
        assert_eq!(decisions[0].horizon, DecisionHorizon::Immediate);
    }

    #[test]
    fn test_dormant_maps_to_ignore() {
        let mem = make_memory(
            vec![make_thesis("t1", "Old Topic", ThesisStatus::Dormant, 0)],
            vec![],
        );
        let decisions = map_theses_to_decisions(&mem);
        assert_eq!(decisions[0].decision_type, DecisionType::Ignore);
    }

    #[test]
    fn test_decision_priority_ordering() {
        let mut mem = make_memory(vec![], vec![]);
        mem.test_add_thesis(make_thesis(
            "t1",
            "Strengthening",
            ThesisStatus::Strengthening,
            5,
        ));
        mem.test_add_thesis(make_thesis("t2", "Dormant", ThesisStatus::Dormant, 0));
        mem.test_add_thesis(make_thesis("t3", "Active", ThesisStatus::Active, 3));
        let decisions = map_theses_to_decisions(&mem);
        // Build (priority 2) should come before Monitor (5) and Ignore (6)
        assert_eq!(decisions[0].decision_type, DecisionType::Build);
        assert_eq!(decisions[1].decision_type, DecisionType::Monitor);
        assert_eq!(decisions[2].decision_type, DecisionType::Ignore);
    }
}
