use std::collections::HashSet;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::Result;
use crate::queue::{load_candidates, plan};
use crate::session::due_count;
use crate::settings::Settings;
use crate::types::parse_day;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TodayStats {
    pub study_day: NaiveDate,
    pub answered_today: u32,
    pub remaining_today: u32,
    pub new_introduced_today: u32,
    pub due_tomorrow: u32,
    pub streak: u32,
}

/// Study days that count toward the streak: a main session that day finished all its items,
/// or at least `session_target` main reviews were recorded.
async fn qualifying_days(pool: &SqlitePool, s: &Settings) -> Result<HashSet<NaiveDate>> {
    let days: Vec<String> = sqlx::query_scalar(
        "SELECT s.study_day FROM sessions s WHERE s.kind = 'main' GROUP BY s.study_day
         HAVING (SELECT count(*) FROM reviews r JOIN sessions s2 ON s2.id = r.session_id
                 WHERE s2.kind = 'main' AND s2.study_day = s.study_day) >= ?1
             OR EXISTS (SELECT 1 FROM sessions s3
                        WHERE s3.kind = 'main' AND s3.study_day = s.study_day AND s3.ended_at IS NOT NULL
                          AND NOT EXISTS (SELECT 1 FROM session_items i WHERE i.session_id = s3.id AND i.review_id IS NULL))",
    )
    .bind(s.session_target as i64)
    .fetch_all(pool)
    .await?;
    days.iter().map(|d| parse_day(d)).collect()
}

/// Consecutive qualifying study days ending today, or ending yesterday if today isn't done yet.
pub async fn streak(pool: &SqlitePool, s: &Settings, now: DateTime<Utc>) -> Result<u32> {
    let days = qualifying_days(pool, s).await?;
    let today = s.study_day(now);
    let mut day = if days.contains(&today) { today } else { today - Duration::days(1) };
    let mut n = 0;
    while days.contains(&day) {
        n += 1;
        day -= Duration::days(1);
    }
    Ok(n)
}

pub async fn today(pool: &SqlitePool, s: &Settings, now: DateTime<Utc>) -> Result<TodayStats> {
    let day = s.study_day(now);
    let day_s = day.to_string();
    let answered: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM reviews r JOIN sessions s ON s.id = r.session_id WHERE s.kind = 'main' AND s.study_day = ?",
    )
    .bind(&day_s)
    .fetch_one(pool)
    .await?;
    let open: Option<i64> = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM session_items i WHERE i.session_id = s.id AND i.review_id IS NULL)
         FROM sessions s WHERE s.kind = 'main' AND s.ended_at IS NULL AND s.study_day = ? ORDER BY s.id DESC LIMIT 1",
    )
    .bind(&day_s)
    .fetch_optional(pool)
    .await?;
    let (due, new, pinned, introduced) = load_candidates(pool, s, day, now).await?;
    let remaining = match open {
        Some(n) if n > 0 => n as u32,
        _ => plan(due, new, pinned, introduced, s).items.len() as u32,
    };
    Ok(TodayStats {
        study_day: day,
        answered_today: answered as u32,
        remaining_today: remaining,
        new_introduced_today: introduced,
        due_tomorrow: due_count(pool, s, day + Duration::days(1), now).await?,
        streak: streak(pool, s, now).await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::session::{self, ReviewInput};
    use crate::testutil::{self, all_hit, day, utc};

    async fn setup() -> (SqlitePool, Settings) {
        let p = test_pool().await;
        crate::settings::set(&p, "timezone", "UTC").await.unwrap();
        crate::settings::set(&p, "day_rollover_hour", "0").await.unwrap();
        crate::settings::set(&p, "session_target", "2").await.unwrap();
        let s = Settings::load(&p).await.unwrap();
        (p, s)
    }

    /// A finished main session on `d` with `n` reviews (all items reviewed).
    async fn finished_day(p: &SqlitePool, d: &str, n: usize) {
        let t = testutil::topic(p, None, &format!("T{d}")).await;
        let sid = testutil::session(p, d).await;
        for i in 0..n {
            let q = testutil::question(p, t).await;
            let r = testutil::review_row(p, q, sid, &format!("{d}T12:00:00Z"), 3).await;
            sqlx::query("INSERT INTO session_items (session_id, ordinal, question_id, kind, review_id) VALUES (?, ?, ?, 'review', ?)")
                .bind(sid).bind(i as i64).bind(q).bind(r).execute(p).await.unwrap();
        }
        sqlx::query("UPDATE sessions SET ended_at = ? WHERE id = ?").bind(format!("{d}T13:00:00+00:00")).bind(sid).execute(p).await.unwrap();
    }

    #[tokio::test]
    async fn streak_counts_consecutive_days() {
        let (p, s) = setup().await;
        for d in ["2026-10-21", "2026-10-22", "2026-10-23"] {
            finished_day(&p, d, 2).await;
        }
        assert_eq!(streak(&p, &s, utc("2026-10-23T20:00:00Z")).await.unwrap(), 3);
    }

    #[tokio::test]
    async fn streak_not_broken_before_today_done() {
        let (p, s) = setup().await;
        finished_day(&p, "2026-10-21", 2).await;
        finished_day(&p, "2026-10-22", 2).await;
        assert_eq!(streak(&p, &s, utc("2026-10-23T08:00:00Z")).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn short_day_with_all_due_done_qualifies() {
        let (p, _) = setup().await;
        crate::settings::set(&p, "session_target", "10").await.unwrap();
        let s10 = Settings::load(&p).await.unwrap();
        finished_day(&p, "2026-10-23", 3).await;
        assert_eq!(streak(&p, &s10, utc("2026-10-23T20:00:00Z")).await.unwrap(), 1);
        // An open session with unreviewed items and fewer than target reviews does not qualify.
        let t = testutil::topic(&p, None, "Open").await;
        let q = testutil::question(&p, t).await;
        let sid = testutil::session(&p, "2026-10-24").await;
        sqlx::query("INSERT INTO session_items (session_id, ordinal, question_id, kind) VALUES (?, 0, ?, 'new')")
            .bind(sid).bind(q).execute(&p).await.unwrap();
        assert_eq!(streak(&p, &s10, utc("2026-10-24T20:00:00Z")).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn gap_resets_streak() {
        let (p, s) = setup().await;
        finished_day(&p, "2026-10-19", 2).await;
        finished_day(&p, "2026-10-21", 2).await;
        finished_day(&p, "2026-10-22", 2).await;
        assert_eq!(streak(&p, &s, utc("2026-10-22T20:00:00Z")).await.unwrap(), 2);
        assert_eq!(streak(&p, &s, utc("2026-10-25T20:00:00Z")).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn today_stats_remaining_matches_plan() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let due = testutil::question(&p, t).await;
        testutil::due_state(&p, due, "2026-10-23", 3.0).await;
        testutil::question(&p, t).await;
        testutil::question(&p, t).await;
        let now = utc("2026-10-23T18:00:00Z");
        let st = today(&p, &s, now).await.unwrap();
        assert_eq!((st.study_day, st.answered_today, st.remaining_today), (day("2026-10-23"), 0, 3));
        let sess = session::start(&p, &s, now).await.unwrap();
        let nq = session::next_question(&p, sess.session_id, now).await.unwrap().unwrap();
        let input = ReviewInput {
            item_id: nq.item_id, question_id: nq.question.id, rubric_result: all_hit(), hints_used: 0, fluent: false,
            user_answer: "a".into(), asked_prompt: "q".into(), feedback: String::new(), duration_ms: None,
        };
        session::record_review(&p, &s, sess.session_id, input, now).await.unwrap();
        let st = today(&p, &s, now).await.unwrap();
        assert_eq!((st.answered_today, st.remaining_today), (1, 2));
        assert_eq!(st.new_introduced_today, if nq.kind == crate::queue::ItemKind::New { 1 } else { 0 });
    }
}
