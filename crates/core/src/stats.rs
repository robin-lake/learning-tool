use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::Result;
use crate::settings::Settings;

/// Consecutive qualifying study days. Implemented in the stats task.
pub async fn streak(_pool: &SqlitePool, _s: &Settings, _now: DateTime<Utc>) -> Result<u32> {
    Ok(0)
}
