//! Assessment Registry — 判断身份注册表
//!
//! 每个战略判断（Assessment）拥有稳定的 ASM-XXXX ID，
//! 不随 LLM 重命名或重聚类而改变。
//!
//! Registry 职责：
//!   1. 给每个新 Assessment 分配唯一 ID（ASM-0001, ASM-0002...）
//!   2. 通过标题相似度匹配已有 Assessment（Alias 解析）
//!   3. 持久化到 assessment_registry.json
//!
//! 文件存储：
//!   vault_path/assessment_registry.json

use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};

/// 单个 Assessment 的注册记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentEntry {
    /// 规范标题（首次创建时的标题）
    pub canonical_title: String,
    /// 已知别名（后续 LLM 产生的其他标题变体）
    #[serde(default)]
    pub aliases: Vec<String>,
    /// 首次创建日期
    pub created: String,
    /// 对应的内部 thesis.id（可能随 memory engine 更新）
    pub thesis_id: String,
}

/// Assessment Registry — 全量注册表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentRegistry {
    /// 下一个可用 ID 序号（单调递增）
    pub next_id: u32,
    /// ASM-ID → AssessmentEntry 映射
    pub assessments: HashMap<String, AssessmentEntry>,
}

impl Default for AssessmentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AssessmentRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            assessments: HashMap::new(),
        }
    }

    /// 从文件加载；文件不存在则返回空 Registry
    pub fn load_or_new(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// 持久化到文件
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// 查找与 title 相似的已有 Assessment，返回 ASM-ID（若找到）
    ///
    /// 匹配策略：词级 Jaccard ≥ 0.75，或子集匹配（短标题的所有词出现在长标题中）
    /// 检查 canonical_title 和所有 aliases
    pub fn find_similar(&self, title: &str) -> Option<String> {
        for (asm_id, entry) in &self.assessments {
            let candidates = std::iter::once(&entry.canonical_title)
                .chain(entry.aliases.iter());
            for known in candidates {
                if jaccard_similarity(title, known) >= 0.75 || is_subset_match(title, known) {
                    return Some(asm_id.clone());
                }
            }
        }
        None
    }

    /// 注册新 Assessment，返回分配的 ASM-ID
    pub fn register(&mut self, title: &str, today: &str, thesis_id: &str) -> String {
        let asm_id = format!("ASM-{:04}", self.next_id);
        self.next_id += 1;
        self.assessments.insert(
            asm_id.clone(),
            AssessmentEntry {
                canonical_title: title.to_string(),
                aliases: vec![],
                created: today.to_string(),
                thesis_id: thesis_id.to_string(),
            },
        );
        asm_id
    }

    /// 给已有 Assessment 添加 alias（若不重复）
    pub fn add_alias(&mut self, asm_id: &str, alias: &str) {
        if let Some(entry) = self.assessments.get_mut(asm_id) {
            if entry.canonical_title != alias && !entry.aliases.contains(&alias.to_string()) {
                entry.aliases.push(alias.to_string());
            }
        }
    }

    /// 更新 thesis_id 绑定（当 memory engine 创建新 Thesis 时）
    pub fn update_thesis_id(&mut self, asm_id: &str, thesis_id: &str) {
        if let Some(entry) = self.assessments.get_mut(asm_id) {
            entry.thesis_id = thesis_id.to_string();
        }
    }
}

/// 词级 Jaccard 相似度（大小写不敏感）
fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let words_a: std::collections::HashSet<String> =
        a.split_whitespace().map(|w| w.to_lowercase()).collect();
    let words_b: std::collections::HashSet<String> =
        b.split_whitespace().map(|w| w.to_lowercase()).collect();
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    intersection as f64 / union as f64
}

/// 子集匹配：较短标题的所有词（≥2）出现在较长标题中
fn is_subset_match(a: &str, b: &str) -> bool {
    let words_a: Vec<String> = a.split_whitespace().map(|w| w.to_lowercase()).collect();
    let words_b: Vec<String> = b.split_whitespace().map(|w| w.to_lowercase()).collect();
    if words_a.len() < 2 || words_b.len() < 2 {
        return false;
    }
    let all_a_in_b = words_a.iter().all(|w| words_b.contains(w));
    let all_b_in_a = words_b.iter().all(|w| words_a.contains(w));
    all_a_in_b || all_b_in_a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_similar_match() {
        let mut reg = AssessmentRegistry::new();
        reg.register("AI Infrastructure Consolidation", "2026-06-12", "t1");
        // Exact match
        assert!(reg.find_similar("AI Infrastructure Consolidation").is_some());
        // High overlap match
        assert!(reg.find_similar("AI Infrastructure").is_some());
    }

    #[test]
    fn test_find_similar_no_match() {
        let mut reg = AssessmentRegistry::new();
        reg.register("AI Infrastructure", "2026-06-12", "t1");
        assert!(reg.find_similar("OpenAI Consumer Layer").is_none());
    }

    #[test]
    fn test_register_increments_id() {
        let mut reg = AssessmentRegistry::new();
        let id1 = reg.register("Topic A", "2026-06-26", "t1");
        let id2 = reg.register("Topic B", "2026-06-26", "t2");
        assert_eq!(id1, "ASM-0001");
        assert_eq!(id2, "ASM-0002");
        assert_eq!(reg.next_id, 3);
    }

    #[test]
    fn test_alias_dedup() {
        let mut reg = AssessmentRegistry::new();
        let id = reg.register("AI Infrastructure", "2026-06-12", "t1");
        reg.add_alias(&id, "AI Infra");
        reg.add_alias(&id, "AI Infra"); // duplicate
        assert_eq!(reg.assessments[&id].aliases.len(), 1);
    }
}
