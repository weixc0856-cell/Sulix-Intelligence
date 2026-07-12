//! ThesisGenerationStep — 从信号到可追踪判断
//!
//! 这是 Intelligence Pipeline 的核心价值步骤。
//! 输入: contract::Signal（带解释的信号）
//! 输出: contract::Thesis（可证伪的判断）
//!
//! 功能：
//! - 匹配信号到已有 Thesis（标题级重叠匹配）
//! - 未匹配信号 → LLM 生成新 Thesis
//! - 更新 Thesis 状态机（Strengthening / Weakening）
//!
//! 保留的旧逻辑（来自 `engine/memory.rs`）:
//! - feed_intel 轻量匹配
//! - update_from_analysis evidence 追加
//! - 状态机计算

use anyhow::Result;

use sulix_config::LlmConfig;
use sulix_contract as contract;
use sulix_llm as llm;

use super::context::StepContext;
use super::step::PipelineStep;

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
    /// 1. 对信号按 domain 分组
    /// 2. 尝试匹配已有 Thesis
    /// 3. 匹配 → 追加 Evidence
    /// 4. 未匹配 → LLM 生成新 Thesis
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

        // Phase 1: 轻量匹配 — 将信号匹配到已有 Thesis
        let mut updated_theses = self.existing_theses.clone();
        let mut unmatched_signals: Vec<&contract::Signal> = Vec::new();

        for signal in &signals {
            let matched = updated_theses.iter_mut().find(|thesis| {
                title_overlap(&thesis.claim, &signal.why) > 0.3
            });

            match matched {
                Some(thesis) => {
                    // 匹配到已有 Thesis → 追加 Signal ID 作为证据
                    if !thesis.evidence.contains(&signal.id) {
                        thesis.evidence.push(signal.id.clone());
                        log::info!("  📎 匹配 Thesis '{}' ← Signal {}", thesis.claim, signal.id);
                    }
                }
                None => {
                    unmatched_signals.push(signal);
                }
            }
        }

        // Phase 2: 未匹配信号 → LLM 生成新 Thesis
        if !unmatched_signals.is_empty() {
            let llm_client = llm::create_client(120)?;
            match self.generate_theses_llm(&unmatched_signals, &llm_client).await {
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

        let raw = llm::call_with_retry_raw(
            client,
            &self.api_key,
            &self.llm_config,
            system_prompt,
            &user_prompt,
        )
        .await?;

        let parsed: serde_json::Value = llm::parse_json_lenient(&raw)?;
        let entries = parsed["theses"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("LLM response missing 'theses' array: {}", raw))?;

        let theses: Vec<contract::Thesis> = entries
            .iter()
            .map(|entry| {
                let claim = entry["claim"].as_str().unwrap_or("Untitled thesis").to_string();
                let falsifications: Vec<String> = entry["falsification_conditions"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let time_horizon = entry["time_horizon"]
                    .as_str()
                    .unwrap_or("12_months")
                    .to_string();
                let theme = entry["theme"].as_str().map(String::from);
                let belief = entry["belief_statement"].as_str().map(String::from);
                let confidence = entry["confidence"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0);

                contract::Thesis {
                    id: format!("thesis_{:04}", chrono::Utc::now().timestamp_subsec_millis()),
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

/// 标题重叠匹配（来自 engine/memory.rs 的 title_overlap 简化版）
/// 返回 0.0~1.0 的重叠比例
fn title_overlap(a: &str, b: &str) -> f64 {
    let words_a: Vec<&str> = a.split_whitespace().collect();
    let words_b: Vec<&str> = b.split_whitespace().collect();

    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }

    let common = words_a
        .iter()
        .filter(|w| words_b.contains(w))
        .count();

    common as f64 / words_a.len().max(words_b.len()) as f64
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_overlap_exact() {
        assert!((title_overlap("hello world", "hello world") - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_title_overlap_partial() {
        let overlap = title_overlap("AI Agent growth", "AI Infrastructure growth");
        assert!(overlap > 0.2 && overlap < 0.8);
    }

    #[test]
    fn test_title_overlap_none() {
        assert!((title_overlap("abc", "xyz")).abs() < 0.01);
    }

    #[test]
    fn test_title_overlap_empty() {
        assert!((title_overlap("", "test")).abs() < 0.01);
    }
}


