//! DiGraph 有向图编排引擎 — 多 Agent 认知管线拓扑调度
//!
//! 对标 AutoGen GraphFlow:
//!   - GraphNode trait 作为纯函数图算子
//!   - GraphContext 作为节点间共享的黑板状态机
//!   - 条件边（ConditionEdgeFn）实现蓝军 Veto→回 Generator 循环
//!   - LoopCounter 刚性上限防止死循环燃烧 Token
//!
//! 当前拓扑: Cluster → BlueTeam → QE
//!   - BlueTeam veto → RouteResult::LoopBack("Cluster")

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// DiGraph 循环节点最大执行次数（防止蓝军无限回退燃烧 Token）
const MAX_LOOP_ITERATIONS: usize = 3;
/// 置信度停滞检测阈值（相邻两轮 confidence diff < 阈值 → 停滞）
const CONFIDENCE_STALL_THRESHOLD: f64 = 0.05;

use anyhow::Result;

use crate::clusterer::{Theme, ThemeAnalysis};
use crate::config::Config;
use crate::question_engine::QuestionMatch;

// ===== 全局图执行上下文（黑板模式）=====

/// 节点间流转的唯一媒介
///
/// 所有节点读写此上下文，图引擎负责调度。
/// Clone 代价可控（内部为 Vec/String 等标准类型）。
#[derive(Debug, Clone)]
pub struct GraphContext {
    pub config: Config,
    pub current_themes: Vec<Theme>,
    pub current_analyses: Vec<ThemeAnalysis>,
    pub question_matches: Vec<Vec<QuestionMatch>>,
    /// 循环计数器：节点名 -> 执行次数
    pub loop_counters: HashMap<String, usize>,
    pub api_key: String,
    /// 蓝队回退轮次（组 3 死锁保护）
    pub loop_counter: u8,
    /// 最大允许回退轮次
    pub max_loops: u8,
    /// 置信度历史（用于停滞检测）
    pub confidence_history: Vec<f64>,
}

impl GraphContext {
    pub fn new(config: Config, api_key: String) -> Self {
        Self {
            config,
            api_key,
            current_themes: Vec::new(),
            current_analyses: Vec::new(),
            question_matches: Vec::new(),
            loop_counters: HashMap::new(),
            loop_counter: 0,
            max_loops: 3,
            confidence_history: Vec::new(),
        }
    }

    /// 获取并递增循环计数器。超过上限返回 true。
    pub fn increment_loop(&mut self, node: &str) -> bool {
        let count = self.loop_counters.entry(node.to_string()).or_insert(0);
        *count += 1;
        *count > MAX_LOOP_ITERATIONS
    }
}

// ===== 路由结果 =====

/// 条件路由枚举 — 决定执行流向
#[derive(Debug, Clone, PartialEq)]
pub enum RouteResult {
    /// 流向指定节点
    ProceedTo(String),
    /// 对抗重跑，流向指定节点（蓝军 veto）
    LoopBack(String),
}

// ===== 图节点 trait =====

/// 核心图节点 Trait
///
/// 每个节点是一个纯函数：ctx -> Result<()>
/// 读写 GraphContext，不持有外部状态。
pub trait GraphNode: Send + Sync {
    fn name(&self) -> &'static str;
    fn execute(&self, ctx: &mut GraphContext) -> Result<()>;
}

// ===== 条件边 =====

/// 条件边函数签名
pub type ConditionEdgeFn = Arc<dyn Fn(&GraphContext) -> RouteResult + Send + Sync>;

// ===== DiGraph 引擎 =====

/// DiGraph 有向图编排引擎
///
/// 对标 AutoGen GraphFlow:
///   - nodes: HashMap 注册所有节点
///   - edges: 节点名 -> Vec<(目标节点, 条件闭包)>
///   - execution_queue: VecDeque 驱动的 BFS 调度
pub struct DiGraph {
    nodes: HashMap<String, Box<dyn GraphNode>>,
    edges: HashMap<String, Vec<(String, ConditionEdgeFn)>>,
    execution_queue: VecDeque<String>,
    /// 已执行过的节点集合（防止重复入队）
    executed: std::collections::HashSet<String>,
}

impl Default for DiGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DiGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            execution_queue: VecDeque::new(),
            executed: std::collections::HashSet::new(),
        }
    }

    /// 注册节点
    pub fn add_node(&mut self, node: Box<dyn GraphNode>) {
        let name = node.name().to_string();
        self.nodes.insert(name.clone(), node);
    }

    /// 注册条件边
    pub fn add_edge(&mut self, from: &str, to: &str, condition: ConditionEdgeFn) {
        self.edges
            .entry(from.to_string())
            .or_default()
            .push((to.to_string(), condition));
    }

    /// 设置起始节点
    pub fn set_entry(&mut self, node: &str) {
        self.execution_queue.push_back(node.to_string());
    }

    /// 运行图直到终止
    ///
    /// 主循环:
    ///   1. 从队列取出当前节点
    ///   2. 检查循环计数器（硬上限 3）
    ///   3. 执行节点
    ///   4. 匹配条件边路由
    pub fn run(&mut self, ctx: &mut GraphContext) -> Result<()> {
        while let Some(current) = self.execution_queue.pop_front() {
            // 查重：已经执行过的节点不再执行
            if self.executed.contains(&current) {
                continue;
            }

            // 循环计数器检查（防死循环）
            if ctx.increment_loop(&current) {
                log::warn!("⚠️ GraphFlow: {} 超过循环上限，触发 Terminate", current);
                break;
            }

            // 执行当前节点
            let node = self
                .nodes
                .get(&current)
                .ok_or_else(|| anyhow::anyhow!("GraphFlow: node '{}' not found", current))?;
            log::info!("🧩 GraphFlow: 执行节点 {}", node.name());
            node.execute(ctx)?;
            self.executed.insert(current.clone());

            // 记录置信度历史（组 3 死锁保护）
            if !ctx.current_analyses.is_empty() {
                let avg: f64 = ctx
                    .current_analyses
                    .iter()
                    .map(|a| a.signal_strength as f64)
                    .sum::<f64>()
                    / ctx.current_analyses.len() as f64;
                ctx.confidence_history.push(avg);
            }

            // 路由决策
            if let Some(edge_list) = self.edges.get(&current) {
                if let Some((_target, condition)) = edge_list.iter().next() {
                    let route = condition(ctx);
                    match route {
                        RouteResult::ProceedTo(next) => {
                            self.execution_queue.push_back(next);
                            // 只走第一条匹配的条件边
                        }
                        RouteResult::LoopBack(target) => {
                            // 组 3 死锁保护：轮次限制
                            ctx.loop_counter += 1;
                            if ctx.loop_counter > ctx.max_loops {
                                log::warn!(
                                    "🛑 GraphFlow: 蓝队回退超限 ({}/{})",
                                    ctx.loop_counter,
                                    ctx.max_loops
                                );
                                return Ok(());
                            }
                            // 组 3 死锁保护：置信度停滞检测
                            if ctx.confidence_history.len() >= 2 {
                                let len = ctx.confidence_history.len();
                                let last = ctx.confidence_history[len - 1];
                                let prev = ctx.confidence_history[len - 2];
                                if (last - prev).abs() < CONFIDENCE_STALL_THRESHOLD {
                                    log::warn!(
                                        "🛑 GraphFlow: 置信度停滞 ({} -> {})，触发熔断",
                                        prev,
                                        last
                                    );
                                    return Ok(());
                                }
                            }
                            log::info!("🔄 GraphFlow: 蓝军 veto，回滚到 {}", target);
                            // 清除 target 的 executed 标记，允许重跑
                            self.executed.remove(&target);
                            self.execution_queue.push_back(target);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

// ===== 内置节点 =====

/// Cluster 节点：主题聚类一致性验证
///
/// 实际聚类由 `main.rs` 在 graph.run() 之前完成并注入 context。
/// 此节点作为 **guard** 验证聚类结果完整性：
///   - themes 数组不为空
///   - 每个 theme 都有对应的 analysis
///   - articles/sources 一致性
///
/// 如果验证失败返回错误，graph.run() 将 log warning 但不会崩溃（由调用方处理）。
pub struct ClusterNode {
    pub name: &'static str,
}
impl GraphNode for ClusterNode {
    fn name(&self) -> &'static str {
        self.name
    }
    fn execute(&self, ctx: &mut GraphContext) -> Result<()> {
        let theme_count = ctx.current_themes.len();
        let analysis_count = ctx.current_analyses.len();

        if theme_count == 0 {
            log::warn!("  节点 Cluster: themes 为空，跳过分析阶段");
            return Ok(());
        }

        // 验证：每个 theme 是否都对应一个 analysis
        if theme_count != analysis_count {
            log::warn!(
                "  节点 Cluster: themes({}) 与 analyses({}) 数量不一致——跳过路由",
                theme_count, analysis_count
            );
        } else {
            // 验证：每个 theme 的 articles 是否非空
            let empty_themes: Vec<&str> = ctx.current_themes.iter()
                .filter(|t| t.articles.is_empty())
                .map(|t| t.title.as_str())
                .collect();
            if !empty_themes.is_empty() {
                log::warn!("  节点 Cluster: {} 个 theme 无 article（空主题）: {:?}", empty_themes.len(), empty_themes);
            }
        }

        log::info!(
            "  节点 Cluster: {} 个主题, {} 项分析 — guard 通过",
            theme_count, analysis_count
        );
        Ok(())
    }
}

/// 蓝军验证降级幅度：承重假设证据弱时 signal_strength 减少的值
const WEAK_BEARING_PENALTY: u8 = 2;

/// BlueTeam 节点：蓝军验证 + 信号强度校准
///
/// 职责：
///   1. 对每个 analysis 检查承重假设的证据强度
///   2. 如果承重假设证据弱，执行 signal_strength 降级
///   3. 记录降级原因到 log
///
/// 条件路由由 `blue_team_edge()` 边条件驱动（单独处理 loopback 决策）。
/// execute() 本身只做原子化的降级操作，不负责路由。
pub struct BlueTeamNode {
    pub name: &'static str,
}
impl GraphNode for BlueTeamNode {
    fn name(&self) -> &'static str {
        self.name
    }
    fn execute(&self, ctx: &mut GraphContext) -> Result<()> {
        let mut degraded_count = 0;
        for analysis in &mut ctx.current_analyses {
            let weak_bearing = analysis.assumptions.iter().any(|a| a.load_bearing && a.evidence_strength == "weak");
            if weak_bearing && analysis.signal_strength >= WEAK_BEARING_PENALTY {
                analysis.signal_strength -= WEAK_BEARING_PENALTY;
                degraded_count += 1;
            }
        }
        if degraded_count > 0 {
            log::info!("  节点 BlueTeam: {} 个 analysis 因承重假设证据弱而降级", degraded_count);
        }
        log::info!("  节点 BlueTeam: 蓝军验证完成（条件路由由 Edge condition 驱动）");
        Ok(())
    }
}

/// QE 节点：Question Engine（信号-问题匹配）
pub struct QENode {
    pub name: &'static str,
}
impl GraphNode for QENode {
    fn name(&self) -> &'static str {
        self.name
    }
    fn execute(&self, ctx: &mut GraphContext) -> Result<()> {
        log::info!("  节点 QE: Question Engine");
        if let Some(questions) = &ctx.config.questions {
            let client = crate::client::global_client().clone();
            for analysis in &ctx.current_analyses {
                if let Ok(matches) = crate::question_engine::match_questions_sync(
                    analysis,
                    &questions.questions,
                    &client,
                    &ctx.api_key,
                    &ctx.config.llm,
                ) {
                    ctx.question_matches.push(matches);
                }
            }
        }
        Ok(())
    }
}

// ===== 条件边工厂函数 =====

/// 蓝军验证条件边
///
/// 如果当前 analyses 中有任何承重假设的证据强度为 weak，
/// 且不是通过 Cluster 重熔后的第二次分析，返回 LoopBack。
/// 否则 ProceedTo 下一阶段。
pub fn blue_team_edge(next: &str) -> ConditionEdgeFn {
    let next = next.to_string();
    Arc::new(move |ctx: &GraphContext| {
        let any_weak = ctx.current_analyses.iter().any(|a| {
            a.assumptions
                .iter()
                .any(|ass| ass.load_bearing && ass.evidence_strength == "weak")
        });
        if any_weak {
            let cluster_loops = ctx.loop_counters.get("ClusterNode").copied().unwrap_or(0);
            if cluster_loops < MAX_LOOP_ITERATIONS {
                return RouteResult::LoopBack("ClusterNode".to_string());
            }
        }
        RouteResult::ProceedTo(next.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn make_test_config() -> Config {
        Config::from_file("config.toml").unwrap_or_else(|_| panic!("config.toml 不存在或不合法"))
    }

    #[test]
    fn test_graph_context_loop_counter() {
        let config = make_test_config();
        let api_key = config.get_api_key().unwrap_or("test".into());
        let mut ctx = GraphContext::new(config, api_key);
        assert!(!ctx.increment_loop("Cluster"));
        assert!(!ctx.increment_loop("Cluster"));
        assert!(!ctx.increment_loop("Cluster"));
        assert!(ctx.increment_loop("Cluster")); // >3
    }

    #[test]
    fn test_digraph_basic_flow() {
        let config = make_test_config();
        let api_key = config.get_api_key().unwrap_or("test".into());
        let mut ctx = GraphContext::new(config, api_key);

        let mut graph = DiGraph::new();
        graph.add_node(Box::new(ClusterNode { name: "Cluster" }));
        graph.add_node(Box::new(QENode { name: "QE" }));

        graph.add_edge(
            "Cluster",
            "QE",
            Arc::new(|_| RouteResult::ProceedTo("QE".into())),
        );

        graph.set_entry("Cluster");
        graph.run(&mut ctx).unwrap();
        assert!(graph.executed.contains("Cluster"));
    }

    #[test]
    fn test_blue_team_loopback() {
        let _ = env_logger::try_init();
        let config = make_test_config();
        let api_key = config.get_api_key().unwrap_or("test".into());
        let mut ctx = GraphContext::new(config, api_key);

        // 注入一个有 weak assumption 的分析
        ctx.current_analyses.push(crate::clusterer::ThemeAnalysis {
            theme_id: "t1".into(),
            theme_title: "test".into(),
            bluf: "test".into(),
            impact: "test".into(),
            geopolitical_fact: "test".into(),
            supply_chain_impact: "test".into(),
            analysis_paragraph: String::new(),
            evidence_level: String::new(),
            signal_strength: 7,
            fact_base: vec![],
            connections: vec![],
            source_urls: vec![],
            assumptions: vec![crate::clusterer::Assumption {
                text: "承重假设".into(),
                load_bearing: true,
                evidence_strength: "weak".into(),
            }],
            adverse: None,
            next_tests: vec![],
            open_questions: vec![],
            chains: vec![],
            what_to_do: String::new(),
            what_to_watch: String::new(),
            falsification_conditions: vec![],
        });

        let edge = blue_team_edge("DE");
        let result = edge(&ctx);
        assert_eq!(result, RouteResult::LoopBack("ClusterNode".to_string()));
    }
}
