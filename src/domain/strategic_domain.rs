//! Strategic Domain Taxonomy — 固定情报战略领域分类
//!
//! Sulix 的所有 Canonical Object 共享同一套 StrategicDomain 分类体系。
//! LLM 只能**映射**到已有领域，不能自由生成新分类。
//!
//! 认知定位：
//!   StrategicDomain 是情报的组织维度，不是内容的"标签"。
//!   它回答的问题是："这个判断属于哪个战略领域？"
//!
//! 约束：
//!   - Taxonomy 是 fixed 的（新增领域需改代码，不能由 LLM 动态创建）
//!   - 每个 Canonical Object 必须有一个 primary_domain
//!   - 可以有多个 secondary_domains（跨领域问题）
//!   - Domain 在 Object 生命周期内不变
//!
//! 领域覆盖：
//!   AI · 半导体 · 航天 · 机器人 · 国防 · 能源 · 宏观 · 医疗

use serde::{Deserialize, Serialize};

/// 固定 Taxonomy — 情报战略领域
///
/// LLM 提示词约束：你必须从以下领域中选择最匹配的一个。
/// 如果无法确定，选择 Other。允许选择多个 secondary 领域。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum StrategicDomain {
    /// 人工智能 — LLM、Agent、模型能力、AI 政策、AI 创业
    #[serde(rename = "ai")]
    AI,
    /// 半导体 — 芯片、制程、GPU、出口管制、Foundry
    #[serde(rename = "semiconductor")]
    Semiconductor,
    /// 航天 — 火箭、卫星、Starlink、NASA、SpaceX
    #[serde(rename = "space")]
    Space,
    /// 机器人 — 人形机器人、自动驾驶、具身智能
    #[serde(rename = "robotics")]
    Robotics,
    /// 国防 — 军事技术、地缘冲突、国防采购
    #[serde(rename = "defense")]
    Defense,
    /// 能源 — 电力、数据中心能源、核能、新能源
    #[serde(rename = "energy")]
    Energy,
    /// 宏观 — 宏观经济、贸易政策、货币政策、地缘经济
    #[serde(rename = "macro")]
    Macro,
    /// 医疗 — 生物技术、制药、医疗 AI
    #[serde(rename = "healthcare")]
    Healthcare,
    /// 其他 — 无法归入以上任何领域
    #[default]
    #[serde(rename = "other")]
    Other,
}

impl StrategicDomain {
    /// 人类可读标签
    pub fn label(&self) -> &'static str {
        match self {
            StrategicDomain::AI => "AI",
            StrategicDomain::Semiconductor => "Semiconductor",
            StrategicDomain::Space => "Space",
            StrategicDomain::Robotics => "Robotics",
            StrategicDomain::Defense => "Defense",
            StrategicDomain::Energy => "Energy",
            StrategicDomain::Macro => "Macro",
            StrategicDomain::Healthcare => "Healthcare",
            StrategicDomain::Other => "Other",
        }
    }

    /// Emoji 图标（前端展示用）
    pub fn emoji(&self) -> &'static str {
        match self {
            StrategicDomain::AI => "🧠",
            StrategicDomain::Semiconductor => "💾",
            StrategicDomain::Space => "🚀",
            StrategicDomain::Robotics => "🤖",
            StrategicDomain::Defense => "🛡️",
            StrategicDomain::Energy => "⚡",
            StrategicDomain::Macro => "📊",
            StrategicDomain::Healthcare => "🏥",
            StrategicDomain::Other => "📡",
        }
    }

    /// 所有非 Other 领域的列表
    pub fn all() -> [StrategicDomain; 8] {
        [
            StrategicDomain::AI,
            StrategicDomain::Semiconductor,
            StrategicDomain::Space,
            StrategicDomain::Robotics,
            StrategicDomain::Defense,
            StrategicDomain::Energy,
            StrategicDomain::Macro,
            StrategicDomain::Healthcare,
        ]
    }

    /// 从 serde 标签字符串解析
    pub fn from_key(key: &str) -> Option<StrategicDomain> {
        match key {
            "ai" => Some(StrategicDomain::AI),
            "semiconductor" => Some(StrategicDomain::Semiconductor),
            "space" => Some(StrategicDomain::Space),
            "robotics" => Some(StrategicDomain::Robotics),
            "defense" => Some(StrategicDomain::Defense),
            "energy" => Some(StrategicDomain::Energy),
            "macro" => Some(StrategicDomain::Macro),
            "healthcare" => Some(StrategicDomain::Healthcare),
            "other" => Some(StrategicDomain::Other),
            _ => None,
        }
    }

    // ── 关键词列表（Bootstrap 分类用） ──

    fn keywords(&self) -> &'static [&'static str] {
        match self {
            StrategicDomain::AI => &[
                "ai",
                "artificial intelligence",
                "llm",
                "gpt",
                "openai",
                "anthropic",
                "claude",
                "gemini",
                "deepmind",
                "model",
                "transformer",
                "agent",
                "chatgpt",
                "copilot",
                "foundation model",
                "alignment",
                "agi",
                "deepseek",
                "reasoning",
                "inference",
                "training",
                "fine-tune",
                "langchain",
                "vector database",
                "embedding",
                "rag",
                "retrieval",
                "multimodal",
                "rlhf",
                "mixture of experts",
                "人工智能",
                "大模型",
                "智能体",
                "深度学习",
            ],
            StrategicDomain::Semiconductor => &[
                "chip",
                "semiconductor",
                "tsmc",
                "nvidia",
                "hbm",
                "gpu",
                "tpu",
                "wafer",
                "foundry",
                "lithography",
                "euv",
                "samsung",
                "micron",
                "sk hynix",
                "broadcom",
                "qualcomm",
                "arm",
                "risc-v",
                "cowos",
                "advanced packaging",
                "substrate",
                "silicon",
                "export control",
                "chip ban",
                "bIs",
                "芯片",
                "半导体",
                "晶圆",
                "光刻",
                "制程",
            ],
            StrategicDomain::Space => &[
                "space",
                "spacex",
                "starship",
                "rocket",
                "nasa",
                "satellite",
                "starlink",
                "orbit",
                "launch",
                "blue origin",
                "mars",
                "lunar",
                "iss",
                "space station",
                "asteroid",
                "leo",
                "geo",
                "falcon",
                "航天",
                "火箭",
                "卫星",
                "轨道",
            ],
            StrategicDomain::Robotics => &[
                "robot",
                "robotics",
                "humanoid",
                "autonomous",
                "drone",
                "boston dynamics",
                "figure",
                "tesla bot",
                "optimus",
                "embodied",
                "manipulation",
                "locomotion",
                "slam",
                "ros",
                "self-driving",
                "机器人",
                "自动驾驶",
                "无人机",
                "具身智能",
            ],
            StrategicDomain::Defense => &[
                "defense",
                "military",
                "pentagon",
                "dod",
                "weapon",
                "army",
                "navy",
                "air force",
                "missile",
                "cyber warfare",
                "nato",
                "国防",
                "军事",
                "武器",
                "五角大楼",
            ],
            StrategicDomain::Energy => &[
                "energy",
                "power",
                "nuclear",
                "solar",
                "wind",
                "grid",
                "datacenter power",
                "fusion",
                "battery",
                "electricity",
                "reactor",
                "sMR",
                "能源",
                "电力",
                "核能",
                "太阳能",
                "电池",
            ],
            StrategicDomain::Macro => &[
                "economy",
                "gdp",
                "inflation",
                "fed",
                "interest rate",
                "trade war",
                "tariff",
                "decoupling",
                "supply chain",
                "fiscal",
                "monetary",
                "imf",
                "world bank",
                "recession",
                "宏观",
                "经济",
                "通胀",
                "利率",
                "贸易战",
                "关税",
            ],
            StrategicDomain::Healthcare => &[
                "health",
                "medical",
                "pharma",
                "drug",
                "fda",
                "biotech",
                "crispr",
                "mrna",
                "vaccine",
                "clinical trial",
                "diagnostic",
                "医疗",
                "生物",
                "制药",
                "疫苗",
            ],
            StrategicDomain::Other => &[],
        }
    }

    // ── Bootstrap 分类（关键词匹配） ──

    /// 单个领域的关键词匹配分数
    fn keyword_score(&self, text: &str) -> usize {
        let lower = text.to_lowercase();
        self.keywords()
            .iter()
            .filter(|kw| lower.contains(*kw))
            .map(|kw| kw.len())
            .sum()
    }

    /// 基于标题+摘要的关键词匹配分类
    ///
    /// 返回 (primary, 所有匹配的 domains 按分数排序)
    /// 当没有关键词匹配时返回 (Other, vec![])
    ///
    /// 这是 Bootstrap 机制——当 LLM 不可用或快速启动时使用。
    /// 长期不应依赖关键词作为唯一分类来源。
    pub fn classify(text: &str) -> (StrategicDomain, Vec<StrategicDomain>) {
        let all_domains = StrategicDomain::all();

        // 收集所有有分数的领域
        let mut scored: Vec<(usize, StrategicDomain)> = all_domains
            .iter()
            .map(|d| {
                let score = d.keyword_score(text);
                (score, *d)
            })
            .filter(|(score, _)| *score > 0)
            .collect();

        // 按分数降序排序
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0).then_with(|| {
                // 平局时按优先级：Semiconductor > AI > Space > ...
                priority(a.1).cmp(&priority(b.1))
            })
        });

        if scored.is_empty() {
            return (StrategicDomain::Other, vec![]);
        }

        let primary = scored[0].1;
        let secondary: Vec<StrategicDomain> = scored.iter().skip(1).map(|(_, d)| *d).collect();

        (primary, secondary)
    }

    /// 快速分类——只返回 primary，用于简单场景
    pub fn classify_primary(text: &str) -> StrategicDomain {
        StrategicDomain::classify(text).0
    }

    /// 得分类阈值——低于此值表示分类不确定，应使用 LLM refine
    pub fn low_confidence_threshold() -> usize {
        10 // 单个领域关键词匹配总长度 < 10 认为不可靠
    }

    /// 检查关键词分类是否低置信度
    pub fn is_classify_low_confidence(text: &str) -> bool {
        let (primary, _secondary) = StrategicDomain::classify(text);
        if primary == StrategicDomain::Other {
            return true;
        }
        primary.keyword_score(text) < StrategicDomain::low_confidence_threshold()
    }

    // ── LLM Classification ──

    /// LLM 分类的 Prompt 片段
    ///
    /// 当关键词匹配不可靠时，交给 LLM 做最终判断。
    /// LLM 被约束为只能输出预定义的领域枚举值（一行一个）。
    pub fn llm_classification_prompt() -> &'static str {
        "Classify this topic into strategic domains. \
         Output exactly ONE primary domain, then zero or more secondary domains (one per line). \
         Valid domains: ai, semiconductor, space, robotics, defense, energy, macro, healthcare, other\n\
         Format:\n  primary: <domain>\n  secondary: <domain>  (optional, repeat for each)\n\
         Examples:\n\
         - \"OpenAI releases GPT-5 with new reasoning\" → primary: ai\n\
         - \"TSMC 3nm yield improves, NVIDIA benefits\" → primary: semiconductor\n\
         - \"OpenAI partners with TSMC for custom AI chips\" → primary: ai\n    secondary: semiconductor\n\
         - \"US export controls on AI chips to China\" → primary: semiconductor\n    secondary: macro\n    secondary: ai\n\
         - \"SpaceX Starship reaches orbit, NASA applauds\" → primary: space\n\
         - \"Fed raises rates, tech stocks fall\" → primary: macro\n\
         If truly ambiguous, output: primary: other"
    }

    /// 从 LLM 响应文本解析 StrategicDomain
    /// 期望格式：
    ///   primary: ai
    ///   secondary: semiconductor
    ///   secondary: defense
    pub fn parse_llm_response(response: &str) -> (StrategicDomain, Vec<StrategicDomain>) {
        let mut primary = StrategicDomain::Other;
        let mut secondary = Vec::new();

        for line in response.lines() {
            let trimmed = line.trim().to_lowercase();
            if let Some(domain_str) = trimmed.strip_prefix("primary:") {
                if let Some(d) = StrategicDomain::from_key(domain_str.trim()) {
                    primary = d;
                }
            } else if let Some(domain_str) = trimmed.strip_prefix("secondary:") {
                if let Some(d) = StrategicDomain::from_key(domain_str.trim()) {
                    if d != StrategicDomain::Other && !secondary.contains(&d) {
                        secondary.push(d);
                    }
                }
            }
        }

        // Validate: primary shouldn't be in secondary
        secondary.retain(|d| *d != primary);

        (primary, secondary)
    }

    /// 校验 LLM 输出是否合法
    pub fn validate_llm_output(response: &str) -> bool {
        let (primary, _) = StrategicDomain::parse_llm_response(response);
        // 只要 primary 被正确识别就算合法
        primary != StrategicDomain::Other || response.contains("primary: other")
    }
}

/// 领域优先级（用于平局排序）
fn priority(d: StrategicDomain) -> u8 {
    match d {
        StrategicDomain::Semiconductor => 1,
        StrategicDomain::AI => 2,
        StrategicDomain::Space => 3,
        StrategicDomain::Robotics => 4,
        StrategicDomain::Defense => 5,
        StrategicDomain::Energy => 6,
        StrategicDomain::Macro => 7,
        StrategicDomain::Healthcare => 8,
        StrategicDomain::Other => 9,
    }
}

impl std::fmt::Display for StrategicDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Primary classification ──

    #[test]
    fn test_classify_ai() {
        let (p, _) = StrategicDomain::classify("OpenAI releases GPT-5 with new reasoning");
        assert_eq!(p, StrategicDomain::AI);
        let (p, _) = StrategicDomain::classify("DeepSeek open-sources new model");
        assert_eq!(p, StrategicDomain::AI);
        let (p, _) = StrategicDomain::classify("大模型训练成本下降");
        assert_eq!(p, StrategicDomain::AI);
    }

    #[test]
    fn test_classify_semiconductor() {
        let (p, _) = StrategicDomain::classify("TSMC 3nm yield reaches 80%");
        assert_eq!(p, StrategicDomain::Semiconductor);
        // "chip" has higher keyword weight than "ai" → should be Semiconductor
        let (p, _) = StrategicDomain::classify("AI chip demand surges");
        assert_eq!(p, StrategicDomain::Semiconductor);
    }

    #[test]
    fn test_classify_space() {
        let (p, _) = StrategicDomain::classify("SpaceX Starship completes orbital flight");
        assert_eq!(p, StrategicDomain::Space);
    }

    #[test]
    fn test_classify_robotics() {
        let (p, _) = StrategicDomain::classify("Tesla Optimus robot walks on factory floor");
        assert_eq!(p, StrategicDomain::Robotics);
    }

    #[test]
    fn test_classify_other() {
        let (p, _) = StrategicDomain::classify("The weather is nice today");
        assert_eq!(p, StrategicDomain::Other);
    }

    // ── Multi-domain ──

    #[test]
    fn test_multi_domain_cross_ai_chip() {
        let (primary, secondary) =
            StrategicDomain::classify("OpenAI partners with TSMC for custom AI chip manufacturing");
        assert_eq!(primary, StrategicDomain::Semiconductor); // chip-related keywords dominate
        assert!(secondary.contains(&StrategicDomain::AI));
    }

    #[test]
    fn test_multi_domain_trade_war() {
        let (primary, secondary) = StrategicDomain::classify(
            "US export controls on AI chips to China spark trade war concerns",
        );
        // Should pick up semiconductor + macro + AI
        assert!(primary == StrategicDomain::Semiconductor || primary == StrategicDomain::Macro);
    }

    #[test]
    fn test_multi_domain_no_secondary_for_single_topic() {
        let (primary, secondary) = StrategicDomain::classify("FDA approves new cancer drug");
        assert_eq!(primary, StrategicDomain::Healthcare);
        assert!(secondary.is_empty());
    }

    // ── Low confidence detection ──

    #[test]
    fn test_low_confidence_short_text() {
        let text = "Something about technology";
        assert!(StrategicDomain::is_classify_low_confidence(text));
    }

    #[test]
    fn test_high_confidence_rich_text() {
        let text = "OpenAI releases GPT-5 with advanced reasoning and agent capabilities, \
                    Anthropic announces Claude 4 with vision, deep learning breakthrough";
        assert!(!StrategicDomain::is_classify_low_confidence(text));
    }

    // ── LLM response parsing ──

    #[test]
    fn test_parse_llm_primary_only() {
        let response = "primary: ai\n";
        let (p, s) = StrategicDomain::parse_llm_response(response);
        assert_eq!(p, StrategicDomain::AI);
        assert!(s.is_empty());
    }

    #[test]
    fn test_parse_llm_with_secondaries() {
        let response = "primary: semiconductor\nsecondary: ai\nsecondary: macro\n";
        let (p, s) = StrategicDomain::parse_llm_response(response);
        assert_eq!(p, StrategicDomain::Semiconductor);
        assert_eq!(s.len(), 2);
        assert!(s.contains(&StrategicDomain::AI));
        assert!(s.contains(&StrategicDomain::Macro));
    }

    #[test]
    fn test_parse_llm_primary_not_in_secondary() {
        let response = "primary: ai\nsecondary: ai\nsecondary: semiconductor\n";
        let (p, s) = StrategicDomain::parse_llm_response(response);
        assert_eq!(p, StrategicDomain::AI);
        assert_eq!(s.len(), 1);
        assert!(s.contains(&StrategicDomain::Semiconductor));
    }

    #[test]
    fn test_parse_llm_other() {
        let response = "primary: other\n";
        let (p, s) = StrategicDomain::parse_llm_response(response);
        assert_eq!(p, StrategicDomain::Other);
        assert!(s.is_empty());
    }

    #[test]
    fn test_parse_llm_invalid_ignored() {
        let response = "primary: blockchain\nsecondary: ai\n";
        let (p, _) = StrategicDomain::parse_llm_response(response);
        assert_eq!(p, StrategicDomain::Other);
    }

    #[test]
    fn test_validate_llm_output_valid() {
        assert!(StrategicDomain::validate_llm_output("primary: ai\n"));
    }

    #[test]
    fn test_validate_llm_output_other() {
        assert!(StrategicDomain::validate_llm_output("primary: other\n"));
    }

    #[test]
    fn test_validate_llm_output_invalid() {
        assert!(!StrategicDomain::validate_llm_output(
            "primary: blockchain\n"
        ));
    }

    // ── Serialization ──

    #[test]
    fn test_serialization() {
        let ai = StrategicDomain::AI;
        let json = serde_json::to_string(&ai).unwrap();
        assert_eq!(json, "\"ai\"");

        let deser: StrategicDomain = serde_json::from_str("\"ai\"").unwrap();
        assert_eq!(deser, StrategicDomain::AI);
    }

    #[test]
    fn test_label_and_emoji() {
        assert_eq!(StrategicDomain::AI.label(), "AI");
        assert_eq!(StrategicDomain::AI.emoji(), "🧠");
        assert_eq!(StrategicDomain::Semiconductor.emoji(), "💾");
        assert_eq!(StrategicDomain::Other.emoji(), "📡");
    }

    #[test]
    fn test_classify_primary_shortcut() {
        let p = StrategicDomain::classify_primary("OpenAI releases GPT-5");
        assert_eq!(p, StrategicDomain::AI);
    }
}
