use crate::fetch::http_get;
use crate::FetchError;
use scraper::{Html, Selector};

/// Fetch the full text of a single article URL using CSS selectors.
/// Only called for feeds with `extraction_level = 'full_text'`.
/// The `article.url` originates from third-party feed data, so
/// `guard_public_url` is applied here too.
pub async fn extract_full_text(url: &str) -> Result<String, FetchError> {
    let (status, body, _etag, _lm) = http_get(url, None, None, 10_000).await?;

    if status >= 400 {
        return Err(FetchError::Status(status));
    }

    let document = Html::parse_document(&body);

    // Ordered list of content selectors, from most specific to fallback.
    let selectors = ["article", "main", ".post-content", ".entry-content", "#content", ".content", ".article-body"];

    for raw in &selectors {
        if let Ok(sel) = Selector::parse(raw) {
            if let Some(el) = document.select(&sel).next() {
                let text = el.text().collect::<Vec<_>>().join(" ");
                let trimmed = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !trimmed.is_empty() {
                    return Ok(trimmed);
                }
            }
        }
    }

    // Fallback: concatenate all <p> text.
    if let Ok(sel) = Selector::parse("p") {
        let text: String =
            document.select(&sel).map(|el| el.text().collect::<String>()).collect::<Vec<_>>().join("\n\n");
        let trimmed = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    Err(FetchError::Extraction("no readable content found".into()))
}
