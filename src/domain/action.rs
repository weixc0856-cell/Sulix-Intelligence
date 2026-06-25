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

/// 决策时间尺度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DecisionHorizon {
    /// 立即行动
    Immediate,
    /// 30 天内
    ThirtyDays,
    /// 90 天内
    NinetyDays,
    /// 180 天内
    OneEightyDays,
}

impl DecisionHorizon {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Immediate => "立即",
            Self::ThirtyDays => "30天内",
            Self::NinetyDays => "90天内",
            Self::OneEightyDays => "180天内",
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

/// 行动建议
///
/// 每一条 Action 对应一个具体的建议，附带置信度、时间视窗和依据。
///
/// 注意：Action 是终局对象（Observation → Thesis → Decision → Action → Outcome 闭环），
/// 当前管线尚未构造 Action 实例，保留为领域冻结状态。
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// 唯一 ID
    pub id: String,
    /// 行动类型
    pub action_type: ActionType,
    /// 关联的 Thesis ID（可选）
    pub thesis_id: Option<String>,
    /// 行动描述——"建议做什么"
    pub description: String,
    /// 置信度 0.0-1.0
    pub confidence: f64,
    /// 时间视窗
    pub time_horizon: String,
    /// 建议依据
    pub rationale: String,
}

#[allow(dead_code)]
impl Action {
    /// 创建一个观察建议
    pub fn observe(description: &str, rationale: &str) -> Self {
        Self {
            id: String::new(),
            action_type: ActionType::Observe,
            thesis_id: None,
            description: description.to_string(),
            confidence: 0.6,
            time_horizon: "持续".into(),
            rationale: rationale.to_string(),
        }
    }

    /// 创建一个退出建议（最高优先级）
    pub fn exit(description: &str, rationale: &str) -> Self {
        Self {
            id: String::new(),
            action_type: ActionType::Exit,
            thesis_id: None,
            description: description.to_string(),
            confidence: 0.8,
            time_horizon: "立即".into(),
            rationale: rationale.to_string(),
        }
    }
}
