//! 行动建议领域模型
//!
//! Action 是 Sulix 对用户的输出——不是替用户决策，而是提供决策支持。
//!
//! 核心转变：
//!   旧（DecisionType）：NoChange / CourseCorrect / StrategicPivot / UrgentAction
//!     → 替用户决策，像咨询公司
//!   新（ActionType）：Observe / Explore / Invest / Execute / Exit
//!     → 建议用户行动，像决策支持系统
//!
//! 认知链路定位：
//!   ... → 决策行为（ASI × SVI × Confidence）→ Action（建议输出）
//!                                              ↑
//!                                         Action 在此
//!
//! Action 不是"命令"，而是"建议"。用户有最终决定权。

use serde::{Deserialize, Serialize};

/// 行动类型
///
/// 五级行动建议，从"关注"到"退出"：
///   Observe  — "建议关注"：值得留意，不需立即行动
///   Explore  — "建议验证"：需要更多信息才能判断
///   Invest   — "建议投入"：高置信度，值得投入资源
///   Execute  — "建议执行"：时机成熟，立即行动
///   Exit     — "建议退出"：信号反转，及时止损
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionType {
    /// 建议关注（Observe）—— 值得留意，不需立即行动
    Observe,
    /// 建议验证（Explore）—— 需要更多信息才能判断
    Explore,
    /// 建议投入（Invest）—— 高置信度，值得投入资源
    Invest,
    /// 建议执行（Execute）—— 时机成熟，立即行动
    Execute,
    /// 建议退出（Exit）—— 信号反转，及时止损
    Exit,
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
