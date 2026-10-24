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
    /// Pinned questions that didn't fit today's new-question slots; they're asked on a later day.
    pub pinned_waiting: u32,
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
    pub question_id: i64,
    pub level: u8,
    /// The question to ask (you may rephrase the surface, keeping the target).
    pub prompt: String,
    pub topic_path: Vec<String>,
    pub source_citation: Option<String>,
    pub lapses: u32,
    pub is_leech: bool,
    /// True if Robin pinned this question for this session.
    pub pinned: bool,
    pub recent_reviews: Vec<ReviewRow>,
    /// For grading only — contains the answer and rubric. Listed last so it never shows in a
    /// truncated preview; don't reveal it before the learner answers.
    pub question: Question,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReviewInput {
    /// The item_id returned by next_question for this question.
    pub item_id: i64,
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
    let (due, new, pinned, introduced) = load_candidates(pool, s, today, now).await?;
    let queue = plan(due, new, pinned, introduced, s);
    let stamp = ts(now);

    let (session_id, resumed) = match open {
        Some(id) if unreviewed_counts(pool, id).await?.0 > 0 => (id, true),
        _ => {
            let mut tx = crate::db::begin_write(pool).await?;
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
            for (i, item) in queue.items.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO session_items (session_id, ordinal, question_id, kind, pin_id) VALUES (?, ?, ?, ?, ?)",
                )
                    .bind(id)
                    .bind(base + i as i64)
                    .bind(item.question_id)
                    .bind(item.kind.as_str())
                    .bind(item.pin_id)
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
        pinned_waiting: queue.pinned_waiting,
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
        let item: Option<(i64, i64, String, i64, Option<i64>)> = sqlx::query_as(
            "SELECT id, question_id, kind, ordinal, pin_id FROM session_items
             WHERE session_id = ? AND review_id IS NULL ORDER BY ordinal LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(pool)
        .await?;
        let Some((item_id, qid, kind, ordinal, pin_id)) = item else { return Ok(None) };
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
            question_id: question.id,
            level: question.level,
            prompt: question.prompt.clone(),
            topic_path: detail.topic_path,
            source_citation: citation(pool, &question).await?,
            lapses,
            is_leech: lapses >= settings.leech_lapses,
            pinned: pin_id.is_some(),
            recent_reviews,
            question,
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

    let mut tx = crate::db::begin_write(pool).await?;
    let item: Option<(i64, String, Option<i64>, Option<i64>)> =
        sqlx::query_as("SELECT question_id, kind, review_id, pin_id FROM session_items WHERE id = ? AND session_id = ?")
            .bind(input.item_id)
            .bind(session_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((item_question, kind, existing, pin_id)) = item else {
        return Err(Error::NotFound { entity: "session item", id: input.item_id });
    };
    if existing.is_some() {
        return Err(Error::Conflict(format!("item {} was already recorded", input.item_id)));
    }
    if item_question != input.question_id {
        return Err(Error::Invalid(format!(
            "item {} is question {item_question}, not question {}",
            input.item_id, input.question_id
        )));
    }
    let item_id = input.item_id;
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
    if let Some(pin_id) = pin_id {
        sqlx::query("UPDATE pins SET used_in_session_id = ?1, used_at = ?2, updated_at = ?2 WHERE id = ?3")
            .bind(session_id)
            .bind(&stamp)
            .bind(pin_id)
            .execute(&mut *tx)
            .await?;
    }
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

/// Drops an unanswered item from this session without recording a review. The question stays
/// scheduled and comes back in a later session.
pub async fn skip(pool: &SqlitePool, session_id: i64, item_id: i64) -> Result<()> {
    let item: Option<Option<i64>> = sqlx::query_scalar("SELECT review_id FROM session_items WHERE id = ? AND session_id = ?")
        .bind(item_id)
        .bind(session_id)
        .fetch_optional(pool)
        .await?;
    match item {
        None => Err(Error::NotFound { entity: "session item", id: item_id }),
        Some(Some(_)) => Err(Error::Conflict(format!("item {item_id} was already answered"))),
        Some(None) => {
            sqlx::query("DELETE FROM session_items WHERE id = ?").bind(item_id).execute(pool).await?;
            Ok(())
        }
    }
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
    async fn input_for(p: &SqlitePool, sid: i64, q: i64, results: Vec<PointResult>) -> ReviewInput {
        let item_id: i64 = sqlx::query_scalar(
            "SELECT id FROM session_items WHERE session_id = ? AND question_id = ? AND review_id IS NULL ORDER BY ordinal LIMIT 1",
        )
        .bind(sid).bind(q).fetch_optional(p).await.unwrap().unwrap_or(0);
        let mut i = input(q, results);
        i.item_id = item_id;
        i
    }
    fn input(q: i64, results: Vec<PointResult>) -> ReviewInput {
        ReviewInput {
            item_id: 0,
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
        assert_eq!((st.planned, st.due_total, st.new_allowed), (0, 0, 0));
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
            let out = record_review(&p, &s, st.session_id, input_for(&p, st.session_id, nq.question.id, all_hit()).await, utc(NOW)).await.unwrap();
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
        let out = record_review(&p, &s, st.session_id, input_for(&p, st.session_id, q, none_hit()).await, utc(NOW)).await.unwrap();
        assert_eq!((out.grade, out.requeued), (Grade::Again, true));
        let re = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        assert_eq!((re.question.id, re.kind), (nq.question.id, ItemKind::Requeue));
        let out = record_review(&p, &s, st.session_id, input_for(&p, st.session_id, q, none_hit()).await, utc(NOW)).await.unwrap();
        assert!(!out.requeued);
        assert!(next_question(&p, st.session_id, utc(NOW)).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn record_review_twice_is_rejected() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let first = input_for(&p, st.session_id, q, all_hit()).await;
        record_review(&p, &s, st.session_id, first.clone(), utc(NOW)).await.unwrap();
        let err = record_review(&p, &s, st.session_id, first, utc(NOW)).await;
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
        record_review(&p, &s, a.session_id, input_for(&p, a.session_id, q, all_hit()).await, utc(NOW)).await.unwrap();
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
        record_review(&p, &s, st.session_id, input_for(&p, st.session_id, q, all_hit()).await, utc("2026-10-23T11:10:00Z")).await.unwrap();
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
        let out = record_review(&p, &s, st.session_id, input_for(&p, st.session_id, q, none_hit()).await, utc(NOW)).await.unwrap();
        assert_eq!((out.lapses, out.is_leech), (6, true));
        let re = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        assert!(re.is_leech);
        record_review(&p, &s, st.session_id, input_for(&p, st.session_id, q, all_hit()).await, utc(NOW)).await.unwrap();
        assert_eq!(end(&p, &s, st.session_id, utc(NOW)).await.unwrap().leeches, vec![q]);
    }

    #[tokio::test]
    async fn unicode_and_multiline_answers_round_trip() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let mut i = input_for(&p, st.session_id, q, all_hit()).await;
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
        let err = record_review(&p, &s, st.session_id, input_for(&p, st.session_id, q, bad).await, utc(NOW)).await.unwrap_err();
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

    #[tokio::test]
    async fn retry_after_again_is_rejected_and_keeps_requeue() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let nq = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        let mut i = input(q, none_hit());
        i.item_id = nq.item_id;
        assert!(record_review(&p, &s, st.session_id, i.clone(), utc(NOW)).await.unwrap().requeued);
        assert!(matches!(record_review(&p, &s, st.session_id, i, utc(NOW)).await, Err(Error::Conflict(_))));
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM reviews").fetch_one(&p).await.unwrap();
        assert_eq!(n, 1);
        let re = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        assert_eq!(re.kind, ItemKind::Requeue);
    }

    #[tokio::test]
    async fn item_must_match_question() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let a = testutil::question(&p, t).await;
        let b = testutil::question(&p, t).await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let mut i = input_for(&p, st.session_id, a, all_hit()).await;
        i.question_id = b;
        assert!(matches!(record_review(&p, &s, st.session_id, i, utc(NOW)).await, Err(Error::Invalid(_))));
    }

    #[tokio::test]
    async fn skip_drops_item_for_this_session() {
        let (p, s) = setup().await;
        let t1 = testutil::topic(&p, None, "A").await;
        let t2 = testutil::topic(&p, None, "B").await;
        testutil::question(&p, t1).await;
        testutil::question(&p, t2).await;
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let first = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        skip(&p, st.session_id, first.item_id).await.unwrap();
        let second = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        assert_ne!(second.question.id, first.question.id);
        assert_eq!(second.remaining, 1);
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM reviews").fetch_one(&p).await.unwrap();
        assert_eq!(n, 0);
        assert!(matches!(skip(&p, st.session_id, first.item_id).await, Err(Error::NotFound { .. })));
        let mut i = input(second.question.id, all_hit());
        i.item_id = second.item_id;
        record_review(&p, &s, st.session_id, i, utc(NOW)).await.unwrap();
        assert!(matches!(skip(&p, st.session_id, second.item_id).await, Err(Error::Conflict(_))));
    }

    #[tokio::test]
    async fn answer_and_rubric_come_after_the_prompt() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        sqlx::query("UPDATE questions SET prompt = 'What does a horn do?', answer = 'SECRET-ANSWER' WHERE id = ?")
            .bind(q).execute(&p).await.unwrap();
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let nq = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        assert_eq!((nq.question_id, nq.prompt.as_str(), nq.level), (q, "What does a horn do?", 0));
        let json = serde_json::to_string_pretty(&nq).unwrap();
        let head: String = json.lines().take(12).collect::<Vec<_>>().join("\n");
        assert!(head.contains("What does a horn do?"), "{head}");
        assert!(!head.contains("SECRET-ANSWER") && !head.contains("rubric"), "{head}");
    }

    #[tokio::test]
    async fn pinned_item_served_first_and_flagged() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        testutil::question(&p, t).await;
        let q2 = testutil::question(&p, t).await;
        crate::pins::pin(&p, q2, utc(NOW)).await.unwrap();
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let first = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        assert_eq!((first.question_id, first.pinned), (q2, true));
    }

    #[tokio::test]
    async fn pin_marked_used_on_record_not_on_plan() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        crate::pins::pin(&p, q, utc(NOW)).await.unwrap();
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        assert!(crate::pins::is_pinned(&p, q).await.unwrap());
        record_review(&p, &s, st.session_id, input_for(&p, st.session_id, q, all_hit()).await, utc(NOW)).await.unwrap();
        assert!(!crate::pins::is_pinned(&p, q).await.unwrap());
        let used: Option<i64> = sqlx::query_scalar("SELECT used_in_session_id FROM pins").fetch_one(&p).await.unwrap();
        assert_eq!(used, Some(st.session_id));
    }

    #[tokio::test]
    async fn skipped_pin_stays_pinned() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        crate::pins::pin(&p, q, utc(NOW)).await.unwrap();
        let st = start(&p, &s, utc(NOW)).await.unwrap();
        let nq = next_question(&p, st.session_id, utc(NOW)).await.unwrap().unwrap();
        skip(&p, st.session_id, nq.item_id).await.unwrap();
        assert!(crate::pins::is_pinned(&p, q).await.unwrap());
    }

    #[tokio::test]
    async fn open_session_not_changed_by_new_pin() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "A").await;
        testutil::question(&p, t).await;
        let extra = testutil::question(&p, t).await;
        sqlx::query("UPDATE settings SET value = '1' WHERE key = 'new_per_day'").execute(&p).await.unwrap();
        let s1 = Settings::load(&p).await.unwrap();
        let a = start(&p, &s1, utc(NOW)).await.unwrap();
        crate::pins::pin(&p, extra, utc(NOW)).await.unwrap();
        let b = start(&p, &s1, utc(NOW)).await.unwrap();
        assert_eq!((b.session_id, b.planned), (a.session_id, 1));
        let first = next_question(&p, b.session_id, utc(NOW)).await.unwrap().unwrap();
        assert!(!first.pinned);
        let _ = s;
    }

    #[tokio::test]
    async fn start_reports_pins_waiting_for_a_slot() {
        let (p, _) = setup().await;
        sqlx::query("UPDATE settings SET value = '0' WHERE key = 'new_per_day'").execute(&p).await.unwrap();
        let s0 = Settings::load(&p).await.unwrap();
        let t = testutil::topic(&p, None, "A").await;
        let q = testutil::question(&p, t).await;
        crate::pins::pin(&p, q, utc(NOW)).await.unwrap();
        let st = start(&p, &s0, utc(NOW)).await.unwrap();
        assert_eq!((st.planned, st.pinned_waiting), (0, 1));
    }
}
