//! MemoryEngine — 信念追踪系统
//!
//! 职责：维护 `Thesis: Vec<Evidence>` 结构，每日更新。
//! 输出不公开，只存储。目标是形成 "判断→证据→修正" 的连续积累。
//!
//! v2 新增:
//!   - Evidence 去重 (sha256 hash)
//!   - 事件驱动的置信度追踪 (仅记录有意义的变化)
//!   - 状态变更历史
//!   - 生命周期: 30d Dormant / 90d Retired
//!   - Proposed 创建 / Resurrected 检测

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::clusterer::{Theme, ThemeAnalysis};

// ===== 数据模型（从 domain 层导入）=====

pub use crate::domain::evidence::{Evidence, Stance};
pub use crate::domain::investigation::Investigation;
pub use crate::domain::outcome::{Outcome, OutcomeVerdict};
pub use crate::domain::reflection::Reflection;
pub use crate::domain::thesis::{
    ConfidenceSnapshot, ConfidenceTrigger, StatusTransition, Thesis, ThesisStatus,
    TransitionTrigger,
};

/// 信念追踪引擎
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEngine {
    /// 所有 Thesis
    theses: Vec<Thesis>,
    /// 所有 Outcome 记录（判断 vs 实际结果）
    #[serde(default)]
    outcomes: Vec<Outcome>,
    /// 所有 Reflection 记录（复盘分析）
    #[serde(default)]
    reflections: Vec<Reflection>,
    /// 所有 Investigation 记录（Thesis 的问题集）
    #[serde(default)]
    investigations: Vec<Investigation>,
    /// memory_db.json 路径
    #[serde(skip)]
    memory_path: PathBuf,
}

/// Evidence 去重 hash
fn evidence_hash(title: &str, source: &str) -> String {
    use sha2::{Digest, Sha256};
    let normalized = format!(
        "{}:{}",
        title.trim().to_lowercase(),
        source.trim().to_lowercase()
    );
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 计算置信度 0.0-1.0（从证据 Support/Challenge 比例）
pub fn compute_confidence(evidences: &[Evidence]) -> f64 {
    let support = evidences
        .iter()
        .filter(|e| e.stance == Stance::Supports)
        .count() as f64;
    let challenge = evidences
        .iter()
        .filter(|e| e.stance == Stance::Challenges)
        .count() as f64;
    let total = support + challenge;
    if total == 0.0 {
        0.5
    } else {
        let ratio = support / total;
        (0.5 + (ratio - 0.5) * 0.8).clamp(0.1, 0.98)
    }
}

/// 计算闲置天数
fn idle_days(today: &str, updated: &str) -> Option<u32> {
    let updated_d = chrono::NaiveDate::parse_from_str(updated, "%Y-%m-%d").ok()?;
    let today_d = chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d").ok()?;
    Some((today_d - updated_d).num_days() as u32)
}

// ===== 实现 =====

impl MemoryEngine {
    /// 创建新实例（不自动加载）
    pub fn new(memory_path: PathBuf) -> Self {
        Self {
            theses: Vec::new(),
            outcomes: Vec::new(),
            reflections: Vec::new(),
            investigations: Vec::new(),
            memory_path,
        }
    }

    /// 从 `memory_db.json` 加载
    pub fn load(&mut self) -> Result<()> {
        if self.memory_path.exists() {
            let content = std::fs::read_to_string(&self.memory_path)?;
            let loaded: MemoryEngineData = serde_json::from_str(&content)?;
            self.theses = loaded.theses;
            self.outcomes = loaded.outcomes;
            self.reflections = loaded.reflections;
            self.investigations = loaded.investigations;
        }
        Ok(())
    }

    /// 写入 `memory_db.json`
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.memory_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = MemoryEngineData::from(self);
        let json = serde_json::to_string_pretty(&data)?;
        std::fs::write(&self.memory_path, json)?;
        Ok(())
    }

    /// 获取所有 Thesis
    pub fn theses(&self) -> &[Thesis] {
        &self.theses
    }

    /// 获取所有 Investigation 记录
    #[allow(dead_code)]
    pub fn investigations(&self) -> &[Investigation] {
        &self.investigations
    }

    /// 获取指定 Thesis 的 Investigation
    pub fn get_investigation_for_thesis(&self, thesis_id: &str) -> Option<&Investigation> {
        self.investigations
            .iter()
            .find(|inv| inv.thesis_id == thesis_id)
    }

    /// 核心更新：将当日分析结果融入信念系统
    ///
    /// v2 新增:
    ///   - Evidence 去重 (sha256 hash)
    ///   - 事件驱动的置信度快照
    ///   - 状态变更历史记录
    ///   - 30d Dormant / 90d Retired 生命周期
    pub fn update_from_analysis(
        &mut self,
        today: &str,
        themes: &[Theme],
        analyses: &[ThemeAnalysis],
    ) -> Result<()> {
        for (theme, analysis) in themes.iter().zip(analyses.iter()) {
            let title = &theme.title;
            let source = theme
                .sources
                .first()
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());

            // 尝试匹配现有 Thesis
            let matched = self.match_thesis(title);

            if let Some(idx) = matched {
                // Evidence 去重检查
                let hash = evidence_hash(title, &source);
                let is_duplicate = self.theses[idx]
                    .evidences
                    .iter()
                    .any(|e| evidence_hash(&e.title, &e.source) == hash);

                if is_duplicate {
                    // 跳过重复，但仍更新 timestamp 和状态
                    self.theses[idx].updated = today.to_string();
                    self.theses[idx].status = self.recompute_status(idx, today);
                    continue;
                }

                // 追加证据
                self.theses[idx].evidences.push(Evidence {
                    date: today.to_string(),
                    title: title.clone(),
                    source,
                    summary: analysis.bluf.clone(),
                    stance: if analysis.signal_strength >= 3 {
                        Stance::Supports
                    } else {
                        Stance::Challenges
                    },
                    signal_strength: analysis.signal_strength,
                });
                self.theses[idx].updated = today.to_string();

                // 同步证伪条件（覆盖更新，条件会随证据演化）
                if !analysis.falsification_conditions.is_empty() {
                    self.theses[idx].falsification_conditions = analysis.falsification_conditions.clone();
                }

                // 记录状态变更
                let old_status = self.theses[idx].status.clone();
                let new_status = self.recompute_status(idx, today);
                if old_status != new_status {
                    let desc = format!(
                        "{:?} → {:?} (evidence: {})",
                        old_status,
                        new_status,
                        self.theses[idx].evidences.len()
                    );
                    self.record_status_transition_inner(
                        idx,
                        new_status,
                        TransitionTrigger::EvidenceThreshold,
                        &desc,
                    );
                } else {
                    self.theses[idx].status = new_status;
                }

                // 事件驱动的置信度快照
                let trigger = if old_status != self.theses[idx].status {
                    ConfidenceTrigger::StatusChange
                } else {
                    ConfidenceTrigger::SignificantChange
                };
                self.record_confidence_inner(idx, trigger, "daily update");
            } else {
                // 新建 Thesis（状态: Active）
                let new_thesis = Thesis {
                    id: format!("thesis-{}", chrono::Utc::now().timestamp()),
                    title: title.clone(),
                    created: today.to_string(),
                    updated: today.to_string(),
                    evidences: vec![Evidence {
                        date: today.to_string(),
                        title: title.clone(),
                        source,
                        summary: analysis.bluf.clone(),
                        stance: Stance::Supports,
                        signal_strength: analysis.signal_strength,
                    }],
                    assumptions: analysis.assumptions.clone(),
                    status: ThesisStatus::Active,
                    confidence_history: vec![],
                    status_history: vec![],
                    parent_id: None,
                    merged_ids: vec![],
                    related_thesis_ids: vec![],
                    metadata: HashMap::new(),
                    investigation_id: None,
                    decision_history: vec![],
                    falsification_conditions: analysis.falsification_conditions.clone(),
                    assessment_id: None,
                };
                self.theses.push(new_thesis);
                let new_idx = self.theses.len() - 1;
                // 记录初始置信度
                self.record_confidence_inner(new_idx, ConfidenceTrigger::Initial, "thesis created");
            }
        }

        // 生命周期管理: 30d Dormant / 90d Retired
        self.check_idle_timeouts(today, 30, 90);

        Ok(())
    }

    /// 计算两个标题的匹配度
    ///
    /// 先尝试基于单词的匹配（按空白/分隔符切分）。
    /// 如果单词数过少且长度偏长（通常为中文/无分隔符文本），
    /// 回退到字符级别的 Jaccard 相似度。
    fn title_overlap(query: &str, target: &str) -> bool {
        let words: Vec<&str> = query
            .split(|c: char| c.is_whitespace() || c == '/' || c == '-' || c == '—')
            .filter(|w| w.len() > 1)
            .collect();
        if words.is_empty() {
            return false;
        }

        let target_words: Vec<&str> = target
            .split(|c: char| c.is_whitespace() || c == '/' || c == '-' || c == '—')
            .filter(|w| w.len() > 1)
            .collect();
        if target_words.is_empty() {
            return false;
        }

        // 单词级匹配
        let overlap = words
            .iter()
            .filter(|w| target_words.iter().any(|tw| tw.eq_ignore_ascii_case(w)))
            .count();
        let max_len = words.len().max(target_words.len());
        if max_len > 0 {
            let ratio = overlap as f64 / max_len as f64;
            if ratio >= 0.5 {
                return true;
            }

            // 如果含有多字节词（中文特征），使用字符级 Jaccard 相似度作为后备。
            // 阈值 0.3（比英文宽松），因为中文同一 topic 在 LLM 输出里变异度高。
            let is_likely_cjk =
                words.iter().any(|w| w.len() > 5) || target_words.iter().any(|w| w.len() > 5);
            if is_likely_cjk {
                // 字符级 Jaccard：计算 query 和 target 的字符交集/并集
                let query_chars: std::collections::HashSet<char> = query.chars().collect();
                let target_chars: std::collections::HashSet<char> = target.chars().collect();
                let intersection = query_chars.intersection(&target_chars).count();
                let union = query_chars.union(&target_chars).count();
                if union > 0 && intersection as f64 / union as f64 >= 0.3 {
                    return true;
                }
            }

            // 子集匹配（Determinism 改进）：
            // 如果 A 的所有词都包含在 B 中，或反之，视为同一 thesis。
            // 处理 LLM 每次稍微改变标题的情况（如"AI Infrastructure" vs "AI Infrastructure Consolidation"）。
            if words.len() >= 2 {
                let all_q_in_t = words.iter().all(|w| target_words.iter().any(|tw| tw.eq_ignore_ascii_case(w)));
                let all_t_in_q = target_words.len() >= 2 && target_words.iter().all(|tw| words.iter().any(|w| w.eq_ignore_ascii_case(tw)));
                if all_q_in_t || all_t_in_q {
                    return true;
                }
            }
        }

        false
    }

    /// 标题关键词重叠匹配（>= 50% 关键词重叠视为同一 Thesis）
    ///
    /// 排除 Retired 和 Proposed。Dormant 可匹配（auto-wake 机制）。
    fn match_thesis(&self, title: &str) -> Option<usize> {
        self.theses
            .iter()
            .enumerate()
            .filter(|(_, t)| !matches!(t.status, ThesisStatus::Retired | ThesisStatus::Proposed))
            .find(|(_, t)| Self::title_overlap(title, &t.title))
            .map(|(i, _)| i)
    }

    /// 重新计算 Thesis 状态
    fn recompute_status(&self, idx: usize, today: &str) -> ThesisStatus {
        let thesis = &self.theses[idx];

        // 7 天窗口（today 只解析一次，放循环外避免重复解析）
        let today_date = chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d")
            .unwrap_or_else(|_| chrono::Local::now().date_naive());
        let recent: Vec<&Evidence> = thesis
            .evidences
            .iter()
            .filter(|e| {
                chrono::NaiveDate::parse_from_str(&e.date, "%Y-%m-%d")
                    .is_ok_and(|d| (0..=7).contains(&(today_date - d).num_days()))
            })
            .collect();

        // 无近期证据 → 弱化（而不是误标为 Active）
        if recent.is_empty() {
            return ThesisStatus::Weakening;
        }

        let support_count = recent
            .iter()
            .filter(|e| e.stance == Stance::Supports)
            .count();
        let challenge_count = recent
            .iter()
            .filter(|e| e.stance == Stance::Challenges)
            .count();

        if challenge_count > support_count && challenge_count > 0 {
            ThesisStatus::Weakening
        } else if support_count >= 2 {
            ThesisStatus::Strengthening
        } else {
            ThesisStatus::Active
        }
    }

    /// 生命周期管理: 30d Dormant / 90d Retired
    fn check_idle_timeouts(&mut self, today: &str, dormant_days: u32, retired_days: u32) {
        for i in 0..self.theses.len() {
            let should_skip = matches!(
                self.theses[i].status,
                ThesisStatus::Retired | ThesisStatus::Dormant | ThesisStatus::Proposed
            );
            if should_skip {
                continue;
            }
            if let Some(idle) = idle_days(today, &self.theses[i].updated) {
                if idle >= retired_days {
                    let desc = format!("{} days idle (>={})", idle, retired_days);
                    self.record_status_transition_inner(
                        i,
                        ThesisStatus::Retired,
                        TransitionTrigger::IdleTimeout,
                        &desc,
                    );
                } else if idle >= dormant_days {
                    let desc = format!("{} days idle (>={})", idle, dormant_days);
                    self.record_status_transition_inner(
                        i,
                        ThesisStatus::Dormant,
                        TransitionTrigger::IdleTimeout,
                        &desc,
                    );
                }
            }
        }
    }

    // ===== 内部辅助方法 =====

    /// 记录状态变更（内部：按 index 操作）
    fn record_status_transition_inner(
        &mut self,
        idx: usize,
        new_status: ThesisStatus,
        trigger: TransitionTrigger,
        description: &str,
    ) {
        let old_status = self.theses[idx].status.clone();
        self.theses[idx].status_history.push(StatusTransition {
            from: old_status,
            to: new_status.clone(),
            date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            trigger,
            description: description.to_string(),
        });
        self.theses[idx].status = new_status;
    }

    /// 记录置信度快照（内部：事件驱动）
    fn record_confidence_inner(&mut self, idx: usize, trigger: ConfidenceTrigger, reason: &str) {
        let new_value = compute_confidence(&self.theses[idx].evidences);

        // 事件驱动：仅在有意义的变化时记录
        let should_record = match trigger {
            ConfidenceTrigger::Initial => true,
            ConfidenceTrigger::StatusChange => true,
            ConfidenceTrigger::OutcomeRecorded => true,
            ConfidenceTrigger::ManualUpdate => true,
            ConfidenceTrigger::SignificantChange => {
                let last = self.theses[idx]
                    .confidence_history
                    .last()
                    .map(|s| s.value)
                    .unwrap_or(0.0);
                (new_value - last).abs() > 0.1
            }
        };

        if should_record {
            self.theses[idx]
                .confidence_history
                .push(ConfidenceSnapshot {
                    date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                    value: new_value,
                    trigger,
                    reason: reason.to_string(),
                });
        }
    }

    // ===== Meta Layer: Outcome & Reflection =====

    /// 记录 Thesis 的实际结果
    ///
    /// 比较 Thesis 的 prediction 与 reality，记录偏差。
    /// 自动触发：
    ///   - 置信度快照（OutcomeRecorded）
    ///   - 反思复盘生成（LLM draft）
    pub fn record_outcome(&mut self, outcome: Outcome) -> Result<()> {
        // 验证 thesis 存在
        if !self.theses.iter().any(|t| t.id == outcome.thesis_id) {
            anyhow::bail!("Thesis '{}' not found", outcome.thesis_id);
        }
        let thesis_id = outcome.thesis_id.clone();
        let description = outcome.description.clone();
        self.outcomes.push(outcome);

        // 记录置信度快照
        if let Some(idx) = self.theses.iter().position(|t| t.id == thesis_id) {
            self.record_confidence_inner(
                idx,
                ConfidenceTrigger::OutcomeRecorded,
                "outcome recorded",
            );
        }

        // 自动生成反思复盘
        if let Ok(reflection) = self.generate_reflection(&thesis_id) {
            self.reflections.push(reflection);
            log::info!("🧠 Meta Layer: {} — 反思复盘已生成", description);
        }

        Ok(())
    }

    /// 获取某个 Thesis 的所有 Outcome 记录
    #[allow(dead_code)]
    pub fn outcome_history(&self, thesis_id: &str) -> Vec<&Outcome> {
        self.outcomes
            .iter()
            .filter(|o| o.thesis_id == thesis_id)
            .collect()
    }

    /// 获取所有 Outcome 记录
    pub fn all_outcomes(&self) -> &[Outcome] {
        &self.outcomes
    }

    /// 记录今日决策到 thesis.decision_history（Stability Layer 持久化）
    ///
    /// 在 publishing.rs 的 map_theses_to_decisions() 之后调用，
    /// 确保决策历史在 memory.save() 前写入。
    pub fn record_decision(&mut self, thesis_id: &str, today: &str, decision_type: &str, confidence: f64) {
        if let Some(thesis) = self.theses.iter_mut().find(|t| t.id == thesis_id) {
            // 避免同一天重复写入
            let already_today = thesis.decision_history.last()
                .is_some_and(|s| s.date == today);
            if !already_today {
                thesis.decision_history.push(crate::domain::thesis::DecisionSnapshot {
                    date: today.to_string(),
                    decision_type: decision_type.to_string(),
                    confidence,
                });
            }
            // 只保留最近 30 天（防止无限增长）
            let keep_from = thesis.decision_history.len().saturating_sub(30);
            thesis.decision_history.drain(..keep_from);
        }
    }

    /// Registry-aware 版本：分配 Assessment ID 后再 update_from_analysis
    ///
    /// 在 publishing.rs 中调用（有 registry 时），替代 update_from_analysis。
    /// Registry 负责给每个 Thesis 分配稳定的 ASM-XXXX ID。
    pub fn update_from_analysis_with_registry(
        &mut self,
        today: &str,
        themes: &[Theme],
        analyses: &[ThemeAnalysis],
        registry: &mut crate::engine::registry::AssessmentRegistry,
    ) -> Result<()> {
        // 先执行常规更新
        self.update_from_analysis(today, themes, analyses)?;

        // 为每个 theme 匹配/分配 assessment_id
        for (theme, _analysis) in themes.iter().zip(analyses.iter()) {
            let title = &theme.title;

            // 查 Registry：找到相似 → 复用 ASM-ID + 加 alias
            let asm_id = if let Some(existing) = registry.find_similar(title) {
                registry.add_alias(&existing, title);
                existing
            } else {
                // 未找到 → 注册新 Assessment
                // thesis_id 先用占位符，后面找到 thesis 后更新
                registry.register(title, today, "pending")
            };

            // 给对应 thesis 绑定 assessment_id
            if let Some(thesis) = self.theses.iter_mut().find(|t| Self::title_overlap(title, &t.title)) {
                if thesis.assessment_id.is_none() {
                    thesis.assessment_id = Some(asm_id.clone());
                }
                // 更新 registry 的 thesis_id
                registry.update_thesis_id(&asm_id, &thesis.id);
            }
        }

        Ok(())
    }

    /// 生成 Thesis 的反思复盘
    ///
    /// 基于 Outcome 记录生成结构化反思：
    /// 什么判断错了、为什么错、学到了什么。
    pub fn generate_reflection(&self, thesis_id: &str) -> Result<Reflection> {
        let thesis = self
            .theses
            .iter()
            .find(|t| t.id == thesis_id)
            .ok_or_else(|| anyhow::anyhow!("Thesis '{}' not found", thesis_id))?;

        let outcomes: Vec<&Outcome> = self
            .outcomes
            .iter()
            .filter(|o| o.thesis_id == thesis_id)
            .collect();

        if outcomes.is_empty() {
            anyhow::bail!("No outcomes recorded for thesis '{}'", thesis_id);
        }

        // 找出最终结果
        let latest = outcomes.last().expect("non-empty after bail check above");
        let error_reason = match latest.verdict {
            OutcomeVerdict::Confirmed | OutcomeVerdict::PartiallyConfirmed => {
                "判断基本正确，但细节有偏差".to_string()
            }
            OutcomeVerdict::Invalidated => {
                // 从 Thesis 的 assumptions 推断错误原因
                let wrong_assumptions: Vec<String> = thesis
                    .assumptions
                    .iter()
                    .filter(|a| a.load_bearing && a.evidence_strength == "weak")
                    .map(|a| a.text.clone())
                    .collect();
                if wrong_assumptions.is_empty() {
                    "判断依赖的关键假设被证伪".to_string()
                } else {
                    format!("承重假设错误: {}", wrong_assumptions.join("; "))
                }
            }
            OutcomeVerdict::Unknown => "证据不足，尚无法判定".to_string(),
        };

        let lessons = vec![
            format!("结果: {}", latest.description),
            format!("错误原因: {}", error_reason),
        ];

        let verdict = match latest.verdict {
            OutcomeVerdict::Confirmed => "confirmed",
            OutcomeVerdict::PartiallyConfirmed => "partially-confirmed",
            OutcomeVerdict::Invalidated => "invalidated",
            OutcomeVerdict::Unknown => "unknown",
        };

        let support_count = thesis
            .evidences
            .iter()
            .filter(|e| e.stance == Stance::Supports)
            .count();
        let challenge_count = thesis
            .evidences
            .iter()
            .filter(|e| e.stance == Stance::Challenges)
            .count();
        let total_ev = support_count + challenge_count;
        let conf_val = if total_ev == 0 {
            0.5
        } else {
            let ratio = support_count as f64 / total_ev as f64;
            (0.5 + (ratio - 0.5) * 0.8).clamp(0.1, 0.98)
        };

        let reflection = Reflection {
            id: format!("reflection-{}", chrono::Utc::now().timestamp()),
            thesis_id: thesis_id.to_string(),
            outcome_id: latest.id.clone(),
            verdict: verdict.to_string(),
            error_reason,
            lessons,
            confidence_at_creation: conf_val,
            confidence_now: conf_val,
            created_at: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        };

        Ok(reflection)
    }

    /// 保存反思记录
    #[allow(dead_code)]
    pub fn save_reflection(&mut self, reflection: Reflection) {
        self.reflections.push(reflection);
    }

    /// 获取所有反思记录
    pub fn all_reflections(&self) -> &[Reflection] {
        &self.reflections
    }

    /// 按标题查找活跃 Thesis
    pub fn find_by_title(&self, title: &str) -> Option<&Thesis> {
        self.theses
            .iter()
            .filter(|t| t.status != ThesisStatus::Retired)
            .find(|t| Self::title_overlap(title, &t.title))
    }

    /// 按标题查找活跃 Thesis（可变引用），供 Hermes 写入 Evidence
    pub fn find_by_title_mut(&mut self, title: &str) -> Option<&mut Thesis> {
        let pos = self
            .theses
            .iter()
            .enumerate()
            .filter(|(_, t)| t.status != ThesisStatus::Retired)
            .find(|(_, t)| Self::title_overlap(title, &t.title))
            .map(|(i, _)| i);
        pos.map(move |i| &mut self.theses[i])
    }

    /// 强制创建一个新 Thesis（供 Hermes discovery 使用）
    pub fn force_thesis(&mut self, title: String, today: &str, bluf: &str) {
        if self.find_by_title(&title).is_some() {
            return;
        }
        self.theses.push(Thesis {
            id: format!("thesis-{}", chrono::Utc::now().timestamp()),
            title,
            created: today.to_string(),
            updated: today.to_string(),
            evidences: vec![Evidence {
                date: today.to_string(),
                title: "Hermes 自动发现".into(),
                source: "Hermes.Discovery".into(),
                summary: bluf.to_string(),
                stance: Stance::Supports,
                signal_strength: 6,
            }],
            assumptions: vec![],
            status: ThesisStatus::Proposed,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
        });
    }

    // ===== Public API: 置信度 & 状态 =====

    /// 记录置信度快照（公开 API，按 thesis_id）
    #[allow(dead_code)]
    pub fn record_confidence(
        &mut self,
        thesis_id: &str,
        trigger: ConfidenceTrigger,
        reason: &str,
    ) -> Result<()> {
        let idx = self
            .theses
            .iter()
            .position(|t| t.id == thesis_id)
            .ok_or_else(|| anyhow::anyhow!("Thesis '{}' not found", thesis_id))?;
        self.record_confidence_inner(idx, trigger, reason);
        Ok(())
    }

    /// 记录状态变更（公开 API，按 thesis_id）
    #[allow(dead_code)]
    pub fn record_status_transition(
        &mut self,
        thesis_id: &str,
        new_status: ThesisStatus,
        trigger: TransitionTrigger,
        description: &str,
    ) -> Result<()> {
        let idx = self
            .theses
            .iter()
            .position(|t| t.id == thesis_id)
            .ok_or_else(|| anyhow::anyhow!("Thesis '{}' not found", thesis_id))?;
        self.record_status_transition_inner(idx, new_status, trigger, description);
        Ok(())
    }

    /// 计算 Thesis 的历史正确率
    #[allow(dead_code)]
    pub fn historical_accuracy(&self, thesis_id: &str) -> Option<f64> {
        let outcomes: Vec<&Outcome> = self
            .outcomes
            .iter()
            .filter(|o| o.thesis_id == thesis_id)
            .collect();
        if outcomes.is_empty() {
            return None;
        }
        let total = outcomes.len() as f64;
        let score: f64 = outcomes
            .iter()
            .map(|o| match o.verdict {
                OutcomeVerdict::Confirmed => 1.0,
                OutcomeVerdict::PartiallyConfirmed => 0.5,
                OutcomeVerdict::Invalidated | OutcomeVerdict::Unknown => 0.0,
            })
            .sum();
        Some(score / total)
    }

    // ===== Test helpers (only available in #[cfg(test)]) =====

    /// 测试用：直接添加 Thesis（不经过 update_from_analysis）
    #[cfg(test)]
    pub(crate) fn test_add_thesis(&mut self, thesis: crate::domain::thesis::Thesis) {
        self.theses.push(thesis);
    }

    /// 测试用：直接添加 Outcome（不经过管线）
    #[cfg(test)]
    pub(crate) fn test_add_outcome(&mut self, outcome: Outcome) {
        self.outcomes.push(outcome);
    }
}

/// JSON 持久化包装
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryEngineData {
    theses: Vec<Thesis>,
    #[serde(default)]
    outcomes: Vec<Outcome>,
    #[serde(default)]
    reflections: Vec<Reflection>,
    #[serde(default)]
    investigations: Vec<Investigation>,
}

impl From<&MemoryEngine> for MemoryEngineData {
    fn from(mem: &MemoryEngine) -> Self {
        Self {
            theses: mem.theses.clone(),
            outcomes: mem.outcomes.clone(),
            reflections: mem.reflections.clone(),
            investigations: mem.investigations.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::evidence::Stance;
    use crate::domain::theme::Assumption;
    use crate::domain::thesis::ThesisStatus;

    fn make_theme(title: &str, sources: Vec<String>) -> Theme {
        Theme {
            id: "t1".into(),
            title: title.into(),
            summary: "test summary".into(),
            articles: vec![],
            sources,
        }
    }

    fn make_analysis(
        title: &str,
        bluf: &str,
        signal_strength: u8,
        assumptions: Vec<Assumption>,
    ) -> ThemeAnalysis {
        ThemeAnalysis {
            theme_id: "t1".into(),
            theme_title: title.into(),
            bluf: bluf.into(),
            impact: "test".into(),
            geopolitical_fact: "test".into(),
            supply_chain_impact: "test".into(),
            analysis_paragraph: String::new(),
            evidence_level: String::new(),
            signal_strength,
            fact_base: vec![],
            connections: vec![],
            source_urls: vec![],
            assumptions,
            adverse: None,
            next_tests: vec![],
            open_questions: vec![],
            chains: vec![],
            what_to_do: String::new(),
            what_to_watch: String::new(),
            falsification_conditions: vec![],
        }
    }

    fn make_memory() -> MemoryEngine {
        let tmp = std::env::temp_dir().join("test_memory_db.json");
        let _ = std::fs::remove_file(&tmp);
        MemoryEngine::new(tmp)
    }

    #[test]
    fn test_match_thesis_exact_word() {
        let mut mem = make_memory();
        // 注入一个英文 thesis
        mem.theses.push(Thesis {
            id: "t1".into(),
            title: "AI Commoditization".into(),
            created: "2026-06-01".into(),
            updated: "2026-06-01".into(),
            evidences: vec![],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
        });

        // 完全匹配
        let result = mem.match_thesis("AI Commoditization");
        assert!(result.is_some(), "exact match should find thesis");
    }

    #[test]
    fn test_match_thesis_word_overlap() {
        let mut mem = make_memory();
        mem.theses.push(Thesis {
            id: "t1".into(),
            title: "AI Commoditization Trends".into(),
            created: "2026-06-01".into(),
            updated: "2026-06-01".into(),
            evidences: vec![],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
        });

        // "AI Commoditization" 与 "AI Commoditization Trends" 有 2/3 重叠
        let result = mem.match_thesis("AI Commoditization");
        assert!(result.is_some(), "word overlap should match");
    }

    #[test]
    fn test_match_thesis_chinese_char_fallback() {
        let mut mem = make_memory();
        mem.theses.push(Thesis {
            id: "t1".into(),
            title: "模型商品化".into(),
            created: "2026-06-01".into(),
            updated: "2026-06-01".into(),
            evidences: vec![],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
        });

        // "模型商品化趋势" 应通过字符级 Jaccard 后备匹配 "模型商品化"
        // 字符级 Jaccard: {"模","型","商","品","化","趋","势"} ∩ {"模","型","商","品","化"} = 5
        // ∪ = 7, 5/7 ≈ 0.71 >= 0.4
        let result = mem.match_thesis("模型商品化趋势");
        assert!(
            result.is_some(),
            "Chinese char-level Jaccard fallback should match: {:?}",
            result
        );
    }

    #[test]
    fn test_match_thesis_no_match_unrelated() {
        let mut mem = make_memory();
        mem.theses.push(Thesis {
            id: "t1".into(),
            title: "AI Commoditization".into(),
            created: "2026-06-01".into(),
            updated: "2026-06-01".into(),
            evidences: vec![],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
        });

        let result = mem.match_thesis("Weather Forecast");
        assert!(result.is_none(), "unrelated titles should not match");
    }

    #[test]
    fn test_match_thesis_skips_retired() {
        let mut mem = make_memory();
        mem.theses.push(Thesis {
            id: "t1".into(),
            title: "AI Commoditization".into(),
            created: "2026-06-01".into(),
            updated: "2026-06-01".into(),
            evidences: vec![],
            assumptions: vec![],
            status: ThesisStatus::Retired,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
        });

        let result = mem.match_thesis("AI Commoditization");
        assert!(result.is_none(), "retired thesis should not match");
    }

    #[test]
    fn test_recompute_status_empty_window() {
        let mut mem = make_memory();
        // Thesis 有过期证据（30 天前），7天窗口内无证据
        mem.theses.push(Thesis {
            id: "t1".into(),
            title: "test".into(),
            created: "2026-01-01".into(),
            updated: "2026-01-01".into(),
            evidences: vec![Evidence {
                date: "2026-01-01".into(),
                title: "old evidence".into(),
                source: "test".into(),
                summary: "old".into(),
                stance: Stance::Supports,
                signal_strength: 5,
            }],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
        });

        let status = mem.recompute_status(0, "2026-06-24");
        assert_eq!(
            status,
            ThesisStatus::Weakening,
            "no recent evidence should produce Weakening, not Active"
        );
    }

    #[test]
    fn test_recompute_status_strengthening() {
        let mut mem = make_memory();
        // 2 条支持证据在 7 天内
        mem.theses.push(Thesis {
            id: "t1".into(),
            title: "test".into(),
            created: "2026-06-01".into(),
            updated: "2026-06-24".into(),
            evidences: vec![
                Evidence {
                    date: "2026-06-23".into(),
                    title: "support 1".into(),
                    source: "test".into(),
                    summary: "s1".into(),
                    stance: Stance::Supports,
                    signal_strength: 7,
                },
                Evidence {
                    date: "2026-06-24".into(),
                    title: "support 2".into(),
                    source: "test".into(),
                    summary: "s2".into(),
                    stance: Stance::Supports,
                    signal_strength: 7,
                },
            ],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
        });

        let status = mem.recompute_status(0, "2026-06-24");
        assert_eq!(status, ThesisStatus::Strengthening);
    }

    #[test]
    fn test_recompute_status_weakening() {
        let mut mem = make_memory();
        // 挑战证据多于支持证据
        mem.theses.push(Thesis {
            id: "t1".into(),
            title: "test".into(),
            created: "2026-06-01".into(),
            updated: "2026-06-24".into(),
            evidences: vec![Evidence {
                date: "2026-06-24".into(),
                title: "challenge".into(),
                source: "test".into(),
                summary: "c1".into(),
                stance: Stance::Challenges,
                signal_strength: 7,
            }],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
        });

        let status = mem.recompute_status(0, "2026-06-24");
        assert_eq!(status, ThesisStatus::Weakening);
    }

    #[test]
    fn test_retire_stale_idle() {
        let mut mem = make_memory();
        mem.theses.push(Thesis {
            id: "t1".into(),
            title: "stale thesis".into(),
            created: "2026-01-01".into(),
            updated: "2026-01-01".into(),
            evidences: vec![],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
        });

        // 超过 30 天 idle
        mem.check_idle_timeouts("2026-06-24", 30, 90);
        assert_eq!(mem.theses[0].status, ThesisStatus::Retired);
    }

    #[test]
    fn test_retire_stale_not_yet() {
        let mut mem = make_memory();
        mem.theses.push(Thesis {
            id: "t1".into(),
            title: "fresh thesis".into(),
            created: "2026-06-24".into(),
            updated: "2026-06-24".into(),
            evidences: vec![],
            assumptions: vec![],
            status: ThesisStatus::Active,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
        });

        // 仅 0 天 idle
        mem.check_idle_timeouts("2026-06-24", 30, 90);
        assert_eq!(mem.theses[0].status, ThesisStatus::Active);
    }

    #[test]
    fn test_retire_stale_skips_retired() {
        let mut mem = make_memory();
        mem.theses.push(Thesis {
            id: "t1".into(),
            title: "already retired".into(),
            created: "2026-01-01".into(),
            updated: "2026-01-01".into(),
            evidences: vec![],
            assumptions: vec![],
            status: ThesisStatus::Retired,
            confidence_history: vec![],
            status_history: vec![],
            parent_id: None,
            merged_ids: vec![],
            related_thesis_ids: vec![],
            metadata: std::collections::HashMap::new(),
            investigation_id: None,
            decision_history: vec![],
            falsification_conditions: vec![],
            assessment_id: None,
        });

        // 即使 idle 超过 30 天，已 Retired 的应被跳过
        mem.check_idle_timeouts("2026-06-24", 30, 90);
        assert_eq!(mem.theses[0].status, ThesisStatus::Retired);
    }

    #[test]
    fn test_force_thesis_creates_new() {
        let mut mem = make_memory();
        mem.force_thesis("New Thesis".into(), "2026-06-24", "test bluf");
        assert_eq!(mem.theses.len(), 1);
        assert_eq!(mem.theses[0].title, "New Thesis");
    }

    #[test]
    fn test_force_thesis_duplicate_guard() {
        let mut mem = make_memory();
        mem.force_thesis("AI Commoditization".into(), "2026-06-24", "first");
        mem.force_thesis("AI Commoditization".into(), "2026-06-24", "second");
        assert_eq!(
            mem.theses.len(),
            1,
            "duplicate force_thesis should not create second thesis"
        );
    }

    #[test]
    fn test_update_from_analysis_creates_thesis() {
        let mut mem = make_memory();
        let themes = vec![make_theme("AI Commoditization", vec!["source1".into()])];
        let analyses = vec![make_analysis("AI Commoditization", "test bluf", 7, vec![])];

        mem.update_from_analysis("2026-06-24", &themes, &analyses)
            .unwrap();
        assert_eq!(mem.theses.len(), 1);
        assert_eq!(mem.theses[0].title, "AI Commoditization");
    }

    #[test]
    fn test_update_from_analysis_source_sentinel() {
        let mut mem = make_memory();
        // 空 sources 列表
        let themes = vec![make_theme("AI Commoditization", vec![])];
        let analyses = vec![make_analysis("AI Commoditization", "test bluf", 7, vec![])];

        mem.update_from_analysis("2026-06-24", &themes, &analyses)
            .unwrap();
        assert_eq!(
            mem.theses[0].evidences[0].source, "unknown",
            "empty source should fall back to sentinel"
        );
    }

    #[test]
    fn test_update_from_analysis_appends_evidence() {
        let mut mem = make_memory();
        let themes = vec![make_theme("AI Commoditization", vec!["src".into()])];
        let analyses = vec![make_analysis("AI Commoditization", "bluf", 7, vec![])];

        // 第一次更新创建 thesis
        mem.update_from_analysis("2026-06-23", &themes, &analyses)
            .unwrap();
        assert_eq!(
            mem.theses[0].evidences.len(),
            1,
            "first run creates 1 evidence"
        );

        // 第二次更新：相同 title+source → dedup，不追加
        mem.update_from_analysis("2026-06-24", &themes, &analyses)
            .unwrap();
        assert_eq!(
            mem.theses[0].evidences.len(),
            1,
            "dedup: same title+source should not create duplicate"
        );

        // 第三次更新：不同 source → 应追加
        let themes2 = vec![make_theme("AI Commoditization", vec!["other-src".into()])];
        mem.update_from_analysis("2026-06-25", &themes2, &analyses)
            .unwrap();
        assert_eq!(
            mem.theses[0].evidences.len(),
            2,
            "different source should create new evidence"
        );
    }
}
