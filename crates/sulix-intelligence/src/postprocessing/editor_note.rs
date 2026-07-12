//! Editor Note — 个人影响分析（幕僚长）
//!
//! 分析今日管线输出对用户个人决策的影响。
//! 规则驱动（非 LLM），确保快速、可预测。

use sulix_contract as contract;

/// 个人影响分析结果
#[derive(Debug, Clone)]
pub struct EditorNote {
    /// 关联的 Thesis ID
    pub thesis_id: String,
    /// 影响类型: "reinforces" | "challenges"
    pub impact_type: String,
    /// 影响描述
    pub description: String,
    /// 影响程度（-5 ~ +5）
    pub magnitude: i8,
}

/// 分析今日信息对用户个人决策的影响
///
/// 规则:
/// - 置信度 >= 0.8: 过度自信警告
/// - 置信度 <= 0.2: 低确信度提示
/// - Invalidated thesis: 证伪影响
/// - Exit 决策: 退出影响
pub fn analyze_personal_impact(
    theses: &[contract::Thesis],
    decisions: &[contract::Decision],
) -> Vec<EditorNote> {
    let mut notes = Vec::new();

    for thesis in theses {
        if thesis.confidence >= 0.8 {
            notes.push(EditorNote {
                thesis_id: thesis.id.clone(),
                impact_type: "challenges".into(),
                description: format!(
                    "判断 '{:.60}' 置信度极高({:.0}%)，请审视是否有反证被忽略",
                    thesis.claim, thesis.confidence * 100.0
                ),
                magnitude: -2,
            });
        }
        if thesis.confidence <= 0.2 {
            notes.push(EditorNote {
                thesis_id: thesis.id.clone(),
                impact_type: "challenges".into(),
                description: format!(
                    "判断 '{:.60}' 置信度极低({:.0}%)，考虑是否应丢弃或补充证据",
                    thesis.claim, thesis.confidence * 100.0
                ),
                magnitude: -1,
            });
        }
        if matches!(thesis.status, contract::ThesisStatus::Invalidated) {
            notes.push(EditorNote {
                thesis_id: thesis.id.clone(),
                impact_type: "challenges".into(),
                description: format!(
                    "判断 '{:.60}' 已被证伪，需立即调整相关决策方向",
                    thesis.claim
                ),
                magnitude: -5,
            });
        }
    }

    for decision in decisions {
        if matches!(decision.action, contract::DecisionType::Exit) {
            notes.push(EditorNote {
                thesis_id: decision.thesis_id.clone(),
                impact_type: "reinforces".into(),
                description: format!(
                    "退出决策: {:?} — 资源配置需重新评估",
                    decision.action
                ),
                magnitude: 4,
            });
        }
    }

    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_personal_impact_empty() {
        let notes = analyze_personal_impact(&[], &[]);
        assert!(notes.is_empty());
    }

    #[test]
    fn test_high_confidence_triggers_challenge() {
        let thesis = contract::Thesis {
            id: "t1".into(), claim: "AI will transform everything".into(), confidence: 0.85,
            evidence: vec![], status: contract::ThesisStatus::Active,
            falsification_conditions: vec![], time_horizon: "12_months".into(),
            theme: None, belief_statement: None,
        };
        let notes = analyze_personal_impact(&[thesis], &[]);
        assert!(!notes.is_empty());
        assert_eq!(notes[0].impact_type, "challenges");
    }

    #[test]
    fn test_invalidated_triggers_negative_impact() {
        let thesis = contract::Thesis {
            id: "t2".into(), claim: "Old prediction".into(), confidence: 0.1,
            evidence: vec![], status: contract::ThesisStatus::Invalidated,
            falsification_conditions: vec![], time_horizon: "30_days".into(),
            theme: None, belief_statement: None,
        };
        let notes = analyze_personal_impact(&[thesis], &[]);
        assert!(!notes.is_empty());
        assert!(notes.iter().any(|n| n.magnitude <= -4));
    }

    #[test]
    fn test_exit_decision_triggers_reinforce() {
        let decision = contract::Decision {
            id: "d1".into(), thesis_id: "t1".into(),
            action: contract::DecisionType::Exit, confidence: 0.9,
            horizon: contract::DecisionHorizon::Immediate, reasoning: "".into(),
            made_at: "2026-07-12".into(), rule_passed: true,
            requires_review: false, review_reason: None,
        };
        let notes = analyze_personal_impact(&[], &[decision]);
        assert!(!notes.is_empty());
        assert_eq!(notes[0].impact_type, "reinforces");
    }
}


