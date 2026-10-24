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
        "INSERT INTO topics (parent_id, name, slug, created_at, updated_at) VALUES (?, ?, ?, 'x', 'x') RETURNING id",
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
           VALUES (?, 0, 'q', 'a', '[{"id":"r1","point":"p","weight":1,"essential":false}]', 'x', 'x') RETURNING id"#,
    )
    .bind(topic_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

pub async fn session(pool: &SqlitePool, study_day: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO sessions (kind, study_day, started_at, created_at, updated_at) VALUES ('main', ?, 'x', 'x', 'x') RETURNING id",
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
