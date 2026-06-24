use super::helpers::html_escape;

/// 渲染 SEO meta tags + Open Graph + Twitter Card
pub fn render_seo_meta(title: &str, description: &str, relative_path: &str) -> String {
    format!(
        r#"<meta name="description" content="{description}">
<meta property="og:title" content="{title} | Sulix Intelligence">
<meta property="og:description" content="{description}">
<meta property="og:type" content="article">
<meta property="og:url" content="https://intel.getsulix.com/{relative_path}">
<meta property="og:site_name" content="Sulix Intelligence">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{title}">
<meta name="twitter:description" content="{description}">
<link rel="canonical" href="https://intel.getsulix.com/{relative_path}">"#,
        title = html_escape(title),
        description = html_escape(description),
        relative_path = relative_path
    )
}

/// 渲染 JSON-LD 结构化数据（对标 Google 高价值 TechArticle）
pub fn render_json_ld(title: &str, date: &str, text_snippet: &str) -> String {
    let description = text_snippet
        .chars()
        .take(150)
        .collect::<String>()
        .replace('"', "\\\"");
    format!(
        r#"<script type="application/ld+json">
{{
  "@context": "https://schema.org",
  "@type": "TechArticle",
  "headline": "{title}",
  "datePublished": "{date}",
  "inLanguage": "en",
  "publisher": {{
    "@type": "Organization",
    "name": "Sulix Intelligence"
  }},
  "description": "{description}",
  "dependencies": "USPTO, SEC EDGAR, arXiv"
}}
</script>"#,
        title = html_escape(title),
        date = date,
        description = description
    )
}
