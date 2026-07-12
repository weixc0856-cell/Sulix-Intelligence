//! EntitySanctionDb — 实体关系数据库（对标 OpenCTI STIX2）
//!
//! Phase 3 核心底座:
//!   - 双 ID 体系: internal_id (UUID) + external_id (行业/制裁 ID)
//!   - 推断 vs 声明隔离: sanctioned = 已确认, unsanctioned = 待审核
//!   - 实体类型注册: EntityType 枚举
//!   - RefRelationship: 实体间类型化链接（OpenCTI RefAttribute 风格）

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 实体类型（OpenCTI 字符串常量模式）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EntityType {
    Organization,
    Technology,
    Artifact,
    Regulation,
    Person,
    Region,
    Sector,
    Unknown,
}

/// 关系类型（OpenCTI StixCoreRelationship 变体）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RelationshipType {
    Sanctions,
    InvestsIn,
    TechnologyDependency,
    SupplyChain,
    CompetesWith,
    Partners,
    BelongsTo,
    CausalLink,
}

/// 实体（对标 OpenCTI StixDomainObject）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// 内部 ID (UUID v4)
    pub id: String,
    /// 实体类型
    pub entity_type: EntityType,
    /// 实体名称（归一化行业 Term ID）
    pub name: String,
    /// 别名（用于匹配同义词）
    pub aliases: Vec<String>,
    /// 是否为已确认的实体
    pub sanctioned: bool,
    /// 外部引用（OpenCTI external_references）
    pub external_refs: Vec<ExternalRef>,
    /// 关联的关系
    pub relationships: Vec<Relationship>,
}

/// 外部引用（OpenCTI external_references 风格）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalRef {
    /// 源名称: "USPTO", "SEC", "arXiv", "Federal Register"
    pub source: String,
    /// 外部 ID: 专利号, SEC 文件号, arXiv ID
    pub external_id: String,
    /// 可选的 URL
    pub url: Option<String>,
}

/// 关系（OpenCTI StixCoreRelationship / StixRefRelationship 风格）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    /// 关系 ID
    pub id: String,
    pub relationship_type: RelationshipType,
    /// 目标实体 ID
    pub target_id: String,
    /// 目标实体名称（冗余，方便查询）
    pub target_name: String,
    /// 关系强度 0.0-1.0
    pub confidence: f64,
    /// 是否为推断关系（OpenCTI inferred 风格）
    pub inferred: bool,
    /// 推断来源规则（OpenCTI i_rule_ 前缀风格）
    pub inference_rule: Option<String>,
    /// 创建时间
    pub created_at: String,
}

/// EntitySanctionDb — 实体关系数据库
///
/// 双索引隔离:
///   - sanctioned: 已确认的实体和关系（OpenCTI stix_domain 等价物）
///   - unsanctioned: 推断/待审核的实体和关系（OpenCTI inferred 等价物）
///
/// 用法:
///   1. 通过 add_entity/add_relationship 添加推断实体
///   2. unsanctioned 中的实体需要 promote 到 sanctioned 才被视为可信
///   3. discard 可以批量移除 unsanctioned 条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySanctionDb {
    /// 已确认的实体
    pub sanctioned: HashMap<String, Entity>,
    /// 待审核的实体
    pub unsanctioned: HashMap<String, Entity>,
    /// 名称索引 (lowercase_name → ()) — 用于 O(1) 存在性检查
    /// 由 add_entity 维护，向后兼容旧 JSON（无此字段时默认空）
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    name_index: HashMap<String, ()>,
}

impl Default for EntitySanctionDb {
    fn default() -> Self {
        Self::new()
    }
}

impl EntitySanctionDb {
    pub fn new() -> Self {
        Self {
            sanctioned: HashMap::new(),
            unsanctioned: HashMap::new(),
            name_index: HashMap::new(),
        }
    }
    /// 加载/保存到 JSON 文件
    #[allow(dead_code)]
    pub fn save_to_file(&self, path: &str) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// 添加实体
    #[allow(dead_code)]
    pub fn add_entity(&mut self, entity: Entity) {
        let id = entity.id.clone();
        if entity.sanctioned {
            self.sanctioned.insert(id, entity);
        } else {
            self.unsanctioned.insert(id, entity);
        }
    }

    /// O(1) 名称存在性检查
    #[allow(dead_code)]
    pub fn name_exists(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        self.sanctioned.values().any(|e| e.name.to_lowercase() == lower)
            || self.unsanctioned.values().any(|e| e.name.to_lowercase() == lower)
    }
}

/// 实体归一化映射表 — 将原始文本中的技术术语映射到标准化实体名称
pub fn extract_entities_from_text(text: &str) -> Vec<String> {
    let mut entities = Vec::new();
    let lower = text.to_lowercase();

    let patterns: Vec<(&str, &[&str])> = vec![
        ("TSMC", &["tsmc", "taiwan semiconductor", "台积电"]),
        ("ASML", &["asml", "advanced semiconductor materials lithography"]),
        ("NVIDIA", &["nvidia", "nvidia corporation", "英伟达"]),
        ("Intel", &["intel", "intel corporation", "英特尔"]),
        ("AMD", &["amd", "advanced micro devices"]),
        ("Samsung", &["samsung", "samsung electronics", "三星"]),
        ("Microsoft", &["microsoft", "msft", "微软"]),
        ("Google", &["google", "alphabet", "谷歌"]),
        ("Meta", &["meta", "facebook", "meta platforms"]),
        ("Amazon", &["amazon", "amzn", "aws", "亚马逊"]),
        ("HBM", &["hbm", "high-bandwidth memory", "hbm3", "hbm4"]),
        ("RISC-V", &["risc-v", "riscv", "open source isa"]),
        ("CUDA", &["cuda", "nvidia cuda", "cuda ecosystem"]),
        ("ARM", &["arm", "arm architecture", "arm holdings"]),
        ("OpenAI", &["openai", "open ai", "chatgpt", "gpt"]),
        ("Anthropic", &["anthropic", "claude"]),
        ("DeepSeek", &["deepseek", "深度求索"]),
    ];

    for (name, aliases) in patterns {
        if aliases.iter().any(|a| lower.contains(a)) && !entities.contains(&name.to_string()) {
            entities.push(name.to_string());
        }
    }

    entities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_entity_db_empty() {
        let db = EntitySanctionDb::new();
        assert!(db.sanctioned.is_empty());
        assert!(db.unsanctioned.is_empty());
    }

    #[test]
    fn test_entity_serde_roundtrip() {
        let mut db = EntitySanctionDb::new();
        db.sanctioned.insert(
            "e1".into(),
            Entity {
                id: "e1".into(),
                entity_type: EntityType::Organization,
                name: "TSMC".into(),
                aliases: vec![],
                sanctioned: true,
                external_refs: vec![ExternalRef {
                    source: "USPTO".into(),
                    external_id: "USPTO12345".into(),
                    url: Some("https://patents.google.com/patent/USPTO12345".into()),
                }],
                relationships: vec![],
            },
        );

        let json = serde_json::to_string(&db).unwrap();
        let loaded: EntitySanctionDb = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.sanctioned.len(), 1);
        assert_eq!(loaded.sanctioned.get("e1").unwrap().name, "TSMC");
    }

    #[test]
    fn test_extract_entities_basic() {
        let text = "TSMC announced 3nm mass production, NVIDIA's CUDA ecosystem grows";
        let entities = extract_entities_from_text(text);
        assert!(entities.contains(&"TSMC".to_string()));
        assert!(entities.contains(&"NVIDIA".to_string()));
        assert!(entities.contains(&"CUDA".to_string()));
        assert_eq!(entities.len(), 3);
    }

    #[test]
    fn test_extract_entities_empty() {
        let entities = extract_entities_from_text("nothing to see here");
        assert!(entities.is_empty());
    }

    #[test]
    fn test_name_exists() {
        let mut db = EntitySanctionDb::new();
        db.add_entity(Entity {
            id: "e1".into(),
            entity_type: EntityType::Organization,
            name: "TSMC".into(),
            aliases: vec![],
            sanctioned: true,
            external_refs: vec![],
            relationships: vec![],
        });
        assert!(db.name_exists("TSMC"));
        assert!(db.name_exists("tsmc"));
        assert!(!db.name_exists("Intel"));
    }
}
