//! MemoryEngine — 信念追踪系统
//!
//! 职责：维护 `Thesis: Vec<Evidence>` 结构，每日更新。
//! 输出不公开，只存储。目标是形成 "判断→证据→修正" 的连续积累。
//!
//! 与 belief_engine.rs 并行运行（不替换），独立存储。

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::clusterer::{Theme, ThemeAnalysis};

// ===== 数据模型（从 domain 层导入）=====

pub use crate::domain::evidence::{Evidence, Stance};
pub use crate::domain::outcome::{Outcome, OutcomeType};
pub use crate::domain::reflection::Reflection;
pub use crate::domain::thesis::{Thesis, ThesisStatus};

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
    /// memory_db.json 路径
    #[serde(skip)]
    memory_path: PathBuf,
}

// ===== 实现 =====

impl MemoryEngine {
    /// 创建新实例（不自动加载）
    pub fn new(memory_path: PathBuf) -> Self {
        Self {
            theses: Vec::new(),
            outcomes: Vec::new(),
            reflections: Vec::new(),
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

    /// 核心更新：将当日分析结果融入信念系统
    ///
    /// 遍历 themes/analyses，匹配或新建 Thesis，追加 Evidence，更新状态。
    pub fn update_from_analysis(
        &mut self,
        today: &str,
        themes: &[Theme],
        analyses: &[ThemeAnalysis],
    ) -> Result<()> {
        for (theme, analysis) in themes.iter().zip(analyses.iter()) {
            let title = &theme.title;

            // 尝试匹配现有 Thesis
            let matched = self.match_thesis(title);

            if let Some(idx) = matched {
                // 追加证据
                self.theses[idx].evidences.push(Evidence {
                    date: today.to_string(),
                    title: title.clone(),
                    source: theme
                        .sources
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string()),
                    summary: analysis.bluf.clone(),
                    stance: if analysis.signal_strength >= 3 {
                        Stance::Supports
                    } else {
                        Stance::Challenges
                    },
                    signal_strength: analysis.signal_strength,
                });
                self.theses[idx].updated = today.to_string();
                self.theses[idx].status = self.recompute_status(idx, today);
            } else {
                // 新建 Thesis
                self.theses.push(Thesis {
                    id: format!("thesis-{}", chrono::Utc::now().timestamp()),
                    title: title.clone(),
                    created: today.to_string(),
                    updated: today.to_string(),
                    evidences: vec![Evidence {
                        date: today.to_string(),
                        title: title.clone(),
                        source: theme
                            .sources
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "unknown".to_string()),
                        summary: analysis.bluf.clone(),
                        stance: Stance::Supports,
                        signal_strength: analysis.signal_strength,
                    }],
                    assumptions: analysis.assumptions.clone(),
                    status: ThesisStatus::Active,
                });
            }
        }

        // 退休检查
        self.retire_stale(today, 30);

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

            // 如果单词数很少（1-2 个）且单词偏长（>5 字节，中文特征），
            // 使用字符级 Jaccard 相似度作为后备
            let is_likely_cjk =
                words.iter().any(|w| w.len() > 5) || target_words.iter().any(|w| w.len() > 5);
            if is_likely_cjk && words.len() <= 2 && target_words.len() <= 2 {
                // 字符级 Jaccard：计算 query 和 target 的字符交集/并集
                let query_chars: std::collections::HashSet<char> = query.chars().collect();
                let target_chars: std::collections::HashSet<char> = target.chars().collect();
                let intersection = query_chars.intersection(&target_chars).count();
                let union = query_chars.union(&target_chars).count();
                if union > 0 && intersection as f64 / union as f64 >= 0.4 {
                    return true;
                }
            }
        }

        false
    }

    /// 标题关键词重叠匹配（>= 50% 关键词重叠视为同一 Thesis）
    fn match_thesis(&self, title: &str) -> Option<usize> {
        self.theses
            .iter()
            .enumerate()
            .filter(|(_, t)| t.status != ThesisStatus::Retired)
            .find(|(_, t)| Self::title_overlap(title, &t.title))
            .map(|(i, _)| i)
    }

    /// 重新计算 Thesis 状态
    fn recompute_status(&self, idx: usize, today: &str) -> ThesisStatus {
        let thesis = &self.theses[idx];

        // 7 天窗口
        let recent: Vec<&Evidence> = thesis
            .evidences
            .iter()
            .filter(|e| {
                if let Ok(d) = chrono::NaiveDate::parse_from_str(&e.date, "%Y-%m-%d") {
                    if let Ok(t) = chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d") {
                        let diff = (t - d).num_days();
                        return (0..=7).contains(&diff);
                    }
                }
                false
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

    /// 退休检查：连续 N 天无新 Evidence 的 Thesis → Retired
    fn retire_stale(&mut self, today: &str, max_idle_days: u32) {
        for thesis in &mut self.theses {
            if thesis.status == ThesisStatus::Retired {
                continue;
            }
            if let Ok(updated) = chrono::NaiveDate::parse_from_str(&thesis.updated, "%Y-%m-%d") {
                if let Ok(today_d) = chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d") {
                    let idle = (today_d - updated).num_days() as u32;
                    if idle >= max_idle_days {
                        thesis.status = ThesisStatus::Retired;
                    }
                } else {
                    log::warn!("⚠️ retire_stale: 无法解析 today 日期 '{}'", today);
                }
            } else {
                log::warn!(
                    "⚠️ retire_stale: 无法解析 thesis.updated 日期 '{}' (id: {})",
                    thesis.updated,
                    thesis.id
                );
            }
        }
    }

    // ===== Meta Layer: Outcome & Reflection =====

    /// 记录 Thesis 的实际结果
    ///
    /// 比较 Thesis 的 prediction 与 reality，记录偏差。
    /// 同时触发 PipelineEventType::OutcomeRecorded（由调用方负责推送 EventLog）。
    pub fn record_outcome(&mut self, outcome: Outcome) -> Result<()> {
        // 验证 thesis 存在
        if !self.theses.iter().any(|t| t.id == outcome.thesis_id) {
            anyhow::bail!("Thesis '{}' not found", outcome.thesis_id);
        }
        self.outcomes.push(outcome);
        Ok(())
    }

    /// 获取某个 Thesis 的所有 Outcome 记录
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
        let latest = outcomes.last().unwrap();
        let error_reason = match latest.result {
            OutcomeType::Confirmed | OutcomeType::PartiallyConfirmed => {
                "判断基本正确，但细节有偏差".to_string()
            }
            OutcomeType::Refuted => {
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
            OutcomeType::Inconclusive => "证据不足，尚无法判定".to_string(),
        };

        let lessons = vec![
            format!("预期: {} vs 实际: {}", latest.expected, latest.actual),
            format!("错误原因: {}", error_reason),
        ];

        let verdict = match latest.result {
            OutcomeType::Confirmed => "confirmed",
            OutcomeType::PartiallyConfirmed => "partially-confirmed",
            OutcomeType::Refuted => "refuted",
            OutcomeType::Inconclusive => "inconclusive",
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
            status: ThesisStatus::Active,
        });
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
}

impl From<&MemoryEngine> for MemoryEngineData {
    fn from(mem: &MemoryEngine) -> Self {
        Self {
            theses: mem.theses.clone(),
            outcomes: mem.outcomes.clone(),
            reflections: mem.reflections.clone(),
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
        });

        // 超过 30 天 idle
        mem.retire_stale("2026-06-24", 30);
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
        });

        // 仅 0 天 idle
        mem.retire_stale("2026-06-24", 30);
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
        });

        // 即使 idle 超过 30 天，已 Retired 的应被跳过
        mem.retire_stale("2026-06-24", 30);
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
        // 第二次更新追加证据
        mem.update_from_analysis("2026-06-24", &themes, &analyses)
            .unwrap();

        assert_eq!(mem.theses.len(), 1, "should not create duplicate");
        assert_eq!(
            mem.theses[0].evidences.len(),
            2,
            "should have 2 evidence entries"
        );
    }
}
