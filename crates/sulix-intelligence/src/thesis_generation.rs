//! ThesisGenerationStep — 从信号到可追踪判断
//!
//! 这是 Intelligence Pipeline 的核心价值步骤。
//! 输入: contract::Signal（带解释的信号）
//! 输出: contract::Thesis（可证伪的判断）
//!
//! 三段式信号匹配（参考 ripgrep Fast/Slow 双路径模式）:
//!   - Attach: 高置信度匹配（overlap_score ≥ 0.5），直接追加证据
//!   - Uncertain: 灰区（score 0.25~0.5），送 LLM 确认
//!   - NoMatch: 生成新 Thesis

use std::collections::HashSet;

use anyhow::Result;
use unicode_segmentation::UnicodeSegmentation;

use sulix_config::LlmConfig;
use sulix_contract as contract;
use sulix_llm as llm;

use super::context::StepContext;
use super::step::PipelineStep;

// ===== 停用词 =====

const STOPWORDS: &[&str] = &[
    "the", "is", "a", "an", "and", "or", "of", "to", "in", "for", "on", "with",
    "as", "by", "at", "from", "that", "this", "will", "be", "are", "was", "were",
    "has", "have", "had", "it", "its", "not", "but", "what", "which", "who",
    "的", "是", "在", "了", "和", "就", "也", "都", "要", "会", "有", "及",
    "与", "以", "为", "上", "下", "之", "而", "所", "被", "把", "让", "从",
];

// ===== 分词与相似度 =====

/// 将文本分词为小写 + 去停用词 + 基础词干化的 token 集合
fn tokenize(s: &str) -> HashSet<String> {
    s.unicode_words()
        .map(|w| {
            let w = w.to_lowercase();
            // 基础英语词干化：去掉复数/s/ing/ed
            let w = w.strip_suffix("ing").unwrap_or(&w).to_string();
            let w = w.strip_suffix("ed").unwrap_or(&w).to_string();
            let w = w.strip_suffix("s").unwrap_or(&w).to_string();
            w
        })
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// Jaccard 相似度 — 交集大小 / 并集大小
/// 修复: 原 title_overlap 用单边重叠（common/len_a），
/// 长文本天然高分。Jaccard 对长文本更公平。
/// 修复: split_whitespace 对中文完全失效，unicode_words 按字切开。
fn overlap_score(a: &str, b: &str) -> f64 {
    let (ta, tb) = (tokenize(a), tokenize(b));
    let inter = ta.intersection(&tb).count() as f64;
    inter / ta.union(&tb).count().max(1) as f64
}

// ===== 三段式匹配 =====

/// 匹配裁决
#[derive(Debug, Clone, PartialEq)]
pub enum MatchVerdict {
    /// 高置信匹配，直接追加证据
    Attach(String),
    /// 灰区，送 LLM 确认
    Uncertain(String),
    /// 不匹配
    NoMatch,
}

/// 对单个 Signal 执行三段式匹配
fn match_signal(signal: &contract::Signal, theses: &[contract::Thesis]) -> MatchVerdict {
    let best = theses
        .iter()
        .map(|t| (t.id.clone(), overlap_score(&t.claim, &signal.why)))
        .max_by(|a, b| a.1.total_cmp(&b.1));

    match best {
        Some((id, s)) if s >= 0.33 => MatchVerdict::Attach(id),
        Some((id, s)) if s >= 0.15 => MatchVerdict::Uncertain(id),
        _ => MatchVerdict::NoMatch,
    }
}

/// LLM 灰区确认 — 最小 prompt 批量确认
async fn resolve_uncertain(
    pairs: &[(String, String, String)],  // (thesis_id, claim, why)
    client: &reqwest::Client,
    api_key: &str,
    llm_config: &LlmConfig,
) -> Vec<(String, bool)> {
    if pairs.is_empty() {
        return vec![];
    }

    let mut user_prompt = String::from("判断以下每组「论题」和「信号」是否相关。只回答 JSON 数组:\n");
    for (i, (_, claim, why)) in pairs.iter().enumerate() {
        user_prompt.push_str(&format!("[{}]\n论题: {}\n信号: {}\n\n", i + 1, claim, why));
    }
    user_prompt.push_str(r#"输出格式: [{"attach": true/false}, ...]"#);

    let system_prompt = r#"你是一个匹配判定专家。判断每条信号是否为对应的论题提供直接证据。
规则：
- attach=true: 信号直接支持或反驳该论题
- attach=false: 信号与该论题无关
- 宁可漏判(false)也不要误判(true)
只输出 JSON 数组。"#;

    match llm::call_with_retry_raw(client, api_key, llm_config, system_prompt, &user_prompt).await {
        Ok(raw) => {
            if let Ok(parsed) = llm::parse_json_lenient(&raw) {
                if let Some(arr) = parsed.as_array() {
                    return pairs
                        .iter()
                        .enumerate()
                        .map(|(i, (id, _, _))| {
                            let attach = arr
                                .get(i)
                                .and_then(|v| v["attach"].as_bool())
                                .unwrap_or(false);
                            (id.clone(), attach)
                        })
                        .collect();
                }
            }
            // 解析失败 → 全部按不匹配处理
            log::warn!("⚠️ LLM 灰区确认解析失败，全部视为不匹配");
            pairs.iter().map(|(id, _, _)| (id.clone(), false)).collect()
        }
        Err(e) => {
            log::warn!("⚠️ LLM 灰区确认调用失败: {}", e);
            pairs.iter().map(|(id, _, _)| (id.clone(), false)).collect()
        }
    }
}

/// Thesis Generation 步骤
///
/// 从 Signals 生成或更新可追踪的 Thesis 判断。
pub struct ThesisGenerationStep {
    llm_config: LlmConfig,
    api_key: String,
    /// 已有的 Thesis（从 MemoryEngine 加载），用于匹配和追加证据
    existing_theses: Vec<contract::Thesis>,
}

impl ThesisGenerationStep {
    /// 获取 LlmConfig 引用
    pub fn llm_config(&self) -> &LlmConfig {
        &self.llm_config
    }

    /// 获取已有 Thesis 列表
    pub fn existing_theses(&self) -> &[contract::Thesis] {
        &self.existing_theses
    }

    /// 执行 Thesis 生成
    ///
    /// # 流程
    /// 1. 三段式匹配（Attach / Uncertain / NoMatch）
    /// 2. Attach → 追加 Evidence
    /// 3. Uncertain → LLM 灰区确认
    /// 4. NoMatch → LLM 生成新 Thesis
    /// 5. 返回 Vec<contract::Thesis>
    pub async fn generate(
        &self,
        signals: Vec<contract::Signal>,
        ctx: &StepContext,
    ) -> Result<Vec<contract::Thesis>> {
        if signals.is_empty() {
            return Ok(self.existing_theses.clone());
        }

        log::info!(
            "🧠 ThesisGeneration: {} signals, {} existing theses",
            signals.len(),
            self.existing_theses.len()
        );

        let mut updated_theses = self.existing_theses.clone();
        let mut unmatched_signals: Vec<&contract::Signal> = Vec::new();
        let mut uncertain_pairs: Vec<(String, String, String)> = Vec::new(); // (thesis_id, claim, why)

        // Phase 1: 三段式匹配
        for signal in &signals {
            match match_signal(signal, &updated_theses) {
                MatchVerdict::Attach(thesis_id) => {
                    // 高置信匹配 → 直接追加
                    if let Some(thesis) = updated_theses.iter_mut().find(|t| t.id == thesis_id) {
                        if !thesis.evidence.contains(&signal.id) {
                            thesis.evidence.push(signal.id.clone());
                            log::info!("  📎 Attach '{}' ← Signal {}", thesis.claim, signal.id);
                        }
                    }
                }
                MatchVerdict::Uncertain(thesis_id) => {
                    // 灰区 → 收集
                    if let Some(thesis) = updated_theses.iter().find(|t| t.id == thesis_id) {
                        uncertain_pairs.push((
                            thesis_id.clone(),
                            thesis.claim.clone(),
                            signal.why.clone(),
                        ));
                    } else {
                        unmatched_signals.push(signal);
                    }
                }
                MatchVerdict::NoMatch => {
                    unmatched_signals.push(signal);
                }
            }
        }

        // Phase 2: LLM 灰区确认
        if !uncertain_pairs.is_empty() {
            let llm_client = llm::create_client(120)?;
            let results = resolve_uncertain(
                &uncertain_pairs,
                &llm_client,
                &self.api_key,
                &self.llm_config,
            )
            .await;

            let mut resolved_ids: HashSet<String> = HashSet::new();
            for (thesis_id, attach) in &results {
                if *attach {
                    resolved_ids.insert(thesis_id.clone());
                }
            }

            // 确认匹配的追加证据
            for signal in &signals {
                let verdict = match_signal(signal, &updated_theses);
                if let MatchVerdict::Uncertain(thesis_id) = &verdict {
                    if resolved_ids.contains(thesis_id) {
                        if let Some(thesis) =
                            updated_theses.iter_mut().find(|t| t.id == *thesis_id)
                        {
                            if !thesis.evidence.contains(&signal.id) {
                                thesis.evidence.push(signal.id.clone());
                                log::info!(
                                    "  📎 LLM 确认 Attach '{}' ← Signal {}",
                                    thesis.claim,
                                    signal.id
                                );
                            }
                        }
                    } else {
                        // LLM 确认不匹配 → 加入 unmatched
                        unmatched_signals.push(signal);
                    }
                }
            }
        }

        // Phase 3: 未匹配信号 → LLM 生成新 Thesis
        if !unmatched_signals.is_empty() {
            let llm_client = llm::create_client(120)?;
            match self
                .generate_theses_llm(&unmatched_signals, &llm_client)
                .await
            {
                Ok(new_theses) => {
                    log::info!("  ✨ LLM 生成 {} 个新 Thesis", new_theses.len());
                    updated_theses.extend(new_theses);
                }
                Err(e) => {
                    log::warn!("  ⚠️ Thesis 生成失败: {}", e);
                }
            }
        }

        // Debug mode: write output
        if ctx.should_write_debug() {
            let artifact = super::Artifact::Theses(updated_theses.clone());
            let json = artifact.to_json()?;
            if let Some(dir) = &ctx.debug_dir {
                let path = dir.join(format!("{}.thesis.output.json", ctx.today));
                std::fs::create_dir_all(dir)?;
                std::fs::write(&path, &json)?;
            }
        }

        log::info!("✅ ThesisGeneration: {} theses", updated_theses.len());
        Ok(updated_theses)
    }

    /// 调用 LLM 从未匹配信号生成新 Thesis
    async fn generate_theses_llm(
        &self,
        signals: &[&contract::Signal],
        client: &reqwest::Client,
    ) -> Result<Vec<contract::Thesis>> {
        let system_prompt = r#"你是一个战略分析师。你的任务是从一组信号中提炼出可追踪的战略判断。

每个 Thesis 必须是：
1. 可证伪的（能明确说出"什么情况下我错了"）
2. 有具体时间范围的
3. 基于输入信号的

对每组信号输出：
{
  "theses": [
    {
      "claim": "AI Agent adoption will accelerate in enterprise",
      "confidence": 0.65,
      "falsification_conditions": ["Enterprise adoption remains flat for 12 months", "Major security incidents"],
      "time_horizon": "12_months",
      "theme": "AI Enterprise Adoption",
      "belief_statement": "Enterprise AI adoption is driven by cost reduction pressure"
    }
  ]
}

规则：
- 一组信号最多产生 1 个 Thesis（如果信号间有关联）
- 无关信号不强行合并
- falsification_conditions 必须至少 1 条
- time_horizon 从 30_days / 6_months / 12_months 中选择"#;

        let mut user_prompt = format!("请基于以下 {} 条信号生成 Thesis：\n\n", signals.len());
        for (i, s) in signals.iter().enumerate() {
            user_prompt.push_str(&format!(
                "[{}] 领域: {} | 重要性: {:.2} | 信号类别: {:?} | 原因: {}\n",
                i, s.domain, s.importance, s.category, s.why
            ));
        }

        let raw = llm::call_with_retry_raw(client, &self.api_key, &self.llm_config, system_prompt, &user_prompt).await?;

        let parsed: serde_json::Value = llm::parse_json_lenient(&raw)?;
        let entries = parsed["theses"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("LLM response missing 'theses' array: {}", raw))?;

        let theses: Vec<contract::Thesis> = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let claim = entry["claim"].as_str().unwrap_or("Untitled thesis").to_string();
                let falsifications: Vec<String> = entry["falsification_conditions"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let time_horizon = entry["time_horizon"].as_str().unwrap_or("12_months").to_string();
                let theme = entry["theme"].as_str().map(String::from);
                let belief = entry["belief_statement"].as_str().map(String::from);
                let confidence = entry["confidence"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0);

                let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
                contract::Thesis {
                    id: format!("thesis_{}_{:04}", ts, i + 1),
                    claim,
                    confidence,
                    evidence: vec![],
                    status: contract::ThesisStatus::Proposed,
                    falsification_conditions: falsifications,
                    time_horizon,
                    theme,
                    belief_statement: belief,
                }
            })
            .collect();

        Ok(theses)
    }
}

// ===== ThesisGenerationStepBuilder =====

/// ThesisGenerationStep 构建器
pub struct ThesisGenerationStepBuilder {
    llm_config: LlmConfig,
    api_key: String,
    existing_theses: Vec<contract::Thesis>,
}

impl ThesisGenerationStepBuilder {
    pub fn new(llm_config: LlmConfig, api_key: &str) -> Self {
        Self {
            llm_config,
            api_key: api_key.to_string(),
            existing_theses: vec![],
        }
    }

    pub fn with_existing_theses(mut self, theses: Vec<contract::Thesis>) -> Self {
        self.existing_theses = theses;
        self
    }

    pub fn build(self) -> ThesisGenerationStep {
        ThesisGenerationStep {
            llm_config: self.llm_config,
            api_key: self.api_key,
            existing_theses: self.existing_theses,
        }
    }
}

// ===== PipelineStep trait 实现 =====

impl PipelineStep<contract::Signal, contract::Thesis> for ThesisGenerationStep {
    fn name(&self) -> &'static str {
        "ThesisGeneration"
    }

    async fn run(
        &self,
        input: Vec<contract::Signal>,
        ctx: &StepContext,
    ) -> anyhow::Result<Vec<contract::Thesis>> {
        self.generate(input, ctx).await
    }
}

/// 标题重叠匹配（兼容旧版，保持导出）
#[deprecated(since = "0.2.0", note = "改用 match_signal + overlap_score")]
pub fn title_overlap(a: &str, b: &str) -> f64 {
    overlap_score(a, b)
}

// ===== 匹配黄金测试 =====

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_basic() {
        let tokens = tokenize("AI Agent adoption in enterprise");
        assert!(tokens.contains("ai"));
        assert!(tokens.contains("agent"));
        assert!(tokens.contains("adoption"));
        assert!(tokens.contains("enterprise"));
        assert!(!tokens.contains("in"));
    }

    #[test]
    fn test_tokenize_chinese() {
        let tokens = tokenize("人工智能在企业中的应用");
        // 中文 tokenize: unicode_words 应该切出字
        assert!(tokens.len() >= 4);
    }

    #[test]
    fn test_overlap_score_exact() {
        let score = overlap_score("AI Agent growth", "AI Agent growth");
        assert!((score - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_overlap_score_none() {
        let score = overlap_score("GPU shortage", "Oil price drop");
        assert!(score < 0.01);
    }

    #[test]
    fn test_overlap_score_partial() {
        let score = overlap_score("AI Agent enterprise adoption", "AI enterprise software growth");
        assert!(score > 0.1 && score < 0.8);
    }

    #[test]
    fn test_overlap_score_stopwords_filtered() {
        let score = overlap_score("the AI is in the enterprise", "AI enterprise");
        assert!(score > 0.5);
    }

    #[test]
    fn test_match_signal_attach() {
        let signal = contract::Signal {
            id: "sig_001".into(),
            observation_id: "obs_001".into(),
            importance: 0.8,
            domain: "AI".into(),
            category: contract::SignalCategory::StructuralShift,
            why: "AI Agent enterprise adoption accelerates with new tools".into(),
        };
        let theses = vec![contract::Thesis {
            id: "thesis_001".into(),
            claim: "AI Agent adoption will accelerate in enterprise".into(),
            confidence: 0.7,
            evidence: vec![],
            status: contract::ThesisStatus::Active,
            falsification_conditions: vec![],
            time_horizon: "12_months".into(),
            theme: None,
            belief_statement: None,
        }];
        assert!(matches!(match_signal(&signal, &theses), MatchVerdict::Attach(_)));
    }

    #[test]
    fn test_match_signal_nomatch() {
        let signal = contract::Signal {
            id: "sig_002".into(),
            observation_id: "obs_002".into(),
            importance: 0.6,
            domain: "Macro".into(),
            category: contract::SignalCategory::ContextUpdate,
            why: "Oil prices dropped 5% this week".into(),
        };
        let theses = vec![contract::Thesis {
            id: "thesis_001".into(),
            claim: "AI Agent adoption will accelerate".into(),
            confidence: 0.7,
            evidence: vec![],
            status: contract::ThesisStatus::Active,
            falsification_conditions: vec![],
            time_horizon: "12_months".into(),
            theme: None,
            belief_statement: None,
        }];
        assert!(matches!(match_signal(&signal, &theses), MatchVerdict::NoMatch));
    }

    #[test]
    fn test_match_signal_uncertain() {
        let signal = contract::Signal {
            id: "sig_003".into(),
            observation_id: "obs_003".into(),
            importance: 0.7,
            domain: "Enterprise".into(),
            category: contract::SignalCategory::ContextUpdate,
            why: "Enterprise software see growth in AI tool".into(),
        };
        let theses = vec![contract::Thesis {
            id: "thesis_001".into(),
            claim: "AI Agent adoption will accelerate in enterprise".into(),
            confidence: 0.7,
            evidence: vec![],
            status: contract::ThesisStatus::Active,
            falsification_conditions: vec![],
            time_horizon: "12_months".into(),
            theme: None,
            belief_statement: None,
        }];
        assert!(matches!(match_signal(&signal, &theses), MatchVerdict::Uncertain(_)));
    }

    #[test]
    fn test_matching_golden() {
        let json = include_str!("../tests/fixtures/matching_cases.json");
        let cases: Vec<serde_json::Value> = serde_json::from_str(json).unwrap();

        for case in &cases {
            let id = case["id"].as_str().unwrap();
            let claim = case["claim"].as_str().unwrap();
            let why = case["why"].as_str().unwrap();
            let expect = case["expect"].as_str().unwrap();

            let signal = contract::Signal {
                id: format!("sig_{}", id),
                observation_id: "obs_test".into(),
                importance: 0.7,
                domain: "Test".into(),
                category: contract::SignalCategory::ContextUpdate,
                why: why.to_string(),
            };
            let thesis = contract::Thesis {
                id: format!("thesis_{}", id),
                claim: claim.to_string(),
                confidence: 0.7,
                evidence: vec![],
                status: contract::ThesisStatus::Active,
                falsification_conditions: vec![],
                time_horizon: "12_months".into(),
                theme: None,
                belief_statement: None,
            };

            let verdict = match_signal(&signal, &[thesis]);
            match expect {
                "attach" => {
                    assert!(matches!(verdict, MatchVerdict::Attach(_)), "{}: 应 attach", id);
                }
                "reject" => {
                    assert!(matches!(verdict, MatchVerdict::NoMatch), "{}: 应 NoMatch", id);
                }
                "uncertain" => {
                    assert!(matches!(verdict, MatchVerdict::Uncertain(_)), "{}: 应 Uncertain", id);
                }
                other => panic!("未知 expect 值: {}", other),
            }
        }
    }
}
