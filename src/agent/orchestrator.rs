//! Orchestrator (⚖️ 仲裁者)
//!
//! 接收红军 + 蓝军输出 → 逐条仲裁裁决 → 合并为最终分析结果。
//! 纯逻辑判断，不调用 LLM。

use anyhow::Result;

use crate::llm::{AnalyzedArticle, VerticalAnalysis};

use super::synthesis::SynthesisOutput;
use super::verification::VerificationOutput;

/// 仲裁后的单个 vertical 结果
pub struct ArbitrationResult {
    #[allow(dead_code)]
    pub category: String,
    pub analysis: VerticalAnalysis,
    pub verdict: String,
}

/// 合并红蓝输出 + 逐条仲裁裁决
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
        let mut l4_l5_count = 0usize;
        let mut l1_l2_count = 0usize;

        for narrative in &sv.narratives {
            // 优先按 id 匹配，fallback 到 title
            let rebuttal = blue_by_id
                .get(narrative.id.as_str())
                .or_else(|| blue_by_title.get(narrative.title.as_str()));

            let (rebuttal_text, evidence_level_str, arbitration_text) = match rebuttal {
                Some(r) => {
                    let level = &r.evidence_level;
                    let arb = if level.contains('4') || level.contains('5') {
                        l4_l5_count += 1;
                        format!("⚠️ 蓝军评 {}，建议降低权重，注意假掩护风险", level)
                    } else if level.contains('1') || level.contains('2') {
                        l1_l2_count += 1;
                        format!("✅ 蓝军评 {}，证据等级较高，可以采纳", level)
                    } else {
                        format!("📌 蓝军评 {}，各有依据", level)
                    };
                    (r.counter_narrative.clone(), level.clone(), arb)
                }
                None => (
                    String::new(),
                    "未分析".into(),
                    "🔵 蓝军未就此条提出反驳".into(),
                ),
            };

            let confidence = if evidence_level_str != "未分析" && evidence_level_str != "未匹配"
            {
                evidence_level_str.clone()
            } else {
                narrative.confidence.clone()
            };

            articles.push(AnalyzedArticle {
                title: narrative.title.clone(),
                url: String::new(),
                importance: narrative.signal_strength,
                relevance: narrative.relevance.clone(),
                time_horizon: narrative.time_horizon.clone(),
                action: narrative.action.clone(),
                confidence,
                judgment: narrative.narrative.clone(),
                blue_rebuttal: rebuttal_text,
                arbitration: arbitration_text,
            });
        }

        let verdict = if l4_l5_count > 0 {
            format!(
                "⚠️ 蓝军提出 {} 条证据等级警告(L4/L5)，建议降低权重，注意假掩护风险。",
                l4_l5_count
            )
        } else if l1_l2_count > 0 {
            format!(
                "✅ 蓝军确认 {} 条证据等级较高(L1/L2)，双方基本一致，可以采纳。",
                l1_l2_count
            )
        } else {
            "📌 蓝军未形成明确评级。维持方向，关注后续发展。".into()
        };

        results.push(ArbitrationResult {
            category: sv.category.clone(),
            analysis: VerticalAnalysis {
                category: sv.category.clone(),
                articles,
            },
            verdict,
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
        // Per-article arbitration should mention L4
        assert!(result[0].analysis.articles[0].arbitration.contains("L4"));
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
        assert!(result[0].verdict.contains("未形成明确评级"));
    }

    #[test]
    fn test_arbitrate_empty() {
        let result = arbitrate(vec![], vec![]).unwrap();
        assert!(result.is_empty());
    }
}
