//! Helpers for unit tests: insert minimal rows directly.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::SqlitePool;

pub fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

pub fn day(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

pub async fn topic(pool: &SqlitePool, parent: Option<i64>, name: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO topics (parent_id, name, slug, created_at, updated_at) VALUES (?, ?, ?, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00') RETURNING id",
    )
    .bind(parent)
    .bind(name)
    .bind(name.to_lowercase())
    .fetch_one(pool)
    .await
    .unwrap()
}

/// An active level-0 question with a one-point rubric `r1`.
pub async fn question(pool: &SqlitePool, topic_id: i64) -> i64 {
    sqlx::query_scalar(
        r#"INSERT INTO questions (topic_id, level, prompt, answer, rubric, created_at, updated_at)
           VALUES (?, 0, 'q', 'a', '[{"id":"r1","point":"p","weight":1,"essential":false}]', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00') RETURNING id"#,
    )
    .bind(topic_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

pub async fn session(pool: &SqlitePool, study_day: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO sessions (kind, study_day, started_at, created_at, updated_at) VALUES ('main', ?, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00') RETURNING id",
    )
    .bind(study_day)
    .fetch_one(pool)
    .await
    .unwrap()
}

pub async fn review_row(pool: &SqlitePool, question_id: i64, session_id: i64, at: &str, grade: i64) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO reviews (question_id, session_id, reviewed_at, grade, score, rubric_result, user_answer, asked_prompt, created_at, updated_at)
         VALUES (?, ?, ?, ?, 1.0, '[]', 'ans', 'q', ?, ?) RETURNING id",
    )
    .bind(question_id)
    .bind(session_id)
    .bind(at)
    .bind(grade)
    .bind(at)
    .bind(at)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Marks `question_id` as a reviewed card due on `due_on`, last reviewed the day before.
pub async fn due_state(pool: &SqlitePool, question_id: i64, due_on: &str, stability: f64) {
    let due: NaiveDate = due_on.parse().unwrap();
    let last = due - chrono::Duration::days(1);
    sqlx::query(
        "INSERT INTO question_state (question_id, stability, difficulty, due_on, last_reviewed_at, last_study_day, reps, state)
         VALUES (?, ?, 5, ?, ?, ?, 1, 'review')",
    )
    .bind(question_id)
    .bind(stability)
    .bind(due_on)
    .bind(format!("{last}T18:00:00+00:00"))
    .bind(last.to_string())
    .execute(pool)
    .await
    .unwrap();
}

pub fn all_hit() -> Vec<crate::grading::PointResult> {
    vec![crate::grading::PointResult { id: "r1".into(), hit: true, note: String::new() }]
}

pub fn none_hit() -> Vec<crate::grading::PointResult> {
    vec![crate::grading::PointResult { id: "r1".into(), hit: false, note: String::new() }]
}
