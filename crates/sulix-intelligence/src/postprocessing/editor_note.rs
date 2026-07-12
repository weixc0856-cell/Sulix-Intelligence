//! Editor Note — 个人影响分析（幕僚长）
//!
//! 分析今日信息对用户个人决策的影响。
//! 当前返回空（QuestionEngine 未连线），保留接口。

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
/// 当前返回空列表（QuestionEngine 未连线）。
/// 若未来启用 QuestionEngine，可恢复为完整逻辑。
pub fn analyze_personal_impact(
    _theses: &[contract::Thesis],
    _decisions: &[contract::Decision],
) -> Vec<EditorNote> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_personal_impact_empty() {
        let notes = analyze_personal_impact(&[], &[]);
        assert!(notes.is_empty());
    }
}


