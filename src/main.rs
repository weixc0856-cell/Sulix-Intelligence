//! Sulix Intelligence — 个人创业者的 AI 战略情报助手
//!
//! 全链路管线：RSS 抓取 → SQLite 去重 → 正文提取 → LLM 分析（分批+重试）
//!          → Markdown 日报 → 写入 SulixNote
//!
//! 架构原则（继承 OPC）：
//! - 确定性核心（抓取/去重/路由/渲染）+ LLM 只在判断节点
//! - 正文提取解决 RSS 摘要过程的质量瓶颈（P0）

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

mod agent;
mod config;
mod db;
mod fetcher;
mod llm;
mod renderer;

// Phase B types used inline in the pipeline
use llm::{AnalyzedArticle, VerticalAnalysis};

/// Application entry point
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("🚀 Sulix Intelligence — 启动");

    // 1. 加载配置
    let config = config::Config::from_file("config.toml")?;
    let api_key = config.get_api_key()?;
    log::info!(
        "配置加载完成: {} 个数据源, LLM 模型: {}",
        config.sources.len(),
        config.llm.model
    );

    // 2. 初始化数据库
    let db_path = get_db_path(&config);
    let db = db::Database::open(&db_path)?;
    log::info!("数据库已连接: {}", db_path.display());

    // 3. 拉取所有 RSS 源（并发）
    log::info!("开始拉取 RSS 源...");
    let articles = fetcher::fetch_all_sources(&config.sources).await?;
    log::info!("拉取完成: {} 篇文章", articles.len());

    // 4. 去重（只保留新文章）
    let mut new_articles = db.dedup_and_insert(&articles)?;
    log::info!(
        "去重完成: {} 篇新文章, {} 篇已存在",
        new_articles.len(),
        articles.len() - new_articles.len()
    );
    // 释放已用缓冲区
    drop(articles);

    if new_articles.is_empty() {
        log::info!("今日无新文章，跳过分析。");
        return Ok(());
    }

    // 打印新文章概览
    println!("\n📋 === 今日新增 {} 篇 ===\n", new_articles.len());
    for article in &new_articles {
        println!("  [{}/{}] {}", article.category, article.source, article.title);
    }

    // 5. 【P0】正文提取 — RSS 摘要不足时，去原文抓取正文
    log::info!("📄 检查并补充正文...");
    let enriched = fetcher::enrich_articles_content(&mut new_articles, 5).await;
    if enriched > 0 {
        log::info!("正文补充完成: {} 篇", enriched);
    } else {
        log::info!("所有文章正文内容已充足，无需补充");
    }

    // 6. 按 vertical 分组（用于 Scan Agent + LLM 分析）
    let grouped = llm::group_by_category(&new_articles);
    log::info!("分组完成: {} 个 vertical", grouped.len());

    // === Phase A: Scan Agent 初筛 ===
    let total_new = new_articles.len(); // 在 move 前保存计数
    let keep_articles: Vec<fetcher::Article> =
        if let Some(ref scan_config) = config.scan_agent {
            if scan_config.enabled && !grouped.is_empty() {
                match agent::scan::scan_and_filter(
                    &grouped,
                    scan_config.threshold,
                    &api_key,
                    &config.llm,
                )
                .await
                {
                    Ok(filtered) => {
                        log::info!(
                            "Scan Agent 完成: {} 篇保留, {} 篇跳过 (阈值≤{})",
                            filtered.keep.len(),
                            filtered.filtered_out.len(),
                            scan_config.threshold,
                        );
                        // debug 日志打印被过滤文章
                        for a in &filtered.filtered_out {
                            log::debug!("  ↳ 跳过: [{}] {}", a.category, a.title);
                        }
                        filtered.keep
                    }
                    Err(e) => {
                        // 防呆：失败时回退全线分析
                        log::warn!(
                            "⚠️ Scan Agent 失败 ({}), 回退到全线分析 ({} 篇)",
                            e,
                            new_articles.len()
                        );
                        new_articles
                    }
                }
            } else {
                new_articles
            }
        } else {
            new_articles
        };

    // 检查是否仍有文章需要分析
    if keep_articles.is_empty() {
        log::info!("所有文章均被 Scan Agent 过滤，跳过 LLM 分析。");
        println!(
            "\n📋 今日 {} 篇新文章全部被过滤（噪音/广告/无关），跳过 LLM 分析。\n",
            total_new
        );
        return Ok(());
    }

    // 7. 重新分组（过滤后 vertical 可能减少）
    let grouped_keep = llm::group_by_category(&keep_articles);
    log::info!(
        "Scan Agent 处理后: {} 个 vertical, {} 篇待分析",
        grouped_keep.len(),
        keep_articles.len()
    );

    // 8. Phase B: 红蓝对抗（或回退到传统 LLM 分析）
    let use_red_blue = config.agent.as_ref().is_some_and(|a| a.synthesis_enabled);

    let (analysis, debate_data) = if use_red_blue && !grouped_keep.is_empty() {
        // Phase B: 红蓝对抗管线
        log::info!("🔴 开始 Synthesis (红军) 分析...");
        let synthesis =
            agent::synthesis::synthesize(
                &grouped_keep,
                &config.prompts.base,
                config.prompts.vertical_overrides.get("AI").map(|s| s.as_str()),
                &api_key,
                &config.llm,
            )
            .await?;
        log::info!("✅ Synthesis 完成: {} 个 vertical", synthesis.len());

        let use_verification = config.agent.as_ref().is_some_and(|a| a.verification_enabled);

        let debate = if use_verification && !synthesis.is_empty() {
            log::info!("🔵 开始 Verification (蓝军) 分析...");
            let verification =
                agent::verification::verify(
                    &synthesis,
                    &config.prompts.base,
                    None,
                    &api_key,
                    &config.llm,
                )
                .await?;
            log::info!("✅ Verification 完成: {} 个 vertical", verification.len());

            log::info!("⚖️ 开始仲裁...");
            let arbitrated = agent::orchestrator::arbitrate(synthesis, verification)?;
            log::info!("✅ 仲裁完成: {} 个 vertical", arbitrated.len());

            arbitrated
        } else {
            // 仅红军，无蓝军 → 跳过仲裁
            log::info!("⚖️ 蓝军未启用，跳过仲裁");
            let analysis: Vec<agent::orchestrator::ArbitrationResult> = synthesis
                .into_iter()
                .map(|s| {
                    let articles: Vec<AnalyzedArticle> = s
                        .narratives
                        .into_iter()
                        .map(|n| AnalyzedArticle {
                            title: n.title,
                            url: String::new(),
                            importance: n.signal_strength,
                            relevance: "待定".into(),
                            time_horizon: "短期".into(),
                            action: "观察".into(),
                            confidence: "低".into(),
                            judgment: n.narrative,
                        })
                        .collect();
                    let analysis = VerticalAnalysis {
                        category: s.category.clone(),
                        articles,
                    };
                    agent::orchestrator::ArbitrationResult {
                        category: s.category,
                        analysis,
                        verdict: String::new(),
                        red_summary: String::new(),
                        blue_summary: String::new(),
                    }
                })
                .collect();
            analysis
        };

        let analysis: Vec<VerticalAnalysis> =
            debate.iter().map(|r| r.analysis.clone()).collect();
        (analysis, Some(debate))
    } else {
        // 传统 LLM 分析（向后兼容）
        log::info!("🤖 开始 LLM 分析...");
        let analysis = llm::analyze(&grouped_keep, &config.prompts, &api_key, &config.llm).await?;
        log::info!("LLM 分析完成: {} 个 vertical", analysis.len());
        (analysis, None)
    };

    // === Phase C: 认知校准 ===
    let calibration_text = agent::calibration::calibrate(&analysis, &api_key, &config.llm).await?;

    // 9. 渲染日报（支持辩论痕迹 + 认知校准）
    let report = renderer::render_daily_report(&analysis, debate_data.as_deref(), Some(&calibration_text))?;
    log::info!("日报渲染完成 ({} 字符)", report.len());

    // 10. 写入 SulixNote Vault
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let report_dir = PathBuf::from(&config.output.vault_path);
    fs::create_dir_all(&report_dir)?;
    let report_path = report_dir.join(format!("{}.md", today));
    fs::write(&report_path, &report)?;

    // 11. 记录到数据库
    db.record_report(&today, &report, total_new)?;

    // === Phase D: Decay Agent 记忆墓地 ===
    if let Some(ref grave_config) = config.graveyard {
        if grave_config.enabled {
            match agent::decay::run_maintenance(
                &db,
                &keep_articles,
                &api_key,
                &config.llm,
                grave_config,
            )
            .await
            {
                Ok(decay_report) => {
                    if decay_report.buried > 0 || !decay_report.wakeups.is_empty() {
                        log::info!(
                            "🪦 Decay Agent: {} 篇埋葬, {} 条唤醒",
                            decay_report.buried,
                            decay_report.wakeups.len()
                        );
                    }
                    // 如果有唤醒条目，追加到日报
                    if !decay_report.wakeups.is_empty() {
                        let mut wakeup_md = String::from("\n\n---\n## 📢 从墓地唤醒\n\n");
                        for entry in &decay_report.wakeups {
                            wakeup_md.push_str(&format!(
                                "- **{}** ({}): {}\n",
                                entry.title,
                                entry.category,
                                if entry.compressed_content.is_empty() {
                                    "旧信号今日重新激活"
                                } else {
                                    &entry.compressed_content
                                },
                            ));
                        }
                        // 追加到已写入的日报文件
                        let mut full_report = fs::read_to_string(&report_path)?;
                        full_report.push_str(&wakeup_md);
                        fs::write(&report_path, &full_report)?;
                        log::info!("📢 唤醒标记已追加到日报");
                    }
                }
                Err(e) => {
                    log::warn!("⚠️ Decay Agent 失败: {}", e);
                }
            }
        }
    }

    println!("\n✅ 日报已生成: {}", report_path.display());
    log::info!("✅ Sulix Intelligence 全链路执行完成");
    Ok(())
}

/// 获取数据库路径
fn get_db_path(config: &config::Config) -> PathBuf {
    let data_dir = config
        .storage
        .as_ref()
        .and_then(|s| s.data_dir.as_deref())
        .unwrap_or("data");
    PathBuf::from(data_dir).join("intel.db")
}
