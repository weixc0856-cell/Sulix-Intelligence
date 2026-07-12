//! SignalClassificationStep — 从纯事实到信号解释
//!
//! 这是 Intelligence Pipeline 的第一层。
//! 输入: contract::Observation（纯事实，不含解释）
//! 输出: contract::Signal（带重要性/领域/分类/推理的解释）
//!
//! 双路径设计（类比 ripgrep Fast/Slow Path）:
//!   - Fast Path:  规则分类（源评分 + 关键词匹配），零 LLM 成本
//!   - Slow Path:  LLM 语义分类（精确但贵）
//!   - Auto 选择:  根据源评分、内容质量自动选择路径
//!
//! 保留的旧逻辑（来自 `agent/scan.rs`）:
//! - 源乘数（source score multiplier）
//! - LLM 批次失败时的兜底值（is_fallback）

use anyhow::Result;
use std::collections::HashMap;

use sulix_config::LlmConfig;
use sulix_contract as contract;
use sulix_llm as llm;

use super::context::StepContext;
use super::step::PipelineStep;

/// 处理路径选择 — 类比 ripgrep `is_line_by_line_fast()`
///
/// Fast Path 条件（任一满足即走规则路径）:
///   - 源评分 < 3（低可信源，如社交噪音）
///   - 内容为空（无可分析文本）
///   - 源 layer >= 4（纯市场数据，非叙事内容）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SignalPath {
    Fast,
    Slow,
}

impl SignalPath {
    pub fn auto_select(obs: &contract::Observation, source_score: Option<f64>) -> Self {
        // 空内容 → 快速路径
        if obs.raw_content.is_empty() && obs.title.is_empty() {
            return Self::Fast;
        }
        // 低评分源 → 快速路径
        if let Some(score) = source_score {
            if score < 3.0 {
                return Self::Fast;
            }
        }
        Self::Slow
    }

    pub fn needs_llm(&self) -> bool {
        matches!(self, Self::Slow)
    }
}

/// Signal Classification 步骤配置
#[derive(Debug, Clone)]
pub struct SignalClassificationConfig {
    /// 源可信度乘数（source_name → score）
    pub source_scores: HashMap<String, f64>,
    /// 每次 LLM 调用的最大 Observation 数量
    pub batch_size: usize,
    /// 重试次数
    pub retry_max: u32,
    /// Fast Path 关键词映射（domain → [keyword..]）
    pub domain_keywords: HashMap<String, Vec<String>>,
}

impl Default for SignalClassificationConfig {
    fn default() -> Self {
        Self {
            source_scores: HashMap::new(),
            batch_size: 8,
            retry_max: 3,
            domain_keywords: HashMap::new(),
        }
    }
}

/// Signal Classification 步骤
pub struct SignalClassificationStep {
    llm_config: LlmConfig,
    api_key: String,
    config: SignalClassificationConfig,
}

impl SignalClassificationStep {
    /// 获取配置引用
    pub fn config(&self) -> &SignalClassificationConfig {
        &self.config
    }

    /// 获取 LlmConfig 引用
    pub fn llm_config(&self) -> &LlmConfig {
        &self.llm_config
    }

    /// 执行信号分类 — 双路径自动选择
    ///
    /// # 流程
    /// 1. 为每条 Observation 选择路径（Fast or Slow）
    /// 2. Fast Path: 规则分类（零 LLM）
    /// 3. Slow Path: LLM 语义分类（分批 + 重试）
    /// 4. 合并结果
    pub async fn classify(
        &self,
        observations: Vec<contract::Observation>,
        ctx: &StepContext,
    ) -> Result<Vec<contract::Signal>> {
        if observations.is_empty() {
            return Ok(vec![]);
        }

        log::info!(
            "🔍 SignalClassification: {} observations (batch={})",
            observations.len(),
            self.config.batch_size
        );

        // Phase 1: 分类路径选择
        let mut fast_obs: Vec<(usize, &contract::Observation)> = Vec::new();
        let mut slow_obs: Vec<(usize, &contract::Observation)> = Vec::new();

        for (i, obs) in observations.iter().enumerate() {
            let source_score = self.config.source_scores.get(&obs.source).copied();
            if SignalPath::auto_select(obs, source_score) == SignalPath::Fast {
                fast_obs.push((i, obs));
            } else {
                slow_obs.push((i, obs));
            }
        }

        log::info!("  🛣️ Fast: {}, Slow: {}", fast_obs.len(), slow_obs.len());

        // Phase 2: Fast Path — 规则分类
        let mut signals: Vec<Option<contract::Signal>> = vec![None; observations.len()];

        for (idx, obs) in &fast_obs {
            signals[*idx] = Some(self.fast_classify(obs, *idx));
        }

        // Phase 3: Slow Path — LLM 分类（分批）
        if !slow_obs.is_empty() {
            let slow_indices: Vec<usize> = slow_obs.iter().map(|(i, _)| *i).collect();
            let slow_batch: Vec<&contract::Observation> =
                slow_obs.iter().map(|(_, o)| *o).collect();

            let llm_client = llm::create_client(120)?;
            for chunk in slow_batch.chunks(self.config.batch_size) {
                match self.classify_batch(chunk, &llm_client).await {
                    Ok(batch_signals) => {
                        for (chunk_i, sig) in batch_signals.into_iter().enumerate() {
                            if let Some(orig_idx) = slow_indices.get(chunk_i) {
                                signals[*orig_idx] = Some(sig);
                            }
                        }
                        log::info!("  ✓ Slow batch: {} signals", chunk.len());
                    }
                    Err(e) => {
                        log::warn!("  ⚠️ Slow batch failed: {}", e);
                        for (chunk_i, obs) in chunk.iter().enumerate() {
                            if let Some(orig_idx) = slow_indices.get(chunk_i) {
                                signals[*orig_idx] = Some(self.fallback_classify_one(obs));
                            }
                        }
                    }
                }
            }
        }

        // Phase 4: 填充缺失（防御）
        let result: Vec<contract::Signal> = signals
            .into_iter()
            .enumerate()
            .map(|(i, sig)| sig.unwrap_or_else(|| self.fallback_classify_one(&observations[i])))
            .collect();

        // Debug mode
        if ctx.should_write_debug() {
            let artifact = super::Artifact::Signals(result.clone());
            let json = artifact.to_json()?;
            if let Some(dir) = &ctx.debug_dir {
                let path = dir.join(format!("{}.signal.output.json", ctx.today));
                std::fs::create_dir_all(dir)?;
                std::fs::write(&path, &json)?;
            }
        }

        log::info!("✅ SignalClassification: {} signals", result.len());
        Ok(result)
    }

    /// Fast Path: 规则分类 — 零 LLM 调用
    ///
    /// 基于源评分 + 关键词匹配生成信号。
    /// 类比 ripgrep 的 `match_by_line_fast()`：
    ///   快速、确定性、零外部调用。
    fn fast_classify(&self, obs: &contract::Observation, idx: usize) -> contract::Signal {
        let source_score = self
            .config
            .source_scores
            .get(&obs.source)
            .copied()
            .unwrap_or(5.0);
        let importance = (source_score / 10.0).clamp(0.1, 0.9);

        // 关键词匹配 → domain 检测
        let (domain, category) = self.match_keywords(obs);

        let why = format!(
            "规则分类: source={}, score={}, domain={}",
            obs.source, source_score, domain
        );

        contract::Signal {
            id: format!("sig_{:04}", idx + 1),
            observation_id: obs.id.clone(),
            importance,
            domain,
            category,
            why,
        }
    }

    /// 关键词匹配 — 从标题/内容中检测领域和信号类别
    fn match_keywords(&self, obs: &contract::Observation) -> (String, contract::SignalCategory) {
        let text = format!("{} {}", obs.title, obs.raw_content).to_lowercase();
        let mut matched_domain = "General".to_string();

        for (domain, keywords) in &self.config.domain_keywords {
            for kw in keywords {
                if text.contains(&kw.to_lowercase()) {
                    matched_domain = domain.clone();
                    break;
                }
            }
            if matched_domain != "General" {
                break;
            }
        }

        // 无关键词 → 保守分类为 ContextUpdate
        let category = if matched_domain == "General" {
            contract::SignalCategory::ContextUpdate
        } else {
            // 有领域匹配 → 保守设为 ContextUpdate（LLM 才能区分 StructuralShift）
            contract::SignalCategory::ContextUpdate
        };

        (matched_domain, category)
    }

    /// Slow Path: 对一批 Observations 调用 LLM 进行分类
    async fn classify_batch(
        &self,
        batch: &[&contract::Observation],
        client: &reqwest::Client,
    ) -> Result<Vec<contract::Signal>> {
        let system_prompt = r#"你是一个情报分析师。你的任务是将原始事实转换为结构化信号。

对每条 Observation 输出：
1. importance (0.0~1.0): 这条信息对创业者/投资者的战略重要性
2. domain: 所属战略领域（如 "AI Infrastructure", "Semiconductor", "Space", "BioTech", "Policy"）
3. category: 信号类别（structural_shift / competitive_signal / context_update / noise）
4. why (一句话): 为什么要关注这条信号

规则：
- importance 只给 0.0~1.0 连续值，不要离散
- domain 从预定义领域中选择，不要创造新领域
- noise 类别保留给明显无价值的信息
- why 必须包含具体的推理链"#;

        let mut user_prompt = format!("请分析以下 {} 条观测：\n\n", batch.len());
        for (i, obs) in batch.iter().enumerate() {
            let preview = if obs.raw_content.len() > 500 {
                let end = obs.raw_content.floor_char_boundary(500);
                format!("{}...", &obs.raw_content[..end])
            } else {
                obs.raw_content.clone()
            };
            user_prompt.push_str(&format!(
                "[{}] 标题: {} | 来源: {} | 内容: {}\n",
                i, obs.title, obs.source, preview
            ));
        }

        let raw = llm::call_with_retry_raw(
            client,
            &self.api_key,
            &self.llm_config,
            system_prompt,
            &user_prompt,
        )
        .await?;
        let parsed: serde_json::Value = llm::parse_json_lenient(&raw)?;
        let entries = parsed["signals"]
            .as_array()
            .or_else(|| parsed.as_array())
            .ok_or_else(|| anyhow::anyhow!("LLM 响应缺少 'signals' 数组: {}", raw))?;

        let mut signals = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            if i >= batch.len() {
                break;
            }
            let obs = batch[i];
            let importance = entry["importance"]
                .as_f64()
                .unwrap_or_else(|| {
                    log::warn!(
                        "⚠️ LLM 响应缺少 importance 字段 (idx {}), 使用默认值 0.5",
                        i
                    );
                    0.5
                })
                .clamp(0.0, 1.0);
            let domain = entry["domain"]
                .as_str()
                .unwrap_or_else(|| {
                    log::warn!(
                        "⚠️ LLM 响应缺少 domain 字段 (idx {}), 使用默认值 General",
                        i
                    );
                    "General"
                })
                .to_string();
            let category_str = entry["category"].as_str().unwrap_or_else(|| {
                log::warn!(
                    "⚠️ LLM 响应缺少 category 字段 (idx {}), 使用默认值 context_update",
                    i
                );
                "context_update"
            });
            let why = entry["why"]
                .as_str()
                .unwrap_or_else(|| {
                    log::warn!("⚠️ LLM 响应缺少 why 字段 (idx {}), 使用空字符串", i);
                    ""
                })
                .to_string();
            let category = match category_str {
                "structural_shift" => contract::SignalCategory::StructuralShift,
                "competitive_signal" => contract::SignalCategory::CompetitiveSignal,
                "noise" => contract::SignalCategory::Noise,
                _ => contract::SignalCategory::ContextUpdate,
            };
            let source_multiplier = self
                .config
                .source_scores
                .get(&obs.source)
                .copied()
                .unwrap_or(1.0);
            let weighted_importance = (importance * source_multiplier).clamp(0.0, 1.0);
            signals.push(contract::Signal {
                id: format!("sig_{:04}", i + 1),
                observation_id: obs.id.clone(),
                importance: weighted_importance,
                domain,
                category,
                why,
            });
        }
        Ok(signals)
    }

    /// 单条 Observation 的 fallback
    fn fallback_classify_one(&self, obs: &contract::Observation) -> contract::Signal {
        contract::Signal {
            id: format!("sig_{:04}", 0),
            observation_id: obs.id.clone(),
            importance: 0.3,
            domain: "General".into(),
            category: contract::SignalCategory::ContextUpdate,
            why: "LLM 分类失败，使用兜底评分".into(),
        }
    }
}

// ===== SignalClassificationStepBuilder — 参考 ripgrep SearcherBuilder 设计 =====

/// SignalClassificationStep 构建器
pub struct SignalClassificationStepBuilder {
    llm_config: LlmConfig,
    api_key: String,
    config: SignalClassificationConfig,
}

impl SignalClassificationStepBuilder {
    pub fn new(llm_config: LlmConfig, api_key: &str) -> Self {
        Self {
            llm_config,
            api_key: api_key.to_string(),
            config: SignalClassificationConfig::default(),
        }
    }

    pub fn with_config(mut self, config: SignalClassificationConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_source_scores(mut self, scores: HashMap<String, f64>) -> Self {
        self.config.source_scores = scores;
        self
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.config.batch_size = size;
        self
    }

    pub fn with_domain_keywords(mut self, kw: HashMap<String, Vec<String>>) -> Self {
        self.config.domain_keywords = kw;
        self
    }

    pub fn build(self) -> SignalClassificationStep {
        SignalClassificationStep {
            llm_config: self.llm_config,
            api_key: self.api_key,
            config: self.config,
        }
    }
}

// ===== PipelineStep trait 实现 =====

impl PipelineStep<contract::Observation, contract::Signal> for SignalClassificationStep {
    fn name(&self) -> &'static str {
        "SignalClassification"
    }

    async fn run(
        &self,
        input: Vec<contract::Observation>,
        ctx: &StepContext,
    ) -> anyhow::Result<Vec<contract::Signal>> {
        self.classify(input, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_obs(source: &str, title: &str, content: &str) -> contract::Observation {
        contract::Observation {
            id: "obs_001".into(),
            title: title.into(),
            source: source.into(),
            source_id: String::new(),
            url: "https://test.com".into(),
            published_at: "2026-07-12".into(),
            captured_at: "2026-07-12T00:00:00Z".into(),
            content_hash: "abc".into(),
            raw_content: content.into(),
            entities: vec![],
        }
    }

    fn test_step() -> SignalClassificationStep {
        SignalClassificationStepBuilder::new(
            LlmConfig {
                api_key: Some("test".into()),
                provider: "test".into(),
                model: "test".into(),
                base_url: "http://test".into(),
                max_tokens: 100,
                temperature: 0.0,
                perplexity_key: None,
            },
            "test",
        )
        .build()
    }

    #[test]
    fn test_config_default() {
        let c = SignalClassificationConfig::default();
        assert_eq!(c.batch_size, 8);
    }

    // ===== SignalPath tests =====

    #[test]
    fn test_signal_path_empty_content_is_fast() {
        let obs = test_obs("test", "", "");
        assert_eq!(SignalPath::auto_select(&obs, Some(5.0)), SignalPath::Fast);
    }

    #[test]
    fn test_signal_path_low_source_score_is_fast() {
        let obs = test_obs("reddit", "some post", "some content");
        assert_eq!(SignalPath::auto_select(&obs, Some(2.0)), SignalPath::Fast);
    }

    #[test]
    fn test_signal_path_high_value_is_slow() {
        let obs = test_obs("OpenAI Blog", "GPT-5 release", "long content here");
        assert_eq!(SignalPath::auto_select(&obs, Some(8.0)), SignalPath::Slow);
        assert!(SignalPath::auto_select(&obs, Some(8.0)).needs_llm());
    }

    #[test]
    fn test_signal_path_no_score_slow_by_default() {
        let obs = test_obs("test", "title", "content");
        assert_eq!(SignalPath::auto_select(&obs, None), SignalPath::Slow);
    }

    // ===== Fast classify tests =====

    #[test]
    fn test_fast_classify_source_score_importance() {
        let mut cfg = SignalClassificationConfig::default();
        cfg.source_scores.insert("OpenAI Blog".into(), 9.0);
        let step = SignalClassificationStepBuilder::new(
            LlmConfig {
                api_key: Some("test".into()),
                provider: "test".into(),
                model: "test".into(),
                base_url: "http://test".into(),
                max_tokens: 100,
                temperature: 0.0,
                perplexity_key: None,
            },
            "test",
        )
        .with_config(cfg)
        .build();
        let obs = test_obs("OpenAI Blog", "GPT-5", "content");
        let signal = step.fast_classify(&obs, 0);
        assert!(
            (signal.importance - 0.9).abs() < 0.01,
            "importance should be 0.9, got {}",
            signal.importance
        );
    }

    #[test]
    fn test_fast_classify_low_source_low_importance() {
        let mut cfg = SignalClassificationConfig::default();
        cfg.source_scores.insert("twitter".into(), 1.0);
        let step = SignalClassificationStepBuilder::new(
            LlmConfig {
                api_key: Some("test".into()),
                provider: "test".into(),
                model: "test".into(),
                base_url: "http://test".into(),
                max_tokens: 100,
                temperature: 0.0,
                perplexity_key: None,
            },
            "test",
        )
        .with_config(cfg)
        .build();
        let obs = test_obs("twitter", "some tweet", "short");
        let signal = step.fast_classify(&obs, 0);
        assert!(
            (signal.importance - 0.1).abs() < 0.01,
            "importance should be 0.1, got {}",
            signal.importance
        );
    }

    #[test]
    fn test_fast_classify_no_source_score_default() {
        let step = test_step();
        let obs = test_obs("unknown", "test", "content");
        let signal = step.fast_classify(&obs, 0);
        assert!((signal.importance - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_fast_classify_category_is_context_update() {
        let step = test_step();
        let obs = test_obs("test", "title", "content");
        let signal = step.fast_classify(&obs, 0);
        assert!(matches!(
            signal.category,
            contract::SignalCategory::ContextUpdate
        ));
    }

    #[test]
    fn test_keyword_matching_detects_domain() {
        let mut cfg = SignalClassificationConfig::default();
        cfg.domain_keywords.insert(
            "AI Infrastructure".into(),
            vec!["gpu".into(), "nvidia".into(), "ai model".into()],
        );
        let step = SignalClassificationStepBuilder::new(
            LlmConfig {
                api_key: Some("test".into()),
                provider: "test".into(),
                model: "test".into(),
                base_url: "http://test".into(),
                max_tokens: 100,
                temperature: 0.0,
                perplexity_key: None,
            },
            "test",
        )
        .with_config(cfg)
        .build();
        let obs = test_obs(
            "test",
            "NVIDIA releases new GPU",
            "content about ai model training",
        );
        let signal = step.fast_classify(&obs, 0);
        assert_eq!(signal.domain, "AI Infrastructure");
    }

    #[test]
    fn test_fallback_classify_produces_low_importance() {
        let step = test_step();
        let obs = test_obs("test", "title", "content");
        let signal = step.fallback_classify_one(&obs);
        assert!((signal.importance - 0.3).abs() < 0.01);
        assert!(matches!(
            signal.category,
            contract::SignalCategory::ContextUpdate
        ));
    }
}
