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
}
