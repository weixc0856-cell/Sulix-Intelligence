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
pub use crate::domain::thesis::{Thesis, ThesisStatus};

/// 信念追踪引擎
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEngine {
    /// 所有 Thesis
    theses: Vec<Thesis>,
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
            memory_path,
        }
    }

    /// 从 `memory_db.json` 加载
    pub fn load(&mut self) -> Result<()> {
        if self.memory_path.exists() {
            let content = std::fs::read_to_string(&self.memory_path)?;
            let loaded: MemoryEngineData = serde_json::from_str(&content)?;
            self.theses = loaded.theses;
        }
        Ok(())
    }

    /// 写入 `memory_db.json`
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.memory_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = MemoryEngineData {
            theses: self.theses.clone(),
        };
        let json = serde_json::to_string_pretty(&data)?;
        std::fs::write(&self.memory_path, json)?;
        Ok(())
    }

    /// 获取所有 Thesis
    pub fn theses(&self) -> &[Thesis] {
        &self.theses
    }

    /// 获取活跃（非 Retired）Thesis
    #[allow(dead_code)]
    pub fn active_theses(&self) -> Vec<&Thesis> {
        self.theses
            .iter()
            .filter(|t| t.status != ThesisStatus::Retired)
            .collect()
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
                    source: theme.sources.first().cloned().unwrap_or_default(),
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
                        source: theme.sources.first().cloned().unwrap_or_default(),
                        summary: analysis.bluf.clone(),
                        stance: Stance::Supports,
                        signal_strength: analysis.signal_strength,
                    }],
                    status: ThesisStatus::Active,
                });
            }
        }

        // 退休检查
        self.retire_stale(today, 30);

        Ok(())
    }

    /// 标题关键词重叠匹配（>= 50% 关键词重叠视为同一 Thesis）
    fn match_thesis(&self, title: &str) -> Option<usize> {
        let words: Vec<&str> = title
            .split(|c: char| c.is_whitespace() || c == '/' || c == '-' || c == '—')
            .filter(|w| w.len() > 1)
            .collect();
        if words.is_empty() {
            return None;
        }

        for (i, thesis) in self.theses.iter().enumerate() {
            if thesis.status == ThesisStatus::Retired {
                continue;
            }
            let thesis_words: Vec<&str> = thesis
                .title
                .split(|c: char| c.is_whitespace() || c == '/' || c == '-' || c == '—')
                .filter(|w| w.len() > 1)
                .collect();
            if thesis_words.is_empty() {
                continue;
            }

            let overlap = words
                .iter()
                .filter(|w| thesis_words.iter().any(|tw| tw.eq_ignore_ascii_case(w)))
                .count();
            let max_len = words.len().max(thesis_words.len());
            if max_len > 0 && overlap as f64 / max_len as f64 >= 0.5 {
                return Some(i);
            }
        }

        None
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
                }
            }
        }
    }

    /// 按标题查找活跃 Thesis
    pub fn find_by_title(&self, title: &str) -> Option<&Thesis> {
        let words: Vec<&str> = title
            .split(|c: char| c.is_whitespace() || c == '/' || c == '-' || c == '—')
            .filter(|w| w.len() > 1)
            .collect();
        if words.is_empty() {
            return None;
        }
        self.theses.iter().find(|t| {
            if t.status == ThesisStatus::Retired {
                return false;
            }
            let tw: Vec<&str> = t
                .title
                .split(|c: char| c.is_whitespace() || c == '/' || c == '-' || c == '—')
                .filter(|w| w.len() > 1)
                .collect();
            if tw.is_empty() {
                return false;
            }
            let overlap = words
                .iter()
                .filter(|w| tw.iter().any(|tw| tw.eq_ignore_ascii_case(w)))
                .count();
            let max_len = words.len().max(tw.len());
            max_len > 0 && overlap as f64 / max_len as f64 >= 0.5
        })
    }

    /// 按标题查找活跃 Thesis（可变引用），供 Hermes 写入 Evidence
    pub fn find_by_title_mut(&mut self, title: &str) -> Option<&mut Thesis> {
        let words: Vec<&str> = title
            .split(|c: char| c.is_whitespace() || c == '/' || c == '-' || c == '—')
            .filter(|w| w.len() > 1)
            .collect();
        if words.is_empty() {
            return None;
        }
        let pos = self.theses.iter().position(|t| {
            if t.status == ThesisStatus::Retired {
                return false;
            }
            let tw: Vec<&str> = t
                .title
                .split(|c: char| c.is_whitespace() || c == '/' || c == '-' || c == '—')
                .filter(|w| w.len() > 1)
                .collect();
            if tw.is_empty() {
                return false;
            }
            let overlap = words
                .iter()
                .filter(|w| tw.iter().any(|tw| tw.eq_ignore_ascii_case(w)))
                .count();
            let max_len = words.len().max(tw.len());
            max_len > 0 && overlap as f64 / max_len as f64 >= 0.5
        });
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
            status: ThesisStatus::Active,
        });
    }
}

/// JSON 持久化包装
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryEngineData {
    theses: Vec<Thesis>,
}
