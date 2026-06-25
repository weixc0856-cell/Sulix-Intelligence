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
