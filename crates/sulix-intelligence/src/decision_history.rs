//! DecisionHistory — 最小 Memory 存储
//!
//! 这是 Memory 的第一阶段：只存储"我过去做了什么决策"。
//! 格式为 JSONL (JSON Lines)，append-only，每天追加。
//!
//! 记录示例：
//!   {"decision_id":"dec_001","thesis_id":"thesis_001","made_at":"2026-07-12",
//!    "action":"Invest","confidence":0.7,"outcome":null}
//!
//! Phase 3 扩展方向：
//!   - outcome 字段从 null 填充为 Confirmed/Invalidated
//!   - 自动 Reflection 生成
//!   - Belief Update

use anyhow::Result;

/// 单条决策历史记录
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecisionRecord {
    /// 决策 ID
    pub decision_id: String,
    /// 关联的 Thesis ID
    pub thesis_id: String,
    /// 决策时间（ISO 8601）
    pub made_at: String,
    /// 决策类型
    pub action: String,
    /// 决策时置信度
    pub confidence: f64,
    /// 结果（Phase 3 填充）
    #[serde(default)]
    pub outcome: Option<String>,
}

/// DecisionHistory 存储
///
/// # 设计原则
/// - Append-only JSONL：不会修改历史记录
/// - 每天读取时去重加载
/// - 文件锁：不适用于并发写入（管线是单线程的）
#[derive(Debug)]
pub struct DecisionHistory {
    /// JSONL 文件路径
    path: std::path::PathBuf,
    /// 内存中的记录（已去重）
    records: Vec<DecisionRecord>,
}

impl DecisionHistory {
    /// 打开或创建 DecisionHistory 存储
    pub fn open(path: impl Into<std::path::PathBuf>) -> Result<Self> {
        let path = path.into();
        let mut history = Self {
            records: Vec::new(),
            path: path.clone(),
        };

        // 如果文件存在，加载已有记录
        if path.exists() {
            history.reload()?;
            log::info!("📜 DecisionHistory: 已加载 {} 条历史记录", history.records.len());
        } else {
            log::info!("📜 DecisionHistory: 新实例");
        }

        Ok(history)
    }

    /// 重新加载 JSONL 文件（去重加载）
    pub fn reload(&mut self) -> Result<()> {
        if !self.path.exists() {
            self.records.clear();
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.path)?;
        let mut seen = std::collections::HashSet::new();
        self.records = content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }
                match serde_json::from_str::<DecisionRecord>(line) {
                    Ok(record) => {
                        // 按 decision_id 去重
                        if seen.insert(record.decision_id.clone()) {
                            Some(record)
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        log::warn!("⚠️ DecisionHistory: 忽略损坏的记录行: {}", e);
                        None
                    }
                }
            })
            .collect();

        Ok(())
    }

    /// 追加一条决策记录
    pub fn append(&mut self, record: DecisionRecord) -> Result<()> {
        // 检查是否已存在（内存中去重）
        if self.records.iter().any(|r| r.decision_id == record.decision_id) {
            return Ok(()); // 已存在，跳过
        }

        // 写入 JSONL
        let json = serde_json::to_string(&record)?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", json)?;

        self.records.push(record);
        Ok(())
    }

    /// 追加多个决策记录
    pub fn append_many(&mut self, records: Vec<DecisionRecord>) -> Result<usize> {
        let mut count = 0;
        for record in records {
            if self.append(record).is_ok() {
                count += 1;
            }
        }
        Ok(count)
    }

    /// 从 contract::Decision 转换并追加
    pub fn append_from_decisions(
        &mut self,
        decisions: &[sulix_contract::Decision],
        today: &str,
    ) -> Result<usize> {
        let records: Vec<DecisionRecord> = decisions
            .iter()
            .map(|d| DecisionRecord {
                decision_id: d.id.clone(),
                thesis_id: d.thesis_id.clone(),
                made_at: today.to_string(),
                action: format!("{:?}", d.action),
                confidence: d.confidence,
                outcome: None,
            })
            .collect();
        self.append_many(records)
    }

    /// 获取所有历史记录
    pub fn all(&self) -> &[DecisionRecord] {
        &self.records
    }

    /// 历史记录数量
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn unique_test_path(name: &str) -> PathBuf {
        let tmp = std::env::temp_dir().join(format!("test_decision_history_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        tmp
    }

    #[test]
    fn test_decision_history_new() {
        let path = unique_test_path("new");
        let history = DecisionHistory::open(&path).unwrap();
        assert!(history.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_decision_history_append_and_load() {
        let path = unique_test_path("append_load");
        let mut history = DecisionHistory::open(&path).unwrap();

        history
            .append(DecisionRecord {
                decision_id: "dec_001".into(),
                thesis_id: "thesis_001".into(),
                made_at: "2026-07-12".into(),
                action: "Invest".into(),
                confidence: 0.7,
                outcome: None,
            })
            .unwrap();

        assert_eq!(history.len(), 1);

        // 重新加载验证持久化
        let loaded = DecisionHistory::open(path.clone()).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.all()[0].decision_id, "dec_001");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_decision_history_dedup() {
        let path = unique_test_path("dedup");
        let mut history = DecisionHistory::open(&path).unwrap();

        history
            .append(DecisionRecord {
                decision_id: "dec_001".into(),
                thesis_id: "thesis_001".into(),
                made_at: "2026-07-12".into(),
                action: "Invest".into(),
                confidence: 0.7,
                outcome: None,
            })
            .unwrap();

        // 重复追加同一 ID
        history
            .append(DecisionRecord {
                decision_id: "dec_001".into(),
                thesis_id: "thesis_001".into(),
                made_at: "2026-07-12".into(),
                action: "Invest".into(),
                confidence: 0.7,
                outcome: None,
            })
            .unwrap();

        assert_eq!(history.len(), 1); // 去重

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_append_from_contract_decisions() {
        let path = unique_test_path("contract");
        let mut history = DecisionHistory::open(&path).unwrap();

        let decisions = vec![sulix_contract::Decision {
            id: "dec_001".into(),
            thesis_id: "thesis_001".into(),
            action: sulix_contract::DecisionType::Invest,
            confidence: 0.7,
            horizon: sulix_contract::DecisionHorizon::Days90,
            reasoning: "test".into(),
            made_at: "2026-07-12".into(),
            rule_passed: true,
            requires_review: false,
            review_reason: None,
        }];

        let count = history.append_from_decisions(&decisions, "2026-07-12").unwrap();
        assert_eq!(count, 1);
        assert_eq!(history.len(), 1);

        let _ = std::fs::remove_file(&path);
    }
}


