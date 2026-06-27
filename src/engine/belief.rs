//! Belief Engine Phase B — WayneOPC 信念系统
//!
//! 核心信念来自 WaynOPC lens，每个信念附带：
//! - 当前置信度 0-100
//! - 支持/挑战证据列表
//! - 置信度变化历史
//!
//! 与 belief_engine.rs 的区别：
//!   belief_engine.rs 是 DiGraph 中的轻量规则引擎（计数）
//!   此模块是更丰富的信念追踪，支持历史记录和趋势

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 单条核心信念
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreBelief {
    /// 信念 ID (B1-B10)
    pub id: String,
    /// 信念陈述（如 "AI应用大于模型"）
    pub statement: String,
    /// 当前置信度 0-100
    pub confidence: u8,
    /// 所属类别
    pub category: String,
    /// 置信度变化历史
    #[serde(default)]
    pub history: Vec<ConfidencePoint>,
}

/// 置信度时间点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidencePoint {
    pub date: String,
    pub confidence: u8,
    pub reason: String,
    pub delta: i8,
}

/// 信念引擎
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BeliefEngineV2 {
    pub beliefs: HashMap<String, CoreBelief>,
}

impl BeliefEngineV2 {
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 config 加载信念
    pub fn load_from_config(&mut self, config_beliefs: &[CoreBelief]) {
        for cb in config_beliefs {
            self.beliefs.insert(cb.id.clone(), cb.clone());
        }
    }

    /// 应用置信度更新
    pub fn apply_update(&mut self, belief_id: &str, delta: i8, reason: &str, date: &str) {
        if let Some(belief) = self.beliefs.get_mut(belief_id) {
            let old_confidence = belief.confidence;
            let new_confidence = (old_confidence as i16 + delta as i16).clamp(0, 100) as u8;
            belief.confidence = new_confidence;
            belief.history.push(ConfidencePoint {
                date: date.to_string(),
                confidence: new_confidence,
                reason: reason.to_string(),
                delta,
            });
        }
    }

    /// 获取最近 N 条更新
    pub fn recent_changes(&self, n: usize) -> Vec<(&CoreBelief, Option<&ConfidencePoint>)> {
        let mut result: Vec<(&CoreBelief, Option<&ConfidencePoint>)> = self
            .beliefs
            .values()
            .filter_map(|b| {
                let last = b.history.last();
                if last.is_some() {
                    Some((b, last))
                } else {
                    None
                }
            })
            .collect();
        result.sort_by(|a, b| {
            b.1.map(|p| p.date.as_str())
                .unwrap_or("")
                .cmp(a.1.map(|p| p.date.as_str()).unwrap_or(""))
        });
        result.truncate(n);
        result
    }

    /// 从分析结果更新信念
    pub fn update_from_analyses(
        &mut self,
        analyses: &[crate::domain::theme::ThemeAnalysis],
        today: &str,
    ) {
        for analysis in analyses {
            let text = format!("{} {}", analysis.bluf, analysis.impact).to_lowercase();
            for belief in self.beliefs.clone().values() {
                let keywords: Vec<&str> = belief.statement.split_whitespace().collect();
                let matches = keywords
                    .iter()
                    .filter(|kw| text.contains(&kw.to_lowercase()))
                    .count();
                if matches >= 2 {
                    let delta = if analysis.signal_strength >= 7 { 5 } else { 2 };
                    self.apply_update(&belief.id, delta, &analysis.bluf, today);
                }
            }
        }
    }
}

/// 渲染信念变化为 HTML 区块
pub(crate) fn render_belief_changes_html(engine: &BeliefEngineV2) -> String {
    let changes = engine.recent_changes(5);
    if changes.is_empty() {
        return String::new();
    }

    let mut html = String::from(
        r#"<div style="margin-top:1rem;padding:0.75rem;background:#fafafa;border-radius:0.25rem;border-left:3px solid #2563eb">
  <div style="font-family:'JetBrains Mono',monospace;font-size:0.75rem;font-weight:700;text-transform:uppercase;letter-spacing:0.05em;color:#171717;margin-bottom:0.5rem">🎯 信念更新 (Belief Engine)</div>"#,
    );

    for (belief, point) in &changes {
        let delta_str = if let Some(pt) = point {
            if pt.delta > 0 {
                format!("+{}", pt.delta)
            } else {
                pt.delta.to_string()
            }
        } else {
            "0".into()
        };
        let color = if let Some(pt) = point {
            if pt.delta > 0 {
                "#16a34a"
            } else {
                "#dc2626"
            }
        } else {
            "#a3a3a3"
        };

        html.push_str(&format!(
            r#"<div style="display:flex;align-items:flex-start;gap:0.375rem;padding:0.25rem 0;border-bottom:1px solid #f0f0f0">
  <span style="font-family:'JetBrains Mono',monospace;font-size:0.625rem;color:#a3a3a3;min-width:2rem">{}</span>
  <div style="flex:1">
    <div style="font-size:0.75rem;color:#171717">{}</div>
    <div style="font-size:0.6875rem;color:#737373">{}</div>
  </div>
  <span style="font-family:'JetBrains Mono',monospace;font-size:0.75rem;font-weight:600;color:{}">{}</span>
</div>"#,
            belief.id,
            belief.statement,
            point.map(|p| p.reason.as_str()).unwrap_or(""),
            color,
            delta_str,
        ));
    }

    html.push_str("</div>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_belief_engine_new() {
        let engine = BeliefEngineV2::new();
        assert!(engine.beliefs.is_empty());
    }

    #[test]
    fn test_load_from_config() {
        let mut engine = BeliefEngineV2::new();
        let beliefs = vec![CoreBelief {
            id: "B1".into(),
            statement: "AI应用大于模型".into(),
            confidence: 75,
            category: "AI".into(),
            history: vec![],
        }];
        engine.load_from_config(&beliefs);
        assert_eq!(engine.beliefs.len(), 1);
        assert_eq!(engine.beliefs["B1"].confidence, 75);
    }

    #[test]
    fn test_apply_update() {
        let mut engine = BeliefEngineV2::new();
        engine.load_from_config(&[CoreBelief {
            id: "B1".into(),
            statement: "test".into(),
            confidence: 50,
            category: "T".into(),
            history: vec![],
        }]);
        engine.apply_update("B1", 10, "new evidence", "2026-06-25");
        assert_eq!(engine.beliefs["B1"].confidence, 60);
        assert_eq!(engine.beliefs["B1"].history.len(), 1);
    }

    #[test]
    fn test_recent_changes() {
        let mut engine = BeliefEngineV2::new();
        engine.load_from_config(&[
            CoreBelief {
                id: "B1".into(),
                statement: "test1".into(),
                confidence: 50,
                category: "T".into(),
                history: vec![],
            },
            CoreBelief {
                id: "B2".into(),
                statement: "test2".into(),
                confidence: 50,
                category: "T".into(),
                history: vec![],
            },
        ]);
        engine.apply_update("B1", 5, "ev", "2026-06-25");
        let changes = engine.recent_changes(5);
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_render_html_empty() {
        let engine = BeliefEngineV2::new();
        assert_eq!(render_belief_changes_html(&engine), "");
    }
}
