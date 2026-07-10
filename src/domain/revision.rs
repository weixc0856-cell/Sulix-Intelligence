//! Revision — 版本化决策的基石
//!
//! "Git for Strategic Decisions"
//!
//! Sulix 记录的不是"今天的新闻"，而是"判断如何一步一步演化，以及为什么演化"。
//!
//! Revision 从分散的快照中聚合出一个统一的版本时间线。
//! 它是 Projection（派生层），不是 Storage（存储层）。
//!
//! 每个 Revision 回答三个问题：
//!   1. What changed? (confidence / decision / status / evidence)
//!   2. Why did it change? (trigger + rationale)
//!   3. When did it change? (date + version number)
//!
//! 未来方向 — Revision Diff：
//!   任意两个 Revision 之间的 diff 才是真正的 Decision Explainability。
//!   `diff(r1, r2)` → 清晰展示 confidence/decision/evidence 的变化。

use crate::domain::evidence::{Evidence, Stance};
use crate::domain::thesis::{
    ConfidenceSnapshot, ConfidenceTrigger, DecisionSnapshot, StatusTransition, Thesis,
};
use serde::{Deserialize, Serialize};

/// 一个统一化的判断修订 — 类似 Git revision
///
/// 当以下任一事件发生时创建新修订：
///   - 置信度变化 ≥ 5%
///   - 决策类型变化
///   - 状态变更
///   - 有新证据加入
///
/// 如果同一天发生多个事件，它们合并为一个 Revision。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
    /// 修订号，从 1 开始递增
    pub version: u32,
    /// 日期 YYYY-MM-DD
    pub date: String,

    // ── What changed ──
    /// 置信度变化（百分比，如 +14 表示上升 14%）
    pub confidence_delta: Option<f64>,
    /// 当前置信度（百分比，如 76.0）
    pub confidence: f64,
    /// 置信度变化触发原因
    pub confidence_trigger: Option<String>,

    /// 决策变化 (from, to)，无变化则为 None
    pub decision_change: Option<(String, String)>,

    /// 状态变化 (from, to)，无变化则为 None
    pub status_change: Option<(String, String)>,

    // ── What evidence was added ──
    /// 本修订新增的支持证据标题列表
    pub evidence_added: Vec<String>,
    /// 本修订新增的反对证据标题列表
    pub challenges_added: Vec<String>,

    // ── Why it changed ──
    /// 变更理由（从快照的 reason/description 字段合成）
    pub rationale: String,

    // ── Context ──
    /// 支撑证据总数
    pub total_support: usize,
    /// 反对证据总数
    pub total_challenge: usize,
}

impl Revision {
    /// 是否为有意义的变更（非 noise）
    pub fn is_meaningful(&self) -> bool {
        self.decision_change.is_some()
            || self.status_change.is_some()
            || self
                .confidence_delta
                .map(|d| d.abs() >= 5.0)
                .unwrap_or(false)
            || !self.evidence_added.is_empty()
            || !self.challenges_added.is_empty()
    }

    /// 变更摘要（< 80 字符，用于卡片展示）
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = vec![];

        if let Some((ref from, ref to)) = self.decision_change {
            parts.push(format!(
                "Decision: {} → {}",
                from.to_uppercase(),
                to.to_uppercase()
            ));
        }

        if let Some(delta) = self.confidence_delta {
            if delta.abs() >= 3.0 {
                let dir = if delta > 0.0 { "↑" } else { "↓" };
                parts.push(format!("Confidence {}{:.0}%", dir, delta.abs()));
            }
        }

        if let Some((ref _from, ref to)) = self.status_change {
            if to == "retired" || to == "dormant" {
                parts.push(format!("Status → {}", to));
            }
        }

        let ev_total = self.evidence_added.len() + self.challenges_added.len();
        if ev_total > 0 {
            parts.push(format!("{} new evidence", ev_total));
        }

        if parts.is_empty() {
            parts.push("Revision recorded".to_string());
        }

        parts.join(" · ")
    }

    /// Diff this revision against the previous state.
    /// Returns human-readable lines describing what changed.
    /// This is the foundation for Revision Diff UI — future feature.
    pub fn diff_lines(&self) -> Vec<String> {
        let mut lines = vec![];

        if let Some(delta) = self.confidence_delta {
            let dir = if delta > 0.0 { "+" } else { "" };
            lines.push(format!("Confidence: {}{:.0}%", dir, delta));
        }

        if let Some((ref from, ref to)) = self.decision_change {
            lines.push(format!("Decision: {} → {}", from, to));
        }

        if let Some((ref from, ref to)) = self.status_change {
            lines.push(format!("Status: {} → {}", from, to));
        }

        for ev in &self.evidence_added {
            lines.push(format!("+ Evidence: {}", ev));
        }

        for ch in &self.challenges_added {
            lines.push(format!("- Challenge: {}", ch));
        }

        lines
    }
}

/// 从 Thesis 的已有数据聚合出统一的修订时间线
///
/// 合并策略：
///   1. 收集所有事件（confidence 快照 + evidence 记录 + status 变迁）
///   2. 按日期分组
///   3. 同一天的事件合并为一个 Revision
///
/// 注意：这是一个**派生**函数 — 它不修改任何存储，只重组已有数据。
pub fn build_revision_history(thesis: &Thesis) -> Vec<Revision> {
    if thesis.confidence_history.is_empty() && thesis.evidences.is_empty() {
        return vec![];
    }

    // Collect all events with their dates
    let mut date_events: Vec<String> = vec![];

    // Confidence snapshots
    for snap in &thesis.confidence_history {
        date_events.push(snap.date.clone());
    }

    // Evidence records
    for ev in &thesis.evidences {
        date_events.push(ev.date.clone());
    }

    // Status transitions
    for st in &thesis.status_history {
        date_events.push(st.date.clone());
    }

    // Dedup and sort
    date_events.sort();
    date_events.dedup();

    // Build revisions by date
    let mut revisions: Vec<Revision> = vec![];
    let mut prev_confidence: f64 = 0.0;
    let mut prev_decision: Option<String> = None;
    let mut prev_status: Option<String> = None;
    let mut cumulative_support: usize = 0;
    let mut cumulative_challenge: usize = 0;

    // Get initial state from first confidence snapshot if available
    if let Some(first) = thesis.confidence_history.first() {
        prev_confidence = first.value;
    }

    for date in &date_events {
        // Confidence changes on this date
        let confidence_snaps: Vec<&ConfidenceSnapshot> = thesis
            .confidence_history
            .iter()
            .filter(|s| &s.date == date)
            .collect();

        let confidence = confidence_snaps
            .last()
            .map(|s| s.value)
            .unwrap_or(prev_confidence);

        let confidence_delta = if revision_count(&revisions) > 0 || !confidence_snaps.is_empty() {
            let delta = (confidence - prev_confidence) * 100.0;
            if delta.abs() >= 1.0 {
                Some(delta)
            } else {
                None
            }
        } else {
            None
        };

        let confidence_trigger = confidence_snaps.last().map(|s| trigger_label(&s.trigger));

        // Decision changes on this date
        let decision_snaps: Vec<&DecisionSnapshot> = thesis
            .decision_history
            .iter()
            .filter(|s| &s.date == date)
            .collect();

        let decision_change = if let Some(last) = decision_snaps.last() {
            let new_type = &last.decision_type;
            if prev_decision.as_ref() != Some(new_type) && prev_decision.is_some() {
                let change = Some((prev_decision.clone().unwrap_or_default(), new_type.clone()));
                prev_decision = Some(new_type.clone());
                change
            } else {
                if prev_decision.is_none() {
                    prev_decision = Some(new_type.clone());
                }
                None
            }
        } else {
            None
        };

        // Status changes on this date
        let status_transitions: Vec<&StatusTransition> = thesis
            .status_history
            .iter()
            .filter(|s| &s.date == date)
            .collect();

        let status_change = if let Some(last) = status_transitions.last() {
            let new_status = format!("{:?}", last.to).to_lowercase();
            if prev_status.as_ref() != Some(&new_status) {
                let change = Some((
                    prev_status
                        .clone()
                        .unwrap_or_else(|| format!("{:?}", last.from).to_lowercase()),
                    new_status.clone(),
                ));
                prev_status = Some(new_status);
                change
            } else {
                None
            }
        } else {
            None
        };

        // Evidence added on this date
        let evidence_on_date: Vec<&Evidence> = thesis
            .evidences
            .iter()
            .filter(|e| &e.date == date)
            .collect();

        let evidence_added: Vec<String> = evidence_on_date
            .iter()
            .filter(|e| e.stance == Stance::Supports)
            .map(|e| e.title.clone())
            .collect();

        let challenges_added: Vec<String> = evidence_on_date
            .iter()
            .filter(|e| e.stance == Stance::Challenges)
            .map(|e| e.title.clone())
            .collect();

        cumulative_support += evidence_added.len();
        cumulative_challenge += challenges_added.len();

        // ── Build rationale ──
        let rationale_parts: Vec<String> = confidence_snaps
            .iter()
            .map(|s| s.reason.clone())
            .chain(status_transitions.iter().map(|s| s.description.clone()))
            .filter(|r| !r.is_empty())
            .collect();

        let rationale = if rationale_parts.is_empty() {
            format!("Evidence update on {}", date)
        } else {
            rationale_parts.join("; ")
        };

        // ── Determine if this is a meaningful revision ──
        let is_first_revision = revisions.is_empty();

        let has_change = is_first_revision
            || confidence_delta.is_some()
            || decision_change.is_some()
            || status_change.is_some()
            || !evidence_added.is_empty()
            || !challenges_added.is_empty();

        if !has_change {
            continue;
        }

        let rev = Revision {
            version: revision_count(&revisions) as u32 + 1,
            date: date.clone(),
            confidence_delta,
            confidence: confidence * 100.0,
            confidence_trigger,
            decision_change,
            status_change,
            evidence_added,
            challenges_added,
            rationale,
            total_support: cumulative_support,
            total_challenge: cumulative_challenge,
        };

        prev_confidence = confidence;
        revisions.push(rev);
    }

    revisions
}

fn trigger_label(trigger: &ConfidenceTrigger) -> String {
    match trigger {
        ConfidenceTrigger::Initial => "Initial".to_string(),
        ConfidenceTrigger::StatusChange => "StatusChange".to_string(),
        ConfidenceTrigger::SignificantChange => "SignificantChange".to_string(),
        ConfidenceTrigger::ManualUpdate => "ManualUpdate".to_string(),
        ConfidenceTrigger::OutcomeRecorded => "OutcomeRecorded".to_string(),
    }
}

fn revision_count(revisions: &[Revision]) -> usize {
    revisions.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::evidence::{Evidence, Stance};
    use crate::domain::strategic_domain::StrategicDomain;
    use crate::domain::thesis::{
        ConfidenceSnapshot, ConfidenceTrigger, DecisionSnapshot, Thesis, ThesisStatus,
    };

    fn make_evidence(date: &str, title: &str, stance: Stance) -> Evidence {
        Evidence {
            date: date.to_string(),
            title: title.to_string(),
            source: "test".to_string(),
            summary: "test summary".to_string(),
            stance,
            signal_strength: 5,
        }
    }

    #[test]
    fn test_empty_thesis_no_revisions() {
        let thesis = Thesis {
            id: "t1".into(),
            title: "Empty".into(),
            created: "2026-06-01".into(),
            updated: "2026-06-01".into(),
            evidences: vec![],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
            primary_domain: StrategicDomain::default(),
            secondary_domains: vec![],
            lifecycle_events: vec![],
        };
        let revisions = build_revision_history(&thesis);
        assert!(revisions.is_empty());
    }

    #[test]
    fn test_confidence_only_revisions() {
        let thesis = Thesis {
            id: "t1".into(),
            title: "Test".into(),
            created: "2026-06-01".into(),
            updated: "2026-06-05".into(),
            evidences: vec![],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![
                ConfidenceSnapshot {
                    date: "2026-06-01".into(),
                    value: 0.62,
                    trigger: ConfidenceTrigger::Initial,
                    reason: "Initial assessment".into(),
                },
                ConfidenceSnapshot {
                    date: "2026-06-03".into(),
                    value: 0.71,
                    trigger: ConfidenceTrigger::SignificantChange,
                    reason: "New evidence supports thesis".into(),
                },
                ConfidenceSnapshot {
                    date: "2026-06-05".into(),
                    value: 0.83,
                    trigger: ConfidenceTrigger::SignificantChange,
                    reason: "Additional confirmation".into(),
                },
            ],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
            primary_domain: StrategicDomain::default(),
            secondary_domains: vec![],
            lifecycle_events: vec![],
        };

        let revisions = build_revision_history(&thesis);
        assert_eq!(revisions.len(), 3);
        assert_eq!(revisions[0].confidence_delta, None);
        assert_eq!(revisions[1].confidence_delta.unwrap().round(), 9.0);
        assert_eq!(revisions[2].confidence_delta.unwrap().round(), 12.0);
    }

    #[test]
    fn test_decision_change_detected() {
        let thesis = Thesis {
            id: "t1".into(),
            title: "Test".into(),
            created: "2026-06-01".into(),
            updated: "2026-06-03".into(),
            evidences: vec![],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![
                ConfidenceSnapshot {
                    date: "2026-06-01".into(),
                    value: 0.60,
                    trigger: ConfidenceTrigger::Initial,
                    reason: "Start".into(),
                },
                ConfidenceSnapshot {
                    date: "2026-06-03".into(),
                    value: 0.75,
                    trigger: ConfidenceTrigger::SignificantChange,
                    reason: "Strengthening".into(),
                },
            ],
            decision_history: vec![
                DecisionSnapshot {
                    date: "2026-06-01".into(),
                    decision_type: "monitor".into(),
                    confidence: 0.60,
                },
                DecisionSnapshot {
                    date: "2026-06-03".into(),
                    decision_type: "build".into(),
                    confidence: 0.75,
                },
            ],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            falsification_conditions: vec![],
            assessment_id: None,
            primary_domain: StrategicDomain::default(),
            secondary_domains: vec![],
            lifecycle_events: vec![],
        };

        let revisions = build_revision_history(&thesis);
        assert_eq!(revisions.len(), 2);
        assert_eq!(
            revisions[1].decision_change,
            Some(("monitor".into(), "build".into()))
        );
    }

    #[test]
    fn test_evidence_merged_into_revision() {
        let thesis = Thesis {
            id: "t1".into(),
            title: "Test".into(),
            created: "2026-06-01".into(),
            updated: "2026-06-01".into(),
            evidences: vec![
                make_evidence("2026-06-01", "Evidence A", Stance::Supports),
                make_evidence("2026-06-01", "Evidence B", Stance::Supports),
                make_evidence("2026-06-01", "Challenge C", Stance::Challenges),
            ],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![ConfidenceSnapshot {
                date: "2026-06-01".into(),
                value: 0.65,
                trigger: ConfidenceTrigger::Initial,
                reason: "First evidence".into(),
            }],
            decision_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            falsification_conditions: vec![],
            assessment_id: None,
            primary_domain: StrategicDomain::default(),
            secondary_domains: vec![],
            lifecycle_events: vec![],
        };

        let revisions = build_revision_history(&thesis);
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].evidence_added.len(), 2);
        assert_eq!(revisions[0].challenges_added.len(), 1);
        assert_eq!(revisions[0].total_support, 2);
        assert_eq!(revisions[0].total_challenge, 1);
    }

    #[test]
    fn test_is_meaningful() {
        let trivial = Revision {
            version: 1,
            date: "2026-06-01".into(),
            confidence_delta: Some(1.0),
            confidence: 65.0,
            confidence_trigger: None,
            decision_change: None,
            status_change: None,
            evidence_added: vec![],
            challenges_added: vec![],
            rationale: "Minimal change".into(),
            total_support: 1,
            total_challenge: 0,
        };
        assert!(!trivial.is_meaningful());

        let meaningful = Revision {
            version: 2,
            date: "2026-06-02".into(),
            confidence_delta: Some(12.0),
            confidence: 77.0,
            confidence_trigger: None,
            decision_change: None,
            status_change: None,
            evidence_added: vec![],
            challenges_added: vec![],
            rationale: "Big confidence jump".into(),
            total_support: 2,
            total_challenge: 0,
        };
        assert!(meaningful.is_meaningful());
    }

    #[test]
    fn test_diff_lines() {
        let rev = Revision {
            version: 2,
            date: "2026-06-05".into(),
            confidence_delta: Some(12.0),
            confidence: 83.0,
            confidence_trigger: Some("SignificantChange".into()),
            decision_change: Some(("monitor".into(), "build".into())),
            status_change: None,
            evidence_added: vec!["OpenAI confirms training milestone".into()],
            challenges_added: vec![],
            rationale: "Training milestone confirmed".into(),
            total_support: 3,
            total_challenge: 1,
        };

        let diff = rev.diff_lines();
        assert!(diff.iter().any(|l| l.contains("Confidence: +12%")));
        assert!(diff.iter().any(|l| l.contains("Decision: monitor → build")));
        assert!(diff.iter().any(|l| l.contains("+ Evidence: OpenAI")));
    }
}
