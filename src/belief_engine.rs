//! Belief Engine — 追踪信念的建立、挑战和更新
//!
//! 输入: QuestionMatch[] + 已有 BeliefStatement[]
//! 输出: 更新的置信度 + 被挑战的信念列表 + 新证据
//!
//! Code Review 逆向 triage 破局点:
//! 一旦触发 contradicts == true，强制将该信号提升为 Insight 级别，
//! 因为能证伪既有信念的反向黑天鹅信号价值远超 100 个顺向赞同信号。

// ===== 信念模型（从 domain 层导入）=====

pub use crate::domain::evidence::Stance;
pub use crate::domain::thesis::{BeliefDb, BeliefStatement, BeliefUpdate};

impl BeliefDb {
    pub fn new(date: &str) -> Self {
        Self {
            snapshot_date: date.to_string(),
            beliefs: vec![],
            recent_updates: vec![],
            total_support: 0,
            total_challenge: 0,
            contradictions_detected: 0,
        }
    }

    pub fn save_to_file(&self, path: &str) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// 批量应用信念更新：统计 support/challenge/contradictions
    pub fn apply_updates(&mut self, updates: &[BeliefUpdate]) {
        let support = updates
            .iter()
            .filter(|u| matches!(u.evidence_type, Stance::Supports))
            .count();
        let challenge = updates
            .iter()
            .filter(|u| matches!(u.evidence_type, Stance::Challenges))
            .count();
        let contradictions = updates.iter().filter(|u| u.is_contradiction).count();
        self.total_support += support;
        self.total_challenge += challenge;
        self.contradictions_detected += contradictions;
        self.recent_updates = updates.to_vec();
    }
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
                qm.question_text.to_lowercase().contains(
                    &belief
                        .text
                        .chars()
                        .take(20)
                        .collect::<String>()
                        .to_lowercase(),
                )
            })
            .collect();

        if related.is_empty() {
            continue;
        }

        // 综合评估证据类型
        let support_count = related
            .iter()
            .filter(|r| r.evidence_type == "Support")
            .count();
        let challenge_count = related
            .iter()
            .filter(|r| r.evidence_type == "Challenge")
            .count();

        let (evidence_type, delta, is_contradiction) = if challenge_count > support_count {
            let d = -(challenge_count as i8).min(5);
            (Stance::Challenges, d, d <= -3)
        } else if support_count > challenge_count {
            let d = (support_count as i8).min(5);
            (Stance::Supports, d, false)
        } else {
            (Stance::Neutral, 0, false)
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

// （ENTITY_NORMALIZATION + normalize_entities + entity_jaccard +
//  calculate_contradiction_score + check_contradiction 已移除。
//  这些死代码被 MemoryEngine (Thesis: Evidence[]) 替代。）

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
}
