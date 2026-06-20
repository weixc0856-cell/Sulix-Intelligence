//! 数据库模块 — SQLite 去重与存储

use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

use crate::fetcher::Article;

/// 数据库句柄
pub struct Database {
    conn: Connection,
}

impl Database {
    /// 打开或创建 SQLite 数据库
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;

        // 启用 WAL 模式（更好的并发性能）
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        // 初始化表结构
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS articles (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                content TEXT,
                summary TEXT,
                published_at TEXT,
                fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
                category TEXT DEFAULT 'uncategorized',
                is_read INTEGER DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_articles_fetched_at
                ON articles(fetched_at);
            CREATE INDEX IF NOT EXISTS idx_articles_source
                ON articles(source);
            CREATE INDEX IF NOT EXISTS idx_articles_category
                ON articles(category);

            CREATE TABLE IF NOT EXISTS daily_reports (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                content TEXT NOT NULL,
                article_count INTEGER DEFAULT 0,
                status TEXT DEFAULT 'generated'
            );",
        )?;

        Ok(Database { conn })
    }

    /// 去重并插入新文章
    /// 返回之前未存储过的新文章列表
    pub fn dedup_and_insert(&self, articles: &[Article]) -> Result<Vec<Article>> {
        let mut new_articles = Vec::new();

        for article in articles {
            // 检查是否已存在（基于 URL hash）
            let exists: bool = self.conn.query_row(
                "SELECT COUNT(*) > 0 FROM articles WHERE id = ?1",
                params![article.id],
                |row| row.get(0),
            )?;

            if !exists {
                self.conn.execute(
                    "INSERT INTO articles (id, source, title, url, content, summary, published_at, category)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        article.id,
                        article.source,
                        article.title,
                        article.url,
                        article.content,
                        article.summary,
                        article.published_at.map(|d| d.to_rfc3339()),
                        article.category,
                    ],
                )?;
                new_articles.push(article.clone());
            }
        }

        Ok(new_articles)
    }

    /// 统计今日新增文章数
    #[allow(dead_code)]
    pub fn today_count(&self) -> Result<u32> {
        let count: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM articles WHERE date(fetched_at) = date('now')",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// 记录日报
    pub fn record_report(&self, date: &str, content: &str, count: usize) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO daily_reports (id, content, article_count, status)
             VALUES (?1, ?2, ?3, 'generated')",
            params![date, content, count as u32],
        )?;
        Ok(())
    }

    /// 获取最近 N 天的文章统计
    #[allow(dead_code)]
    pub fn recent_stats(&self, days: u32) -> Result<Vec<(String, u32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT date(fetched_at) as day, COUNT(*) as cnt
             FROM articles
             WHERE fetched_at >= datetime('now', ?1)
             GROUP BY day
             ORDER BY day DESC",
        )?;

        let range = format!("-{} days", days);
        let rows = stmt.query_map(params![range], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
        })?;

        let mut stats = Vec::new();
        for row in rows {
            stats.push(row?);
        }
        Ok(stats)
    }
}
