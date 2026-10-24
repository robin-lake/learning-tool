//! Pinned questions: asked first in the next study session, using new-question slots unless due.

use chrono::{DateTime, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::questions::{self, QuestionStatus};
use crate::queue::{self, Candidate};
use crate::types::{parse_ts, ts};
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Pin {
    pub id: i64,
    pub question_id: i64,
    pub pinned_at: DateTime<Utc>,
    pub unpinned_at: Option<DateTime<Utc>>,
    pub used_in_session_id: Option<i64>,
    pub used_at: Option<DateTime<Utc>>,
}

/// An active pin as shown in lists (no answer or rubric).
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct PinnedItem {
    pub pin_id: i64,
    pub question_id: i64,
    pub prompt: String,
    pub level: u8,
    pub topic_path: Vec<String>,
    pub pinned_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PinnedCandidate {
    pub pin_id: i64,
    pub candidate: Candidate,
    pub is_due: bool,
}

type PinRow = (i64, i64, String, Option<String>, Option<i64>, Option<String>);

fn pin_from_row((id, question_id, pinned_at, unpinned_at, used_in_session_id, used_at): PinRow) -> Result<Pin> {
    Ok(Pin {
        id,
        question_id,
        pinned_at: parse_ts(&pinned_at)?,
        unpinned_at: unpinned_at.as_deref().map(parse_ts).transpose()?,
        used_in_session_id,
        used_at: used_at.as_deref().map(parse_ts).transpose()?,
    })
}

async fn active_pin(pool: &SqlitePool, question_id: i64) -> Result<Option<Pin>> {
    let row: Option<PinRow> = sqlx::query_as(
        "SELECT id, question_id, pinned_at, unpinned_at, used_in_session_id, used_at FROM pins
         WHERE question_id = ? AND unpinned_at IS NULL AND used_in_session_id IS NULL",
    )
    .bind(question_id)
    .fetch_optional(pool)
    .await?;
    row.map(pin_from_row).transpose()
}

/// Pins an active question; pinning an already pinned question returns the existing pin.
pub async fn pin(pool: &SqlitePool, question_id: i64, now: DateTime<Utc>) -> Result<Pin> {
    let q = questions::find(pool, question_id).await?;
    if q.status != QuestionStatus::Active {
        return Err(Error::Invalid(format!("question {question_id} is {}; only active questions can be pinned", q.status)));
    }
    if let Some(existing) = active_pin(pool, question_id).await? {
        return Ok(existing);
    }
    let stamp = ts(now);
    sqlx::query("INSERT INTO pins (question_id, pinned_at, created_at, updated_at) VALUES (?1, ?2, ?2, ?2)")
        .bind(question_id)
        .bind(&stamp)
        .execute(pool)
        .await?;
    Ok(active_pin(pool, question_id).await?.expect("pin just inserted"))
}

/// Unpins a question (the row is kept). A question with no active pin is left as is.
pub async fn unpin(pool: &SqlitePool, question_id: i64, now: DateTime<Utc>) -> Result<()> {
    questions::find(pool, question_id).await?;
    let stamp = ts(now);
    sqlx::query(
        "UPDATE pins SET unpinned_at = ?1, updated_at = ?1
         WHERE question_id = ?2 AND unpinned_at IS NULL AND used_in_session_id IS NULL",
    )
    .bind(&stamp)
    .bind(question_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn is_pinned(pool: &SqlitePool, question_id: i64) -> Result<bool> {
    Ok(active_pin(pool, question_id).await?.is_some())
}

type LiveRow = (i64, i64, String, i64, i64, i64, i64, i64, Option<String>, Option<f64>, Option<String>);

/// Active pins on schedulable questions (active, main program, topic and all ancestors active), in pin order.
async fn live_pins(pool: &SqlitePool) -> Result<Vec<LiveRow>> {
    Ok(sqlx::query_as(
        "WITH RECURSIVE live(id, depth) AS (
             SELECT id, 0 FROM topics WHERE parent_id IS NULL AND status = 'active'
             UNION ALL
             SELECT t.id, live.depth + 1 FROM topics t JOIN live ON t.parent_id = live.id WHERE t.status = 'active'
         )
         SELECT p.id, q.id, p.pinned_at, q.topic_id, live.depth, t.position, q.level, q.priority,
                s.due_on, s.stability, s.last_reviewed_at
         FROM pins p
         JOIN questions q ON q.id = p.question_id
         JOIN live ON live.id = q.topic_id
         JOIN topics t ON t.id = q.topic_id
         LEFT JOIN question_state s ON s.question_id = q.id
         WHERE p.unpinned_at IS NULL AND p.used_in_session_id IS NULL
           AND q.status = 'active' AND q.scope = 'main'
         ORDER BY p.pinned_at, p.id",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn active(pool: &SqlitePool) -> Result<Vec<PinnedItem>> {
    let mut out = vec![];
    for (pin_id, question_id, pinned_at, ..) in live_pins(pool).await? {
        let detail = questions::get(pool, question_id).await?;
        out.push(PinnedItem {
            pin_id,
            question_id,
            prompt: detail.question.prompt,
            level: detail.question.level,
            topic_path: detail.topic_path,
            pinned_at: parse_ts(&pinned_at)?,
        });
    }
    Ok(out)
}

/// Pinned questions as queue candidates, with whether each is due on `today`.
pub(crate) async fn load_pinned(pool: &SqlitePool, today: NaiveDate, now: DateTime<Utc>) -> Result<Vec<PinnedCandidate>> {
    let today_s = today.to_string();
    let mut out = vec![];
    for (pin_id, question_id, _, topic_id, depth, position, level, priority, due_on, stability, last) in
        live_pins(pool).await?
    {
        let candidate = queue::candidate(question_id, topic_id, depth, position, level, priority, stability, last, now)?;
        out.push(PinnedCandidate {
            pin_id,
            is_due: due_on.as_deref().is_some_and(|d| d <= today_s.as_str()),
            candidate,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::testutil::{self, utc};
    use crate::Error;

    fn t(min: u32) -> DateTime<Utc> {
        utc(&format!("2026-10-24T18:{min:02}:00Z"))
    }

    #[tokio::test]
    async fn pin_is_idempotent_and_listed_in_order() {
        let p = test_pool().await;
        let topic = testutil::topic(&p, None, "T").await;
        let q1 = testutil::question(&p, topic).await;
        let q2 = testutil::question(&p, topic).await;
        let a = pin(&p, q2, t(0)).await.unwrap();
        pin(&p, q1, t(1)).await.unwrap();
        assert_eq!(pin(&p, q2, t(2)).await.unwrap().id, a.id);
        let ids: Vec<i64> = active(&p).await.unwrap().iter().map(|i| i.question_id).collect();
        assert_eq!(ids, vec![q2, q1]);
        assert!(is_pinned(&p, q1).await.unwrap());
        let item = &active(&p).await.unwrap()[0];
        assert_eq!((item.prompt.as_str(), item.topic_path.clone()), ("q", vec!["T".to_string()]));
    }

    #[tokio::test]
    async fn unpin_keeps_the_row() {
        let p = test_pool().await;
        let topic = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, topic).await;
        pin(&p, q, t(0)).await.unwrap();
        unpin(&p, q, t(1)).await.unwrap();
        assert!(active(&p).await.unwrap().is_empty());
        let (n, unpinned): (i64, Option<String>) =
            sqlx::query_as("SELECT count(*), max(unpinned_at) FROM pins").fetch_one(&p).await.unwrap();
        assert_eq!(n, 1);
        assert!(unpinned.is_some());
        unpin(&p, q, t(2)).await.unwrap();
        assert!(!is_pinned(&p, q).await.unwrap());
    }

    #[tokio::test]
    async fn pin_rejects_non_active_and_missing() {
        let p = test_pool().await;
        let topic = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, topic).await;
        sqlx::query("UPDATE questions SET status = 'suspended' WHERE id = ?").bind(q).execute(&p).await.unwrap();
        assert!(matches!(pin(&p, q, t(0)).await, Err(Error::Invalid(m)) if m.contains("suspended")));
        assert!(matches!(pin(&p, 999, t(0)).await, Err(Error::NotFound { .. })));
        assert!(matches!(unpin(&p, 999, t(0)).await, Err(Error::NotFound { .. })));
    }

    #[tokio::test]
    async fn pin_on_archived_topic_is_ignored() {
        let p = test_pool().await;
        let parent = testutil::topic(&p, None, "P").await;
        let child = testutil::topic(&p, Some(parent), "C").await;
        let q = testutil::question(&p, child).await;
        pin(&p, q, t(0)).await.unwrap();
        sqlx::query("UPDATE topics SET status = 'archived' WHERE id = ?").bind(parent).execute(&p).await.unwrap();
        assert!(active(&p).await.unwrap().is_empty());
        assert!(load_pinned(&p, testutil::day("2026-10-24"), t(1)).await.unwrap().is_empty());
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM pins").fetch_one(&p).await.unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn repin_after_unpin_creates_new_row() {
        let p = test_pool().await;
        let topic = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, topic).await;
        pin(&p, q, t(0)).await.unwrap();
        unpin(&p, q, t(1)).await.unwrap();
        pin(&p, q, t(2)).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM pins").fetch_one(&p).await.unwrap();
        assert_eq!((n, active(&p).await.unwrap().len()), (2, 1));
    }

    #[tokio::test]
    async fn load_pinned_marks_due_state() {
        let p = test_pool().await;
        let topic = testutil::topic(&p, None, "T").await;
        let new_q = testutil::question(&p, topic).await;
        let due_q = testutil::question(&p, topic).await;
        let later_q = testutil::question(&p, topic).await;
        testutil::due_state(&p, due_q, "2026-10-24", 3.0).await;
        testutil::due_state(&p, later_q, "2026-11-30", 30.0).await;
        for (i, q) in [new_q, due_q, later_q].into_iter().enumerate() {
            pin(&p, q, t(i as u32)).await.unwrap();
        }
        let got = load_pinned(&p, testutil::day("2026-10-24"), t(5)).await.unwrap();
        let shape: Vec<(i64, bool, bool)> =
            got.iter().map(|c| (c.candidate.question_id, c.is_due, c.candidate.retrievability.is_some())).collect();
        assert_eq!(shape, vec![(new_q, false, false), (due_q, true, true), (later_q, false, true)]);
    }
}
