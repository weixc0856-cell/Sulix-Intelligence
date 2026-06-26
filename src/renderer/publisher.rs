//! Publisher Trait — 统一渲染输出抽象
//!
//! MDX 已成为主要输出格式（2026-06-24 ADR-003）。
//! Rust 不再生成 HTML 页面，仅输出 MDX 知识资产供 Astro 前端消费。
//!
//! 当前活跃实现：
//!   - MdxPublisher:      核心输出，生成 MDX 供 Astro Content Collections
//!   - MarkdownPublisher: 备用 Substrack Markdown 输出
//!
//! 已移除（第一代遗产）:
//!   - HtmlPublisher       → MDX 取代
//!   - DashboardPublisher  → intel-web 前端职责
//!   - SeoPublisher        → Astro Head/Layout 组件职责

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::clusterer::{Theme, ThemeAnalysis};
use crate::domain::reflection::Reflection;
use crate::domain::thesis::Thesis;
use crate::engine::memory::Outcome;
use crate::engine::premium::PremiumReport;

/// 发布上下文 — 所有发布器共享的数据
///
/// 仅包含活跃发布器（MdxPublisher / MarkdownPublisher）实际读取的字段。
/// 已移除字段（Phase 1 清理）：
///   calibration, attributable_sources, flash_headline, change_summary,
///   archive_entries, archive_entries_zh, source_statuses, decisions,
///   css_content, analyses_zh, language, watchlist_count
pub struct PublishContext {
    pub themes: Vec<Theme>,
    pub analyses: Vec<ThemeAnalysis>,
    pub date: String,
    pub theses: Vec<Thesis>,
    pub reports: Vec<PremiumReport>,
    /// ASI/Confidence 评分 per theme_title → (asi, confidence, final)
    pub asi_scores: HashMap<String, (f64, f64, f64)>,
    /// Editor Agent 分析结果（个人影响分析）
    pub editor_notes: Vec<crate::agent::editor::EditorNote>,
    /// Belief Engine HTML 区块
    pub belief_notes_html: String,
    /// 今日原始文章列表（用于 Signal Feed 板块）
    pub articles: Vec<crate::fetcher::Article>,
    pub output_dir: PathBuf,
    /// MDX 输出目录（如 output/），None = 不输出 MDX
    pub mdx_output_dir: Option<PathBuf>,
    /// Reflection 记录
    pub reflections: Vec<Reflection>,
    /// Decision Intelligence: Thesis → Decision 映射
    pub thesis_decisions: Vec<crate::engine::decision::ThesisDecision>,
    /// Outcome 记录（用于 MDX frontmatter 中的 Historical Accuracy 展示）
    pub outcomes: Vec<Outcome>,
    /// Canonical Decision records (DEC-XXXX)
    pub canonical_decisions: Vec<crate::engine::decision::DecisionRecord>,
}

/// 发布器 Trait
///
/// 每个输出格式实现此 trait。
/// publish() 接收共享上下文，将结果直接写入磁盘。
pub trait Publisher {
    /// 执行发布
    fn publish(&self, ctx: &PublishContext) -> Result<()>;
}

// ===== MarkdownPublisher =====

pub struct MarkdownPublisher;

impl Default for MarkdownPublisher {
    fn default() -> Self {
        Self
    }
}

impl MarkdownPublisher {
    pub fn new() -> Self {
        Self
    }
}

impl Publisher for MarkdownPublisher {
    fn publish(&self, ctx: &PublishContext) -> Result<()> {
        for (theme, analysis) in ctx.themes.iter().zip(ctx.analyses.iter()) {
            let slug = theme
                .title
                .to_lowercase()
                .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
                .replace(' ', "-");
            let md = crate::renderer::markdown::render_signal_markdown(theme, analysis, &ctx.date);
            let path = PathBuf::from("content/posts").join(format!("{}-{}.md", ctx.date, slug));
            std::fs::write(&path, &md)?;
        }

        Ok(())
    }
}

// ===== MdxPublisher =====

pub struct MdxPublisher;

impl Default for MdxPublisher {
    fn default() -> Self {
        Self
    }
}

impl MdxPublisher {
    pub fn new() -> Self {
        Self
    }
}

/// ASCII-safe slug: drop non-ASCII, lowercase, spaces → hyphens, collapse hyphens.
pub(crate) fn ascii_slug(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_ascii())
        .collect::<String>()
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

/// Stable short ID from thesis.id (e.g. "thesis-1750000001" → "75000000").
/// Used as fallback when ascii_slug returns empty (pure non-ASCII titles).
pub(crate) fn short_id_from_thesis(thesis_id: &str) -> String {
    let digits = thesis_id.trim_start_matches("thesis-");
    digits.get(digits.len().saturating_sub(8)..).unwrap_or(digits).to_string()
}

impl Publisher for MdxPublisher {
    fn publish(&self, ctx: &PublishContext) -> Result<()> {
        let mdx_dir = match &ctx.mdx_output_dir {
            Some(d) => d.clone(),
            None => return Ok(()),
        };

        // 1. Daily signals → output/daily/
        let daily_dir = mdx_dir.join("daily");
        std::fs::create_dir_all(&daily_dir)?;

        for (theme, analysis) in ctx.themes.iter().zip(ctx.analyses.iter()) {
            let asi = ctx.asi_scores.get(&theme.title).map(|s| s.0).unwrap_or(0.0);
            let conf = ctx.asi_scores.get(&theme.title).map(|s| s.1).unwrap_or(0.0);
            let mdx = crate::renderer::mdx::render_daily_mdx(
                theme,
                analysis,
                &ctx.date,
                asi,
                conf,
                &ctx.editor_notes,
            );
            let slug = ascii_slug(&theme.title);
            let path = daily_dir.join(format!("{}-{}.md", ctx.date, slug));
            std::fs::write(&path, &mdx)?;
        }

        // 2. Assessment → output/assessment/ (stable ASM-ID filenames)
        // Legacy: also write to output/thesis/ for backward compat during transition
        let thesis_dir = mdx_dir.join("thesis");
        let assessment_dir = mdx_dir.join("assessment");
        let decision_dir = mdx_dir.join("decision");
        std::fs::create_dir_all(&thesis_dir)?;
        std::fs::create_dir_all(&assessment_dir)?;
        std::fs::create_dir_all(&decision_dir)?;
        // Build decision lookup: thesis_id → ThesisDecision
        let decision_map: std::collections::HashMap<
            &str,
            &crate::engine::decision::ThesisDecision,
        > = ctx
            .thesis_decisions
            .iter()
            .map(|d| (d.thesis_id.as_str(), d))
            .collect();
        // Build canonical Decision record lookup: asm_id → DecisionRecord
        let dec_record_map: std::collections::HashMap<
            &str,
            &crate::engine::decision::DecisionRecord,
        > = ctx
            .canonical_decisions
            .iter()
            .map(|d| (d.asm_id.as_str(), d))
            .collect();
        // Build outcome lookup: thesis_id → Vec<Outcome>
        let outcomes_map: std::collections::HashMap<&str, Vec<&Outcome>> = ctx
            .outcomes
            .iter()
            .fold(std::collections::HashMap::new(), |mut map, o| {
                map.entry(o.thesis_id.as_str()).or_default().push(o);
                map
            });
        for thesis in &ctx.theses {
            let decision = decision_map.get(thesis.id.as_str()).copied();
            let dec_record = thesis.assessment_id.as_deref()
                .and_then(|asm_id| dec_record_map.get(asm_id).copied());
            let thesis_outcomes: Vec<Outcome> = outcomes_map
                .get(thesis.id.as_str())
                .map(|v| v.iter().map(|o| (*o).clone()).collect())
                .unwrap_or_default();
            let mdx = crate::renderer::mdx::render_thesis_mdx(thesis, &thesis_outcomes, decision, dec_record);

            // Primary: stable ASM-ID filename (if assessment_id assigned)
            if let Some(ref asm_id) = thesis.assessment_id {
                let asm_path = assessment_dir.join(format!("{}.md", asm_id));
                std::fs::write(&asm_path, &mdx)?;
            }

            // Fallback / legacy: date+slug filename in output/thesis/ (old format)
            let slug_base = ascii_slug(&thesis.title);
            let slug = if slug_base.is_empty() {
                short_id_from_thesis(&thesis.id)
            } else {
                slug_base
            };
            let path = thesis_dir.join(format!("{}-{}.md", thesis.created, slug));
            std::fs::write(&path, &mdx)?;
        }

        // Write output/decision/DEC-XXXX.md (standalone canonical Decision files)
        for dec in &ctx.canonical_decisions {
            let dec_mdx = crate::renderer::mdx::render_decision_mdx(dec);
            let dec_path = decision_dir.join(format!("{}.md", dec.id));
            std::fs::write(&dec_path, &dec_mdx)?;
        }

        // 3. Premium research → output/research/
        if !ctx.reports.is_empty() {
            let research_dir = mdx_dir.join("research");
            std::fs::create_dir_all(&research_dir)?;
            for report in &ctx.reports {
                let mdx = crate::renderer::mdx::render_research_mdx(report);
                let slug = ascii_slug(&report.theme_title);
                let path = research_dir.join(format!("{}-{}.md", ctx.date, slug));
                std::fs::write(&path, &mdx)?;
            }
        }

        // 4. Reflections → output/reflection/
        if !ctx.reflections.is_empty() {
            let reflection_dir = mdx_dir.join("reflection");
            std::fs::create_dir_all(&reflection_dir)?;
            for reflection in &ctx.reflections {
                let thesis_title = ctx
                    .theses
                    .iter()
                    .find(|t| t.id == reflection.thesis_id)
                    .map(|t| t.title.as_str())
                    .unwrap_or("Unknown Thesis");
                let mdx = crate::renderer::mdx::render_reflection_mdx(reflection, thesis_title);
                let slug = format!("reflection-{}", reflection.id.replace(':', "-"));
                let path = reflection_dir.join(format!("{}.md", slug));
                std::fs::write(&path, &mdx)?;
            }
        }

        // 5. Article digest → output_dir/digest/ (local only, not part of intel-web content)
        if !ctx.articles.is_empty() {
            let digest_dir = ctx.output_dir.join("digest");
            std::fs::create_dir_all(&digest_dir)?;
            let mdx = crate::renderer::mdx::render_digest_mdx(&ctx.articles, &ctx.date);
            let path = digest_dir.join(format!("{}.md", ctx.date));
            std::fs::write(&path, &mdx)?;
        }

        log::info!(
            "📝 MDX 输出: {} daily, {} thesis, {} reflections, {} research, {} digest articles",
            ctx.themes.len(),
            ctx.theses.len(),
            ctx.reflections.len(),
            ctx.reports.len(),
            ctx.articles.len(),
        );

        Ok(())
    }
}
