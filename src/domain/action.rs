//! 行动建议领域模型
//!
//! Action 是 Sulix 对用户的输出——不是替用户决策，而是提供决策支持。
//!
//! 核心转变：
//!   旧（DecisionType）：NoChange / CourseCorrect / StrategicPivot / UrgentAction
//!     → 替用户决策，像咨询公司
//!   新（ActionType/Observe/Explore/Invest/Execute/Exit + DecisionType/Build/Invest/Monitor/Learn/Ignore/Exit）
//!     → 建议用户行动，像决策支持系统
//!
//! 认知链路定位：
//!   ... → Thesis → Investigation → Decision（行动建议）→ Outcome
//!                                              ↑
//!                                         Decision 在此
//!
//! Action 不是"命令"，而是"建议"。用户有最终决定权。

use serde::{Deserialize, Serialize};

/// 行动类型（五级行动建议，从"关注"到"退出"）
///
/// 与 DecisionType 的区别：
///   ActionType 是原始的五级信号（Observe-Execute），来自 Editor Agent。
///   DecisionType 是 Thesis → Decision 的映射结果，带有时间尺度和优先级。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionType {
    Observe,
    Explore,
    Invest,
    Execute,
    Exit,
}

/// 决策类型 — Thesis → Decision Intelligence 的映射结果
///
/// 创业者真正关心的不是"Observe"或"Invest"，
/// 而是："我要干什么？什么时候干？"
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DecisionType {
    /// 确定方向，开始投入资源。对应 Strengthening + 高 SVI
    Build,
    /// 已有信号支持，加大关注。对应 Strengthening
    Invest,
    /// 值得关注，但不行动。对应 Active
    Monitor,
    /// 需要更多认知，安排研究。对应 Weakening / Proposed
    Learn,
    /// 噪音，不浪费注意力。对应 Dormant
    Ignore,
    /// 信号反转，之前的判断需要放弃。对应 Retired + Invalidated
    Exit,
}

/// 决策时间尺度 — variants produce machine-readable codes via as_str()
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DecisionHorizon {
    /// immediate — 立即行动
    Immediate,
    /// 30d — 30 天内
    ThirtyDays,
    /// 90d — 90 天内
    NinetyDays,
    /// 180d — 180 天内
    OneEightyDays,
}

impl DecisionHorizon {
    /// Returns a machine-readable horizon code.
    /// Display values are handled by the frontend translation layer.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Immediate => "immediate",
            Self::ThirtyDays => "30d",
            Self::NinetyDays => "90d",
            Self::OneEightyDays => "180d",
        }
    }
}

/// 决策稳定性 — outcome history 驱动的稳定度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DecisionStability {
    /// 尚无 outcome，不稳定
    Volatile,
    /// 有 outcome 但 majority 为 confirmed/partial
    Stable,
    /// 已 invalidated 或 thesis 已 retired
    Final,
}

impl DecisionStability {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Volatile => "volatile",
            Self::Stable => "stable",
            Self::Final => "final",
        }
    }
}

impl DecisionType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Build => "Build",
            Self::Invest => "Invest",
            Self::Monitor => "Monitor",
            Self::Learn => "Learn",
            Self::Ignore => "Ignore",
            Self::Exit => "Exit",
        }
    }

    pub fn priority(&self) -> u8 {
        match self {
            Self::Exit => 1,
            Self::Build => 2,
            Self::Invest => 3,
            Self::Learn => 4,
            Self::Monitor => 5,
            Self::Ignore => 6,
        }
    }
}