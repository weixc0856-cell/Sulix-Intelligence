//! USPTO 专利 API 适配器 — JSON REST API → RawSignal
//!
//! 美国专利商标局（USPTO）官方公开 API，无需 Token。
//! 过滤先进制程/半导体/AI 相关专利，输出统一 RawSignal。
//! 数据为 Public Domain，零版权风险。

use anyhow::Result;
use serde::Deserialize;

use crate::config::SourceConfig;
use crate::source::RawSignal;

/// USPTO API 返回结构
#[derive(Debug, Deserialize)]
struct UsptoResponse {
    results: Vec<UsptoItem>,
}

/// 单条专利
#[derive(Debug, Deserialize)]
struct UsptoItem {
    #[serde(rename = "patentApplicationNumber")]
    pub application_number: String,
    #[serde(rename = "inventionTitle")]
    pub title: String,
    #[serde(rename = "abstractText")]
    pub abstract_text: Vec<String>,
    #[serde(rename = "publicationDate")]
    pub pub_date: String,
    #[serde(rename = "applicantName")]
    #[allow(dead_code)]
    pub applicants: Vec<String>,
}

/// 从 USPTO API 拉取并过滤专利信号
pub async fn fetch_patents(config: &SourceConfig, date_range: &str) -> Result<Vec<RawSignal>> {
    use crate::source::rss::parse_date_range;
    use chrono::Utc;

    let client = crate::client::global_client().clone();

    let url = "https://developer.uspto.gov/ibd-api/v1/patent/application/publications";
    let resp: UsptoResponse = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?
        .json()
        .await?;

    let cutoff = Utc::now() - parse_date_range(date_range);
    let source_id = config.id.clone().unwrap_or_else(|| "uspto".into());
    let mut signals = Vec::new();

    for item in resp.results {
        // 日期过滤
        // USPTO 日期格式 YYYY-MM-DD
        if let Ok(pub_date) = chrono::NaiveDate::parse_from_str(&item.pub_date, "%Y-%m-%d") {
            let pub_utc = pub_date.and_hms_opt(0, 0, 0).unwrap_or_default().and_utc();
            if pub_utc < cutoff {
                continue;
            }
        }

        let title_lower = item.title.to_lowercase();
        // 只保留硬科技相关专利
        let is_relevant = title_lower.contains("semiconductor")
            || title_lower.contains("lithography")
            || title_lower.contains("neural")
            || title_lower.contains("machine learning")
            || title_lower.contains("quantum")
            || title_lower.contains("photonics")
            || title_lower.contains("chiplet")
            || title_lower.contains("heterogeneous")
            || title_lower.contains("advanced packaging")
            || title_lower.contains("hbm")
            || title_lower.contains("memory controller");

        if !is_relevant {
            continue;
        }

        let content = if item.abstract_text.is_empty() {
            None
        } else {
            Some(item.abstract_text.join("\n"))
        };

        let patent_url = format!(
            "https://patents.google.com/patent/US{}",
            item.application_number
        );

        signals.push(RawSignal {
            id: format!("uspto_{}", item.application_number),
            title: item.title,
            url: patent_url,
            content,
            summary: None,
            published_at: None,
            source: config.name.clone(),
            source_id: source_id.clone(),
            category: config.category.clone(),
            metrics: None,
            requires_sanitization: false,
            is_internal: false,
        });
    }

    log::info!("✅ [USPTO/{}] → {} 条专利信号", config.name, signals.len());
    Ok(signals)
}
