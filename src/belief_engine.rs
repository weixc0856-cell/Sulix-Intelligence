//! Belief Engine — 追踪信念的建立、挑战和更新
//!
//! 输入: QuestionMatch[] + 已有 BeliefStatement[]
//! 输出: 更新的置信度 + 被挑战的信念列表 + 新证据
//!
//! Code Review 逆向 triage 破局点:
//! 一旦触发 contradicts == true，强制将该信号提升为 Insight 级别，
//! 因为能证伪既有信念的反向黑天鹅信号价值远超 100 个顺向赞同信号。

use serde::{Deserialize, Serialize};

/// 信念声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefStatement {
    pub id: String,
    pub text: String,
    /// 当前置信度 1-10
    pub confidence: u8,
    pub category: String,
    /// 支撑该信念的证据 ID 列表
    pub evidence_ids: Vec<String>,
}

/// 信念更新记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefUpdate {
    pub belief_id: String,
    /// 置信度变化 (-10 to +10)
    pub delta: i8,
    pub evidence_type: EvidenceType,
    pub reasoning: String,
    /// 是否为反向证伪信号
    pub is_contradiction: bool,
}

/// 证据类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvidenceType {
    /// 新证据支持该信念
    Support,
    /// 新证据挑战该信念
    Challenge,
    /// 无关
    Neutral,
}

/// 更新信念状态
///
/// 对每个信念检查新的 QuestionMatch 是否与之相关。
/// 如果 evidence_type == Challenge 且 delta < -3:
/// 标记为 contradiction（强制保留而非丢弃）。
pub fn update_beliefs(
    question_matches: &[crate::question_engine::QuestionMatch],
    current_beliefs: &[BeliefStatement],
) -> Vec<BeliefUpdate> {
    let mut updates = Vec::new();

    for belief in current_beliefs {
        // 寻找与该信念相关的匹配
        let related: Vec<&crate::question_engine::QuestionMatch> = question_matches
            .iter()
            .filter(|qm| {
                qm.question_text
                    .to_lowercase()
                    .contains(&belief.text.chars().take(20).collect::<String>().to_lowercase())
            })
            .collect();

        if related.is_empty() {
            continue;
        }

        // 综合评估证据类型
        let support_count = related.iter().filter(|r| r.evidence_type == "Support").count();
        let challenge_count = related.iter().filter(|r| r.evidence_type == "Challenge").count();

        let (evidence_type, delta, is_contradiction) = if challenge_count > support_count {
            let d = -(challenge_count as i8).min(5);
            (EvidenceType::Challenge, d, d <= -3)
        } else if support_count > challenge_count {
            let d = (support_count as i8).min(5);
            (EvidenceType::Support, d, false)
        } else {
            (EvidenceType::Neutral, 0, false)
        };

        updates.push(BeliefUpdate {
            belief_id: belief.id.clone(),
            delta,
            evidence_type,
            reasoning: format!(
                "Support: {}, Challenge: {} → delta: {}",
                support_count, challenge_count, delta
            ),
            is_contradiction,
        });
    }

    updates
}

/// 技术实体归一化映射表（OpenCTI 双 ID 风格）
///
/// Expert Refinement: entity_overlap 前不做字符串字面匹配。
/// 强制依赖标准化行业 Term ID，解决"Advanced Packaging"与"Heterogeneous Integration"
/// 物理相同但 Jaccard=0 的同义词灾难。
#[allow(dead_code)]
const ENTITY_NORMALIZATION: &[(&str, &[&str])] = &[
    ("advanced-packaging", &["advanced packaging", "heterogeneous integration", "chiplet", "3d packaging", "fan-out", "interposer"]),
    ("euv-lithography", &["euv lithography", "extreme ultraviolet", "nxe", "high-na euv", "0.33na"]),
    ("gaa-transistor", &["gaa", "gate-all-around", "nanosheet", "forksheet", "cfet", "complementary fet"]),
    ("ai-accelerator", &["ai accelerator", "ai chip", "npu", "tpu", "inference chip", "neural processor", "deep learning accelerator"]),
    ("hbm-memory", &["hbm", "high-bandwidth memory", "hbm2", "hbm3", "hbm4", "stacked memory", "3d dram"]),
    ("silicon-photonics", &["silicon photonics", "photonic interconnect", "optical interconnect", "silicon photonic", "integrated photonics"]),
    ("cuda-ecosystem", &["cuda", "nvidia cuda", "cuda ecosystem", "cuda platform", "cuda gpu"]),
    ("risc-v", &["risc-v", "riscv", "open-source isa", "open instruction set"]),
    ("export-control", &["export control", "entity list", "bis", "commerce control", "license requirement", "technology denial", "export restriction"]),
    ("supply-chain-relocation", &["supply chain relocation", "reshoring", "nearshoring", "friend-shoring", "china+1", "supply chain diversification"]),
];

/// 将技术实体归一化为标准行业 Term ID
///
/// 输入: "Heterogeneous Integration with 3D packaging"
/// 输出: ["advanced-packaging", "advanced-packaging"]
/// 去重后: ["advanced-packaging"]
#[allow(dead_code)]
fn normalize_entities(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut normalized = Vec::new();
    for (term_id, aliases) in ENTITY_NORMALIZATION {
        if aliases.iter().any(|a| lower.contains(a)) {
            normalized.push(term_id.to_string());
        }
    }
    normalized.sort();
    normalized.dedup();
    normalized
}

/// 计算实体重叠的 Jaccard 相似度（归一化后）
#[allow(dead_code)]
fn entity_jaccard(entities_a: &[String], entities_b: &[String]) -> f64 {
    let a_set: std::collections::HashSet<&str> = entities_a.iter().map(|s| s.as_str()).collect();
    let b_set: std::collections::HashSet<&str> = entities_b.iter().map(|s| s.as_str()).collect();
    let intersection = a_set.intersection(&b_set).count();
    let union = a_set.union(&b_set).count();
    if union == 0 { 0.0 } else { intersection as f64 / union as f64 }
}

/// 计算 contradiction score
///
/// contradict_score = entity_overlap * (1 - content_similarity)
/// - entity_overlap: 归一化后的实体 Jaccard 相似度
/// - content_similarity: 文本级余弦/关键词相似度
/// 阈值门控 >= 0.3 判定为矛盾，O(n²) 上限 500 个事实
#[allow(dead_code)]
pub fn calculate_contradiction_score(
    text_a: &str,
    entities_a: &[String],
    text_b: &str,
    entities_b: &[String],
) -> f64 {
    let entity_overlap = entity_jaccard(entities_a, entities_b);
    if entity_overlap < 0.2 { return 0.0; } // 实体都不重叠就不用比了

    // content_similarity: 简单关键词重叠
    let a_lower = text_a.to_lowercase();
    let b_lower = text_b.to_lowercase();
    let words_a: std::collections::HashSet<&str> = a_lower
        .split_whitespace().filter(|w| w.len() > 3).collect();
    let words_b: std::collections::HashSet<&str> = b_lower
        .split_whitespace().filter(|w| w.len() > 3).collect();
    let common = words_a.intersection(&words_b).count();
    let content_similarity = if words_a.is_empty() || words_b.is_empty() {
        0.0
    } else {
        let max_len = words_a.len().max(words_b.len()) as f64;
        common as f64 / max_len
    };

    entity_overlap * (1.0 - content_similarity)
}

/// 检查文章是否与现有信念冲突
///
/// Code Review 逆向 triage:
/// 如果 contradicts == true，调用方应强制保留到 Insight。
///
/// Expert Refinement: 使用归一化实体计算 contradiction_score，
/// 解决同义词灾难（Advanced Packaging vs Heterogeneous Integration）。
#[allow(dead_code)]
pub fn check_contradiction(
    article_title: &str,
    article_content: &str,
    active_beliefs: &[BeliefStatement],
) -> Option<crate::agent::scan::ContradictionRecord> {
    let combined = format!("{} {}", article_title, article_content);
    let combined_normalized = normalize_entities(&combined);

    for belief in active_beliefs {
        let belief_normalized = normalize_entities(&belief.text);

        // 使用归一化后的实体计算 contradiction_score
        let score = calculate_contradiction_score(
            &combined, &combined_normalized,
            &belief.text, &belief_normalized,
        );

        // 阈值门控 >= 0.3，且实体必须重叠
        if score >= 0.3 && !combined_normalized.is_empty() && !belief_normalized.is_empty() {
            return Some(crate::agent::scan::ContradictionRecord {
                article_id: article_title.chars().take(50).collect::<String>(),
                belief_key: belief.id.clone(),
                contradicts: true,
                retained: true,
                created_at: chrono::Local::now().format("%Y-%m-%d").to_string(),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::question_engine::QuestionMatch;

    #[test]
    fn test_update_beliefs_support() {
        let matches = vec![QuestionMatch {
            question_id: "q1".into(),
            question_text: "先进制程产能受限分析".into(),
            relevance: 8,
            reasoning: "匹配".into(),
            evidence_type: "Support".into(),
        }];
        let beliefs = vec![BeliefStatement {
            id: "b1".into(),
            text: "先进制程产能".into(),
            confidence: 7,
            category: "Tech".into(),
            evidence_ids: vec![],
        }];
        let updates = update_beliefs(&matches, &beliefs);
        assert_eq!(updates.len(), 1);
        assert!(updates[0].delta > 0);
        assert!(!updates[0].is_contradiction);
    }

    #[test]
    fn test_check_contradiction_triggers() {
        let beliefs = vec![BeliefStatement {
            id: "b2".into(),
            text: "US semiconductor export controls are tightening — BIS entity list sanctions".into(),
            confidence: 8,
            category: "Tech".into(),
            evidence_ids: vec![],
        }];
        let result = check_contradiction(
            "ASML gets export exemption for China",
            "Dutch government approves ASML to continue servicing Chinese customers despite US export controls",
            &beliefs,
        );
        assert!(result.is_some(), "Contradiction should be detected: sanction vs exemption");
    }

    #[test]
    fn test_check_contradiction_no_false_positive() {
        // 使用归一化后不同实体的信念-文章对，确保不触发矛盾
        let beliefs = vec![BeliefStatement {
            id: "b3".into(),
            text: "RISC-V open source ISA gaining industry adoption momentum".into(),
            confidence: 8,
            category: "Tech".into(),
            evidence_ids: vec![],
        }];
        let result = check_contradiction(
            "BIS adds new Chinese chip companies to entity list",
            "US Commerce Dept expands export controls on semiconductor equipment to China",
            &beliefs,
        );
        assert!(result.is_none(), "Unrelated topics should not trigger contradiction");
    }

    #[test]
    fn test_entity_normalization_resolves_synonym_crisis() {
        // "Advanced Packaging" vs "Heterogeneous Integration" — 物理相同实体
        let text_a = "TSMC's advanced packaging capacity expansion for 3D chiplet integration";
        let text_b = "Intel's heterogeneous integration roadmap using fan-out interposer technology";
        let ent_a = normalize_entities(text_a);
        let ent_b = normalize_entities(text_b);
        assert!(ent_a.contains(&"advanced-packaging".to_string()));
        assert!(ent_b.contains(&"advanced-packaging".to_string()));
        // 归一化后 Jaccard 应该 > 0
        let jaccard = entity_jaccard(&ent_a, &ent_b);
        assert!(jaccard > 0.0, "Synonyms should match after normalization: Jaccard={}", jaccard);
    }

    #[test]
    fn test_contradiction_score_formula() {
        // 同一实体但内容对立 -> 高 contradiction score
        let score = calculate_contradiction_score(
            "BIS expands export controls on semiconductor equipment",
            &["export-control".to_string()],
            "ASML receives export exemption for China service",
            &["export-control".to_string()],
        );
        assert!(score > 0.3, "Contradictory on same entity: score={}", score);
    }
}
