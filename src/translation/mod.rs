//! 翻译 Agent — 过渡桥梁
//!
//! 文件级翻译：将英文 MDX 翻译为 zh-cn / zh-tw，写入 `mdx_dir/{locale}/{type}/`。
//!
//! 过渡桥梁：对象级翻译（Step 3.5: 对象接入 delivery 后，MDX 从 Localized 字段渲染）
//! 就绪后，本模块的文件遍历逻辑弃用。

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use sha2::{Sha256, Digest};
use anyhow::Result;
use serde_json;

use crate::config::{Config, LlmConfig};

/// 翻译覆盖度报告
#[derive(Debug, Clone, Default)]
pub struct TranslationCoverage {
    pub total_files: usize,
    pub translated: usize,
    pub skipped: usize,
    pub stale: usize,
    pub failed: usize,
    pub duration_seconds: f64,
}

/// 预处理后的英文源文件
struct SourceFile {
    /// 相对于 mdx_dir 的路径，如 "thesis/xxx.md"
    relative_path: String,
    /// 分类目录，如 "thesis"
    dir_type: String,
    /// 文件名，如 "xxx.md"
    file_name: String,
    /// 完整内容
    content: String,
    /// 正文（不含 frontmatter）的 SHA256
    body_hash: String,
}

/// 从文件中提取 YAML frontmatter 和正文
fn split_frontmatter(content: &str) -> (&str, &str) {
    let content = content.trim_start();
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("\n---") {
            let frontmatter = &content[3..3 + end];
            let body = &content[3 + end + 5..];
            return (frontmatter.trim(), body.trim());
        }
    }
    ("", content.trim())
}

/// 计算正文的 SHA256 摘要
fn hash_body(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 解析 frontmatter 为键值对
fn parse_frontmatter(fm: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in fm.lines() {
        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim().to_string();
            let value = line[pos + 1..].trim().to_string();
            map.insert(key, value);
        }
    }
    map
}

/// 从译文中读取 translation_source_hash（如果有）
fn read_translation_hash(content: &str) -> Option<String> {
    let (fm, _) = split_frontmatter(content);
    let map = parse_frontmatter(fm);
    map.get("translation_source_hash").cloned()
}

/// 收集需要翻译的源文件
fn collect_source_files(mdx_dir: &Path, translate_dirs: &[String], max_files: usize) -> Vec<SourceFile> {
    let mut files = Vec::new();
    for dir_type in translate_dirs {
        let dir_path = mdx_dir.join(dir_type);
        if !dir_path.exists() {
            continue;
        }
        let entries = match std::fs::read_dir(&dir_path) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(true, |e| e != "md") {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            let relative_path = format!("{}/{}", dir_type, file_name);
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let (_, body) = split_frontmatter(&content);
            let body_hash = hash_body(body);
            files.push(SourceFile {
                relative_path,
                dir_type: dir_type.clone(),
                file_name,
                content,
                body_hash,
            });
            if files.len() >= max_files {
                return files;
            }
        }
    }
    files
}

/// 构建翻译 prompt
fn build_translation_prompt(content: &str, target_locale: &str) -> String {
    let immutable_fields = [
        "id", "slug", "date", "version", "dec_id", "asm_id", "inv_id", "assessment_id",
        "svi", "asi", "confidence", "evidences", "challenges", "signal_strength",
        "locale", "lang", "status", "decision", "decision_type", "horizon", "stability",
        "primary_domain", "secondary_domains", "state", "stage", "is_premium",
        "contract_version", "created", "updated", "generated_at",
        "thesis_ref", "question", "verdict", "type", "source",
        "translation_source_hash",
    ];
    let translate_fields = ["title", "summary", "question", "rationale", "decision_rationale"];

    format!(
        r#"你是一个专业的战略内容翻译。保留原文的决策力度、不确定性表达和判断框架。

规则：
1. 不可改动的系统字段（逐字节保留原值）：{}
2. 必须翻译的文本字段：{}
3. 保留 Markdown 结构：标题层级、列表、引用、代码块
4. 保留 YAML frontmatter 中未特别说明的所有其他字段原值
5. 专业名词首次出现保留英文括号标注：如 "大型语言模型 (LLM)"
6. 不增加原文不存在的观点或判断
7. 不弱化原文的确定性表述
8. 将 YAML frontmatter 中的 text 字段翻译后填入正确位置
9. target locale: {}

译文必须包含完整的 YAML frontmatter（--- 包裹）和正文。

以 JSON 格式返回，包含 translated 字段：{{"translated": "---\ntitle: ...\n---\n\nbody..."}}

待翻译文档：
---
{}
---"#,
        immutable_fields.join(", "),
        translate_fields.join(", "),
        target_locale,
        content,
    )
}

/// 调用 LLM 翻译单个文件
///
/// LLM 返回 JSON: `{"translated": "---\ntitle: ...\n---\n\nbody..."}`
/// 提取 translated 字段得到完整 MDX。
async fn translate_file(
    content: &str,
    target_locale: &str,
    api_key: &str,
    llm: &LlmConfig,
) -> Result<String> {
    let prompt = build_translation_prompt(content, target_locale);
    let result = crate::llm::call_and_parse(api_key, llm, &prompt, "").await?;
    // LLM 返回 JSON 对象 — 尝试提取 translated 字段
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
        if let Some(translated) = parsed.get("translated").and_then(|v| v.as_str()) {
            return Ok(translated.to_string());
        }
    }
    // Fallback: 直接返回（可能 response_format 未生效时）
    Ok(result)
}

/// 将翻译结果写入磁盘
fn write_translation(
    mdx_dir: &Path,
    locale: &str,
    dir_type: &str,
    file_name: &str,
    translated_content: &str,
    body_hash: &str,
) -> Result<()> {
    let target_dir = mdx_dir.join(locale).join(dir_type);
    std::fs::create_dir_all(&target_dir)?;
    let target_path = target_dir.join(file_name);

    // 在 frontmatter 中注入 translation_source_hash
    let (fm, body) = split_frontmatter(translated_content);
    let hash_line = format!("translation_source_hash: {}", body_hash);

    let new_content = if fm.is_empty() {
        // 没有 frontmatter，创建一个
        format!("---\n{}\n---\n\n{}", hash_line, body)
    } else {
        // 在现有 frontmatter 末尾加入 hash
        format!("---\n{}\n{}\n---\n\n{}", fm, hash_line, body)
    };

    std::fs::write(&target_path, new_content)?;
    Ok(())
}

/// 发布翻译—遍历英文 MDX，对各目标语言调用 LLM 补齐
///
/// 永不返回 Err：失败记入 coverage 而非中断管线。
pub async fn publish_translate(
    config: &Config,
    api_key: &str,
) -> TranslationCoverage {
    let start = std::time::Instant::now();
    let translation_cfg = match &config.translation {
        Some(cfg) if cfg.enabled => cfg,
        _ => return TranslationCoverage::default(),
    };

    let mdx_dir = match &config.output.mdx_dir {
        Some(dir) => PathBuf::from(dir),
        None => {
            log::warn!("📖 translation: mdx_dir not configured, skipping");
            return TranslationCoverage::default();
        }
    };

    let sources = collect_source_files(&mdx_dir, &translation_cfg.translate_dirs, translation_cfg.max_files_per_run);
    let total_locales = translation_cfg.target_locales.len();
    if sources.is_empty() {
        log::info!("📖 translation: no source files to translate");
        return TranslationCoverage::default();
    }

    let mut coverage = TranslationCoverage {
        total_files: sources.len() * total_locales,
        ..Default::default()
    };

    for locale in &translation_cfg.target_locales {
        for source in &sources {
            if !source.content.starts_with("---") {
                coverage.skipped += 1;
                log::debug!("📖 translation: skip {} (no frontmatter)", source.relative_path);
                continue;
            }

            // Check if target exists and hash matches
            let target_path = mdx_dir.join(locale).join(&source.dir_type).join(&source.file_name);
            if target_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&target_path) {
                    if let Some(stored_hash) = read_translation_hash(&content) {
                        if stored_hash == source.body_hash {
                            coverage.skipped += 1;
                            continue; // 一致 → skip
                        } else {
                            coverage.stale += 1; // 不一致 → stale, retranslate
                        }
                    }
                }
            }

            match translate_file(
                &source.content,
                locale,
                api_key,
                &config.llm,
            ).await {
                Ok(translated) => {
                    if let Err(e) = write_translation(
                        &mdx_dir, locale,
                        &source.dir_type, &source.file_name,
                        &translated, &source.body_hash,
                    ) {
                        coverage.failed += 1;
                        log::warn!("📖 translation: write failed [{}]: {}", source.relative_path, e);
                    } else {
                        coverage.translated += 1;
                        log::info!("📖 translation: {} → {}/{}", source.relative_path, locale, source.file_name);
                    }
                }
                Err(e) => {
                    coverage.failed += 1;
                    log::warn!("📖 translation: LLM failed [{}]: {}", source.relative_path, e);
                }
            }
        }
    }

    coverage.duration_seconds = start.elapsed().as_secs_f64();
    log::info!("📖 translation: {}/{} translated ({} skipped, {} stale, {} failed) in {:.1}s",
        coverage.translated, coverage.total_files,
        coverage.skipped, coverage.stale, coverage.failed,
        coverage.duration_seconds);
    coverage
}
