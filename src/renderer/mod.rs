//! 渲染模块 — MDX 知识资产输出（主要）+ Substrack Markdown（次要）
//!
//! MDX 是主要输出格式（ADR-003），供 Astro 前端 Content Collections 消费。
//! 字体授权声明（SIL Open Font License，100% 免费商用）:
//! - Lora (serif, 大标题): SIL OFL, 免费商用
//! - Inter (sans-serif, 正文): SIL OFL, 免费商用
//! - JetBrains Mono (monospace, 日期/标签): SIL OFL, 免费商用
//!
//! 已移除（第一代渲染器遗产）:
//!   html.rs       → MDX 取代
//!   dashboard.rs  → intel-web 前端职责
//!   seo.rs        → Astro Head/Layout 组件职责

pub mod helpers;
pub mod markdown;
pub mod mdx;
pub mod premium;
pub mod publisher;

pub use markdown::render_substack_markdown;
pub use premium::render_premium_report;

#[cfg(test)]
mod tests {
    use super::helpers;

    #[test]
    fn test_html_escape_ampersand_first() {
        assert_eq!(helpers::html_escape("&lt;"), "&amp;lt;");
        assert_eq!(helpers::html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(helpers::html_escape("\"quote\""), "&quot;quote&quot;");
        assert_eq!(helpers::html_escape("'it's'"), "&#x27;it&#x27;s&#x27;");
        assert_eq!(helpers::html_escape("safe text"), "safe text");
        assert_eq!(helpers::html_escape(""), "");
    }

    #[test]
    fn test_html_escape_edge_cases() {
        assert_eq!(
            helpers::html_escape("a&b<c>d\"e'f"),
            "a&amp;b&lt;c&gt;d&quot;e&#x27;f"
        );
        assert_eq!(helpers::html_escape("&&&"), "&amp;&amp;&amp;");
    }
}
