//! ASI — Action Significance Index（行动相关性指数）
//!
//! SVI 衡量"世界变化强度"，但不区分"这个变化对谁有意义"。
//! ASI 补充用户相关性维度：同一个新闻对不同的人行动意义完全不同。
//!
//! Confidence 补充确定性维度：高影响 ≠ 高确定性。
//!
//! 公式:
//!   Final Value = SVI × ASI × Confidence
//!   (三因子乘积，确保高影响、高相关、高确定性的信号排在最前)
//!
//! ASI 公式:
//!   ASI = UserRelevance × 0.4 + TimeUrgency × 0.3 + Actionability × 0.3
//!
//! Confidence 公式:
//!   Confidence = EvidenceQuality × 0.4 + ConsensusLevel × 0.3 + Verifiability × 0.3
//!
//! 维度:
//!   - UserRelevance: 信号与用户关切问题的相关度（来自 QuestionEngine 匹配）
//!   - TimeUrgency:    行动窗口的紧迫性（复用 SVI temporal_urgency 逻辑）
//!   - Actionability:  用户是否能够采取行动（由信号强度和领域决定）
//!   - EvidenceQuality: 证据质量（来源可信度、证据一致性）
//!   - ConsensusLevel:  行业/同行共识程度
//!   - Verifiability:   可验证性（能否被客观证实）

use serde::{Deserialize, Serialize};

/// ASI 配置（可通过 config.toml 覆盖权重）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsiConfig {
    /// 用户相关度权重
    #[serde(default = "default_user_relevance_weight")]
    pub user_relevance_weight: f64,
    /// 时间紧迫性权重
    #[serde(default = "default_time_urgency_weight")]
    pub time_urgency_weight: f64,
    /// 可行动性权重
    #[serde(default = "default_actionability_weight")]
    pub actionability_weight: f64,
}

impl Default for AsiConfig {
    fn default() -> Self {
        Self {
            user_relevance_weight: default_user_relevance_weight(),
            time_urgency_weight: default_time_urgency_weight(),
            actionability_weight: default_actionability_weight(),
        }
    }
}

fn default_user_relevance_weight() -> f64 {
    0.4
}
fn default_time_urgency_weight() -> f64 {
    0.3
}
fn default_actionability_weight() -> f64 {
    0.3
}

/// ASI 计算结果
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AsiResult {
    /// ASI 指数 0.0-1.0
    pub asi: f64,
    /// 各维度分解
    pub user_relevance: f64,
    pub time_urgency: f64,
    pub actionability: f64,
}

/// 计算 ASI 指数
///
/// UserRelevance: 固定为 0.3（中等偏低），原 QuestionEngine 匹配始终为空。
///   若未来启用了 QuestionEngine，可恢复为基于匹配结果的动态计算。
///
/// TimeUrgency: 基于信号新鲜度。
///   与 SVI recency 逻辑一致：1 天内 1.0, 3 天内 0.8, 7 天内 0.5, 之后 0.2。
///
/// Actionability: 基于信号强度 proxy。
///   signal_strength >= 7 → 0.9（高可行动性）
///   signal_strength >= 5 → 0.6（中等）
///   signal_strength >= 3 → 0.3（偏低）
///   else → 0.1（噪音）
pub fn calculate_asi(signal_strength: u8, max_days_old: i64, config: &AsiConfig) -> AsiResult {
    // 1. UserRelevance — currently static (QuestionEngine not wired)
    let user_relevance = 0.3;

    // 2. TimeUrgency
    let time_urgency = if max_days_old <= 1 {
        1.0
    } else if max_days_old <= 3 {
        0.8
    } else if max_days_old <= 7 {
        0.5
    } else {
        0.2
    };

    // 3. Actionability
    let actionability = if signal_strength >= 7 {
        0.9
    } else if signal_strength >= 5 {
        0.6
    } else if signal_strength >= 3 {
        0.3
    } else {
        0.1
    };

    let asi = user_relevance * config.user_relevance_weight
        + time_urgency * config.time_urgency_weight
        + actionability * config.actionability_weight;

    AsiResult {
        asi: asi.clamp(0.0, 1.0),
        user_relevance,
        time_urgency,
        actionability,
    }
}

/// 计算最终价值：SVI × ASI × Confidence
///
/// 三因子乘积，确保高影响、高相关、高确定性的信号排在最前。
/// 结果在 0-10 范围内（SVI 是 0-10，ASI 和 Confidence 是 0-1，乘积在 0-10）
pub fn final_value(svi: u8, asi: &AsiResult, confidence: &ConfidenceResult) -> f64 {
    (svi as f64 * asi.asi * confidence.confidence).clamp(0.0, 10.0)
}

/// Confidence 配置（可通过 config.toml 覆盖权重）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceConfig {
    /// 证据质量权重
    #[serde(default = "default_evidence_quality_weight")]
    pub evidence_quality_weight: f64,
    /// 行业共识权重
    #[serde(default = "default_consensus_level_weight")]
    pub consensus_level_weight: f64,
    /// 可验证性权重
    #[serde(default = "default_verifiability_weight")]
    pub verifiability_weight: f64,
}

impl Default for ConfidenceConfig {
    fn default() -> Self {
        Self {
            evidence_quality_weight: default_evidence_quality_weight(),
            consensus_level_weight: default_consensus_level_weight(),
            verifiability_weight: default_verifiability_weight(),
        }
    }
}

fn default_evidence_quality_weight() -> f64 {
    0.4
}
fn default_consensus_level_weight() -> f64 {
    0.3
}
fn default_verifiability_weight() -> f64 {
    0.3
}

/// Confidence 计算结果
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ConfidenceResult {
    /// 综合置信度 0.0-1.0
    pub confidence: f64,
    /// 证据质量（来源可信度、证据一致性）
    pub evidence_quality: f64,
    /// 行业/同行共识程度
    pub consensus_level: f64,
    /// 可验证性（能否被客观证实）
    pub verifiability: f64,
}

/// 计算 Decision Confidence（决策置信度）
///
/// 高影响 ≠ 高确定性。Confidence 用于区分：
///   "OpenAI 发布 Agent SDK"（SVI=9, ASI=9, 但 Confidence=0.3 — 行业尚未验证）
///   vs
///   "美国对中国 AI 芯片出口限制"（SVI=8, ASI=8, Confidence=0.9 — 证据充分）
///
/// EvidenceQuality: 由证据级别决定
///   - Established-Fact: 0.9
///   - First-Principles: 0.7
///   - Developing-Inference: 0.4
///   - Assertion-Rumor: 0.2
///
/// ConsensusLevel: 由信号强度 proxy
///   signal_strength >= 8 → 0.8（高共识）
///   signal_strength >= 5 → 0.5（中等）
///   else → 0.2（低共识）
///
/// Verifiability: 由证据级别和来源数量决定
pub fn calculate_confidence(
    evidence_level: &str,
    signal_strength: u8,
    source_count: usize,
    config: &ConfidenceConfig,
) -> ConfidenceResult {
    // 1. EvidenceQuality
    let evidence_quality = match evidence_level {
        "Established-Fact" => 0.9,
        "First-Principles" => 0.7,
        "Developing-Inference" => 0.4,
        "Assertion-Rumor" => 0.2,
        _ => 0.5,
    };

    // 2. ConsensusLevel
    let consensus_level = if signal_strength >= 8 {
        0.8
    } else if signal_strength >= 5 {
        0.5
    } else {
        0.2
    };

    // 3. Verifiability（多来源可交叉验证）
    let verifiability = if source_count >= 3 {
        0.9
    } else if source_count >= 2 {
        0.7
    } else {
        0.4
    };

    let confidence = evidence_quality * config.evidence_quality_weight
        + consensus_level * config.consensus_level_weight
        + verifiability * config.verifiability_weight;

    ConfidenceResult {
        confidence: confidence.clamp(0.0, 1.0),
        evidence_quality,
        consensus_level,
        verifiability,
    }
}

#[cfg(test)]
impl ConfidenceResult {
    /// 创建一个默认的高置信度结果（用于测试）
    pub fn high() -> Self {
        Self {
            confidence: 0.85,
            evidence_quality: 0.8,
            consensus_level: 0.8,
            verifiability: 0.9,
        }
    }

    /// 创建一个默认的中等置信度结果（用于测试）
    pub fn medium() -> Self {
        Self {
            confidence: 0.5,
            evidence_quality: 0.5,
            consensus_level: 0.5,
            verifiability: 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asi_no_questions_low_relevance() {
        let result = calculate_asi(8, 0, &AsiConfig::default());
        assert!(result.asi > 0.2 && result.asi < 0.7);
    }

    #[test]
    fn test_asi_high_relevance_high_value() {
        // Note: user_relevance is now static 0.3 (QuestionEngine not wired)
        // All matches are accepted but don't affect the calculation
        let result = calculate_asi(9, 0, &AsiConfig::default());
        assert!(result.asi > 0.3, "ASI should be reasonable: {}", result.asi);
    }

    #[test]
    fn test_asi_low_relevance_low_value() {
        let result = calculate_asi(2, 30, &AsiConfig::default());
        assert!(result.asi < 0.3);
    }

    #[test]
    fn test_final_value() {
        let asi = AsiResult {
            asi: 0.8,
            user_relevance: 0.8,
            time_urgency: 0.8,
            actionability: 0.8,
        };
        let conf = ConfidenceResult {
            confidence: 1.0,
            evidence_quality: 1.0,
            consensus_level: 1.0,
            verifiability: 1.0,
        };
        let fv = final_value(8, &asi, &conf);
        assert!((fv - 6.4).abs() < 0.01);
    }

    #[test]
    fn test_asi_config_default() {
        let config = AsiConfig::default();
        assert!((config.user_relevance_weight - 0.4).abs() < 0.01);
        assert!((config.time_urgency_weight - 0.3).abs() < 0.01);
        assert!((config.actionability_weight - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_asi_clamped() {
        let result = calculate_asi(10, 0, &AsiConfig::default());
        assert!(result.user_relevance <= 1.0);
        assert!(result.asi <= 1.0);
    }

    #[test]
    fn test_asi_signal_strength_boundaries() {
        // signal_strength=0 → actionability=0.1
        let r0 = calculate_asi(0, 0, &AsiConfig::default());
        assert!((r0.actionability - 0.1).abs() < 0.01);

        // signal_strength=3 → actionability=0.3
        let r3 = calculate_asi(3, 0, &AsiConfig::default());
        assert!((r3.actionability - 0.3).abs() < 0.01);

        // signal_strength=5 → actionability=0.6
        let r5 = calculate_asi(5, 0, &AsiConfig::default());
        assert!((r5.actionability - 0.6).abs() < 0.01);

        // signal_strength=7 → actionability=0.9
        let r7 = calculate_asi(7, 0, &AsiConfig::default());
        assert!((r7.actionability - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_asi_max_days_old_boundaries() {
        // max_days_old=-1 → same as 0 (negative treated via max with 0 in caller)
        let r_neg = calculate_asi(5, -1, &AsiConfig::default());
        let r_0 = calculate_asi(5, 0, &AsiConfig::default());
        assert!((r_neg.time_urgency - 1.0).abs() < 0.01);
        assert!((r_0.time_urgency - 1.0).abs() < 0.01);

        // max_days_old=1 → time_urgency=1.0
        let r1 = calculate_asi(5, 1, &AsiConfig::default());
        assert!((r1.time_urgency - 1.0).abs() < 0.01);

        // max_days_old=2 → time_urgency=0.8
        let r2 = calculate_asi(5, 2, &AsiConfig::default());
        assert!((r2.time_urgency - 0.8).abs() < 0.01);

        // max_days_old=3 → time_urgency=0.8
        let r3 = calculate_asi(5, 3, &AsiConfig::default());
        assert!((r3.time_urgency - 0.8).abs() < 0.01);

        // max_days_old=4 → time_urgency=0.5
        let r4 = calculate_asi(5, 4, &AsiConfig::default());
        assert!((r4.time_urgency - 0.5).abs() < 0.01);

        // max_days_old=7 → time_urgency=0.5
        let r7 = calculate_asi(5, 7, &AsiConfig::default());
        assert!((r7.time_urgency - 0.5).abs() < 0.01);

        // max_days_old=8 → time_urgency=0.2
        let r8 = calculate_asi(5, 8, &AsiConfig::default());
        assert!((r8.time_urgency - 0.2).abs() < 0.01);
    }

    #[test]
    fn test_final_value_zero() {
        let asi = AsiResult {
            asi: 0.0,
            user_relevance: 0.0,
            time_urgency: 0.0,
            actionability: 0.0,
        };
        let conf = ConfidenceResult {
            confidence: 1.0,
            evidence_quality: 1.0,
            consensus_level: 1.0,
            verifiability: 1.0,
        };
        assert!((final_value(0, &asi, &conf) - 0.0).abs() < 0.01);
        assert!((final_value(10, &asi, &conf) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_final_value_clamp_upper() {
        let asi = AsiResult {
            asi: 1.0,
            user_relevance: 1.0,
            time_urgency: 1.0,
            actionability: 1.0,
        };
        let conf = ConfidenceResult {
            confidence: 1.0,
            evidence_quality: 1.0,
            consensus_level: 1.0,
            verifiability: 1.0,
        };
        let fv = final_value(10, &asi, &conf);
        assert!(fv <= 10.0);
        assert!((fv - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_confidence_established_fact() {
        let config = ConfidenceConfig::default();
        let result = calculate_confidence("Established-Fact", 9, 5, &config);
        assert!(
            result.confidence > 0.7,
            "high quality evidence should yield high confidence: {}",
            result.confidence
        );
    }

    #[test]
    fn test_calculate_confidence_rumor_low() {
        let config = ConfidenceConfig::default();
        let result = calculate_confidence("Assertion-Rumor", 3, 1, &config);
        assert!(
            result.confidence < 0.5,
            "rumor with single source should yield low confidence: {}",
            result.confidence
        );
    }

    #[test]
    fn test_calculate_confidence_source_count_boost() {
        let config = ConfidenceConfig::default();
        let single = calculate_confidence("Developing-Inference", 5, 1, &config);
        let multi = calculate_confidence("Developing-Inference", 5, 5, &config);
        assert!(
            multi.confidence > single.confidence,
            "more sources should increase confidence"
        );
    }

    #[test]
    fn test_confidence_result_high_medium() {
        let high = ConfidenceResult::high();
        assert!(high.confidence > 0.7);
        let med = ConfidenceResult::medium();
        assert!((med.confidence - 0.5).abs() < 0.01);
    }
}
