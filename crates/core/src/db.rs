use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
pub use sqlx::SqlitePool;

use crate::Result;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

const GUIDE_V1: &str = include_str!("../seed/guide-v1.md");

/// Where all persistent data lives: `~/.learning-tool/` unless `LEARNING_TOOL_HOME` is set.
#[derive(Debug, Clone)]
pub struct DataDir {
    pub root: PathBuf,
}

impl DataDir {
    pub fn from_env() -> DataDir {
        let root = match std::env::var_os("LEARNING_TOOL_HOME") {
            Some(p) if !p.is_empty() => PathBuf::from(p),
            _ => {
                let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| ".".into());
                home.join(".learning-tool")
            }
        };
        DataDir { root }
    }

    pub fn db_path(&self) -> PathBuf {
        self.root.join("learning.db")
    }

    pub fn references_dir(&self) -> PathBuf {
        self.root.join("references")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn ensure(&self) -> Result<()> {
        for dir in [self.root.clone(), self.references_dir(), self.backups_dir(), self.logs_dir()] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

fn connect_options(path: &Path) -> Result<SqliteConnectOptions> {
    Ok(SqliteConnectOptions::from_str("sqlite:")?
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true))
}

/// Opens (creating if needed) the database at `path`, migrates it, and seeds the first guide.
pub async fn open(path: &Path) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(connect_options(path)?)
        .await?;
    MIGRATOR.run(&pool).await?;
    seed(&pool).await?;
    Ok(pool)
}

async fn seed(pool: &SqlitePool) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO guide_versions (content, change_summary, created_at, updated_at)
         SELECT ?1, 'Initial guide', ?2, ?2
         WHERE NOT EXISTS (SELECT 1 FROM guide_versions)",
    )
    .bind(GUIDE_V1)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

/// In-memory, migrated and seeded pool for tests (single connection so the database is shared).
#[cfg(any(test, feature = "test-util"))]
pub async fn test_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:").unwrap().foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    seed(&pool).await.unwrap();
    pool
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_apply_and_fts_exists() {
        let pool = test_pool().await;
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE name = 'reference_chunks_fts'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn guide_v1_seeded_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.db");
        open(&path).await.unwrap().close().await;
        let pool = open(&path).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM guide_versions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn concurrent_writers_do_not_fail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.db");
        let a = open(&path).await.unwrap();
        let b = open(&path).await.unwrap();
        let write = |pool: SqlitePool, tag: &'static str| async move {
            for i in 0..50 {
                let mut tx = pool.begin().await?;
                sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?)")
                    .bind(format!("scratch-{tag}-{i}"))
                    .bind("x")
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
            }
            Ok::<_, sqlx::Error>(())
        };
        let (ra, rb) = tokio::join!(tokio::spawn(write(a, "a")), tokio::spawn(write(b, "b")));
        ra.unwrap().unwrap();
        rb.unwrap().unwrap();
    }

    #[test]
    fn data_dir_layout() {
        let d = DataDir { root: "/tmp/lt".into() };
        assert_eq!(d.db_path(), std::path::Path::new("/tmp/lt/learning.db"));
        assert_eq!(d.references_dir(), std::path::Path::new("/tmp/lt/references"));
        assert_eq!(d.backups_dir(), std::path::Path::new("/tmp/lt/backups"));
        assert_eq!(d.logs_dir(), std::path::Path::new("/tmp/lt/logs"));
    }
}
