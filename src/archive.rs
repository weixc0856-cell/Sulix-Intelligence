//! 编年史看板（Chronicle Dashboard）
//!
//! 维护一个基于 JSON 的历史数据库，每次运行追加当日条目。
//! 总索引页从此数据库读取，按时间倒序排列，形成长线专题追踪。

use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 单日条目（写入 database.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronicleEntry {
    pub date: String,
    pub topic: String,
    pub headline: String,
    pub entities: Vec<String>,
    pub signal_strength: u8,
    pub language: String, // "en" | "zh"
}

/// 历史数据库
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronicleDb {
    pub entries: Vec<ChronicleEntry>,
}

impl ChronicleDb {
    /// 从 JSON 文件加载，不存在则创建空库
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            Ok(serde_json::from_str(&content).unwrap_or_else(|e| {
                log::warn!(
                    "ChronicleDb parse error at {:?}, returning empty: {}",
                    path,
                    e
                );
                ChronicleDb { entries: vec![] }
            }))
        } else {
            Ok(ChronicleDb { entries: vec![] })
        }
    }

    /// 追加当日条目
    pub fn push(&mut self, entry: ChronicleEntry) {
        // 去重：如果同日期同主题已存在，更新而非追加
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.date == entry.date && e.topic == entry.topic)
        {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// 保存到 JSON 文件
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, &json)?;
        Ok(())
    }

    /// 获取排序后的条目（最新在前）
    pub fn sorted(&self) -> Vec<ChronicleEntry> {
        let mut sorted = self.entries.clone();
        sorted.sort_by(|a, b| b.date.cmp(&a.date));
        sorted
    }
}
