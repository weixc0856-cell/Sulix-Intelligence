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

mod config;
mod db;
mod fetcher;
mod llm;
mod renderer;

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

    // 6. 按 vertical 分组
    let grouped = llm::group_by_category(&new_articles);
    log::info!("分组完成: {} 个 vertical", grouped.len());

    // 7. LLM 分析（带分批 + 重试）
    log::info!("🤖 开始 LLM 分析...");
    let analysis = llm::analyze(&grouped, &config.prompts, &api_key, &config.llm).await?;
    log::info!("LLM 分析完成: {} 个 vertical", analysis.len());

    // 8. 渲染日报
    let report = renderer::render_daily_report(&analysis)?;
    log::info!("日报渲染完成 ({} 字符)", report.len());

    // 9. 写入 SulixNote Vault
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let report_dir = PathBuf::from(&config.output.vault_path);
    fs::create_dir_all(&report_dir)?;
    let report_path = report_dir.join(format!("{}.md", today));
    fs::write(&report_path, &report)?;

    // 10. 记录到数据库
    db.record_report(&today, &report, new_articles.len())?;

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
