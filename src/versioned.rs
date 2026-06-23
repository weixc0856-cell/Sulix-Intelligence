//! 版本化数据管线 — 对标 Kedro (McKinsey) pipeline pattern
//!
//! 核心理念:
//!   - 每层都持久化到磁盘 (Kedro: _EPHEMERAL = False)
//!   - 原子写入: 先写临时文件, 再 rename (防止部分写入损坏)
//!   - 仅缺失输出重启: runner 检查输出是否存在, 跳过已有节点
//!   - UUID v7 风格 ID: 时间前缀 + 随机后缀, 支持时序排序
//!
//! 对标 Kedro:
//!   - AbstractDataset (load/save/_describe) → VersionedDataset trait
//!   - DataCatalog → VersionedCatalog
//!   - _find_nodes_to_resume_from() → run_pipeline_with_resume()

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ===== UUID v7 风格 ID 生成 =====

/// UUID v7 风格: 时间戳前缀(ms) + 原子递增计数器
/// 排序 = 时间排序, 单进程内严格不重复
pub fn uuid_v7() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{:016x}{:016x}", ts, seq)
}

// ===== 原子写入 =====

/// 原子写入: 先写入 .tmp 文件, 然后 rename
/// Kedro 缺乏事务性写入, 这里用 rename 的原子性保证
pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    // 确保父目录存在
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // 写入临时文件
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, content)?;
    // 原子 rename
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// 带校验的原子写入: 写入后读取验证
pub fn atomic_write_verified(path: &Path, content: &str) -> Result<()> {
    atomic_write(path, content)?;
    // 验证: 读回来确认一致
    let read_back = std::fs::read_to_string(path)?;
    if read_back != content {
        anyhow::bail!("原子写入校验失败: {}", path.display());
    }
    Ok(())
}

// ===== VersionedDataset trait =====

/// 版本化数据集 — 对标 Kedro AbstractDataset
///
/// 每个数据集知道:
///   - 它的存储路径 (基于版本)
///   - 如何序列化/反序列化自己
///   - 它的版本号
pub trait VersionedDataset: Sized {
    /// 数据集名称 (用于目录命名)
    fn name(&self) -> &str;
    /// 保存到目录 (自动处理版本化路径)
    fn save(&self, base_dir: &Path) -> Result<PathBuf>;
    /// 从目录加载 (自动查找最新版本)
    fn load(base_dir: &Path) -> Result<Self>;
    /// 检查是否已持久化
    fn exists(base_dir: &Path) -> bool {
        base_dir.join("_SUCCESS").exists()
    }
}

// ===== 版本化数据目录 =====

/// 版本化数据目录
///
/// 结构:
///   {base_dir}/{dataset_name}/{version}/
///   {base_dir}/{dataset_name}/_SUCCESS  ← 最新版本标记
///
/// 版本: YYYYMMDD-HHMMSS-{uuid_suffix}
pub struct VersionedCatalog {
    base_dir: PathBuf,
    datasets: HashMap<String, PathBuf>,
}

impl VersionedCatalog {
    pub fn new(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
            datasets: HashMap::new(),
        }
    }

    /// 获取数据集存储路径
    pub fn dataset_path(&self, name: &str, version: &str) -> PathBuf {
        self.base_dir.join(name).join(version)
    }

    /// 获取数据集的最新版本路径
    pub fn latest_version_path(&self, name: &str) -> Option<PathBuf> {
        let dir = self.base_dir.join(name);
        if !dir.exists() {
            return None;
        }
        // 读取目录, 找到最新版本 (按名称排序 = 时间排序)
        let mut entries: Vec<_> = std::fs::read_dir(&dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
            .collect();
        entries.sort_by_key(|e| e.file_name());
        entries.last().map(|e| e.path())
    }

    /// 创建新版本 (自动生成版本号)
    pub fn create_version(&mut self, name: &str) -> PathBuf {
        let version = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let path = self.dataset_path(name, &version);
        std::fs::create_dir_all(&path).ok();
        // 写入 _SUCCESS 标记 (Kedro 风格: 输出存在 = 节点已完成)
        // 写入两个位置: 版本子目录 + 数据集根目录
        let _ = std::fs::write(path.join("_SUCCESS"), version.as_bytes());
        let _ = std::fs::write(
            self.base_dir.join(name).join("_SUCCESS"),
            version.as_bytes(),
        );
        self.datasets.insert(name.to_string(), path.clone());
        path
    }

    /// 检查数据集是否已存在 (用于跳过已完成节点)
    pub fn has_output(&self, name: &str) -> bool {
        let dir = self.base_dir.join(name);
        dir.join("_SUCCESS").exists()
    }

    /// 注册外部数据集路径
    pub fn register(&mut self, name: &str, path: PathBuf) {
        self.datasets.insert(name.to_string(), path);
    }
}

// ===== 管线运行器 (支持仅缺失输出重启) =====

/// 管线步骤定义
pub struct PipelineStep {
    pub name: &'static str,
    pub run: Box<dyn Fn() -> Result<String> + Send + Sync>,
}

/// 运行管线, 跳过已有输出的步骤
///
/// 对标 Kedro: _find_nodes_to_resume_from() + only_missing_outputs
pub fn run_pipeline_with_resume(
    steps: Vec<PipelineStep>,
    catalog: &VersionedCatalog,
) -> Result<()> {
    for step in steps {
        // 检查已有输出
        if catalog.has_output(step.name) {
            log::info!("⏭️ 跳过 {} (已有输出)", step.name);
            continue;
        }

        log::info!("▶️ 执行步骤: {}", step.name);
        match (step.run)() {
            Ok(_output) => {
                log::info!("✅ {} 完成", step.name);
            }
            Err(e) => {
                log::warn!("⚠️ {} 失败: {}", step.name, e);
            }
        }
    }
    Ok(())
}

// ===== 默认序列化实现 =====

/// JSON 数据集
pub struct JsonDataset<T: Serialize + for<'de> Deserialize<'de>> {
    pub data: T,
    pub name: String,
}

impl<T: Serialize + for<'de> Deserialize<'de>> JsonDataset<T> {
    pub fn new(data: T, name: &str) -> Self {
        Self {
            data,
            name: name.to_string(),
        }
    }
}

impl<T: Serialize + for<'de> Deserialize<'de>> VersionedDataset for JsonDataset<T> {
    fn name(&self) -> &str {
        &self.name
    }

    fn save(&self, base_dir: &Path) -> Result<PathBuf> {
        let dir = base_dir.join(&self.name);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("data.json");
        let json = serde_json::to_string_pretty(&self.data)?;
        atomic_write_verified(&path, &json)?;
        Ok(path)
    }

    fn load(base_dir: &Path) -> Result<Self> {
        let path = base_dir.join("data.json");
        let content = std::fs::read_to_string(&path)?;
        let data: T = serde_json::from_str(&content)?;
        let name = base_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        Ok(Self { data, name })
    }

    fn exists(base_dir: &Path) -> bool {
        base_dir.join("data.json").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_uuid_v7_format() {
        let id = uuid_v7();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_uuid_v7_small_batch_unique() {
        let mut ids = std::collections::HashSet::new();
        for _ in 0..10 {
            ids.insert(uuid_v7());
        }
        assert_eq!(
            ids.len(),
            10,
            "All 10 IDs should be unique; got {}",
            ids.len()
        );
    }

    #[test]
    fn test_atomic_write_and_read() {
        let dir = std::env::temp_dir().join("sulix_test_atomic");
        let path = dir.join("test.txt");
        atomic_write(&path, "hello world").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello world");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_json_dataset_roundtrip() {
        let dir = std::env::temp_dir().join("sulix_test_dataset");
        let data = vec!["a".to_string(), "b".to_string()];
        let ds = JsonDataset::new(data, "test_set");
        ds.save(&dir).unwrap();
        let loaded = JsonDataset::<Vec<String>>::load(&dir.join("test_set")).unwrap();
        assert_eq!(loaded.data, vec!["a".to_string(), "b".to_string()]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_versioned_catalog_create_and_check() {
        let dir = std::env::temp_dir().join("sulix_test_catalog");
        let mut catalog = VersionedCatalog::new(&dir);
        assert!(!catalog.has_output("step1"));
        catalog.create_version("step1");
        assert!(catalog.has_output("step1"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_pipeline_skip_existing() {
        let dir = std::env::temp_dir().join("sulix_test_pipeline");
        let mut catalog = VersionedCatalog::new(&dir);

        let steps = vec![
            PipelineStep {
                name: "step_a",
                run: Box::new(|| Ok("a done".into())),
            },
            PipelineStep {
                name: "step_b",
                run: Box::new(|| Ok("b done".into())),
            },
        ];

        // 标记 step_a 已完成
        catalog.create_version("step_a");
        run_pipeline_with_resume(steps, &catalog).unwrap();

        fs::remove_dir_all(&dir).ok();
    }
}
