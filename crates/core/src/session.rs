use chrono::{DateTime, Duration, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::grading::{self, Grade, PointResult};
use crate::questions::{self, Question, QuestionStatus, ReviewRow};
pub use crate::queue::ItemKind;
use crate::queue::{load_candidates, plan};
use crate::scheduler::{self, SchedParams};
use crate::settings::Settings;
use crate::types::{parse_day, ts};
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SessionStart {
    pub session_id: i64,
    pub study_day: NaiveDate,
    /// Questions left to answer in this session.
    pub planned: u32,
    pub due_total: u32,
    pub new_allowed: u32,
    /// Overdue reviews held back for later days (backlog recovery).
    pub backlog_carried: u32,
    pub streak: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct NextQuestion {
    pub item_id: i64,
    pub kind: ItemKind,
    /// 1-based position in the session.
    pub position: u32,
    /// Questions left including this one.
    pub remaining: u32,
    pub question: Question,
    pub topic_path: Vec<String>,
    pub recent_reviews: Vec<ReviewRow>,
    pub lapses: u32,
    pub is_leech: bool,
    pub source_citation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReviewInput {
    pub question_id: i64,
    /// One entry per rubric point.
    pub rubric_result: Vec<PointResult>,
    #[serde(default)]
    pub hints_used: u32,
    /// True if recall was immediate and complete.
    #[serde(default)]
    pub fluent: bool,
    /// The learner's answer, verbatim.
    pub user_answer: String,
    /// The wording actually used when asking.
    pub asked_prompt: String,
    /// What Claude told the learner.
    #[serde(default)]
    pub feedback: String,
    #[serde(default)]
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviewOutcome {
    pub review_id: i64,
    pub grade: Grade,
    pub score: f32,
    pub due_on: NaiveDate,
    pub lapses: u32,
    pub is_leech: bool,
    /// True if the question will be asked again later in this session.
    pub requeued: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SessionSummary {
    pub answered: u32,
    /// Counts of again, hard, good, easy.
    pub by_grade: [u32; 4],
    pub streak: u32,
    pub due_tomorrow: u32,
    pub leeches: Vec<i64>,
}

struct SessionRow {
    study_day: NaiveDate,
    ended: bool,
}

async fn session_row(pool: &SqlitePool, id: i64) -> Result<SessionRow> {
    let row = sqlx::query("SELECT study_day, ended_at FROM sessions WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(Error::NotFound { entity: "session", id })?;
    Ok(SessionRow {
        study_day: parse_day(row.try_get("study_day")?)?,
        ended: row.try_get::<Option<String>, _>("ended_at")?.is_some(),
    })
}

async fn unreviewed_counts(pool: &SqlitePool, session_id: i64) -> Result<(u32, u32, u32)> {
    let (total, reviews, new): (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*), COALESCE(SUM(kind <> 'new'), 0), COALESCE(SUM(kind = 'new'), 0)
         FROM session_items WHERE session_id = ? AND review_id IS NULL",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await?;
    Ok((total as u32, reviews as u32, new as u32))
}

/// Starts today's main session, or resumes an open one from the same study day.
pub async fn start(pool: &SqlitePool, s: &Settings, now: DateTime<Utc>) -> Result<SessionStart> {
    let today = s.study_day(now);
    let open: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM sessions WHERE kind = 'main' AND ended_at IS NULL AND study_day = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(today.to_string())
    .fetch_optional(pool)
    .await?;
    let (due, new, introduced) = load_candidates(pool, s, today, now).await?;
    let queue = plan(due, new, introduced, s);
    let stamp = ts(now);

    let (session_id, resumed) = match open {
        Some(id) if unreviewed_counts(pool, id).await?.0 > 0 => (id, true),
        _ => {
            let mut tx = pool.begin().await?;
            let id = match open {
                Some(id) => id,
                None => sqlx::query_scalar(
                    "INSERT INTO sessions (kind, study_day, planned_count, started_at, created_at, updated_at)
                     VALUES ('main', ?1, 0, ?2, ?2, ?2) RETURNING id",
                )
                .bind(today.to_string())
                .bind(&stamp)
                .fetch_one(&mut *tx)
                .await?,
            };
            let base: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(ordinal) + 1, 0) FROM session_items WHERE session_id = ?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
            for (i, (qid, kind)) in queue.items.iter().enumerate() {
                sqlx::query("INSERT INTO session_items (session_id, ordinal, question_id, kind) VALUES (?, ?, ?, ?)")
                    .bind(id)
                    .bind(base + i as i64)
                    .bind(qid)
                    .bind(kind.as_str())
                    .execute(&mut *tx)
                    .await?;
            }
            sqlx::query("UPDATE sessions SET planned_count = planned_count + ?, updated_at = ? WHERE id = ?")
                .bind(queue.items.len() as i64)
                .bind(&stamp)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            (id, false)
        }
    };
    let (planned, due_left, new_left) = unreviewed_counts(pool, session_id).await?;
    Ok(SessionStart {
        session_id,
        study_day: today,
        planned,
        due_total: if resumed { due_left } else { queue.due_total },
        new_allowed: if resumed { new_left } else { queue.new_allowed },
        backlog_carried: queue.backlog_carried,
        streak: crate::stats::streak(pool, s, now).await?,
    })
}

async fn citation(pool: &SqlitePool, q: &Question) -> Result<Option<String>> {
    let Some(sid) = q.source_id else { return Ok(None) };
    let title: Option<String> = sqlx::query_scalar("SELECT title FROM sources WHERE id = ?")
        .bind(sid)
        .fetch_optional(pool)
        .await?;
    Ok(title.map(|t| match &q.source_locator {
        Some(loc) => format!("{t}, {loc}"),
        None => t,
    }))
}

/// The next unanswered question in the session, or `None` when done. Serving again without
/// recording a review returns the same item.
pub async fn next_question(pool: &SqlitePool, session_id: i64, now: DateTime<Utc>) -> Result<Option<NextQuestion>> {
    session_row(pool, session_id).await?;
    let settings = Settings::load(pool).await?;
    loop {
        let item: Option<(i64, i64, String, i64)> = sqlx::query_as(
            "SELECT id, question_id, kind, ordinal FROM session_items
             WHERE session_id = ? AND review_id IS NULL ORDER BY ordinal LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(pool)
        .await?;
        let Some((item_id, qid, kind, ordinal)) = item else { return Ok(None) };
        let question = questions::find(pool, qid).await?;
        if question.status != QuestionStatus::Active {
            // Retired, suspended or reworded since planning: drop it from the queue.
            sqlx::query("DELETE FROM session_items WHERE id = ?").bind(item_id).execute(pool).await?;
            continue;
        }
        sqlx::query("UPDATE session_items SET served_at = COALESCE(served_at, ?) WHERE id = ?")
            .bind(ts(now))
            .bind(item_id)
            .execute(pool)
            .await?;
        let position: i64 = sqlx::query_scalar("SELECT count(*) FROM session_items WHERE session_id = ? AND ordinal <= ?")
            .bind(session_id)
            .bind(ordinal)
            .fetch_one(pool)
            .await?;
        let detail = questions::get(pool, qid).await?;
        let lapses = detail.state.lapses;
        let recent_reviews = detail.reviews.iter().rev().take(3).cloned().collect();
        return Ok(Some(NextQuestion {
            item_id,
            kind: ItemKind::parse(&kind)?,
            position: position as u32,
            remaining: unreviewed_counts(pool, session_id).await?.0,
            source_citation: citation(pool, &question).await?,
            question,
            topic_path: detail.topic_path,
            recent_reviews,
            lapses,
            is_leech: lapses >= settings.leech_lapses,
        }));
    }
}

/// Scores and grades an answer, appends it to the review log, and reschedules the question.
pub async fn record_review(
    pool: &SqlitePool,
    s: &Settings,
    session_id: i64,
    input: ReviewInput,
    now: DateTime<Utc>,
) -> Result<ReviewOutcome> {
    let session = session_row(pool, session_id).await?;
    if session.ended {
        return Err(Error::Conflict(format!("session {session_id} has ended; start a new session")));
    }
    let question = questions::find(pool, input.question_id).await?;
    let score = grading::score(&question.rubric, &input.rubric_result)?;
    let grade = grading::grade(&score, question.pass_threshold, input.hints_used, input.fluent);
    let stamp = ts(now);

    let mut tx = pool.begin().await?;
    let item: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, kind FROM session_items WHERE session_id = ? AND question_id = ? AND review_id IS NULL
         ORDER BY ordinal LIMIT 1",
    )
    .bind(session_id)
    .bind(input.question_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((item_id, kind)) = item else {
        return Err(Error::Conflict(format!(
            "question {} is already recorded or not in session {session_id}",
            input.question_id
        )));
    };
    let review_id: i64 = sqlx::query_scalar(
        "INSERT INTO reviews (question_id, session_id, reviewed_at, grade, score, rubric_result, hints_used, fluent,
             user_answer, asked_prompt, feedback, duration_ms, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?3, ?3) RETURNING id",
    )
    .bind(input.question_id)
    .bind(session_id)
    .bind(&stamp)
    .bind(grade as i64)
    .bind(score.score as f64)
    .bind(serde_json::to_string(&input.rubric_result).expect("serializes"))
    .bind(input.hints_used as i64)
    .bind(input.fluent as i64)
    .bind(&input.user_answer)
    .bind(&input.asked_prompt)
    .bind(&input.feedback)
    .bind(input.duration_ms)
    .fetch_one(&mut *tx)
    .await?;
    let prev = scheduler::load_state_conn(&mut tx, input.question_id).await?;
    let state = scheduler::next(&prev, grade, session.study_day, now, &SchedParams::main(s))?;
    scheduler::save_state(&mut tx, &state).await?;
    sqlx::query("UPDATE session_items SET review_id = ?, served_at = COALESCE(served_at, ?) WHERE id = ?")
        .bind(review_id)
        .bind(&stamp)
        .bind(item_id)
        .execute(&mut *tx)
        .await?;
    let requeued = grade == Grade::Again && kind != ItemKind::Requeue.as_str();
    if requeued {
        sqlx::query(
            "INSERT INTO session_items (session_id, ordinal, question_id, kind)
             SELECT ?1, COALESCE(MAX(ordinal) + 1, 0), ?2, 'requeue' FROM session_items WHERE session_id = ?1",
        )
        .bind(session_id)
        .bind(input.question_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(ReviewOutcome {
        review_id,
        grade,
        score: score.score,
        due_on: state.due_on.expect("reviewed state has a due date"),
        lapses: state.lapses,
        is_leech: state.lapses >= s.leech_lapses,
        requeued,
    })
}

/// Active main-program questions due on or before `day`.
pub(crate) async fn due_count(pool: &SqlitePool, s: &Settings, day: NaiveDate, now: DateTime<Utc>) -> Result<u32> {
    Ok(load_candidates(pool, s, day, now).await?.0.len() as u32)
}

/// Ends the session (idempotent) and summarizes it.
pub async fn end(pool: &SqlitePool, s: &Settings, session_id: i64, now: DateTime<Utc>) -> Result<SessionSummary> {
    let session = session_row(pool, session_id).await?;
    sqlx::query("UPDATE sessions SET ended_at = COALESCE(ended_at, ?1), updated_at = ?1 WHERE id = ?2")
        .bind(ts(now))
        .bind(session_id)
        .execute(pool)
        .await?;
    let grades: Vec<(i64, i64)> = sqlx::query_as("SELECT grade, count(*) FROM reviews WHERE session_id = ? GROUP BY grade")
        .bind(session_id)
        .fetch_all(pool)
        .await?;
    let mut by_grade = [0u32; 4];
    for (g, n) in grades {
        by_grade[(g - 1) as usize] = n as u32;
    }
    let leeches = sqlx::query_scalar(
        "SELECT DISTINCT r.question_id FROM reviews r JOIN question_state st ON st.question_id = r.question_id
         WHERE r.session_id = ? AND st.lapses >= ? ORDER BY r.question_id",
    )
    .bind(session_id)
    .bind(s.leech_lapses as i64)
    .fetch_all(pool)
    .await?;
    Ok(SessionSummary {
        answered: by_grade.iter().sum(),
        by_grade,
        streak: crate::stats::streak(pool, s, now).await?,
        due_tomorrow: due_count(pool, s, session.study_day + Duration::days(1), now).await?,
        leeches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::grading::PointResult;
    use crate::testutil::{self, all_hit, day, none_hit, utc};
    use crate::Error;

    async fn setup() -> (SqlitePool, Settings) {
        let p = test_pool().await;
        crate::settings::set(&p, "timezone", "America/Los_Angeles").await.unwrap();
        let s = Settings::load(&p).await.unwrap();
        (p, s)
    }
    fn input(q: i64, results: Vec<PointResult>) -> ReviewInput {
        ReviewInput {
            question_id: q,
            rubric_result: results,
            hints_used: 0,
            fluent: false,
            user_answer: "my answer".into(),
            asked_prompt: "q".into(),
            feedback: "nice".into(),
            duration_ms: None,
        }
    }
    const NOW: &str = "2026-10-23T18:00:00Z";

    #[tokio::test]
    async fn empty_database_session_is_empty() {
        let (p, s) = setup().await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        assert_eq!((st.planned, st.due_total, st.new_allowed), (0, 0, 3));
        assert!(next_question(&p, st.session_id, utc(NOW)).await.unwrap().is_none());
        let sum = end(&p, &s, st.session_id, utc(NOW)).await.unwrap();
        assert_eq!(sum.answered, 0);
    }

    #[tokio::test]
    async fn full_session_flow() {
        let (p, s) = setup().await;
        let t1 = testutil::topic(&p, None, "A").await;
        let t2 = testutil::topic(&p, None, "B").await;
        let d1 = testutil::question(&p, t1).await;
        let d2 = testutil::question(&p, t2).await;
        let n1 = testutil::question(&p, t1).await;
        testutil::due_state(&p, d1, "2026-10-23", 3.0).await;
        testutil::due_state(&p, d2, "2026-10-20", 3.0).await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        assert_eq!((st.planned, st.due_total, st.study_day), (3, 2, day("2026-10-23")));
        let mut seen = vec![];
        while let Some(nq) = next_question(&p, st.session_id, utc(NOW)).await.unwrap() {
            assert_eq!(nq.remaining, 3 - seen.len() as u32);
            assert_eq!(nq.topic_path.len(), 1);
            let out = record_review(&p, &s, st.session_id, input(nq.question.id, all_hit()), utc(NOW)).await.unwrap();
            assert_eq!(out.grade, Grade::Good);
            assert!(out.due_on > day("2026-10-23"));
            seen.push(nq.question.id);
        }
        seen.sort();
        assert_eq!(seen, vec![d1, d2, n1]);
        let sum = end(&p, &s, st.session_id, utc(NOW)).await.unwrap();
        assert_eq!((sum.answered, sum.by_grade), (3, [0, 0, 3, 0]));
        let again = end(&p, &s, st.session_id, utc(NOW)).await.unwrap();
        assert_eq!(again.answered, 3);
    }

    #[tokio::test]
    async fn again_requeues_once() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let nq = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        let out = record_review(&p, &s, st.session_id, input(q, none_hit()), utc(NOW)).await.unwrap();
        assert_eq!((out.grade, out.requeued), (Grade::Again, true));
        let re = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        assert_eq!((re.question.id, re.kind), (nq.question.id, ItemKind::Requeue));
        let out = record_review(&p, &s, st.session_id, input(q, none_hit()), utc(NOW)).await.unwrap();
        assert!(!out.requeued);
        assert!(next_question(&p, st.session_id, utc(NOW)).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn record_review_twice_is_rejected() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        record_review(&p, &s, st.session_id, input(q, all_hit()), utc(NOW)).await.unwrap();
        let err = record_review(&p, &s, st.session_id, input(q, all_hit()), utc(NOW)).await;
        assert!(matches!(err, Err(Error::Conflict(_))));
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM reviews").fetch_one(&p).await.unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn next_question_is_idempotent_until_recorded() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        testutil::question(&p, t).await;
        testutil::question(&p, t).await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let a = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        let b = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        assert_eq!(a.item_id, b.item_id);
        assert!(matches!(next_question(&p, 999, utc(NOW)).await, Err(Error::NotFound { .. })));
    }

    #[tokio::test]
    async fn start_resumes_open_session_same_day() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        testutil::question(&p, t).await;
        let a = start(&p, &s, utc(NOW)).await.unwrap();
        record_review(&p, &s, a.session_id, input(q, all_hit()), utc(NOW)).await.unwrap();
        let b = start(&p, &s, utc("2026-10-23T19:00:00Z")).await.unwrap();
        assert_eq!((b.session_id, b.planned), (a.session_id, 1));
        end(&p, &s, a.session_id, utc(NOW)).await.unwrap();
        let c = start(&p, &s, utc("2026-10-23T20:00:00Z")).await.unwrap();
        assert_ne!(c.session_id, a.session_id);
    }

    #[tokio::test]
    async fn session_keeps_its_study_day_across_rollover() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        // 10:50Z = 03:50 PDT on the 23rd → study day 2026-10-22
        let st = start(&p, &s, utc("2026-10-23T10:50:00Z")).await.unwrap();
        assert_eq!(st.study_day, day("2026-10-22"));
        record_review(&p, &s, st.session_id, input(q, all_hit()), utc("2026-10-23T11:10:00Z")).await.unwrap();
        let state = crate::scheduler::load_state(&p, q).await.unwrap();
        assert_eq!(state.last_study_day, Some(day("2026-10-22")));
    }

    #[tokio::test]
    async fn leech_flag_at_threshold() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        testutil::due_state(&p, q, "2026-10-23", 3.0).await;
        sqlx::query("UPDATE question_state SET lapses = 5 WHERE question_id = ?").bind(q).execute(&p).await.unwrap();
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let out = record_review(&p, &s, st.session_id, input(q, none_hit()), utc(NOW)).await.unwrap();
        assert_eq!((out.lapses, out.is_leech), (6, true));
        let re = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        assert!(re.is_leech);
        record_review(&p, &s, st.session_id, input(q, all_hit()), utc(NOW)).await.unwrap();
        assert_eq!(end(&p, &s, st.session_id, utc(NOW)).await.unwrap().leeches, vec![q]);
    }

    #[tokio::test]
    async fn unicode_and_multiline_answers_round_trip() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let mut i = input(q, all_hit());
        i.user_answer = "Line 1 🎺\n\"quoted\" — Schrödinger's 'horn'\n\tend".into();
        record_review(&p, &s, st.session_id, i.clone(), utc(NOW)).await.unwrap();
        let d = crate::questions::get(&p, q).await.unwrap();
        assert_eq!(d.reviews[0].user_answer, i.user_answer);
    }

    #[tokio::test]
    async fn rubric_mismatch_rejected_without_writing() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let bad = vec![PointResult { id: "nope".into(), hit: true, note: String::new() }];
        let err = record_review(&p, &s, st.session_id, input(q, bad), utc(NOW)).await.unwrap_err();
        assert!(matches!(&err, Error::Invalid(m) if m.contains("nope")));
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM reviews").fetch_one(&p).await.unwrap();
        assert_eq!(n, 0);
        assert!(next_question(&p, st.session_id, utc(NOW)).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn retired_question_is_skipped() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        let q2 = testutil::question(&p, t).await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let first = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap().question.id;
        crate::questions::set_status(&p, first, crate::questions::QuestionStatus::Retired, utc(NOW)).await.unwrap();
        let other = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap().question.id;
        assert_eq!(other, if first == q { q2 } else { q });
    }
}
