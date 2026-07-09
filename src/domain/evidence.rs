//! 证据领域模型
//!
//! 核心类型：FactBaseEntry（事实基础条目）、Evidence（单条证据）、
//! Stance（证据立场）、EvidenceSource（证据来源描述）。

use serde::{Deserialize, Serialize};

/// Fact Base 条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactBaseEntry {
    pub evidence: String,
    pub interpretation: String,
    pub confidence: String,
}

/// 证据立场
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Stance {
    Supports,
    Challenges,
    Neutral,
}

/// 单条证据：一条信号对 Thesis 的支持/挑战记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// 证据出现日期
    pub date: String,
    /// 来源文章标题
    pub title: String,
    /// 来源名称
    pub source: String,
    /// 证据核心内容（~50 字，取自 analysis.bluf）
    pub summary: String,
    /// 立场
    pub stance: Stance,
    /// 当日 SVI 评分 1-10
    pub signal_strength: u8,
}

/// 计算置信度 0.0-1.0（从证据 Support/Challenge 比例）
/// 证据量饱和常数：控制少量证据时的置信度增长速度
/// k 越小，单条证据的置信度上限越低；k=3 时 1 条证据 ~62%，3 条 ~75%，10 条 ~88%
const EVIDENCE_SATURATION_K: f64 = 3.0;

pub fn compute_confidence(evidences: &[Evidence]) -> f64 {
    let support = evidences
        .iter()
        .filter(|e| e.stance == Stance::Supports)
        .count() as f64;
    let challenge = evidences
        .iter()
        .filter(|e| e.stance == Stance::Challenges)
        .count() as f64;
    let total = support + challenge;
    if total == 0.0 {
        0.5
    } else {
        let ratio = support / total;
        // 饱和度因子：少量证据时压低置信度，随证据量渐近收敛
        let saturation = total / (total + EVIDENCE_SATURATION_K);
        (0.5 + (ratio - 0.5) * saturation).clamp(0.1, 0.98)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_confidence_all_support_two_evidences() {
        let evidences = vec![
            Evidence { date: "2026-07-01".into(), title: "A".into(), source: "S".into(), summary: "".into(), stance: Stance::Supports, signal_strength: 5 },
            Evidence { date: "2026-07-01".into(), title: "B".into(), source: "S".into(), summary: "".into(), stance: Stance::Supports, signal_strength: 5 },
        ];
        // 2 条全支持: 0.5 + 0.5 * 2/(2+3) = 0.5 + 0.5 * 0.4 = 0.7
        let c = compute_confidence(&evidences);
        assert!((c - 0.7).abs() < 0.01, "expected ~0.7, got {}", c);
    }

    #[test]
    fn test_compute_confidence_single_support() {
        let evidences = vec![
            Evidence { date: "2026-07-01".into(), title: "A".into(), source: "S".into(), summary: "".into(), stance: Stance::Supports, signal_strength: 5 },
        ];
        // 1 条支持: 0.5 + 0.5 * 1/(1+3) = 0.5 + 0.5 * 0.25 = 0.625
        let c = compute_confidence(&evidences);
        assert!((c - 0.625).abs() < 0.01, "expected ~0.625, got {}", c);
    }

    #[test]
    fn test_compute_confidence_all_challenge() {
        let evidences = vec![
            Evidence { date: "2026-07-01".into(), title: "A".into(), source: "S".into(), summary: "".into(), stance: Stance::Challenges, signal_strength: 5 },
        ];
        // 1 条全挑战: 0.5 + (0.0 - 0.5) * 1/4 = 0.5 + (-0.5) * 0.25 = 0.375
        let c = compute_confidence(&evidences);
        assert!((c - 0.375).abs() < 0.01, "expected ~0.375, got {}", c);
    }

    #[test]
    fn test_compute_confidence_empty() {
        assert!((compute_confidence(&[]) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_compute_confidence_mixed() {
        let evidences = vec![
            Evidence { date: "2026-07-01".into(), title: "A".into(), source: "S".into(), summary: "".into(), stance: Stance::Supports, signal_strength: 5 },
            Evidence { date: "2026-07-01".into(), title: "B".into(), source: "S".into(), summary: "".into(), stance: Stance::Challenges, signal_strength: 5 },
        ];
        // S:1 C:1 → ratio=0.5 → 0.5 + 0 * saturation = 0.5
        let c = compute_confidence(&evidences);
        assert!((c - 0.5).abs() < 0.01, "expected ~0.5, got {}", c);
    }

    #[test]
    fn test_compute_confidence_large_evidence_settles() {
        let mut evidences = Vec::new();
        for i in 0..10 {
            evidences.push(Evidence {
                date: "2026-07-01".into(), title: format!("A{}", i),
                source: "S".into(), summary: "".into(),
                stance: Stance::Supports, signal_strength: 5,
            });
        }
        // 10 条全支持: 0.5 + 0.5 * 10/13 = 0.5 + 0.5 * 0.769 = 0.8846
        let c = compute_confidence(&evidences);
        assert!((c - 0.885).abs() < 0.01, "expected ~0.885, got {}", c);
    }
}
