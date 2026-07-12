//! 数据库模块 — SQLite 去重与存储

use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

use sulix_observation::fetcher::Article;

/// 数据库句柄
pub struct Database {
    conn: Connection,
}

impl Database {
    /// 打开或创建 SQLite 数据库
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA busy_timeout = 5000;")?;

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
                ON articles(category);",
        )?;

        Ok(Database { conn })
    }

    /// 去重并插入新文章
    /// 返回之前未存储过的新文章列表
    /// PK 唯一性保证下，用 INSERT OR IGNORE + changes() 代替 SELECT+INSERT
    pub fn dedup_and_insert(&self, articles: &[Article]) -> Result<Vec<Article>> {
        let tx = self.conn.unchecked_transaction()?;
        let mut new_articles = Vec::new();

        for article in articles {
            let rows = tx.execute(
                "INSERT OR IGNORE INTO articles (id, source, title, url, content, summary, published_at, category)
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
            if rows > 0 {
                new_articles.push(article.clone());
            }
        }

        tx.commit()?;
        Ok(new_articles)
    }
}

/// 获取数据库路径（从配置计算）
pub fn get_db_path(config: &sulix_config::Config) -> std::path::PathBuf {
    let data_dir = config
        .storage
        .as_ref()
        .and_then(|s| s.data_dir.as_deref())
        .unwrap_or("data");
    std::path::PathBuf::from(data_dir).join("intel.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let conn = Connection::open_in_memory().unwrap();
        // Initialize schema (same as Database::open, minus WAL which is file-only)
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS articles (
                id TEXT PRIMARY KEY, source TEXT NOT NULL, title TEXT NOT NULL,
                url TEXT NOT NULL, content TEXT, summary TEXT, published_at TEXT,
                fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
                category TEXT DEFAULT 'uncategorized', is_read INTEGER DEFAULT 0
            );",
        )
        .unwrap();
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
            wiki_summary: None,
            evidence_type: String::new(),
            is_internal: false,
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
        let first = db.dedup_and_insert(std::slice::from_ref(&a)).unwrap();
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
}
