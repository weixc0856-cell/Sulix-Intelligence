//! 类型转换 — 观察层类型到契约类型
//!
//! 将抓取层类型 (Article/RawSignal) 转换为 contract::Observation。

use sulix_contract::Observation;

use crate::fetcher::Article;
use crate::source::RawSignal;

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
            content_hash: String::new(),
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
            content_hash: String::new(),
            raw_content: a.content.unwrap_or_default(),
            entities: Vec::new(),
        }
    }
}
