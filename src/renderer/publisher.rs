//! Publisher Trait — 统一渲染输出抽象
//!
//! 渲染器不应作为自由函数散落在模块中。
//! 每个输出格式应实现 Publisher trait，便于扩展和替换。
//!
//! 核心转变：
//!   旧：renderer::render_html_report(...) — 自由函数，参数膨胀
//!   新：HtmlPublisher::new(...).publish(&ctx) — 统一接口，可组合
//!
//! 当前实现：
//!   - HtmlPublisher:    Bloomberg 风格 HTML 简报
//!   - MarkdownPublisher: Astro 前端 Markdown 帖子
//!   - DashboardPublisher: Chronicle + Thesis 看板
//!   - PremiumPublisher:  深度研报 HTML
//!   - SeoPublisher:      SEO meta 标签 + JSON-LD
//!
//! 未来可扩展：
//!   - EmailPublisher:    邮件摘要
//!   - ApiPublisher:      JSON API 输出
//!   - RssPublisher:      RSS Feed 输出

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::archive::ChronicleEntry;
use crate::clusterer::{ChangeSummary, Theme, ThemeAnalysis};
use crate::config::SourceConfig;
use crate::decision_engine::Decision;
use crate::domain::thesis::Thesis;
use crate::engine::premium::PremiumReport;

/// 发布上下文 — 所有发布器共享的数据
pub struct PublishContext {
    pub themes: Vec<Theme>,
    pub analyses: Vec<ThemeAnalysis>,
    /// 中文分析（可选）
    pub analyses_zh: Vec<ThemeAnalysis>,
    pub date: String,
    pub language: String,
    pub calibration: Option<String>,
    pub attributable_sources: Vec<SourceConfig>,
    pub flash_headline: Option<String>,
    pub change_summary: Option<ChangeSummary>,
    pub theses: Vec<Thesis>,
    pub report: Option<PremiumReport>,
    pub archive_entries: Vec<ChronicleEntry>,
    /// 中文编年史条目（可选）
    pub archive_entries_zh: Vec<ChronicleEntry>,
    pub source_statuses: Vec<(String, bool, usize)>,
    pub decisions: Vec<crate::decision_engine::Decision>,
    /// ASI/Confidence 评分 per theme_title → (asi, confidence, final)
    pub asi_scores: HashMap<String, (f64, f64, f64)>,
    /// Editor Agent 分析结果（个人影响分析）
    pub editor_notes: Vec<crate::agent::editor::EditorNote>,
    /// Belief Engine HTML 区块
    pub belief_notes_html: String,
    pub output_dir: PathBuf,
}

/// 发布输出结果
pub enum PublishedOutput {
    /// 内存字符串（如 HTML 片段）
    Inline {
        content: String,
        label: String,
    },
    /// 写入文件
    File {
        path: PathBuf,
        content: String,
    },
}

/// 发布器 Trait
///
/// 每个输出格式实现此 trait。
/// publish() 接收共享上下文，返回零个或多个输出。
pub trait Publisher {
    /// 发布器名称（用于日志和调试）
    fn name(&self) -> &str;

    /// 执行发布
    fn publish(&self, ctx: &PublishContext) -> Result<Vec<PublishedOutput>>;
}

// ===== HtmlPublisher =====

pub struct HtmlPublisher;

impl HtmlPublisher {
    pub fn new() -> Self {
        Self
    }

    /// 渲染并写入指定语言的 HTML，返回文件路径
    fn render_and_write(
        ctx: &PublishContext,
        language: &str,
        analyses: &[ThemeAnalysis],
    ) -> Result<Option<PathBuf>> {
        if analyses.is_empty() {
            return Ok(None);
        }

        let html = crate::renderer::html::render_html_report(
            &ctx.themes,
            analyses,
            &ctx.date,
            ctx.calibration.as_deref(),
            &ctx.attributable_sources,
            ctx.flash_headline.as_deref(),
            language,
            &ctx.source_statuses,
            ctx.change_summary.as_ref(),
            Some(&ctx.asi_scores),
            Some(&ctx.editor_notes),
            Some(&ctx.belief_notes_html),
        )?;

        let dir = ctx.output_dir.join(language).join(&ctx.date);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("index.html");
        let mut content = html;

        // 注入决策区块（如果有）
        if !ctx.decisions.is_empty() {
            let decision_html = crate::decision_engine::render_decision_html(&ctx.decisions);
            content = content.replacen("</main>", &format!("{decision_html}</main>"), 1);
        }

        std::fs::write(&path, &content)?;
        log::info!("📄 {} 简报: {}", language, path.display());
        Ok(Some(path))
    }
}

impl Publisher for HtmlPublisher {
    fn name(&self) -> &str {
        "HtmlPublisher"
    }

    fn publish(&self, ctx: &PublishContext) -> Result<Vec<PublishedOutput>> {
        let mut outputs = Vec::new();

        // 英文版
        if let Some(path) = Self::render_and_write(ctx, "en", &ctx.analyses)? {
            outputs.push(PublishedOutput::File { path, content: String::new() });
        }

        // 中文版（如有）
        if !ctx.analyses_zh.is_empty() {
            if let Some(path) = Self::render_and_write(ctx, "zh", &ctx.analyses_zh)? {
                outputs.push(PublishedOutput::File { path, content: String::new() });
            }
        }

        Ok(outputs)
    }
}

// ===== MarkdownPublisher =====

pub struct MarkdownPublisher;

impl MarkdownPublisher {
    pub fn new() -> Self {
        Self
    }
}

impl Publisher for MarkdownPublisher {
    fn name(&self) -> &str {
        "MarkdownPublisher"
    }

    fn publish(&self, ctx: &PublishContext) -> Result<Vec<PublishedOutput>> {
        let mut outputs = Vec::new();

        for (theme, analysis) in ctx.themes.iter().zip(ctx.analyses.iter()) {
            let slug = theme.title.to_lowercase()
                .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
                .replace(' ', "-");
            let md = crate::renderer::markdown::render_signal_markdown(
                theme, analysis, &ctx.date,
            );
            outputs.push(PublishedOutput::File {
                path: PathBuf::from("content/posts").join(format!("{}-{}.md", ctx.date, slug)),
                content: md,
            });
        }

        Ok(outputs)
    }
}

// ===== DashboardPublisher =====

pub struct DashboardPublisher;

impl DashboardPublisher {
    pub fn new() -> Self {
        Self
    }
}

impl Publisher for DashboardPublisher {
    fn name(&self) -> &str {
        "DashboardPublisher"
    }

    fn publish(&self, ctx: &PublishContext) -> Result<Vec<PublishedOutput>> {
        let mut outputs = Vec::new();

        // Chronicle 看板（EN → en_root + root）
        if !ctx.archive_entries.is_empty() {
            let archive_html = crate::renderer::html::render_archive_dashboard(&ctx.archive_entries)?;
            let en_root = ctx.output_dir.join("en");
            std::fs::create_dir_all(&en_root)?;
            let en_path = en_root.join("index.html");
            std::fs::write(&en_path, &archive_html)?;
            outputs.push(PublishedOutput::File { path: en_path, content: archive_html.clone() });

            // 同时写入 root
            let root_path = ctx.output_dir.join("index.html");
            std::fs::write(&root_path, &archive_html)?;
            outputs.push(PublishedOutput::File { path: root_path, content: archive_html });
        }

        // 中文 Chronicle 看板
        if !ctx.archive_entries_zh.is_empty() {
            let zh_root = ctx.output_dir.join("zh");
            std::fs::create_dir_all(&zh_root)?;
            if let Ok(zh_archive) = crate::renderer::html::render_archive_dashboard(&ctx.archive_entries_zh) {
                let zh_path = zh_root.join("index.html");
                std::fs::write(&zh_path, &zh_archive)?;
                outputs.push(PublishedOutput::File { path: zh_path, content: zh_archive });
            }
        }

        Ok(outputs)
    }
}

// ===== PremiumPublisher =====

pub struct PremiumPublisher;

impl PremiumPublisher {
    pub fn new() -> Self {
        Self
    }
}

impl Publisher for PremiumPublisher {
    fn name(&self) -> &str {
        "PremiumPublisher"
    }

    fn publish(&self, ctx: &PublishContext) -> Result<Vec<PublishedOutput>> {
        if let Some(ref report) = ctx.report {
            let html = crate::renderer::premium::render_premium_report(report)?;
            let slug = report.theme_title.to_lowercase()
                .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
                .replace(' ', "-");
            return Ok(vec![PublishedOutput::File {
                path: ctx.output_dir.join("premium").join(format!("{}-{}.html", ctx.date, slug)),
                content: html,
            }]);
        }
        Ok(vec![])
    }
}

// ===== SeoPublisher =====

pub struct SeoPublisher;

impl SeoPublisher {
    pub fn new() -> Self {
        Self
    }
}

impl Publisher for SeoPublisher {
    fn name(&self) -> &str {
        "SeoPublisher"
    }

    fn publish(&self, ctx: &PublishContext) -> Result<Vec<PublishedOutput>> {
        let mut outputs = Vec::new();

        for analysis in &ctx.analyses {
            let title = &analysis.theme_title;
            let description = &analysis.bluf;
            let relative_path = format!("en/{}/index.html", ctx.date);

            let seo_meta = crate::renderer::seo::render_seo_meta(title, description, &relative_path);
            let json_ld = crate::renderer::seo::render_json_ld(title, &ctx.date, &analysis.bluf);

            outputs.push(PublishedOutput::Inline {
                content: format!("{}\n{}", seo_meta, json_ld),
                label: format!("seo:{}", title),
            });
        }

        Ok(outputs)
    }
}
