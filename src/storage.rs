//! Storage trait — 统一持久化接口
//!
//! 消除 7 个模块（EntitySanctionDb, BeliefDb, EventLog, ChronicleDb,
//! MemoryEngine, AssessmentRegistry, DecisionRegistry, InvestigationRegistry）
//! 中复制粘贴的 save/load + 损坏备份 + 重建逻辑。
//!
//! Phase 3: 当前提供 trait 定义供各模块实现。
//!          后续可将各模块的实现统一到这些 trait 的默认方法中。

use std::path::Path;

/// 可持久化的数据模型
pub trait Saveable {
    /// 序列化并写入 JSON 文件
    fn save_to_json(&self, path: &Path) -> anyhow::Result<()>;
}

/// 可从磁盘加载的数据模型（含损坏备份保护）
pub trait Loadable: Sized {
    /// 从文件加载；若文件不存在或损坏返回 None
    fn load_from_json(path: &Path) -> anyhow::Result<Option<Self>>;
}

/// load_or_new + 损坏备份保护的便捷实现
///
/// 如果文件不存在，直接返回 fallback()。
/// 如果加载失败，备份损坏文件（加 `.corrupt.{timestamp}` 后缀）并返回 fallback()。
///
/// 预期使用模式:
/// ```ignore
/// let db = with_corrupt_recovery(path, |p| Db::load(p), Db::new);
/// ```
pub fn with_corrupt_recovery<T>(
    path: &Path,
    load_fn: impl FnOnce(&Path) -> anyhow::Result<T>,
    fallback: impl FnOnce() -> T,
) -> T {
    if !path.exists() {
        return fallback();
    }
    match load_fn(path) {
        Ok(value) => value,
        Err(e) => {
            let backup = format!(
                "{}.corrupt.{}",
                path.to_string_lossy(),
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            );
            log::warn!(
                "⚠️ 持久化数据损坏 ({}), 备份到 {} 后重建",
                e,
                backup
            );
            let _ = std::fs::rename(path, &backup);
            fallback()
        }
    }
}
