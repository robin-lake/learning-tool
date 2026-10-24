use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::questions::{self, RatingRow, rating_from_row};
pub use crate::types::RatingClass;
use crate::types::{parse_ts, str_enum, ts};
use crate::{Error, Result};

str_enum!(Verdict { Good => "good", Bad => "bad" });

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RatingView {
    pub rating: RatingRow,
    pub question_prompt: String,
    pub level: u8,
    pub topic_path: Vec<String>,
    pub guide_version_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct GuideVersion {
    pub id: i64,
    pub content: String,
    pub change_summary: String,
    pub parent_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct GuideExample {
    pub question_id: i64,
    pub verdict: Verdict,
    pub level: u8,
    pub reason: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NewExample {
    pub question_id: i64,
    pub verdict: Verdict,
    #[serde(default)]
    pub reason: String,
}

pub async fn rate(
    pool: &SqlitePool,
    question_id: i64,
    usefulness: u8,
    feedback: Option<String>,
    class: Option<RatingClass>,
    review_id: Option<i64>,
    now: DateTime<Utc>,
) -> Result<i64> {
    if !(1..=5).contains(&usefulness) {
        return Err(Error::Invalid(format!("usefulness must be 1-5, got {usefulness}")));
    }
    questions::find(pool, question_id).await?;
    let stamp = ts(now);
    Ok(sqlx::query_scalar(
        "INSERT INTO question_ratings (question_id, review_id, rated_at, usefulness, feedback, classification, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?3, ?3) RETURNING id",
    )
    .bind(question_id)
    .bind(review_id)
    .bind(&stamp)
    .bind(usefulness as i64)
    .bind(feedback.filter(|f| !f.trim().is_empty()))
    .bind(class.map(|c| c.as_str()))
    .fetch_one(pool)
    .await?)
}

pub async fn classify(pool: &SqlitePool, rating_id: i64, class: RatingClass) -> Result<()> {
    let n = sqlx::query("UPDATE question_ratings SET classification = ? WHERE id = ?")
        .bind(class.as_str())
        .bind(rating_id)
        .execute(pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(Error::NotFound { entity: "rating", id: rating_id });
    }
    Ok(())
}

pub async fn list_ratings(pool: &SqlitePool, unprocessed_only: bool) -> Result<Vec<RatingView>> {
    let rows = sqlx::query(
        "SELECT * FROM question_ratings WHERE (?1 = 0 OR processed_in_guide_version_id IS NULL) ORDER BY rated_at, id",
    )
    .bind(unprocessed_only as i64)
    .fetch_all(pool)
    .await?;
    let mut out = vec![];
    for row in rows {
        let rating = rating_from_row(&row)?;
        let detail = questions::get(pool, rating.question_id).await?;
        out.push(RatingView {
            question_prompt: detail.question.prompt,
            level: detail.question.level,
            topic_path: detail.topic_path,
            guide_version_id: detail.question.guide_version_id,
            rating,
        });
    }
    Ok(out)
}

fn version_from_row(r: &SqliteRow) -> Result<GuideVersion> {
    Ok(GuideVersion {
        id: r.try_get("id")?,
        content: r.try_get("content")?,
        change_summary: r.try_get("change_summary")?,
        parent_id: r.try_get("parent_id")?,
        created_at: parse_ts(r.try_get("created_at")?)?,
    })
}

/// The newest guide version and its examples.
pub async fn current(pool: &SqlitePool) -> Result<(GuideVersion, Vec<GuideExample>)> {
    let row = sqlx::query("SELECT * FROM guide_versions ORDER BY id DESC LIMIT 1")
        .fetch_optional(pool)
        .await?
        .ok_or(Error::NotFound { entity: "guide version", id: 0 })?;
    let version = version_from_row(&row)?;
    let examples: Vec<(i64, String, i64, String, String)> = sqlx::query_as(
        "SELECT e.question_id, e.verdict, e.level, e.reason, q.prompt
         FROM guide_examples e JOIN questions q ON q.id = e.question_id
         WHERE e.guide_version_id = ? ORDER BY e.level, e.id",
    )
    .bind(version.id)
    .fetch_all(pool)
    .await?;
    let examples = examples
        .into_iter()
        .map(|(question_id, verdict, level, reason, prompt)| {
            Ok(GuideExample { question_id, verdict: Verdict::parse(&verdict)?, level: level as u8, reason, prompt })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((version, examples))
}

pub async fn save_version(
    pool: &SqlitePool,
    content: String,
    change_summary: String,
    examples: Vec<NewExample>,
    processed_rating_ids: Vec<i64>,
    removed_example_question_ids: Vec<i64>,
    now: DateTime<Utc>,
) -> Result<GuideVersion> {
    if content.trim().is_empty() {
        return Err(Error::Invalid("guide content must not be empty".into()));
    }
    let mut levels = vec![];
    for e in &examples {
        let q = questions::find(pool, e.question_id)
            .await
            .map_err(|_| Error::Invalid(format!("example question {} does not exist", e.question_id)))?;
        levels.push(q.level);
    }
    // The previous version's examples carry forward unless removed or replaced by a new verdict.
    let (parent, previous) = current(pool).await?;
    let mut all: Vec<(NewExample, u8)> = previous
        .into_iter()
        .filter(|p| {
            !removed_example_question_ids.contains(&p.question_id)
                && !examples.iter().any(|e| e.question_id == p.question_id)
        })
        .map(|p| (NewExample { question_id: p.question_id, verdict: p.verdict, reason: p.reason }, p.level))
        .collect();
    all.extend(examples.into_iter().zip(levels));
    let stamp = ts(now);
    let mut tx = crate::db::begin_write(pool).await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO guide_versions (content, change_summary, parent_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4) RETURNING id",
    )
    .bind(&content)
    .bind(&change_summary)
    .bind(parent.id)
    .bind(&stamp)
    .fetch_one(&mut *tx)
    .await?;
    for (e, level) in &all {
        sqlx::query(
            "INSERT INTO guide_examples (guide_version_id, question_id, verdict, level, reason, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        )
        .bind(id)
        .bind(e.question_id)
        .bind(e.verdict.as_str())
        .bind(*level as i64)
        .bind(&e.reason)
        .bind(&stamp)
        .execute(&mut *tx)
        .await?;
    }
    for r in processed_rating_ids {
        sqlx::query("UPDATE question_ratings SET processed_in_guide_version_id = ?, updated_at = ? WHERE id = ?")
            .bind(id)
            .bind(&stamp)
            .bind(r)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(current(pool).await?.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::testutil::{self, utc};
    use crate::Error;

    fn now() -> DateTime<Utc> {
        utc("2026-10-23T18:00:00Z")
    }

    #[tokio::test]
    async fn rate_validates_range() {
        let p = test_pool().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        assert!(matches!(rate(&p, q, 0, None, None, None, now()).await, Err(Error::Invalid(_))));
        assert!(matches!(rate(&p, q, 6, None, None, None, now()).await, Err(Error::Invalid(_))));
        assert!(matches!(rate(&p, 999, 3, None, None, None, now()).await, Err(Error::NotFound { .. })));
        let id = rate(&p, q, 2, Some("too trivial".into()), Some(RatingClass::QuestionQuality), None, now()).await.unwrap();
        classify(&p, id, RatingClass::KnowledgeValue).await.unwrap();
        let r = &crate::questions::get(&p, q).await.unwrap().ratings[0];
        assert_eq!((r.usefulness, r.classification), (2, Some(RatingClass::KnowledgeValue)));
    }

    #[tokio::test]
    async fn list_unprocessed_excludes_processed() {
        let p = test_pool().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        let a = rate(&p, q, 1, Some("bad".into()), None, None, now()).await.unwrap();
        rate(&p, q, 5, Some("great".into()), None, None, now()).await.unwrap();
        save_version(&p, "v2".into(), "x".into(), vec![], vec![a], vec![], now()).await.unwrap();
        let open = list_ratings(&p, true).await.unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].rating.usefulness, 5);
        assert_eq!(open[0].topic_path, vec!["T".to_string()]);
        assert_eq!(list_ratings(&p, false).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn save_version_chains_parent_and_marks_processed() {
        let p = test_pool().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        let r = rate(&p, q, 1, None, None, None, now()).await.unwrap();
        let (v1, _) = current(&p).await.unwrap();
        let ex = NewExample { question_id: q, verdict: Verdict::Bad, reason: "trivia".into() };
        let v2 = save_version(&p, "guide v2".into(), "less trivia".into(), vec![ex], vec![r], vec![], now()).await.unwrap();
        assert_eq!(v2.parent_id, Some(v1.id));
        let processed: Option<i64> = sqlx::query_scalar("SELECT processed_in_guide_version_id FROM question_ratings WHERE id = ?")
            .bind(r).fetch_one(&p).await.unwrap();
        assert_eq!(processed, Some(v2.id));
        let bad = NewExample { question_id: 999, verdict: Verdict::Good, reason: String::new() };
        assert!(matches!(save_version(&p, "v3".into(), String::new(), vec![bad], vec![], vec![], now()).await, Err(Error::Invalid(_))));
        assert_eq!(current(&p).await.unwrap().0.id, v2.id);
    }

    #[tokio::test]
    async fn current_returns_latest_with_examples() {
        let p = test_pool().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        let ex = NewExample { question_id: q, verdict: Verdict::Good, reason: "clear".into() };
        save_version(&p, "v2".into(), String::new(), vec![ex], vec![], vec![], now()).await.unwrap();
        let (v, examples) = current(&p).await.unwrap();
        assert_eq!(v.content, "v2");
        assert_eq!(examples.len(), 1);
        assert_eq!((examples[0].verdict, examples[0].level, examples[0].prompt.as_str()), (Verdict::Good, 0, "q"));
    }

    #[tokio::test]
    async fn seeded_guide_has_level_sections() {
        let p = test_pool().await;
        let (v, _) = current(&p).await.unwrap();
        for needle in ["L0", "L1", "L2", "L3", "rubric", "pass_threshold"] {
            assert!(v.content.contains(needle), "missing {needle}");
        }
        assert_eq!(v.parent_id, None);
    }

    #[tokio::test]
    async fn examples_carry_forward_unless_removed_or_replaced() {
        let p = test_pool().await;
        let t = testutil::topic(&p, None, "T").await;
        let a = testutil::question(&p, t).await;
        let b = testutil::question(&p, t).await;
        let ex = |q, verdict| NewExample { question_id: q, verdict, reason: String::new() };
        save_version(&p, "v2".into(), String::new(), vec![ex(a, Verdict::Good)], vec![], vec![], now()).await.unwrap();
        save_version(&p, "v3".into(), String::new(), vec![ex(b, Verdict::Bad)], vec![], vec![], now()).await.unwrap();
        let ids = |e: &[GuideExample]| e.iter().map(|x| (x.question_id, x.verdict)).collect::<Vec<_>>();
        assert_eq!(ids(&current(&p).await.unwrap().1), vec![(a, Verdict::Good), (b, Verdict::Bad)]);
        save_version(&p, "v4".into(), String::new(), vec![ex(b, Verdict::Good)], vec![], vec![a], now()).await.unwrap();
        assert_eq!(ids(&current(&p).await.unwrap().1), vec![(b, Verdict::Good)]);
    }
}
