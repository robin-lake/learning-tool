use std::path::PathBuf;

use chrono_tz::Tz;
use sqlx::SqlitePool;

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TzSetting {
    System,
    Named(Tz),
}

/// User-tunable settings, stored as key/value rows (defaults inserted by migration).
#[derive(Debug, Clone)]
pub struct Settings {
    pub new_per_day: u32,
    pub new_pause_review_threshold: u32,
    pub backlog_daily_limit: u32,
    pub session_target: u32,
    pub desired_retention_main: f32,
    pub leech_lapses: u32,
    pub day_rollover_hour: u32,
    pub timezone: TzSetting,
    pub backup_dir: Option<PathBuf>,
    pub backups_keep: u32,
    pub web_port: u16,
    pub stale_after_days: u32,
    pub stale_full_days: u32,
}

const KEYS: &[&str] = &[
    "new_per_day",
    "new_pause_review_threshold",
    "backlog_daily_limit",
    "session_target",
    "desired_retention_main",
    "leech_lapses",
    "day_rollover_hour",
    "timezone",
    "backup_dir",
    "backups_keep",
    "web_port",
    "stale_after_days",
    "stale_full_days",
];

fn parse_u32(key: &str, value: &str) -> Result<u32> {
    value
        .trim()
        .parse()
        .map_err(|_| Error::Invalid(format!("{key} must be a non-negative integer, got {value:?}")))
}

fn parse_tz(value: &str) -> Result<TzSetting> {
    match value.trim() {
        "" | "system" => Ok(TzSetting::System),
        name => name
            .parse::<Tz>()
            .map(TzSetting::Named)
            .map_err(|_| Error::Invalid(format!("unknown timezone {name:?}"))),
    }
}

/// Checks that `value` is acceptable for `key`.
fn validate(key: &str, value: &str) -> Result<()> {
    match key {
        "desired_retention_main" => {
            let r: f32 = value
                .trim()
                .parse()
                .map_err(|_| Error::Invalid(format!("{key} must be a number, got {value:?}")))?;
            if !(0.5..=0.99).contains(&r) {
                return Err(Error::Invalid(format!("{key} must be between 0.5 and 0.99")));
            }
        }
        "day_rollover_hour" => {
            if parse_u32(key, value)? > 23 {
                return Err(Error::Invalid(format!("{key} must be 0-23")));
            }
        }
        "new_per_day" | "new_pause_review_threshold" | "session_target" | "leech_lapses" => {
            parse_u32(key, value)?;
        }
        "backlog_daily_limit" | "backups_keep" => {
            if parse_u32(key, value)? == 0 {
                return Err(Error::Invalid(format!("{key} must be at least 1")));
            }
        }
        "timezone" => {
            parse_tz(value)?;
        }
        "backup_dir" => {}
        "stale_after_days" | "stale_full_days" => {
            let days = parse_u32(key, value)?;
            if !(1..=365).contains(&days) {
                return Err(Error::Invalid(format!("{key} must be 1-365 days, got {days}")));
            }
        }
        "web_port" => {
            let port = parse_u32(key, value)?;
            if !(1..=65535).contains(&port) {
                return Err(Error::Invalid(format!("{key} must be 1-65535, got {port}")));
            }
        }
        _ => {
            return Err(Error::Invalid(format!(
                "unknown setting {key:?}; known settings: {}",
                KEYS.join(", ")
            )));
        }
    }
    Ok(())
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            new_per_day: 3,
            new_pause_review_threshold: 20,
            backlog_daily_limit: 30,
            session_target: 10,
            desired_retention_main: 0.9,
            leech_lapses: 6,
            day_rollover_hour: 4,
            timezone: TzSetting::System,
            backup_dir: None,
            backups_keep: 2,
            web_port: 4747,
            stale_after_days: 14,
            stale_full_days: 60,
        }
    }
}

impl Settings {
    pub async fn load(pool: &SqlitePool) -> Result<Settings> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
            .fetch_all(pool)
            .await?;
        let get = |key: &str| -> Result<String> {
            rows.iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .ok_or_else(|| Error::Invalid(format!("setting {key} is missing")))
        };
        let backup_dir = get("backup_dir")?;
        Ok(Settings {
            new_per_day: parse_u32("new_per_day", &get("new_per_day")?)?,
            new_pause_review_threshold: parse_u32(
                "new_pause_review_threshold",
                &get("new_pause_review_threshold")?,
            )?,
            backlog_daily_limit: parse_u32("backlog_daily_limit", &get("backlog_daily_limit")?)?,
            session_target: parse_u32("session_target", &get("session_target")?)?,
            desired_retention_main: get("desired_retention_main")?
                .trim()
                .parse()
                .map_err(|_| Error::Invalid("desired_retention_main is not a number".into()))?,
            leech_lapses: parse_u32("leech_lapses", &get("leech_lapses")?)?,
            day_rollover_hour: parse_u32("day_rollover_hour", &get("day_rollover_hour")?)?,
            timezone: parse_tz(&get("timezone")?)?,
            backup_dir: (!backup_dir.trim().is_empty()).then(|| PathBuf::from(backup_dir.trim())),
            backups_keep: parse_u32("backups_keep", &get("backups_keep")?)?,
            web_port: parse_u32("web_port", &get("web_port")?)? as u16,
            stale_after_days: parse_u32("stale_after_days", &get("stale_after_days")?)?,
            stale_full_days: parse_u32("stale_full_days", &get("stale_full_days")?)?,
        })
    }
}

/// All settings as stored, in a stable order (for `config get`).
pub async fn all(pool: &SqlitePool) -> Result<Vec<(String, String)>> {
    let mut rows = Vec::new();
    for key in KEYS {
        let value: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(pool)
            .await?;
        rows.push((key.to_string(), value.unwrap_or_default()));
    }
    Ok(rows)
}

pub async fn set(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    validate(key, value)?;
    // Staleness starts fading after `stale_after_days` and bottoms out at `stale_full_days`.
    if key == "stale_after_days" || key == "stale_full_days" {
        let current = Settings::load(pool).await?;
        let n = parse_u32(key, value)?;
        let (after, full) =
            if key == "stale_after_days" { (n, current.stale_full_days) } else { (current.stale_after_days, n) };
        if full <= after {
            return Err(Error::Invalid(format!(
                "stale_full_days ({full}) must be greater than stale_after_days ({after})"
            )));
        }
    }
    sqlx::query("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = ?2")
        .bind(key)
        .bind(value.trim())
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::Error;

    #[tokio::test]
    async fn settings_defaults_load() {
        let s = Settings::load(&test_pool().await).await.unwrap();
        assert_eq!(
            (s.new_per_day, s.new_pause_review_threshold, s.backlog_daily_limit, s.session_target),
            (3, 20, 30, 10)
        );
        assert_eq!(s.desired_retention_main, 0.9);
        assert_eq!(s.leech_lapses, 6);
        assert_eq!(s.day_rollover_hour, 4);
        assert_eq!(s.backups_keep, 2);
        assert!(s.backup_dir.is_none());
        assert!(matches!(s.timezone, TzSetting::System));
    }

    #[tokio::test]
    async fn stale_settings_default_and_validate() {
        let p = test_pool().await;
        let s = Settings::load(&p).await.unwrap();
        assert_eq!((s.stale_after_days, s.stale_full_days), (14, 60));
        assert!(matches!(set(&p, "stale_after_days", "0").await, Err(Error::Invalid(_))));
        assert!(matches!(set(&p, "stale_full_days", "400").await, Err(Error::Invalid(_))));
        assert!(matches!(set(&p, "stale_full_days", "10").await, Err(Error::Invalid(_))));
        assert!(matches!(set(&p, "stale_after_days", "60").await, Err(Error::Invalid(_))));
        set(&p, "stale_after_days", "7").await.unwrap();
        set(&p, "stale_full_days", "30").await.unwrap();
        let s = Settings::load(&p).await.unwrap();
        assert_eq!((s.stale_after_days, s.stale_full_days), (7, 30));
    }

    #[tokio::test]
    async fn web_port_default_and_validation() {
        let p = test_pool().await;
        assert_eq!(Settings::load(&p).await.unwrap().web_port, 4747);
        assert!(matches!(set(&p, "web_port", "0").await, Err(Error::Invalid(_))));
        assert!(matches!(set(&p, "web_port", "70000").await, Err(Error::Invalid(_))));
        set(&p, "web_port", "5000").await.unwrap();
        assert_eq!(Settings::load(&p).await.unwrap().web_port, 5000);
    }

    #[tokio::test]
    async fn settings_set_rejects_unknown_key_and_bad_value() {
        let p = test_pool().await;
        assert!(matches!(set(&p, "nope", "1").await, Err(Error::Invalid(_))));
        assert!(matches!(set(&p, "new_per_day", "abc").await, Err(Error::Invalid(_))));
        assert!(matches!(set(&p, "timezone", "Mars/Olympus").await, Err(Error::Invalid(_))));
        set(&p, "timezone", "America/Los_Angeles").await.unwrap();
        let s = Settings::load(&p).await.unwrap();
        assert!(matches!(s.timezone, TzSetting::Named(chrono_tz::America::Los_Angeles)));
    }
}
