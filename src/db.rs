//! 数据库模块 — SQLite 去重与存储

use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

use crate::fetcher::Article;

// ===== Phase D: 记忆墓地数据结构 =====

/// 埋葬条目（写入用）
pub struct BurialEntry {
    pub id: String,
    pub article_id: String,
    pub title: String,
    pub category: String,
    pub source: String,
    pub original_importance: u8,
    pub compressed_content: String,
    pub burial_reason: String,
}

/// 墓地条目（读取用）
#[derive(Debug, Clone)]
pub struct GraveyardEntry {
    pub id: String,
    #[allow(dead_code)]
    pub article_id: String,
    pub title: String,
    pub category: String,
    pub compressed_content: String,
    #[allow(dead_code)]
    pub buried_at: String,
}

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
            );

            -- Phase D: 记忆墓地
            CREATE TABLE IF NOT EXISTS knowledge_graveyard (
                id TEXT PRIMARY KEY,
                article_id TEXT,
                title TEXT NOT NULL,
                category TEXT,
                source TEXT,
                original_importance INTEGER DEFAULT 5,
                compressed_content TEXT,
                burial_reason TEXT DEFAULT 'age',
                buried_at TEXT NOT NULL DEFAULT (datetime('now')),
                wake_up_count INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_graveyard_category
                ON knowledge_graveyard(category);",
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

    // ===== Phase D: 记忆墓地 =====

    /// 查询超过 retention_days 且未被埋葬的文章 ID
    pub fn get_expired_article_ids(&self, retention_days: u32) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id
             FROM articles a
             WHERE a.fetched_at < date('now', ?1)
             AND a.id NOT IN (
                 SELECT g.article_id FROM knowledge_graveyard g WHERE g.article_id IS NOT NULL
             )",
        )?;
        let range = format!("-{} days", retention_days);
        let rows = stmt.query_map(params![range], |row| row.get::<_, String>(0))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// 批量埋葬文章
    pub fn bury_articles(&self, entries: &[BurialEntry]) -> Result<usize> {
        let mut count = 0;
        for entry in entries {
            let rows = self.conn.execute(
                "INSERT OR IGNORE INTO knowledge_graveyard
                 (id, article_id, title, category, source, original_importance, compressed_content, burial_reason)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    entry.id,
                    entry.article_id,
                    entry.title,
                    entry.category,
                    entry.source,
                    entry.original_importance,
                    entry.compressed_content,
                    entry.burial_reason,
                ],
            )?;
            count += rows;
        }
        Ok(count)
    }

    /// 搜索墓地（唤醒匹配）
    pub fn search_graveyard(&self, keyword: &str, category: &str) -> Result<Vec<GraveyardEntry>> {
        let pattern = format!("%{}%", keyword);
        let mut stmt = self.conn.prepare(
            "SELECT id, article_id, title, category, compressed_content, buried_at
             FROM knowledge_graveyard
             WHERE title LIKE ?1 AND category = ?2
             ORDER BY buried_at DESC
             LIMIT 3",
        )?;
        let rows = stmt.query_map(params![pattern, category], |row| {
            Ok(GraveyardEntry {
                id: row.get(0)?,
                article_id: row.get(1)?,
                title: row.get(2)?,
                category: row.get(3)?,
                compressed_content: row.get(4)?,
                buried_at: row.get(5)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// 获取文章详情（用于 Decay Agent 压缩）
    pub fn get_article_by_id(&self, id: &str) -> Result<Option<(String, String, String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT title, category, source, content FROM articles WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            ))
        })?;
        match rows.next() {
            Some(Ok(row)) => Ok(Some(row)),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let conn = Connection::open_in_memory().unwrap();
        // Initialize schema (same as Database::open)
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS articles (
                id TEXT PRIMARY KEY, source TEXT NOT NULL, title TEXT NOT NULL,
                url TEXT NOT NULL, content TEXT, summary TEXT, published_at TEXT,
                fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
                category TEXT DEFAULT 'uncategorized', is_read INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_articles_fetched_at ON articles(fetched_at);
            CREATE INDEX IF NOT EXISTS idx_articles_source ON articles(source);
            CREATE INDEX IF NOT EXISTS idx_articles_category ON articles(category);
            CREATE TABLE IF NOT EXISTS daily_reports (
                id TEXT PRIMARY KEY, created_at TEXT NOT NULL DEFAULT (datetime('now')),
                content TEXT NOT NULL, article_count INTEGER DEFAULT 0, status TEXT DEFAULT 'generated'
            );
            CREATE TABLE IF NOT EXISTS knowledge_graveyard (
                id TEXT PRIMARY KEY, article_id TEXT, title TEXT NOT NULL, category TEXT,
                source TEXT, original_importance INTEGER DEFAULT 5, compressed_content TEXT,
                burial_reason TEXT DEFAULT 'age', buried_at TEXT NOT NULL DEFAULT (datetime('now')),
                wake_up_count INTEGER DEFAULT 0
            );"
        ).unwrap();
        Database { conn }
    }

    fn test_article(id: &str, title: &str, url: &str) -> Article {
        Article {
            id: id.into(),
            source: "test".into(),
            title: title.into(),
            url: url.into(),
            content: None,
            summary: None,
            published_at: None,
            category: "AI".into(),
        }
    }

    #[test]
    fn test_dedup_empty() {
        let db = test_db();
        let result = db.dedup_and_insert(&[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_dedup_all_new() {
        let db = test_db();
        let articles = vec![
            test_article("1", "Article 1", "https://example.com/1"),
            test_article("2", "Article 2", "https://example.com/2"),
        ];
        let new = db.dedup_and_insert(&articles).unwrap();
        assert_eq!(new.len(), 2);
    }

    #[test]
    fn test_dedup_duplicates() {
        let db = test_db();
        let a = test_article("1", "Dup", "https://example.com/dup");
        let first = db.dedup_and_insert(&[a.clone()]).unwrap();
        assert_eq!(first.len(), 1);
        let second = db.dedup_and_insert(&[a]).unwrap();
        assert_eq!(second.len(), 0);
    }

    #[test]
    fn test_dedup_partial() {
        let db = test_db();
        let a1 = test_article("1", "One", "https://example.com/1");
        let a2 = test_article("2", "Two", "https://example.com/2");
        let a3 = test_article("3", "Three", "https://example.com/3");
        db.dedup_and_insert(&[a1.clone(), a2.clone()]).unwrap();
        let result = db.dedup_and_insert(&[a1, a2, a3]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Three");
    }

    #[test]
    fn test_record_report() {
        let db = test_db();
        db.record_report("2026-06-21", "# Test Report", 5).unwrap();
        let count: u32 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM daily_reports WHERE id = '2026-06-21'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
