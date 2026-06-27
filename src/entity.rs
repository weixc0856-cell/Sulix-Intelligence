//! EntitySanctionDb — 实体关系数据库（对标 OpenCTI STIX2）
//!
//! Phase 3 核心底座:
//!   - 双 ID 体系: internal_id (UUID) + external_id (行业/制裁 ID)
//!   - 推断 vs 声明隔离: sanctioned = 已确认, unsanctioned = 待审核
//!   - 实体类型注册: EntityType 枚举 + 字符串常量模式
//!   - RefRelationship: 实体间类型化链接（OpenCTI RefAttribute 风格）
//!
//! 对标 OpenCTI:
//!   - schemaTypesDefinition.register() → EntityType 枚举
//!   - schemaRelationsRefDefinition → RefRelationship
//!   - opencti_inferred_* vs opencti_stix_domain_* → sanctioned vs unsanctioned
//!   - DraftWorkspace → SanctionedDb::promote/discard

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 已知实体白名单 — 统一来源
///
/// 所有模块（publishing.rs / mdx.rs 等）通过此函数获取实体列表，
/// 而非各自维护重复的硬编码数组。
pub fn known_entities() -> &'static [&'static str] {
    &[
        "TSMC", "ASML", "NVIDIA", "OPENAI", "ANTHROPIC",
        "GOOGLE", "META", "MICROSOFT", "INTEL", "AMD", "ARM", "HBM",
    ]
}

/// 实体类型（OpenCTI 字符串常量模式）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EntityType {
    /// 企业/组织 (OpenCTI: Identity/Organization)
    Organization,
    /// 技术/产品 (OpenCTI: Malware/Tool — 技术实体)
    Technology,
    /// 专利/论文 (OpenCTI: Report/Indicator)
    Artifact,
    /// 制裁/法规 (OpenCTI: ThreatActor/Campaign)
    Regulation,
    /// 人物 (OpenCTI: Individual)
    Person,
    /// 地理区域 (OpenCTI: Location)
    Region,
    /// 市场/行业 (OpenCTI: Sector)
    Sector,
    /// 未知/未分类
    Unknown,
}

impl EntityType {
    /// OpenCTI 风格实体类型字符串常量
    pub const ORGANIZATION: &'static str = "Organization";
    pub const TECHNOLOGY: &'static str = "Technology";
    pub const ARTIFACT: &'static str = "Artifact";
    pub const REGULATION: &'static str = "Regulation";
    pub const PERSON: &'static str = "Person";
    pub const REGION: &'static str = "Region";
    pub const SECTOR: &'static str = "Sector";

    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Organization => Self::ORGANIZATION,
            EntityType::Technology => Self::TECHNOLOGY,
            EntityType::Artifact => Self::ARTIFACT,
            EntityType::Regulation => Self::REGULATION,
            EntityType::Person => Self::PERSON,
            EntityType::Region => Self::REGION,
            EntityType::Sector => Self::SECTOR,
            EntityType::Unknown => "Unknown",
        }
    }
}

/// 关系类型（OpenCTI StixCoreRelationship 变体）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RelationshipType {
    /// 制裁/管制 (OpenCTI: indicates/mitigates)
    Sanctions,
    /// 投资/资本 (OpenCTI: targets/uses)
    InvestsIn,
    /// 技术依赖 (OpenCTI: depends-on)
    TechnologyDependency,
    /// 供应关系 (OpenCTI: related-to)
    SupplyChain,
    /// 竞争关系
    CompetesWith,
    /// 合作伙伴
    Partners,
    /// 隶属关系 (OpenCTI: belongs-to)
    BelongsTo,
    /// 因果关联 (OpenCTI: leads-to/inferred)
    CausalLink,
}

impl RelationshipType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RelationshipType::Sanctions => "sanctions",
            RelationshipType::InvestsIn => "invests-in",
            RelationshipType::TechnologyDependency => "technology-dependency",
            RelationshipType::SupplyChain => "supply-chain",
            RelationshipType::CompetesWith => "competes-with",
            RelationshipType::Partners => "partners",
            RelationshipType::BelongsTo => "belongs-to",
            RelationshipType::CausalLink => "causal-link",
        }
    }
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
        }
    }

    /// 添加实体（默认进入 unsanctioned）
    pub fn add_entity(&mut self, entity: Entity) {
        let id = entity.id.clone();
        if entity.sanctioned {
            self.sanctioned.insert(id, entity);
        } else {
            self.unsanctioned.insert(id, entity);
        }
    }

    /// 加载/保存到 JSON 文件
    pub fn save_to_file(&self, path: &str) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }
}

/// 实体归一化映射表（belief_engine.rs 的 ENTITY_NORMALIZATION 升级版）
///
/// 将原始文本中的技术术语映射到 EntitySanctionDb 中的标准化实体名称。
pub fn extract_entities_from_text(text: &str) -> Vec<String> {
    let mut entities = Vec::new();
    let lower = text.to_lowercase();

    let patterns: Vec<(&str, &[&str])> = vec![
        ("TSMC", &["tsmc", "taiwan semiconductor", "台积电"]),
        (
            "ASML",
            &["asml", "advanced semiconductor materials lithography"],
        ),
        ("NVIDIA", &["nvidia", "nvidia corporation", "英伟达"]),
        ("Intel", &["intel", "intel corporation", "英特尔"]),
        ("AMD", &["amd", "advanced micro devices"]),
        ("Samsung", &["samsung", "samsung electronics", "三星"]),
        ("Apple", &["apple", "apple inc", "苹果"]),
        ("Microsoft", &["microsoft", "msft", "微软"]),
        ("Google", &["google", "alphabet", "谷歌"]),
        ("Meta", &["meta", "facebook", "meta platforms"]),
        ("Amazon", &["amazon", "amzn", "aws", "亚马逊"]),
        ("TSA", &["tsa", "先进制程"]),
        ("EUV", &["euv", "extreme ultraviolet", "high-na"]),
        ("HBM", &["hbm", "high-bandwidth memory", "hbm3", "hbm4"]),
        (
            "Chiplet",
            &[
                "chiplet",
                "advanced packaging",
                "heterogeneous integration",
                "3d packaging",
                "fan-out",
            ],
        ),
        ("RISC-V", &["risc-v", "riscv", "open source isa"]),
        ("CUDA", &["cuda", "nvidia cuda", "cuda ecosystem"]),
        ("ARM", &["arm", "arm architecture", "arm holdings"]),
        (
            "BIS",
            &[
                "bis",
                "bureau industry security",
                "entity list",
                "export control",
            ],
        ),
        ("CHIPS", &["chips act", "chips and science"]),
        ("SEC", &["sec", "securities exchange commission", "edgar"]),
        ("USPTO", &["uspto", "patent trademark", "patent office"]),
        ("FRB", &["federal register", "frb", "federal reserve"]),
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
    fn test_entity_extraction() {
        let text = "TSMC's advanced packaging capacity expansion for 3D chiplet integration and NVIDIA's CUDA ecosystem";
        let entities = extract_entities_from_text(text);
        assert!(entities.contains(&"TSMC".to_string()));
        assert!(entities.contains(&"NVIDIA".to_string()));
        assert!(entities.contains(&"Chiplet".to_string()));
        assert!(entities.contains(&"CUDA".to_string()));
    }

    #[test]
    fn test_save_load_roundtrip() {
        let mut db = EntitySanctionDb::new();
        db.add_entity(Entity {
            id: "e6".into(),
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
        });

        let json = serde_json::to_string(&db).unwrap();
        let loaded: EntitySanctionDb = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.sanctioned.len(), 1);
        assert_eq!(loaded.sanctioned.get("e6").unwrap().name, "TSMC");
        assert_eq!(
            loaded.sanctioned.get("e6").unwrap().external_refs[0].source,
            "USPTO"
        );
    }
}
