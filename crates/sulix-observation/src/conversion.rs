//! 类型转换 — 观察层类型到契约类型
//!
//! 将抓取层类型 (Article/RawSignal) 转换为 contract::Observation。
//! 所有转换逻辑归 observation crate 所有。

use sulix_contract::Observation;

use crate::fetcher::Article;
use crate::source::RawSignal;

/// 计算内容哈希（SHA256 截断前 16 字符）
fn content_hash(content: &Option<String>) -> String {
    match content {
        Some(c) if !c.is_empty() => {
            use sha2::Digest;
            let hash = sha2::Sha256::digest(c.as_bytes());
            hex::encode(hash).chars().take(16).collect()
        }
        _ => String::new(),
    }
}

impl From<RawSignal> for Observation {
    fn from(s: RawSignal) -> Self {
        Self {
            id: s.id,
            title: s.title,
            source: s.source,
            source_id: s.source_id.clone(),
            url: s.url,
            published_at: s.published_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
            captured_at: chrono::Utc::now().to_rfc3339(),
            content_hash: content_hash(&s.content),
            raw_content: s.content.unwrap_or_default(),
            entities: Vec::new(),
        }
    }
}

impl From<Article> for Observation {
    fn from(a: Article) -> Self {
        Self {
            id: a.id,
            title: a.title,
            source: a.source,
            source_id: String::new(),
            url: a.url,
            published_at: a.published_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
            captured_at: chrono::Utc::now().to_rfc3339(),
            content_hash: content_hash(&a.content),
            raw_content: a.content.unwrap_or_default(),
            entities: Vec::new(),
        }
    }
}
