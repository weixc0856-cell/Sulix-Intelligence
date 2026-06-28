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

// EditorNote 已迁移至 crate::domain::editor_note
// 保留 re-export 以保持向后兼容
pub use crate::domain::editor_note::EditorNote;

use crate::domain::theme::ThemeAnalysis;
use crate::domain::thesis::Thesis;

/// 分析今日信息对用户个人决策的影响
///
/// 当前返回空列表（QuestionEngine 未连线，question_matches 始终为空）。
/// 若未来启用 QuestionEngine，可恢复为此函数的完整逻辑。
///
/// 输入：
///   - analyses: 今日主题分析
///   - theses: MemoryEngine 中的已有判断
///
/// 输出：
///   - Vec<EditorNote>: 每条问题一条（或零条），描述今日信息如何改变了用户的判断
pub fn analyze_personal_impact(
    analyses: &[ThemeAnalysis],
    theses: &[Thesis],
) -> Vec<EditorNote> {
    // QuestionEngine 未连线，always returns empty
    let _ = analyses;
    let _ = theses;
    Vec::new()
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

    #[test]
    fn test_empty_no_notes() {
        let notes = analyze_personal_impact(&[], &[]);
        assert!(notes.is_empty());
    }

    #[test]
    fn test_with_analysis_returns_empty() {
        let analysis = make_analysis("AI Commoditization", 8);
        // QuestionEngine not wired — always returns empty
        let notes = analyze_personal_impact(&[analysis], &[]);
        assert!(notes.is_empty());
    }
}
