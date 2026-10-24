use std::collections::HashSet;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::Result;
use crate::pins::PinnedItem;
use crate::queue::{load_candidates, plan};
use crate::session::due_count;
use crate::settings::Settings;
use crate::types::parse_day;
use crate::views::{QuestionRef, TopicRef};

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

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct DayActivity {
    pub study_day: NaiveDate,
    pub count: u32,
    /// Counts of again, hard, good, easy.
    pub by_grade: [u32; 4],
    pub topics: Vec<TopicRef>,
}

/// Main-program review activity per study day in `from..=to`; days without reviews are omitted.
pub async fn activity(pool: &SqlitePool, from: NaiveDate, to: NaiveDate) -> Result<Vec<DayActivity>> {
    let rows: Vec<(String, i64, i64, String)> = sqlx::query_as(
        "SELECT s.study_day, r.grade, t.id, t.name
         FROM reviews r
         JOIN sessions s ON s.id = r.session_id
         JOIN questions q ON q.id = r.question_id
         JOIN topics t ON t.id = q.topic_id
         WHERE s.kind = 'main' AND s.study_day BETWEEN ? AND ?
         ORDER BY s.study_day",
    )
    .bind(from.to_string())
    .bind(to.to_string())
    .fetch_all(pool)
    .await?;
    let mut out: Vec<DayActivity> = vec![];
    for (day, grade, topic_id, topic_name) in rows {
        let day = parse_day(&day)?;
        if out.last().is_none_or(|d| d.study_day != day) {
            out.push(DayActivity { study_day: day, count: 0, by_grade: [0; 4], topics: vec![] });
        }
        let d = out.last_mut().expect("just pushed");
        d.count += 1;
        d.by_grade[(grade - 1) as usize] += 1;
        if !d.topics.iter().any(|t| t.id == topic_id) {
            d.topics.push(TopicRef { id: topic_id, name: topic_name });
        }
    }
    for d in &mut out {
        d.topics.sort_by(|a, b| a.name.cmp(&b.name));
    }
    Ok(out)
}

/// The longest run of consecutive qualifying study days.
pub async fn best_streak(pool: &SqlitePool, s: &Settings) -> Result<u32> {
    let mut days: Vec<NaiveDate> = qualifying_days(pool, s).await?.into_iter().collect();
    days.sort();
    let (mut best, mut run) = (0, 0);
    for (i, d) in days.iter().enumerate() {
        run = if i > 0 && *d - days[i - 1] == Duration::days(1) { run + 1 } else { 1 };
        best = best.max(run);
    }
    Ok(best)
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct HomeSummary {
    pub today: TodayStats,
    pub by_grade_today: [u32; 4],
    pub best_streak: u32,
    pub new_waiting: u32,
    pub new_waiting_days: u32,
    pub pinned: Vec<PinnedItem>,
    pub leeches: Vec<QuestionRef>,
    pub ratings_unprocessed: u32,
}

/// Everything the home page shows, in one call.
pub async fn overview(pool: &SqlitePool, s: &Settings, now: DateTime<Utc>) -> Result<HomeSummary> {
    let today = self::today(pool, s, now).await?;
    let grades: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT r.grade, count(*) FROM reviews r JOIN sessions s ON s.id = r.session_id
         WHERE s.kind = 'main' AND s.study_day = ? GROUP BY r.grade",
    )
    .bind(today.study_day.to_string())
    .fetch_all(pool)
    .await?;
    let mut by_grade_today = [0u32; 4];
    for (g, n) in grades {
        by_grade_today[(g - 1) as usize] = n as u32;
    }
    let new_waiting = load_candidates(pool, s, today.study_day, now).await?.1.len() as u32;
    let leech_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT q.id FROM questions q JOIN question_state st ON st.question_id = q.id
         WHERE q.status = 'active' AND q.scope = 'main' AND st.lapses >= ? ORDER BY q.id",
    )
    .bind(s.leech_lapses as i64)
    .fetch_all(pool)
    .await?;
    let mut leeches = vec![];
    for id in leech_ids {
        let d = crate::questions::get(pool, id).await?;
        leeches.push(QuestionRef { id, prompt: d.question.prompt, topic_path: d.topic_path });
    }
    let ratings_unprocessed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM question_ratings WHERE processed_in_guide_version_id IS NULL")
            .fetch_one(pool)
            .await?;
    Ok(HomeSummary {
        best_streak: best_streak(pool, s).await?,
        new_waiting,
        new_waiting_days: new_waiting.div_ceil(s.new_per_day.max(1)),
        pinned: crate::pins::active(pool).await?,
        leeches,
        ratings_unprocessed: ratings_unprocessed as u32,
        by_grade_today,
        today,
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

    async fn graded_review(p: &SqlitePool, topic: i64, sid: i64, at: &str, grade: i64) -> i64 {
        let q = testutil::question(p, topic).await;
        testutil::review_row(p, q, sid, at, grade).await;
        q
    }

    #[tokio::test]
    async fn activity_groups_by_day_with_grades_and_topics() {
        let (p, _) = setup().await;
        let horns = testutil::topic(&p, None, "Horns").await;
        let ac = testutil::topic(&p, None, "AC").await;
        let d1 = testutil::session(&p, "2026-10-22").await;
        graded_review(&p, horns, d1, "2026-10-22T12:00:00Z", 1).await;
        graded_review(&p, ac, d1, "2026-10-22T12:05:00Z", 3).await;
        let d2 = testutil::session(&p, "2026-10-23").await;
        graded_review(&p, horns, d2, "2026-10-23T12:00:00Z", 4).await;
        let a = activity(&p, day("2026-10-01"), day("2026-10-31")).await.unwrap();
        assert_eq!(a.len(), 2);
        assert_eq!((a[0].study_day, a[0].count, a[0].by_grade), (day("2026-10-22"), 2, [1, 0, 1, 0]));
        let names: Vec<&str> = a[0].topics.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["AC", "Horns"]);
        assert_eq!((a[1].count, a[1].by_grade), (1, [0, 0, 0, 1]));
    }

    #[tokio::test]
    async fn activity_ignores_project_sessions_and_range() {
        let (p, _) = setup().await;
        let t = testutil::topic(&p, None, "T").await;
        let main = testutil::session(&p, "2026-10-22").await;
        graded_review(&p, t, main, "2026-10-22T12:00:00Z", 3).await;
        let proj: i64 = sqlx::query_scalar(
            "INSERT INTO sessions (kind, study_day, started_at, created_at, updated_at) VALUES ('project', '2026-10-22', 'x', 'x', 'x') RETURNING id",
        )
        .fetch_one(&p).await.unwrap();
        graded_review(&p, t, proj, "2026-10-22T13:00:00Z", 3).await;
        let old = testutil::session(&p, "2025-01-01").await;
        graded_review(&p, t, old, "2025-01-01T12:00:00Z", 3).await;
        let a = activity(&p, day("2026-10-01"), day("2026-10-31")).await.unwrap();
        assert_eq!(a.iter().map(|d| d.count).collect::<Vec<_>>(), vec![1]);
    }

    #[tokio::test]
    async fn best_streak_finds_longest_run() {
        let (p, s) = setup().await;
        for d in ["2026-10-01", "2026-10-02", "2026-10-03", "2026-10-05", "2026-10-06"] {
            finished_day(&p, d, 2).await;
        }
        assert_eq!(best_streak(&p, &s).await.unwrap(), 3);
    }

    #[tokio::test]
    async fn overview_counts() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "T").await;
        let today = testutil::session(&p, "2026-10-24").await;
        let reviewed = graded_review(&p, t, today, "2026-10-24T12:00:00Z", 2).await;
        sqlx::query("INSERT INTO question_state (question_id, stability, difficulty, due_on, last_reviewed_at, last_study_day, reps, lapses, state) VALUES (?, 1, 5, '2026-10-25', '2026-10-24T12:00:00+00:00', '2026-10-24', 7, 6, 'relearning')")
            .bind(reviewed).execute(&p).await.unwrap();
        for _ in 0..5 {
            testutil::question(&p, t).await;
        }
        let pinned = testutil::question(&p, t).await;
        crate::pins::pin(&p, pinned, utc("2026-10-24T13:00:00Z")).await.unwrap();
        crate::guide::rate(&p, reviewed, 2, None, None, None, utc("2026-10-24T13:00:00Z")).await.unwrap();
        let o = overview(&p, &s, utc("2026-10-24T20:00:00Z")).await.unwrap();
        assert_eq!(o.by_grade_today, [0, 1, 0, 0]);
        assert_eq!((o.new_waiting, o.new_waiting_days), (6, 2));
        assert_eq!(o.pinned.iter().map(|i| i.question_id).collect::<Vec<_>>(), vec![pinned]);
        assert_eq!(o.leeches.iter().map(|q| q.id).collect::<Vec<_>>(), vec![reviewed]);
        assert_eq!(o.ratings_unprocessed, 1);
        let json = serde_json::to_string(&o).unwrap();
        assert!(!json.contains("\"answer\"") && !json.contains("\"rubric\""), "{json}");
    }
}
