//! Stability Layer — Decision 发布前最后一道关卡
//!
//! 按照 §6 设计，所有 Agent 输出在发布前通过 Stability Layer。
//!
//! 职责：
//!   1. Decision Smoothing — 防止小波动导致每日翻牌
//!   2. Minimum Persistence — Decision 至少连续 2 天证据一致才允许切换
//!   3. Confidence Hysteresis — Confidence 变化 <15% 不改变 Recommendation
//!
//! # Integration
//! 在 `publish_infer()` 中 `map_theses_to_decisions()` 之后调用 `stability_gate()`。
//!
//! # Rules
//! - Same decision type as yesterday → keep, stability = "stable"
//! - Different decision, but only 1 day of evidence → suppress switch, keep yesterday's
//! - Different decision, 2+ consecutive days of new evidence → allow switch, stability = "volatile"
//! - Confidence delta < 0.15 → don't escalate to different recommendation tier

use crate::domain::action::{DecisionStability, DecisionType};
use crate::engine::decision::ThesisDecision;

/// Minimum number of consecutive days the same decision type must be observed
/// before a switch is allowed.
const MIN_PERSISTENCE_DAYS: usize = 2;

/// Confidence change below this threshold does not trigger a decision review.
const CONFIDENCE_HYSTERESIS: f64 = 0.15;

/// 将一组今日决策与历史决策进行比较，输出平滑后的决策。
///
/// `today`: 今日 map_theses_to_decisions() 的输出
/// `history`: 历史决策记录（thesis_id → last known decision_type）
/// `consecutive_days`: thesis_id → 连续多少个 run 输出相同 decision
///
/// Returns: 平滑后的决策列表，其中 decision_type 和 stability 已修正。
pub fn stability_gate(
    today: Vec<ThesisDecision>,
    history: &std::collections::HashMap<String, (DecisionType, f64)>,
    consecutive_days: &std::collections::HashMap<String, usize>,
) -> Vec<ThesisDecision> {
    let mut smoothed = Vec::with_capacity(today.len());

    for mut td in today {
        let thesis_id = &td.thesis_id;

        if let Some((last_type, last_confidence)) = history.get(thesis_id) {
            let days = consecutive_days.get(thesis_id).copied().unwrap_or(1);

            // Rule 1: Same decision as yesterday → stable
            if &td.decision_type == last_type {
                td.stability = if days >= 3 {
                    DecisionStability::Final
                } else {
                    DecisionStability::Stable
                };
            }
            // Rule 2: Different decision, check persistence
            else {
                // Rule 2a: Confidence hysteresis — small changes don't flip recommendation
                let confidence_delta = (td.confidence - last_confidence).abs();
                if confidence_delta < CONFIDENCE_HYSTERESIS
                    && td.decision_type != *last_type
                {
                    // Suppress the switch: keep yesterday's decision
                    log::info!(
                        "🛡️ Stability Gate: {} — confidence hysteresis (Δ{:.2}<{:.2}), keeping {:?}",
                        thesis_id,
                        confidence_delta,
                        CONFIDENCE_HYSTERESIS,
                        last_type
                    );
                    td.decision_type = last_type.clone();
                    td.confidence = *last_confidence;
                    td.stability = DecisionStability::Stable;
                }
                // Rule 2b: Not enough consecutive days for new decision
                else if days < MIN_PERSISTENCE_DAYS {
                    log::info!(
                        "🛡️ Stability Gate: {} — insufficient persistence ({}d < {}d), keeping {:?} → {:?}",
                        thesis_id,
                        days,
                        MIN_PERSISTENCE_DAYS,
                        td.decision_type,
                        last_type
                    );
                    td.decision_type = last_type.clone();
                    td.confidence = *last_confidence;
                    td.stability = DecisionStability::Volatile;
                }
                // Rule 3: Sufficient evidence, allow switch
                else {
                    log::info!(
                        "🛡️ Stability Gate: {} — gate cleared ({:?} → {:?}, {}d persistence)",
                        thesis_id,
                        last_type,
                        td.decision_type,
                        days
                    );
                    td.stability = DecisionStability::Stable;
                }
            }
        } else {
            // New thesis — first decision, inherently volatile
            td.stability = DecisionStability::Volatile;
        }

        smoothed.push(td);
    }

    smoothed
}

/// Extract history map from theses in memory engine.
///
/// For each thesis, returns (last_decision_type, last_confidence) if available.
pub fn build_decision_history_map(
    memory: &crate::engine::memory::MemoryEngine,
) -> std::collections::HashMap<String, (DecisionType, f64)> {
    let mut map = std::collections::HashMap::new();
    for thesis in memory.theses() {
        if let Some(last_snapshot) = thesis.decision_history.last() {
            let dt = match last_snapshot.decision_type.as_str() {
                "build" => DecisionType::Build,
                "invest" => DecisionType::Invest,
                "monitor" => DecisionType::Monitor,
                "learn" => DecisionType::Learn,
                "ignore" => DecisionType::Ignore,
                "exit" => DecisionType::Exit,
                _ => continue,
            };
            map.insert(thesis.id.clone(), (dt, last_snapshot.confidence));
        }
    }
    map
}

/// Build a consecutive days map from theses in memory engine.
///
/// Counts how many consecutive days the same decision type has been observed.
pub fn build_consecutive_days_map(
    memory: &crate::engine::memory::MemoryEngine,
) -> std::collections::HashMap<String, usize> {
    let mut map = std::collections::HashMap::new();
    for thesis in memory.theses() {
        if thesis.decision_history.is_empty() {
            continue;
        }
        let last_type = &thesis.decision_history.last().unwrap().decision_type;
        // Count consecutive same-type entries from the end
        let consecutive = thesis.decision_history.iter().rev()
            .take_while(|s| &s.decision_type == last_type)
            .count();
        map.insert(thesis.id.clone(), consecutive);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_decision(thesis_id: &str, dt: DecisionType, confidence: f64) -> ThesisDecision {
        use crate::domain::action::DecisionHorizon;
        ThesisDecision {
            thesis_id: thesis_id.to_string(),
            thesis_title: "Test Thesis".to_string(),
            decision_type: dt,
            confidence,
            rationale: "test".to_string(),
            horizon: DecisionHorizon::NinetyDays,
            priority: 5,
            stability: DecisionStability::Stable,
        }
    }

    #[test]
    fn test_new_thesis_is_volatile() {
        let today = vec![make_decision("t1", DecisionType::Monitor, 0.6)];
        let history = std::collections::HashMap::new();
        let consecutive = std::collections::HashMap::new();

        let result = stability_gate(today, &history, &consecutive);
        assert_eq!(result[0].stability, DecisionStability::Volatile);
        // New thesis should pass through with its original decision
        assert_eq!(result[0].decision_type, DecisionType::Monitor);
    }

    #[test]
    fn test_same_decision_stays_stable() {
        let today = vec![make_decision("t1", DecisionType::Build, 0.8)];
        let mut history = std::collections::HashMap::new();
        history.insert("t1".to_string(), (DecisionType::Build, 0.75));
        let mut consecutive = std::collections::HashMap::new();
        consecutive.insert("t1".to_string(), 4);

        let result = stability_gate(today, &history, &consecutive);
        assert_eq!(result[0].decision_type, DecisionType::Build);
        assert_eq!(result[0].stability, DecisionStability::Final); // 4 days > 3
    }

    #[test]
    fn test_confidence_hysteresis_prevents_switch() {
        let today = vec![make_decision("t1", DecisionType::Exit, 0.82)];
        let mut history = std::collections::HashMap::new();
        history.insert("t1".to_string(), (DecisionType::Build, 0.80));
        let mut consecutive = std::collections::HashMap::new();
        consecutive.insert("t1".to_string(), 5);

        let result = stability_gate(today, &history, &consecutive);
        // Confidence only changed 2% → should keep Build
        assert_eq!(result[0].decision_type, DecisionType::Build);
        assert_eq!(result[0].confidence, 0.80); // kept yesterday's confidence
    }

    #[test]
    fn test_insufficient_persistence_prevents_switch() {
        let today = vec![make_decision("t1", DecisionType::Exit, 0.4)];
        let mut history = std::collections::HashMap::new();
        history.insert("t1".to_string(), (DecisionType::Build, 0.80));
        let mut consecutive = std::collections::HashMap::new();
        consecutive.insert("t1".to_string(), 1); // only 1 day

        let result = stability_gate(today, &history, &consecutive);
        // Not enough persistence → keep Build
        assert_eq!(result[0].decision_type, DecisionType::Build);
        assert_eq!(result[0].stability, DecisionStability::Volatile);
    }

    #[test]
    fn test_sufficient_evidence_allows_switch() {
        let today = vec![make_decision("t1", DecisionType::Exit, 0.3)];
        let mut history = std::collections::HashMap::new();
        history.insert("t1".to_string(), (DecisionType::Build, 0.80));
        let mut consecutive = std::collections::HashMap::new();
        consecutive.insert("t1".to_string(), 3); // 3 days

        let result = stability_gate(today, &history, &consecutive);
        // Enough persistence + confidence change >15% → allow switch
        assert_eq!(result[0].decision_type, DecisionType::Exit);
        assert_eq!(result[0].stability, DecisionStability::Stable);
    }
}