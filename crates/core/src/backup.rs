use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;

pub use crate::db::DataDir;
use crate::settings::{Settings, TzSetting};
use crate::{Error, Result};

fn backup_dir(dirs: &DataDir, s: &Settings) -> PathBuf {
    s.backup_dir.clone().unwrap_or_else(|| dirs.backups_dir())
}

fn local_stamp(s: &Settings, now: DateTime<Utc>) -> String {
    let fmt = "%Y%m%d-%H%M%S";
    match s.timezone {
        TzSetting::System => now.with_timezone(&chrono::Local).format(fmt).to_string(),
        TzSetting::Named(tz) => now.with_timezone(&tz).format(fmt).to_string(),
    }
}

fn backup_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("learning-") && n.ends_with(".db"))
            })
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(e.into()),
    };
    files.sort();
    Ok(files)
}

/// Writes a consistent copy of the database with `VACUUM INTO`, then keeps only the newest
/// `backups_keep` backups.
pub async fn backup(pool: &SqlitePool, dirs: &DataDir, s: &Settings, reason: &str, now: DateTime<Utc>) -> Result<PathBuf> {
    let dir = backup_dir(dirs, s);
    std::fs::create_dir_all(&dir)?;
    let stamp = local_stamp(s, now);
    let mut path = dir.join(format!("learning-{stamp}-{reason}.db"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("learning-{stamp}-{reason}-{n}.db"));
        n += 1;
    }
    sqlx::query("VACUUM INTO ?").bind(path.to_string_lossy().into_owned()).execute(pool).await?;
    let files = backup_files(&dir)?;
    let keep = s.backups_keep.max(1) as usize;
    for old in files.iter().rev().skip(keep) {
        let _ = std::fs::remove_file(old);
    }
    Ok(path)
}

/// Takes a backup unless one already exists for today's local date.
pub async fn daily_if_needed(pool: &SqlitePool, dirs: &DataDir, s: &Settings, now: DateTime<Utc>) -> Result<Option<PathBuf>> {
    let prefix = format!("learning-{}", s.local_date(now).format("%Y%m%d"));
    let exists = backup_files(&backup_dir(dirs, s))?
        .iter()
        .any(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix)));
    if exists {
        return Ok(None);
    }
    backup(pool, dirs, s, "daily", now).await.map(Some)
}

/// Replaces the database with `from`. The current database (and its WAL files) is kept beside it
/// as `learning.db.before-restore-<timestamp>`. No server may be running.
pub async fn restore(dirs: &DataDir, from: &Path) -> Result<()> {
    if !from.is_file() {
        return Err(Error::Invalid(format!("backup not found: {}", from.display())));
    }
    let opts = SqliteConnectOptions::from_str("sqlite:")?.filename(from).read_only(true);
    let valid = async {
        let pool = SqlitePool::connect_with(opts).await?;
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'reviews'")
            .fetch_one(&pool)
            .await?;
        pool.close().await;
        Ok::<_, sqlx::Error>(n == 1)
    }
    .await
    .unwrap_or(false);
    if !valid {
        return Err(Error::Invalid(format!("{} is not a learning-tool database", from.display())));
    }
    let db = dirs.db_path();
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    for suffix in ["", "-wal", "-shm"] {
        let current = PathBuf::from(format!("{}{suffix}", db.display()));
        if current.exists() {
            std::fs::rename(&current, format!("{}.before-restore-{stamp}{suffix}", db.display()))?;
        }
    }
    std::fs::copy(from, &db)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::testutil::utc;

    struct Env {
        _tmp: tempfile::TempDir,
        dirs: DataDir,
    }
    fn env() -> Env {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = DataDir { root: tmp.path().join("data") };
        dirs.ensure().unwrap();
        Env { _tmp: tmp, dirs }
    }
    fn backups(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("learning-")).collect())
            .unwrap_or_default();
        v.sort();
        v
    }
    async fn setup() -> (Env, SqlitePool, Settings) {
        let e = env();
        let pool = db::open(&e.dirs.db_path()).await.unwrap();
        crate::settings::set(&pool, "timezone", "UTC").await.unwrap();
        let s = Settings::load(&pool).await.unwrap();
        (e, pool, s)
    }

    #[tokio::test]
    async fn backup_creates_openable_copy() {
        let (e, pool, s) = setup().await;
        let path = backup(&pool, &e.dirs, &s, "manual", utc("2026-10-23T18:00:00Z")).await.unwrap();
        assert_eq!(path.file_name().unwrap(), "learning-20261023-180000-manual.db");
        let copy = db::open(&path).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM settings").fetch_one(&copy).await.unwrap();
        assert!(n >= 10);
    }

    #[tokio::test]
    async fn prune_keeps_two_newest() {
        let (e, pool, s) = setup().await;
        for t in ["2026-10-20T10:00:00Z", "2026-10-21T10:00:00Z", "2026-10-22T10:00:00Z", "2026-10-22T10:00:00Z"] {
            backup(&pool, &e.dirs, &s, "daily", utc(t)).await.unwrap();
        }
        let left = backups(&e.dirs.backups_dir());
        assert_eq!(left.len(), 2, "{left:?}");
        assert!(left.iter().all(|n| n.starts_with("learning-20261022")), "{left:?}");
    }

    #[tokio::test]
    async fn backup_dir_setting_is_used() {
        let (e, pool, _) = setup().await;
        let custom = e.dirs.root.join("elsewhere");
        crate::settings::set(&pool, "backup_dir", custom.to_str().unwrap()).await.unwrap();
        let s = Settings::load(&pool).await.unwrap();
        backup(&pool, &e.dirs, &s, "manual", utc("2026-10-23T18:00:00Z")).await.unwrap();
        assert_eq!(backups(&custom).len(), 1);
    }

    #[tokio::test]
    async fn daily_if_needed_skips_second_call_same_day() {
        let (e, pool, s) = setup().await;
        assert!(daily_if_needed(&pool, &e.dirs, &s, utc("2026-10-23T08:00:00Z")).await.unwrap().is_some());
        assert!(daily_if_needed(&pool, &e.dirs, &s, utc("2026-10-23T20:00:00Z")).await.unwrap().is_none());
        assert!(daily_if_needed(&pool, &e.dirs, &s, utc("2026-10-24T08:00:00Z")).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn restore_rejects_non_sqlite() {
        let e = env();
        let junk = e.dirs.root.join("junk.db");
        std::fs::write(&junk, b"not a database").unwrap();
        assert!(matches!(restore(&e.dirs, &junk).await, Err(crate::Error::Invalid(_))));
        assert!(matches!(restore(&e.dirs, &e.dirs.root.join("missing.db")).await, Err(crate::Error::Invalid(_))));
    }

    #[tokio::test]
    async fn restore_round_trip_preserves_reviews() {
        let (e, pool, s) = setup().await;
        let t = crate::testutil::topic(&pool, None, "T").await;
        let q = crate::testutil::question(&pool, t).await;
        let sid = crate::testutil::session(&pool, "2026-10-23").await;
        crate::testutil::review_row(&pool, q, sid, "2026-10-23T18:00:00Z", 3).await;
        let snap = backup(&pool, &e.dirs, &s, "manual", utc("2026-10-23T18:00:00Z")).await.unwrap();
        crate::testutil::review_row(&pool, q, sid, "2026-10-23T19:00:00Z", 3).await;
        pool.close().await;
        restore(&e.dirs, &snap).await.unwrap();
        let pool = db::open(&e.dirs.db_path()).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM reviews").fetch_one(&pool).await.unwrap();
        assert_eq!(n, 1);
        let moved = std::fs::read_dir(&e.dirs.root).unwrap().flatten()
            .filter(|f| f.file_name().to_string_lossy().starts_with("learning.db.before-restore-")).count();
        assert!(moved >= 1);
    }

    #[tokio::test]
    async fn pre_migrate_backup_only_when_pending() {
        let e = env();
        // A database migrated with the first migration only.
        let mig_dir = e.dirs.root.join("mig");
        std::fs::create_dir_all(&mig_dir).unwrap();
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0001_schema.sql"), mig_dir.join("0001_schema.sql")).unwrap();
        let old = sqlx::migrate::Migrator::new(mig_dir.as_path()).await.unwrap();
        let opts = sqlx::sqlite::SqliteConnectOptions::new().filename(e.dirs.db_path()).create_if_missing(true);
        let p = SqlitePool::connect_with(opts).await.unwrap();
        old.run(&p).await.unwrap();
        p.close().await;

        db::open(&e.dirs.db_path()).await.unwrap().close().await;
        let after = backups(&e.dirs.backups_dir());
        assert_eq!(after.len(), 1, "{after:?}");
        assert!(after[0].ends_with("-pre-migrate.db"));
        db::open(&e.dirs.db_path()).await.unwrap().close().await;
        assert_eq!(backups(&e.dirs.backups_dir()).len(), 1);

        let fresh = env();
        db::open(&fresh.dirs.db_path()).await.unwrap().close().await;
        assert!(backups(&fresh.dirs.backups_dir()).is_empty());
    }
}
