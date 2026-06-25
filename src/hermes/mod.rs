//! Change Detection — 检测今日信号与历史 chronicle 的冲突/强化关系
//!
//! 两个版本：
//!   - `detect_changes_rule`: 规则版，基于 topic 名称匹配 + 蓝军 adverse 输出
//!   - `detect_changes_llm`: LLM 语义版，对比分析主题与历史 chronicle 条目

mod conflicts;
mod detector;
mod trends;

pub use conflicts::apply_conflicts;
pub use conflicts::discover_theses;
pub use detector::detect_changes_llm;
pub use detector::detect_changes_rule;
pub use trends::analyze_trends;

use serde::{Deserialize, Serialize};

/// 冲突条目
#[derive(Debug, Clone, Serialize)]
pub struct ConflictEntry {
    pub topic: String,
    pub today_signal: String,
    pub prior_belief: String,
}

/// Change Summary 输出
#[derive(Debug, Clone, Serialize)]
pub struct ChangeSummary {
    pub conflicts: Vec<ConflictEntry>,
    pub reinforced: Vec<String>,
    pub new_signals: Vec<String>,
    pub no_change_count: usize,
}

/// 语义关系枚举（LLM Change Detection 使用）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRelation {
    /// 冲突：今日信号推翻或篡改了历史 Belief
    Conflict,
    /// 强化：今日信号是旧事实的深化或因果传导
    Reinforce,
    /// 无关：不同的技术栈或实体
    Irrelevant,
}

/// LLM Change Detection 输出条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDetectionEntry {
    pub topic: String,
    pub relation: SemanticRelation,
    pub justification: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clusterer::{AdverseScenario, ThemeAnalysis};

    #[test]
    fn test_semantic_relation_deserialization() {
        let json = r#"{"topic": "AI Coding", "relation": "conflict", "justification": "test"}"#;
        let entry: ChangeDetectionEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.relation, SemanticRelation::Conflict);

        let json = r#"{"topic": "AI Coding", "relation": "reinforce", "justification": "test"}"#;
        let entry: ChangeDetectionEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.relation, SemanticRelation::Reinforce);

        let json = r#"{"topic": "AI Coding", "relation": "irrelevant", "justification": "test"}"#;
        let entry: ChangeDetectionEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.relation, SemanticRelation::Irrelevant);
    }

    #[test]
    fn test_detect_changes_rule_cold_start() {
        let result = detect_changes_rule(&[], &[]);
        assert!(result.new_signals.is_empty());
        assert_eq!(result.no_change_count, 0);
    }

    #[test]
    fn test_detect_changes_rule_with_data() {
        use crate::archive::ChronicleEntry;
        let entries = vec![ChronicleEntry {
            date: "2026-06-22".into(),
            topic: "AI".into(),
            headline: "test".into(),
            entities: vec![],
            signal_strength: 7,
            language: "en".into(),
        }];
        let analyses = vec![ThemeAnalysis {
            theme_id: "t1".into(),
            theme_title: "AI".into(),
            bluf: "test".into(),
            impact: "".into(),
            geopolitical_fact: "".into(),
            supply_chain_impact: "".into(),
            analysis_paragraph: "".into(),
            evidence_level: "".into(),
            signal_strength: 7,
            fact_base: vec![],
            connections: vec![],
            source_urls: vec![],
            assumptions: vec![],
            adverse: Some(AdverseScenario {
                scenario: "冲突".into(),
                early_warning: "".into(),
                severity: "high".into(),
            }),
            next_tests: vec![],
            open_questions: vec![],
            chains: vec![],
            what_to_do: String::new(),
            what_to_watch: String::new(),
        }];
        let result = detect_changes_rule(&entries, &analyses);
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].topic, "AI");
    }

    #[test]
    fn test_detect_changes_rule_fallback_no_adverse() {
        use crate::archive::ChronicleEntry;
        let entries = vec![ChronicleEntry {
            date: "2026-06-22".into(),
            topic: "AI".into(),
            headline: "test".into(),
            entities: vec![],
            signal_strength: 7,
            language: "en".into(),
        }];
        let analyses = vec![ThemeAnalysis {
            theme_id: "t1".into(),
            theme_title: "AI".into(),
            bluf: "reinforced".into(),
            impact: "".into(),
            geopolitical_fact: "".into(),
            supply_chain_impact: "".into(),
            analysis_paragraph: "".into(),
            evidence_level: "".into(),
            signal_strength: 7,
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
        }];
        let result = detect_changes_rule(&entries, &analyses);
        assert_eq!(result.reinforced.len(), 1);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_detect_changes_llm_json_parsing_fallback() {
        // 模拟 LLM 输出：前后有多余文字
        let raw_with_noise = r#"Here is my analysis:
[
  {"topic": "Test", "relation": "conflict", "justification": "Direct contradiction on CapEx assumptions"}
]
That's my final answer."#;

        let clean = raw_with_noise
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        // 提取 JSON 数组段落
        if let Some(arr_start) = clean.find('[') {
            if let Some(arr_end) = clean.rfind(']') {
                let slice = &clean[arr_start..=arr_end];
                let entries: Vec<ChangeDetectionEntry> = serde_json::from_str(slice).unwrap();
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].topic, "Test");
                assert_eq!(entries[0].relation, SemanticRelation::Conflict);
                return;
            }
        }
        panic!("JSON array extraction failed");
    }
}
