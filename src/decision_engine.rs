//! Decision Engine — "建议关注、验证、投入、执行、还是退出？"
//!
//! 输入: BeliefUpdate[] + 已有决策历史
//! 输出: Vec<Action> 行动建议列表
//!
//! 核心转变：
//!   旧：NoChange / CourseCorrect / StrategicPivot / UrgentAction
//!     → 替用户决策，像咨询公司
//!   新：Observe / Explore / Invest / Execute / Exit
//!     → 建议用户行动，像决策支持系统
//!
//! 五级行动建议：
//!   Observe  — "建议关注"：值得留意，不需立即行动
//!   Explore  — "建议验证"：需要更多信息才能判断
//!   Invest   — "建议投入"：高置信度，值得投入资源
//!   Execute  — "建议执行"：时机成熟，立即行动
//!   Exit     — "建议退出"：信号反转，及时止损

use serde::{Deserialize, Serialize};

pub use crate::domain::action::ActionType;

/// 决策输出
///
/// 兼容层：保留 Decision 以保持 orchestrator.rs 接口不变，
/// 但内部使用 ActionType + Action。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// 影响的领域
    pub domain: String,
    /// 行动类型
    pub action_type: ActionType,
    /// 是否必须改变当前策略
    pub change_required: bool,
    /// 置信度 0.0-1.0
    pub confidence: f64,
    /// 具体行动项
    pub actions: Vec<String>,
    /// 时间视窗
    pub time_horizon: String,
    /// 触发该决策的关键证据
    pub key_evidence: Vec<String>,
    /// 是否为高优先级（Exit / Execute 级别）
    pub is_high_priority: bool,
}

/// 评估信念更新并生成决策建议
///
/// 基于信念更新的综合评估，输出五级行动建议：
/// - Exit:      contradiction 或置信度暴跌 → 建议退出
/// - Execute:  强负面信号 → 建议立即行动
/// - Invest:   强正面信号 → 建议投入
/// - Explore:  弱正面信号 → 建议验证
/// - Observe:  无显著变化 → 建议关注
pub fn evaluate_decisions(
    belief_updates: &[crate::belief_engine::BeliefUpdate],
    _current_decisions: &[Decision],
) -> Vec<Decision> {
    let mut decisions = Vec::new();

    let has_contradiction = belief_updates.iter().any(|u| u.is_contradiction);
    let total_delta: i8 = belief_updates.iter().map(|u| u.delta).sum();
    let challenge_count = belief_updates
        .iter()
        .filter(|u| matches!(u.evidence_type, crate::belief_engine::Stance::Challenges))
        .count();
    let support_count = belief_updates
        .iter()
        .filter(|u| matches!(u.evidence_type, crate::belief_engine::Stance::Supports))
        .count();

    // 决策逻辑（决策支持系统风格）：
    // 1. contradiction → Exit（最高优先级，有证伪信号）
    // 2. 总置信度变化 < -5 → Exit（趋势反转，建议退出）
    // 3. 总置信度变化 < -2 → Observe（谨慎观察，不急于行动）
    // 4. 总置信度变化 > 5 且支持多 → Invest（强正面信号，值得投入）
    // 5. 总置信度变化 > 2 且支持多 → Explore（正面信号，值得验证）
    // 6. 其他 → Observe（默认关注）

    if has_contradiction {
        decisions.push(Decision {
            domain: "综合".into(),
            action_type: ActionType::Exit,
            change_required: true,
            confidence: 0.8,
            actions: vec![
                "重新评估当前判断的基本假设".into(),
                "检查是否有隐藏前提被证伪".into(),
                "寻找替代解释框架".into(),
            ],
            time_horizon: "立即".into(),
            key_evidence: vec![
                format!("{} 个信念被挑战", challenge_count),
                format!("检测到证伪信号 (contradiction)"),
                format!("总置信度变化: {:+}", total_delta),
            ],
            is_high_priority: true,
        });
    } else if total_delta <= -5 {
        decisions.push(Decision {
            domain: "综合".into(),
            action_type: ActionType::Exit,
            change_required: true,
            confidence: 0.7,
            actions: vec![
                "优先审查置信度下降最快的判断".into(),
                "寻找替代假设和应对方案".into(),
                "考虑止损点".into(),
            ],
            time_horizon: "30天内".into(),
            key_evidence: vec![format!("总置信度下降: {}", -total_delta)],
            is_high_priority: true,
        });
    } else if total_delta >= 5 && support_count > challenge_count {
        decisions.push(Decision {
            domain: "综合".into(),
            action_type: ActionType::Invest,
            change_required: false,
            confidence: 0.8,
            actions: vec![
                "增加对该方向的关注和资源投入".into(),
                "制定具体的执行计划".into(),
            ],
            time_horizon: "90天内".into(),
            key_evidence: vec![format!("总置信度上升: {:+}", total_delta)],
            is_high_priority: false,
        });
    } else if total_delta >= 2 {
        decisions.push(Decision {
            domain: "综合".into(),
            action_type: ActionType::Explore,
            change_required: false,
            confidence: 0.6,
            actions: vec![
                "收集更多证据验证该方向".into(),
                "关注相关信号的变化趋势".into(),
            ],
            time_horizon: "持续".into(),
            key_evidence: vec![format!("总置信度变化: {:+}", total_delta)],
            is_high_priority: false,
        });
    } else if total_delta <= -2 {
        decisions.push(Decision {
            domain: "综合".into(),
            action_type: ActionType::Observe,
            change_required: false,
            confidence: 0.7,
            actions: vec![
                "密切关注，但暂不采取行动".into(),
                "等待更多证据再判断".into(),
            ],
            time_horizon: "持续".into(),
            key_evidence: vec![format!("总置信度变化: {:+}", total_delta)],
            is_high_priority: false,
        });
    } else {
        decisions.push(Decision {
            domain: "综合".into(),
            action_type: ActionType::Observe,
            change_required: false,
            confidence: 0.9,
            actions: vec!["维持当前关注，继续监控".into()],
            time_horizon: "持续".into(),
            key_evidence: vec![format!("总置信度变化: {:+}，无显著偏离", total_delta)],
            is_high_priority: false,
        });
    }

    decisions
}

/// 渲染行动建议到 HTML 区块
///
/// 高优先级（Exit / Execute）使用红色警告色块。
pub fn render_decision_html(decisions: &[Decision]) -> String {
    let mut html = String::from(
        "<div class=\"mt-8 pt-4 border-t border-neutral-200\">\n  \
         <span class=\"intel-label text-neutral-400 text-xs font-bold uppercase tracking-wider mb-3 block\">\n    \
         Recommendations\n  </span>\n"
    );

    for decision in decisions {
        let border_color = if decision.is_high_priority {
            "border-red-500 bg-red-50"
        } else if decision.change_required {
            "border-amber-400 bg-amber-50"
        } else {
            "border-neutral-200 bg-white"
        };

        let label = match decision.action_type {
            ActionType::Observe => "👀 Observe",
            ActionType::Explore => "🔍 Explore",
            ActionType::Invest => "💰 Invest",
            ActionType::Execute => "⚡ Execute",
            ActionType::Exit => "🚨 Exit",
        };

        let text_color = if decision.is_high_priority {
            "text-red-600"
        } else {
            "text-neutral-500"
        };

        html.push_str(&format!(
            r#"  <div class="rounded-lg p-4 mb-3 border {}">
    <div class="flex items-center justify-between mb-2">
      <span class="text-xs font-bold uppercase tracking-wider {}">{}</span>
      <span class="text-[10px] font-mono text-neutral-400">{}</span>
    </div>
    <p class="text-sm text-neutral-700 mb-2">{}</p>
    <ul class="list-disc pl-4 space-y-1">
      {}
    </ul>
  </div>
"#,
            border_color,
            text_color,
            label,
            decision.time_horizon,
            decision.domain,
            decision
                .actions
                .iter()
                .map(|a| format!("<li class=\"text-xs text-neutral-600\">{}</li>", a))
                .collect::<Vec<_>>()
                .join("\n"),
        ));
    }

    html.push_str("</div>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::belief_engine::{BeliefUpdate, Stance};

    fn make_update(delta: i8, is_contra: bool) -> BeliefUpdate {
        BeliefUpdate {
            belief_id: "b1".into(),
            delta,
            evidence_type: if delta < 0 {
                Stance::Challenges
            } else {
                Stance::Supports
            },
            reasoning: "test".into(),
            is_contradiction: is_contra,
        }
    }

    #[test]
    fn test_contradiction_triggers_exit() {
        let updates = vec![make_update(-5, true)];
        let decisions = evaluate_decisions(&updates, &[]);
        assert!(decisions.iter().any(|d| d.is_high_priority));
        assert!(decisions.iter().any(|d| d.action_type == ActionType::Exit));
    }

    #[test]
    fn test_observe_when_stable() {
        let updates = vec![make_update(2, false), make_update(1, false)];
        let decisions = evaluate_decisions(&updates, &[]);
        assert!(decisions
            .iter()
            .any(|d| d.action_type == ActionType::Explore));
    }

    #[test]
    fn test_invest_on_strong_positive() {
        let updates = vec![make_update(5, false), make_update(3, false)];
        let decisions = evaluate_decisions(&updates, &[]);
        assert!(decisions
            .iter()
            .any(|d| d.action_type == ActionType::Invest));
    }

    #[test]
    fn test_exit_on_strong_negative() {
        let updates = vec![make_update(-6, false)];
        let decisions = evaluate_decisions(&updates, &[]);
        assert!(decisions.iter().any(|d| d.action_type == ActionType::Exit));
    }

    #[test]
    fn test_render_exit_html() {
        let decisions = vec![Decision {
            domain: "半导体".into(),
            action_type: ActionType::Exit,
            change_required: true,
            confidence: 0.8,
            actions: vec!["重新评估".into()],
            time_horizon: "立即".into(),
            key_evidence: vec!["矛盾信号".into()],
            is_high_priority: true,
        }];
        let html = render_decision_html(&decisions);
        assert!(html.contains("Exit"));
        assert!(html.contains("bg-red-50"));
    }

    #[test]
    fn test_observe_on_neutral() {
        let updates = vec![make_update(0, false)];
        let decisions = evaluate_decisions(&updates, &[]);
        assert!(decisions
            .iter()
            .any(|d| d.action_type == ActionType::Observe));
    }
}
