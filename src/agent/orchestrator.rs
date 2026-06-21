//! Orchestrator (⚖️ 仲裁者)
//!
//! 接收红军 + 蓝军输出 → 仲裁裁决 → 合并为最终分析结果。
//! 纯逻辑判断，不调用 LLM。
//!
//! 仲裁原则：
//! - 不追求"正确"，追求清晰呈现两派逻辑
//! - 如果蓝军完胜（L4/L5）→ 保留红军逻辑供参考
//! - 如果红蓝一致（L1/L2）→ 可以采纳
//! - 双方各有依据 → 维持方向，关注后续

use anyhow::Result;

use crate::llm::{AnalyzedArticle, VerticalAnalysis};

use super::synthesis::SynthesisOutput;
use super::verification::VerificationOutput;

/// 仲裁后的单个 vertical 结果
pub struct ArbitrationResult {
    pub category: String,
    pub analysis: VerticalAnalysis,
    pub verdict: String,
    pub red_summary: String,
    pub blue_summary: String,
}

/// 合并红蓝输出 + 仲裁裁决
pub fn arbitrate(
    synthesis: Vec<SynthesisOutput>,
    verification: Vec<VerificationOutput>,
) -> Result<Vec<ArbitrationResult>> {
    use std::collections::HashMap;

    // 构建蓝军反驳索引: id → Rebuttal（已带 title fallback 做 key）
    let mut blue_by_id: HashMap<&str, &super::verification::Rebuttal> = HashMap::new();
    let mut blue_by_title: HashMap<&str, &super::verification::Rebuttal> = HashMap::new();
    for v in &verification {
        for r in &v.rebuttals {
            if !r.id.is_empty() {
                blue_by_id.insert(r.id.as_str(), r);
            }
            blue_by_title.insert(r.title.as_str(), r);
        }
    }

    let mut results = Vec::new();

    for sv in synthesis {
        let mut articles = Vec::new();
        let mut red_points: Vec<&str> = Vec::new();
        let mut blue_points: Vec<String> = Vec::new();

        for narrative in &sv.narratives {
            // 优先按 id 匹配，fallback 到 title
            let rebuttal = blue_by_id
                .get(narrative.id.as_str())
                .or_else(|| blue_by_title.get(narrative.title.as_str()));

            let (evidence_level, _blue_comment) = match rebuttal {
                Some(r) => {
                    blue_points.push(format!(
                        "证据等级: {} | 反驳: {}",
                        r.evidence_level, r.counter_narrative
                    ));
                    (
                        Some(r.evidence_level.clone()),
                        Some(r.counter_narrative.clone()),
                    )
                }
                None => {
                    blue_points.push("蓝军未提出反驳".into());
                    (None, None)
                }
            };

            red_points.push(&narrative.narrative);

            articles.push(AnalyzedArticle {
                title: narrative.title.clone(),
                url: String::new(),
                importance: narrative.signal_strength,
                relevance: narrative.relevance.clone(),
                time_horizon: narrative.time_horizon.clone(),
                action: narrative.action.clone(),
                confidence: evidence_level
                    .as_deref()
                    .unwrap_or(&narrative.confidence)
                    .into(),
                judgment: narrative.narrative.clone(),
            });
        }

        // 仲裁结论逻辑
        let verdict = if blue_points
            .iter()
            .any(|p| p.contains("L4") || p.contains("L5"))
        {
            format!(
                "⚠️ 蓝军提出证据等级警告(L4/L5)。保留红军逻辑供参考，建议降低权重。\n---\n🔴 红军: {}\n🔵 蓝军: {}",
                red_points.join("; "),
                blue_points.join("; ")
            )
        } else if blue_points
            .iter()
            .any(|p| p.contains("L1") || p.contains("L2"))
        {
            format!(
                "✅ 蓝军确认证据等级较高(L1/L2)。双方基本一致，可以采纳。\n---\n🔴 红军: {}\n🔵 蓝军: {}",
                red_points.join("; "),
                blue_points.join("; ")
            )
        } else {
            format!(
                "📌 各有依据。维持方向，关注后续发展。\n---\n🔴 红军: {}\n🔵 蓝军: {}",
                red_points.join("; "),
                blue_points.join("; ")
            )
        };

        results.push(ArbitrationResult {
            category: sv.category.clone(),
            analysis: VerticalAnalysis {
                category: sv.category.clone(),
                articles,
            },
            verdict,
            red_summary: red_points.join("\n"),
            blue_summary: blue_points.join("\n"),
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::synthesis::{Narrative, SynthesisOutput};
    use crate::agent::verification::{Rebuttal, VerificationOutput};

    fn mock_synthesis(category: &str, titles: &[&str]) -> SynthesisOutput {
        SynthesisOutput {
            category: category.into(),
            narratives: titles
                .iter()
                .map(|t| Narrative {
                    id: format!("id-{}", t),
                    title: t.to_string(),
                    narrative: format!("乐观分析: {}", t),
                    reasoning: format!("推演: {}", t),
                    signal_strength: 7,
                    relevance: "高".into(),
                    time_horizon: "短期".into(),
                    action: "研究".into(),
                    confidence: "中".into(),
                })
                .collect(),
        }
    }

    fn mock_verification(category: &str, titles: &[(&str, &str)]) -> VerificationOutput {
        VerificationOutput {
            category: category.into(),
            rebuttals: titles
                .iter()
                .map(|(t, level)| Rebuttal {
                    id: format!("id-{}", t),
                    title: t.to_string(),
                    counter_narrative: format!("反驳: {}", t),
                    evidence_level: level.to_string(),
                    refutation_strength: 8,
                    ai_myth_flags: vec![],
                })
                .collect(),
        }
    }

    #[test]
    fn test_arbitrate_l4_warning() {
        let s = mock_synthesis("AI", &["Article A"]);
        let v = mock_verification("AI", &[("Article A", "L4")]);
        let result = arbitrate(vec![s], vec![v]).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].verdict.contains("L4"));
    }

    #[test]
    fn test_arbitrate_l1_confirmed() {
        let s = mock_synthesis("AI", &["Article B"]);
        let v = mock_verification("AI", &[("Article B", "L1")]);
        let result = arbitrate(vec![s], vec![v]).unwrap();
        assert!(result[0].verdict.contains("L1"));
        assert!(result[0].verdict.contains("采纳"));
    }

    #[test]
    fn test_arbitrate_neutral() {
        let s = mock_synthesis("AI", &["Article C"]);
        let v = mock_verification("AI", &[("Article C", "L3")]);
        let result = arbitrate(vec![s], vec![v]).unwrap();
        assert!(result[0].verdict.contains("各有依据"));
    }

    #[test]
    fn test_arbitrate_empty() {
        let result = arbitrate(vec![], vec![]).unwrap();
        assert!(result.is_empty());
    }
}
