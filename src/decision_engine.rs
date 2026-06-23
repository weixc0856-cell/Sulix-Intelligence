//! Decision Engine — "需要改变路线吗？"
//!
//! 输入: BeliefUpdate[] + 已有决策历史
//! 输出: 路线变更建议 + 行动项
//!
//! Phase 2: 基于规则的决策触发。
//! Phase 3: 升级为 LLM 驱动的战略推演。
//!
//! Code Review "不易"与"变易":
//! Decision Engine 不生产新闻，只回答一个问题：
//! "基于本次信念更新与反向矛盾点，现有的路线需要发生战略性转向吗？"
//! 如果 DecisionType == StrategicPivot，渲染器必须使用高危警告色块置顶展示。

use serde::{Deserialize, Serialize};

/// 决策类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionType {
    /// 维持现有路线
    NoChange,
    /// 微调
    CourseCorrect,
    /// 战略性转向
    StrategicPivot,
    /// 立即行动
    UrgentAction,
}

/// 决策输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// 影响的领域
    pub domain: String,
    /// 当前决策类型
    pub decision_type: DecisionType,
    /// 是否必须改变当前策略
    pub change_required: bool,
    /// 置信度 1-10
    pub confidence: u8,
    /// 具体行动项
    pub actions: Vec<String>,
    /// 时间视窗
    pub time_horizon: String,
    /// 触发该决策的关键证据
    pub key_evidence: Vec<String>,
    /// 战略性转向时为 true — 渲染器需使用高危警告色块
    pub is_strategic_pivot: bool,
}

/// 评估信念更新并生成决策
///
/// 对标兰德公司报告美学: 不生产新闻，只回答路线变更问题。
/// 决策层级:
/// - StrategicPivot: 需要生成高危警告色块置顶展示
/// - UrgentAction: 立即行动项
/// - CourseCorrect: 微调现有路线
/// - NoChange: 维持不变
pub fn evaluate_decisions(
    belief_updates: &[crate::belief_engine::BeliefUpdate],
    current_decisions: &[Decision],
) -> Vec<Decision> {
    let mut decisions = Vec::new();

    // 检查是否有任何矛盾（证伪信号）
    let has_contradiction = belief_updates.iter().any(|u| u.is_contradiction);
    let total_delta: i8 = belief_updates.iter().map(|u| u.delta).sum();
    let challenge_count = belief_updates
        .iter()
        .filter(|u| {
            matches!(
                u.evidence_type,
                crate::belief_engine::EvidenceType::Challenge
            )
        })
        .count();

    // 决策逻辑:
    // 1. 有 contradiction → StrategicPivot（最高优先级）
    // 2. 总置信度变化 < -5 → UrgentAction
    // 3. 总置信度变化 < -2 → CourseCorrect
    // 4. 其他 → NoChange

    if has_contradiction {
        decisions.push(Decision {
            domain: "综合".into(),
            decision_type: DecisionType::StrategicPivot,
            change_required: true,
            confidence: 8,
            actions: vec![
                "重新评估当前路线的基本假设".into(),
                "召开虚拟智库对抗辩论（三 Agent 重审）".into(),
                "制定情景备用方案".into(),
            ],
            time_horizon: "立即".into(),
            key_evidence: vec![
                format!("{} 个信念被挑战", challenge_count),
                format!("总置信度变化: {}", total_delta),
            ],
            is_strategic_pivot: true,
        });
    } else if total_delta <= -5 {
        decisions.push(Decision {
            domain: "综合".into(),
            decision_type: DecisionType::UrgentAction,
            change_required: true,
            confidence: 7,
            actions: vec![
                "优先处理置信度下降最快的信念".into(),
                "寻找替代假设和应对方案".into(),
            ],
            time_horizon: "30天内".into(),
            key_evidence: vec![format!("总置信度下降: {}", -total_delta)],
            is_strategic_pivot: false,
        });
    } else if total_delta <= -2 {
        decisions.push(Decision {
            domain: "相关领域".into(),
            decision_type: DecisionType::CourseCorrect,
            change_required: true,
            confidence: 6,
            actions: vec![
                "微调关注领域和信号权重".into(),
                "增加对挑战性信号的监控频率".into(),
            ],
            time_horizon: "90天内".into(),
            key_evidence: vec![format!("总置信度变化: {:+}", total_delta)],
            is_strategic_pivot: false,
        });
    } else {
        decisions.push(Decision {
            domain: "无特定领域".into(),
            decision_type: DecisionType::NoChange,
            change_required: false,
            confidence: 9,
            actions: vec!["维持现有路线，继续监控".into()],
            time_horizon: "不适用".into(),
            key_evidence: vec![format!("总置信度变化: {:+}，无显著矛盾", total_delta)],
            is_strategic_pivot: false,
        });
    }

    // 合并已有决策的上下文
    for decision in &mut decisions {
        if let Some(existing) = current_decisions
            .iter()
            .find(|d| d.domain == decision.domain)
        {
            if existing.is_strategic_pivot {
                decision.decision_type = DecisionType::StrategicPivot;
                decision.change_required = true;
                decision.is_strategic_pivot = true;
            }
        }
    }

    decisions
}

/// 渲染决策到 HTML 区块
///
/// 如果 is_strategic_pivot == true，使用高危警告色块置顶展示。
pub fn render_decision_html(decisions: &[Decision]) -> String {
    let mut html = String::from(
        "<div class=\"mt-8 pt-4 border-t border-neutral-200\">\n  \
         <span class=\"intel-label text-neutral-400 text-xs font-bold uppercase tracking-wider mb-3 block\">\n    \
         Decision Required\n  </span>\n"
    );

    for decision in decisions {
        let border_color = if decision.is_strategic_pivot {
            "border-red-500 bg-red-50"
        } else if decision.change_required {
            "border-amber-400 bg-amber-50"
        } else {
            "border-neutral-200 bg-white"
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
            if decision.is_strategic_pivot {
                "text-red-600"
            } else {
                "text-neutral-500"
            },
            match decision.decision_type {
                DecisionType::StrategicPivot => "🔴 Strategic Pivot Required",
                DecisionType::UrgentAction => "🟠 Urgent Action Required",
                DecisionType::CourseCorrect => "🟡 Course Correct",
                DecisionType::NoChange => "✅ No Change Needed",
            },
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
    use crate::belief_engine::{BeliefUpdate, EvidenceType};

    fn make_update(delta: i8, is_contra: bool) -> BeliefUpdate {
        BeliefUpdate {
            belief_id: "b1".into(),
            delta,
            evidence_type: if delta < 0 {
                EvidenceType::Challenge
            } else {
                EvidenceType::Support
            },
            reasoning: "test".into(),
            is_contradiction: is_contra,
        }
    }

    #[test]
    fn test_contradiction_triggers_strategic_pivot() {
        let updates = vec![make_update(-5, true)];
        let decisions = evaluate_decisions(&updates, &[]);
        assert!(decisions.iter().any(|d| d.is_strategic_pivot));
    }

    #[test]
    fn test_no_change_when_stable() {
        let updates = vec![make_update(2, false), make_update(1, false)];
        let decisions = evaluate_decisions(&updates, &[]);
        assert!(decisions
            .iter()
            .any(|d| matches!(d.decision_type, DecisionType::NoChange)));
    }

    #[test]
    fn test_render_strategic_pivot_html() {
        let decisions = vec![Decision {
            domain: "半导体".into(),
            decision_type: DecisionType::StrategicPivot,
            change_required: true,
            confidence: 8,
            actions: vec!["重新评估".into()],
            time_horizon: "立即".into(),
            key_evidence: vec!["矛盾信号".into()],
            is_strategic_pivot: true,
        }];
        let html = render_decision_html(&decisions);
        assert!(html.contains("Strategic Pivot"));
        assert!(html.contains("bg-red-50"));
    }
}
