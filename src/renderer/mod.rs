//! 渲染模块 — 咨询级简报 + Economist 版式 HTML
//!
//! 字体授权声明（SIL Open Font License，100% 免费商用）:
//! - Lora (serif, 大标题): SIL OFL, 免费商用
//! - Inter (sans-serif, 正文): SIL OFL, 免费商用
//! - JetBrains Mono (monospace, 日期/标签): SIL OFL, 免费商用
//!
//! 抄 Reference/ 中 BCG/Deloitte/GS/McKinsey 报告结构
//! 当前活跃路径：render_html_report → render_trend_block → render_signal_markdown

pub mod dashboard;
pub mod helpers;
pub mod html;
pub mod markdown;
pub mod mdx;
pub mod premium;
pub mod publisher;
pub mod seo;

pub use dashboard::{render_memory_dashboard, render_trend_block};
pub use html::{render_archive_dashboard, render_html_report};
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

    #[test]
    fn test_validate_url() {
        assert_eq!(
            helpers::validate_url("https://example.com"),
            "https://example.com"
        );
        assert_eq!(
            helpers::validate_url("http://test.org/page"),
            "http://test.org/page"
        );
        assert_eq!(helpers::validate_url(""), "#invalid-url");
        assert_eq!(helpers::validate_url("javascript:alert(1)"), "#invalid-url");
        assert_eq!(
            helpers::validate_url("data:text/html,<script>"),
            "#invalid-url"
        );
    }
}
