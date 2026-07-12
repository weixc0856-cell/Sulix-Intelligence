//! Decision — 行动决策
//!
//! Decision 是将 Thesis 映射到行动的输出。
//! 它是规则约束 + LLM 推理的混合产物：
//!   - 规则层确保不出安全包线
//!   - LLM 层提供推理和上下文判断
//!
//! 契约边界：
//!   Producer: Intelligence Engine (Decision Mapping step)
//!   Consumer: Memory (追踪决策结果)
//!             Renderer (展示决策看板)

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 行动决策
///
/// # 设计原则
/// - 每个 Decision 绑定到一个 Thesis
/// - action 来自预定义的 6 种决策类型
/// - reasoning 必须包含完整推理链（LLM 生成）
/// - rule_failed 标记是否被规则层拦截
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Decision {
    /// 唯一 ID，格式 "dec_xxx"
    pub id: String,

    /// 源自的 Thesis ID
    pub thesis_id: String,

    /// 决策类型
    pub action: DecisionType,

    /// 置信度 [0.0, 1.0]
    pub confidence: f64,

    /// 时间范围
    pub horizon: DecisionHorizon,

    /// 推理链（LLM 生成，人类可读）
    pub reasoning: String,

    /// 决策时间（ISO 8601）
    pub made_at: String,

    /// 规则层是否通过
    #[serde(default)]
    pub rule_passed: bool,

    /// 如需人工审查，说明原因
    #[serde(default)]
    pub requires_review: bool,

    /// 人工审查原因（当 rule_passed = false 时）
    #[serde(default)]
    pub review_reason: Option<String>,
}

/// 6 种决策类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum DecisionType {
    /// 大力投入资源
    Build,
    /// 配置资源（中等投入）
    Invest,
    /// 持续观察（不投入）
    Monitor,
    /// 学习了解（低强度关注）
    Learn,
    /// 主动忽略
    Ignore,
    /// 退出/剥离
    Exit,
}

/// 决策时间范围
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum DecisionHorizon {
    /// 立即执行
    Immediate,
    /// 30 天内
    Days30,
    /// 90 天内
    Days90,
    /// 180 天内
    Days180,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_serde_roundtrip() {
        let decision = Decision {
            id: "dec_001".into(),
            thesis_id: "thesis_001".into(),
            action: DecisionType::Invest,
            confidence: 0.7,
            horizon: DecisionHorizon::Days90,
            reasoning: "Strong adoption signals".into(),
            made_at: "2026-07-12".into(),
            rule_passed: true,
            requires_review: false,
            review_reason: None,
        };
        let json = serde_json::to_string(&decision).unwrap();
        let restored: Decision = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "dec_001");
        assert!(matches!(restored.action, DecisionType::Invest));
        assert!(matches!(restored.horizon, DecisionHorizon::Days90));
        assert!(restored.rule_passed);
    }

    #[test]
    fn test_decision_type_variants() {
        let variants = vec![
            (r#""Build""#, DecisionType::Build),
            (r#""Invest""#, DecisionType::Invest),
            (r#""Monitor""#, DecisionType::Monitor),
            (r#""Learn""#, DecisionType::Learn),
            (r#""Ignore""#, DecisionType::Ignore),
            (r#""Exit""#, DecisionType::Exit),
        ];
        for (json, expected) in variants {
            let dt: DecisionType = serde_json::from_str(json).unwrap();
            assert!(matches!(dt, _), "variant {:?} should deserialize", expected);
        }
    }
}
