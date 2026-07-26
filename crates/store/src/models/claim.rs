use serde::{Deserialize, Serialize};

/// Claim — 系统的可验证判断单元。
///
/// 核心链路：Evidence → Claim → Signal
/// confidence 通过 evidence strength 聚合计算，非用户手动打分。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub id: i64,
    pub statement: String,
    pub confidence: f64,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewClaim {
    pub statement: String,
    pub status: Option<String>,
}

/// Claim → Evidence 关系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimEvidence {
    pub claim_id: i64,
    pub evidence_id: i64,
    pub strength: f64,
    pub relation: EvidenceRelation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvidenceRelation {
    Supports,
    Contradicts,
    Weakens,
}

impl EvidenceRelation {
    pub fn as_str(&self) -> &'static str {
        match self {
            EvidenceRelation::Supports => "supports",
            EvidenceRelation::Contradicts => "contradicts",
            EvidenceRelation::Weakens => "weakens",
        }
    }
}
