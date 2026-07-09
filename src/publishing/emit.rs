//! Stage 5: Emit — MDX/Markdown rendering + output

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;

use crate::domain::theme::{Theme, ThemeAnalysis};
use crate::config::Config;
use crate::renderer::publisher::Publisher;

use super::infer::InferredState;

/// Emit: Markdown + MDX 渲染 + 输出
#[allow(clippy::too_many_arguments)]
pub async fn publish_emit(
    config: &Config,
    today: &str,
    vault_base: PathBuf,
    themes: &[Theme],
    analyses: &[ThemeAnalysis],
    new_articles: &[crate::fetcher::Article],
    inferred: &InferredState,
) -> Result<()> {
    // Markdown 输出
    let md_ctx = crate::renderer::publisher::PublishContext {
        themes: themes.to_vec(), analyses: analyses.to_vec(),
        date: today.to_string(), locale: "en".to_string(),
        theses: vec![], reports: vec![], canonical_decisions: vec![],
        asi_scores: HashMap::new(), editor_notes: vec![],
        belief_notes_html: String::new(), articles: vec![],
        mdx_output_dir: None, output_dir: vault_base.clone(),
        reflections: vec![], thesis_decisions: vec![], outcomes: vec![],
    };
    crate::renderer::publisher::MarkdownPublisher::new().publish(&md_ctx)?;
    log::info!("📝 Markdown 输出: {} 个主题", themes.len());

    // MDX 输出（主要输出格式）
    if let Some(ref mdx_out) = config.output.mdx_dir {
        let mdx_ctx = crate::renderer::publisher::PublishContext {
            themes: themes.to_vec(), analyses: analyses.to_vec(),
            date: today.to_string(), locale: "en".to_string(),
            theses: inferred.memory.theses().to_vec(),
            reports: inferred.premium_reports.clone(),
            canonical_decisions: inferred.memory.all_decisions().to_vec(),
            asi_scores: inferred.asi_score_map.clone(),
            editor_notes: inferred.editor_notes.clone(),
            belief_notes_html: inferred.beliefs_html.clone(),
            articles: new_articles.to_vec(),
            mdx_output_dir: Some(PathBuf::from(mdx_out)),
            output_dir: vault_base.clone(),
            reflections: inferred.memory.all_reflections().to_vec(),
            thesis_decisions: inferred.thesis_decisions.clone(),
            outcomes: inferred.memory.all_outcomes().to_vec(),
        };
        if let Err(e) = crate::renderer::publisher::MdxPublisher::new().publish(&mdx_ctx) {
            log::warn!("⚠️ MDX 输出失败: {}", e);
        }

        // Investigation MDX
        if !inferred.investigation_reports.is_empty() {
            let inv_dir = std::path::Path::new(mdx_out).join("investigation");
            if let Err(e) = std::fs::create_dir_all(&inv_dir) {
                log::warn!("⚠️ Cannot create investigation dir: {}", e);
            } else {
                for (slug, report, assessment_id, inv_id) in &inferred.investigation_reports {
                    let mdx = crate::renderer::mdx::render_investigation_mdx(
                        report, slug, assessment_id.as_deref(), inv_id.as_deref(), "en",
                    );
                    if let Err(e) = std::fs::write(inv_dir.join(format!("{}.md", slug)), &mdx) {
                        log::warn!("⚠️ Investigation MDX write failed [{}]: {}", slug, e);
                    }
                }
                log::info!("📝 Investigation MDX: {} 篇", inferred.investigation_reports.len());
            }
        }
    }

    Ok(())
}
