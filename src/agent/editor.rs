//! Editor Agent (幕僚长) — Layer 3: 将分析结果与用户个人决策问题关联
//!
//! 这是"新闻 → 行动"的最后一步。
//! Editor Agent 不生产事实，它只回答一个问题：
//! "今天这些信息，对你最重要的 3-5 个决策问题意味着什么？"
//!
//! 定位：
//!   分析结果 → Editor Agent → "强化了做应用的判断 (+3)"
//!                             → "挑战了模型创业的假设 (-2)"
//!                             → 你每天早上看到的第一段话

use serde::{Deserialize, Serialize};

use crate::domain::theme::ThemeAnalysis;
use crate::domain::evidence::Stance;
use crate::domain::thesis::Thesis;
use crate::question_engine::QuestionMatch;

/// Editor 笔记：一条分析结果对你个人决策的影响
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorNote {
    /// 关联的用户决策问题 ID
    pub question_id: String,
    /// 关联的主题标题
    pub theme_title: String,
    /// 影响描述（人类可读）
    pub impact: String,
    /// 置信度变化 -10 到 +10
    pub confidence_delta: i8,
    /// 建议行动
    pub recommended_action: String,
    /// 该影响的依据
    pub rationale: String,
}

/// 分析今日信息对用户个人决策的影响
///
/// 输入：
///   - question_matches: QuestionEngine 输出的匹配结果
///   - analyses: 今日主题分析
///   - theses: MemoryEngine 中的已有判断
///
/// 输出：
///   - Vec<EditorNote>: 每条问题一条（或零条），描述今日信息如何改变了用户的判断
pub fn analyze_personal_impact(
    question_matches: &[QuestionMatch],
    analyses: &[ThemeAnalysis],
    theses: &[Thesis],
) -> Vec<EditorNote> {
    let mut notes: Vec<EditorNote> = Vec::new();

    // 建立 analysis title → Thesis 的快速查找
    let thesis_by_title: std::collections::HashMap<&str, &Thesis> = theses
        .iter()
        .filter(|t| {
            matches!(
                t.status,
                crate::domain::thesis::ThesisStatus::Active
                    | crate::domain::thesis::ThesisStatus::Strengthening
                    | crate::domain::thesis::ThesisStatus::Weakening
            )
        })
        .map(|t| (t.title.as_str(), t))
        .collect();

    for analysis in analyses.iter() {
        let theme_title = &analysis.theme_title;

        // 检查是否有对应的 Thesis（已有判断）
        let existing_thesis = thesis_by_title.get(theme_title.as_str());

        // 找到与该分析相关的问题匹配
        let analysis_lower = theme_title.to_lowercase();
        let theme_matches: Vec<&QuestionMatch> = question_matches
            .iter()
            .filter(|qm| {
                // 检查问题文本是否与主题相关
                let q_lower = qm.question_text.to_lowercase();
                analysis_lower.contains(&q_lower)
                    || q_lower.contains(&analysis_lower)
                    || qm.relevance >= 5
            })
            .collect();

        if theme_matches.is_empty() {
            continue;
        }

        for qm in theme_matches {
            // 计算置信度变化
            let confidence_delta = if qm.evidence_type == "Support" {
                (qm.relevance as i8 / 3).min(5)
            } else if qm.evidence_type == "Challenge" {
                -(qm.relevance as i8 / 2).min(8)
            } else {
                0
            };

            // 结合已有 Thesis 信息修正置信度变化
            let confidence_delta = if let Some(thesis) = existing_thesis {
                let support_count = thesis
                    .evidences
                    .iter()
                    .filter(|e| e.stance == Stance::Supports)
                    .count();
                let challenge_count = thesis
                    .evidences
                    .iter()
                    .filter(|e| e.stance == Stance::Challenges)
                    .count();
                // 如果已有大量证据，单日变化影响相对小
                if (support_count + challenge_count) > 10 {
                    confidence_delta / 2
                } else {
                    confidence_delta
                }
            } else {
                confidence_delta
            };

            // 确定建议行动
            let recommended_action = if confidence_delta >= 4 {
                "Invest".into()
            } else if confidence_delta >= 2 {
                "Explore".into()
            } else if confidence_delta <= -5 {
                "Exit".into()
            } else {
                "Observe".into() // -4 到 +1: 持有观察
            };

            // 构建人类可读的影响描述
            let impact = if confidence_delta > 0 {
                format!(
                    "强化了\u{2018}{}\u{2019}的判断 (+{})",
                    qm.question_text, confidence_delta
                )
            } else if confidence_delta < 0 {
                format!(
                    "挑战了\u{2018}{}\u{2019}的假设 ({})",
                    qm.question_text, confidence_delta
                )
            } else {
                format!("提供了\u{2018}{}\u{2019}的相关信息", qm.question_text)
            };

            let rationale = if !qm.reasoning.is_empty() {
                qm.reasoning.clone()
            } else {
                format!(
                    "信号强度 {}, 证据类型: {}",
                    analysis.signal_strength, qm.evidence_type
                )
            };

            notes.push(EditorNote {
                question_id: qm.question_id.clone(),
                theme_title: theme_title.clone(),
                impact,
                confidence_delta,
                recommended_action,
                rationale,
            });
        }
    }

    // 按置信度变化绝对值排序（最重要的排最前）
    notes.sort_by_key(|n| -(n.confidence_delta.abs()));

    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_analysis(title: &str, strength: u8) -> ThemeAnalysis {
        ThemeAnalysis {
            theme_id: "t1".into(),
            theme_title: title.into(),
            bluf: "test".into(),
            impact: "test".into(),
            geopolitical_fact: "test".into(),
            supply_chain_impact: "test".into(),
            analysis_paragraph: String::new(),
            evidence_level: "Established-Fact".into(),
            signal_strength: strength,
            fact_base: vec![],
            connections: vec![],
            source_urls: vec![],
            assumptions: vec![],
            adverse: None,
            next_tests: vec![],
            open_questions: vec![],
            chains: vec![],
            what_to_do: String::new(),
            what_to_watch: String::new(),
            falsification_conditions: vec![],
        }
    }

    fn make_match(question_id: &str, text: &str, relevance: u8, etype: &str) -> QuestionMatch {
        QuestionMatch {
            question_id: question_id.into(),
            question_text: text.into(),
            relevance,
            reasoning: format!("test match (rel={})", relevance),
            evidence_type: etype.into(),
        }
    }

    #[test]
    fn test_empty_no_notes() {
        let notes = analyze_personal_impact(&[], &[], &[]);
        assert!(notes.is_empty());
    }

    #[test]
    fn test_support_creates_positive_delta() {
        let analysis = make_analysis("AI Commoditization", 8);
        let matches = vec![make_match("q1", "做应用还是做模型", 8, "Support")];
        let notes = analyze_personal_impact(&matches, &[analysis], &[]);
        assert!(!notes.is_empty());
        assert!(notes[0].confidence_delta > 0);
        assert!(notes[0].impact.contains("强化"));
    }

    #[test]
    fn test_challenge_creates_negative_delta() {
        let analysis = make_analysis("Agent Market", 9);
        let matches = vec![make_match("q2", "Agent", 8, "Challenge")];
        let notes = analyze_personal_impact(&matches, &[analysis], &[]);
        assert!(!notes.is_empty());
        assert!(notes[0].confidence_delta < 0);
        assert!(notes[0].impact.contains("挑战"));
    }

    #[test]
    fn test_no_match_skips() {
        let analysis = make_analysis("Unrelated Topic", 5);
        let notes = analyze_personal_impact(&[], &[analysis], &[]);
        assert!(notes.is_empty());
    }
}
