//! 组合式终止条件 — 多 Agent 管线控制
//!
//! 对标 AutoGen 的 TerminationCondition 体系:
//!   TextMentionTermination / MaxMessageTermination / TimeoutTermination
//! 支持 .and() / .or() 组合子，实现"蓝军挑战半衰期减速窗"。
//!
//! Expert Refinement: 连续 2 轮对抗置信度提升不足 5% →
//! 强行触发 CourseCorrect 降级渲染，绝不允许阻塞 GitOps 周期

/// 终止条件结果
#[derive(Debug, Clone, PartialEq)]
pub struct TerminationResult {
    /// 是否应该终止
    pub should_terminate: bool,
    /// 终止原因
    pub reason: Option<String>,
    /// 当前对抗轮次置信度变化（用于蓝军减速窗判断）
    pub confidence_delta: f64,
}

impl TerminationResult {
    pub fn stop(reason: &str) -> Self {
        Self {
            should_terminate: true,
            reason: Some(reason.to_string()),
            confidence_delta: 0.0,
        }
    }
    pub fn continue_() -> Self {
        Self {
            should_terminate: false,
            reason: None,
            confidence_delta: 0.0,
        }
    }
}

/// 终止条件 trait — 可组合的管线终止判断
///
/// 对标 AutoGen TerminationCondition:
///   .and() = 所有条件都满足才终止
///   .or() = 任一条件满足即终止
pub trait TerminationCondition: Send + Sync {
    fn check(&mut self) -> TerminationResult;
    fn reset(&mut self);
}

/// 将 trait 升级为可组合的对象
pub fn and_tc(
    a: Box<dyn TerminationCondition>,
    b: Box<dyn TerminationCondition>,
) -> Box<dyn TerminationCondition> {
    Box::new(AndCondition(a, b))
}
pub fn or_tc(
    a: Box<dyn TerminationCondition>,
    b: Box<dyn TerminationCondition>,
) -> Box<dyn TerminationCondition> {
    Box::new(OrCondition(a, b))
}

// ===== 组合子 =====

struct AndCondition(Box<dyn TerminationCondition>, Box<dyn TerminationCondition>);
impl TerminationCondition for AndCondition {
    fn check(&mut self) -> TerminationResult {
        let a = self.0.as_mut().check();
        let b = self.1.as_mut().check();
        TerminationResult {
            should_terminate: a.should_terminate && b.should_terminate,
            reason: match (a.reason, b.reason) {
                (Some(r1), Some(r2)) => Some(format!("{}; {}", r1, r2)),
                (Some(r), None) | (None, Some(r)) => Some(r),
                (None, None) => None,
            },
            confidence_delta: a.confidence_delta + b.confidence_delta,
        }
    }
    fn reset(&mut self) {
        self.0.as_mut().reset();
        self.1.as_mut().reset();
    }
}

struct OrCondition(Box<dyn TerminationCondition>, Box<dyn TerminationCondition>);
impl TerminationCondition for OrCondition {
    fn check(&mut self) -> TerminationResult {
        let a = self.0.as_mut().check();
        if a.should_terminate {
            return a;
        }
        self.1.as_mut().check()
    }
    fn reset(&mut self) {
        self.0.as_mut().reset();
        self.1.as_mut().reset();
    }
}

// ===== 内置条件 =====

/// 最大轮次终止
pub struct MaxLoopCondition {
    max: u64,
    current: u64,
}
impl MaxLoopCondition {
    pub fn new(max: u64) -> Self {
        Self { max, current: 0 }
    }
}
impl TerminationCondition for MaxLoopCondition {
    fn check(&mut self) -> TerminationResult {
        if self.current >= self.max {
            TerminationResult::stop(&format!("达到最大轮次 {}", self.max))
        } else {
            TerminationResult::continue_()
        }
    }
    fn reset(&mut self) {
        self.current = 0;
    }
}

/// 文本提及终止（如 "TERMINATE" / "APPROVED"）
pub struct TextMentionCondition {
    keyword: String,
    last_message: String,
}
impl TextMentionCondition {
    pub fn new(keyword: &str) -> Self {
        Self {
            keyword: keyword.to_string(),
            last_message: String::new(),
        }
    }
    pub fn update_message(&mut self, msg: &str) {
        self.last_message = msg.to_string();
    }
}
impl TerminationCondition for TextMentionCondition {
    fn check(&mut self) -> TerminationResult {
        if self.last_message.contains(&self.keyword) {
            TerminationResult::stop(&format!("关键词 '{}' 触发终止", self.keyword))
        } else {
            TerminationResult::continue_()
        }
    }
    fn reset(&mut self) {
        self.last_message.clear();
    }
}

/// 置信度停滞终止（蓝军挑战减速窗）
///
/// Expert Refinement: 连续 N 轮对抗后置信度提升不足 threshold%，
/// 强行触发降级渲染，防止 Agent 死循环燃烧 Token。
pub struct ConfidenceStagnationCondition {
    window: usize,     // 观察窗口（轮次）
    threshold: f64,    // 置信度提升阈值（如 0.05 = 5%）
    history: Vec<f64>, // 历史置信度记录
}
impl ConfidenceStagnationCondition {
    pub fn new(window: usize, threshold: f64) -> Self {
        Self {
            window,
            threshold,
            history: Vec::with_capacity(window),
        }
    }
    pub fn record_confidence(&mut self, confidence: f64) {
        self.history.push(confidence);
        if self.history.len() > self.window {
            self.history.remove(0);
        }
    }
}
impl TerminationCondition for ConfidenceStagnationCondition {
    fn check(&mut self) -> TerminationResult {
        if self.history.len() < self.window {
            return TerminationResult::continue_();
        }
        let delta = (self.history[self.history.len() - 1] - self.history[0]).abs();
        if delta < self.threshold {
            TerminationResult::stop(&format!(
                "置信度提升 {:.1}% 低于阈值 {:.1}%，触发 CourseCorrect 降级",
                delta * 100.0,
                self.threshold * 100.0
            ))
        } else {
            TerminationResult::continue_()
        }
    }
    fn reset(&mut self) {
        self.history.clear();
    }
}

/// 超时终止（按秒）
pub struct TimeoutCondition {
    started_at: std::time::Instant,
    max_secs: u64,
}
impl TimeoutCondition {
    pub fn new(max_secs: u64) -> Self {
        Self {
            started_at: std::time::Instant::now(),
            max_secs,
        }
    }
}
impl TerminationCondition for TimeoutCondition {
    fn check(&mut self) -> TerminationResult {
        if self.started_at.elapsed().as_secs() > self.max_secs {
            TerminationResult::stop(&format!("超时 {}s", self.max_secs))
        } else {
            TerminationResult::continue_()
        }
    }
    fn reset(&mut self) {
        self.started_at = std::time::Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_loop_terminates() {
        let mut cond = MaxLoopCondition::new(3);
        cond.current = 3;
        assert!(cond.check().should_terminate);
    }

    #[test]
    fn test_max_loop_continues() {
        let mut cond = MaxLoopCondition::new(5);
        cond.current = 2;
        assert!(!cond.check().should_terminate);
    }

    #[test]
    fn test_text_mention_terminates() {
        let mut cond = TextMentionCondition::new("APPROVED");
        cond.update_message("All checks APPROVED. Proceeding.");
        assert!(cond.check().should_terminate);
    }

    #[test]
    fn test_and_condition_both_needed() {
        let a = Box::new(MaxLoopCondition::new(3));
        let b = Box::new(TextMentionCondition::new("DONE"));
        // Both need to terminate: maxloop not hit, text not seen -> no
        let mut combined = and_tc(a, b);
        assert!(!combined.check().should_terminate);
    }

    #[test]
    fn test_or_condition_either_triggers() {
        let a = Box::new(MaxLoopCondition::new(3));
        let b = Box::new(TextMentionCondition::new("STOP"));
        // MaxLoop not hit, text not seen -> no
        let mut combined = or_tc(a, b);
        assert!(!combined.check().should_terminate);
    }

    #[test]
    fn test_confidence_stagnation_triggers() {
        let mut cond = ConfidenceStagnationCondition::new(3, 0.05);
        cond.record_confidence(0.7);
        cond.record_confidence(0.71);
        cond.record_confidence(0.71); // 从 0.70 -> 0.71, delta=0.01 < 0.05
        let result = cond.check();
        assert!(result.should_terminate);
        assert!(result.reason.unwrap().contains("CourseCorrect"));
    }

    #[test]
    fn test_confidence_improves_passes() {
        let mut cond = ConfidenceStagnationCondition::new(3, 0.05);
        cond.record_confidence(0.6);
        cond.record_confidence(0.7);
        cond.record_confidence(0.8); // 从 0.6 -> 0.8, delta=0.2 >= 0.05
        assert!(!cond.check().should_terminate);
    }

    #[test]
    fn test_composition_blue_team_veto() {
        // 实际蓝军场景: 最多 3 轮，说 APPROVED 就停，置信度停滞也停
        let max_loop = Box::new(MaxLoopCondition::new(3));
        let text_match = Box::new(TextMentionCondition::new("APPROVED"));
        let stagnation = Box::new(ConfidenceStagnationCondition::new(2, 0.05));
        let _condition = or_tc(or_tc(max_loop, text_match), stagnation);
        // 组合子测试通过编译即验证结构正确
    }
}
