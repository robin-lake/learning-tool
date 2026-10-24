use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::grading::{Grade, PointResult, Rubric, validate_rubric};
use crate::scheduler::{self, QState};
use crate::settings::Settings;
use crate::topics;
pub use crate::types::{LinkKind, RatingClass, Scope};
use crate::types::{parse_ts, str_enum, ts, validate_level, validate_weight};
use crate::{Error, Result};

str_enum!(QuestionStatus {
    Proposed => "proposed",
    Active => "active",
    Suspended => "suspended",
    Retired => "retired",
});

pub const DEFAULT_PASS_THRESHOLD: f32 = 0.6;
/// Reviews generated per new question in its first month (research: roughly 7-10).
const REVIEWS_PER_NEW_FIRST_MONTH: f32 = 8.0;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Question {
    pub id: i64,
    pub topic_id: i64,
    pub level: u8,
    pub prompt: String,
    pub answer: String,
    pub rubric: Rubric,
    pub pass_threshold: f32,
    pub source_id: Option<i64>,
    pub source_locator: Option<String>,
    pub scope: Scope,
    pub project_id: Option<i64>,
    pub status: QuestionStatus,
    pub priority: i64,
    pub guide_version_id: Option<i64>,
    pub revision_of: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NewQuestion {
    /// Mastery level 0-3 (L0 orientation … L3 expert).
    pub level: u8,
    /// The canonical question.
    pub prompt: String,
    /// Reference answer, shown only after the learner attempts the question.
    pub answer: String,
    pub rubric: Rubric,
    /// Share of rubric weight needed to pass (default 0.6).
    #[serde(default)]
    pub pass_threshold: Option<f32>,
    #[serde(default)]
    pub source_id: Option<i64>,
    /// Where in the source, e.g. "p. 47" or "§3.2".
    #[serde(default)]
    pub source_locator: Option<String>,
    /// Lower = introduced sooner. Defaults to input order after existing questions.
    #[serde(default)]
    pub priority: Option<i64>,
    #[serde(default)]
    pub guide_version_id: Option<i64>,
    /// Other topics this question also belongs to.
    #[serde(default)]
    pub secondary_topic_ids: Vec<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct QuestionPatch {
    pub prompt: Option<String>,
    pub answer: Option<String>,
    pub rubric: Option<Rubric>,
    pub pass_threshold: Option<f32>,
    pub level: Option<u8>,
    pub priority: Option<i64>,
    pub topic_id: Option<i64>,
    pub source_id: Option<i64>,
    pub source_locator: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LoadForecast {
    pub new_active: u32,
    /// Days of new-question capacity this batch uses at the current new-per-day cap.
    pub days_of_new_capacity: u32,
    pub est_extra_reviews_per_day_first_month: f32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AddResult {
    pub ids: Vec<i64>,
    pub forecast: LoadForecast,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct ReviewRow {
    pub id: i64,
    pub question_id: i64,
    pub session_id: i64,
    pub reviewed_at: DateTime<Utc>,
    pub grade: Grade,
    pub score: f32,
    pub rubric_result: Vec<PointResult>,
    pub hints_used: u32,
    pub fluent: bool,
    pub user_answer: String,
    pub asked_prompt: String,
    pub feedback: String,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct RatingRow {
    pub id: i64,
    pub question_id: i64,
    pub review_id: Option<i64>,
    pub rated_at: DateTime<Utc>,
    pub usefulness: u8,
    pub feedback: Option<String>,
    pub classification: Option<RatingClass>,
    pub processed_in_guide_version_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct QuestionDetail {
    pub question: Question,
    pub topic_path: Vec<String>,
    pub secondary_topic_ids: Vec<i64>,
    pub state: QState,
    /// Reviews of this question and the earlier wordings it revises, oldest first.
    pub reviews: Vec<ReviewRow>,
    pub ratings: Vec<RatingRow>,
    /// Earlier wordings, newest first.
    pub revisions: Vec<i64>,
}

pub(crate) fn question_from_row(r: &SqliteRow) -> Result<Question> {
    let rubric: String = r.try_get("rubric")?;
    Ok(Question {
        id: r.try_get("id")?,
        topic_id: r.try_get("topic_id")?,
        level: r.try_get::<i64, _>("level")? as u8,
        prompt: r.try_get("prompt")?,
        answer: r.try_get("answer")?,
        rubric: serde_json::from_str(&rubric).map_err(|e| Error::Invalid(format!("stored rubric is invalid: {e}")))?,
        pass_threshold: r.try_get::<f64, _>("pass_threshold")? as f32,
        source_id: r.try_get("source_id")?,
        source_locator: r.try_get("source_locator")?,
        scope: Scope::parse(r.try_get("scope")?)?,
        project_id: r.try_get("project_id")?,
        status: QuestionStatus::parse(r.try_get("status")?)?,
        priority: r.try_get("priority")?,
        guide_version_id: r.try_get("guide_version_id")?,
        revision_of: r.try_get("revision_of")?,
        created_at: parse_ts(r.try_get("created_at")?)?,
        updated_at: parse_ts(r.try_get("updated_at")?)?,
    })
}

pub(crate) fn review_from_row(r: &SqliteRow) -> Result<ReviewRow> {
    let rubric_result: String = r.try_get("rubric_result")?;
    Ok(ReviewRow {
        id: r.try_get("id")?,
        question_id: r.try_get("question_id")?,
        session_id: r.try_get("session_id")?,
        reviewed_at: parse_ts(r.try_get("reviewed_at")?)?,
        grade: Grade::from_i64(r.try_get("grade")?)?,
        score: r.try_get::<f64, _>("score")? as f32,
        rubric_result: serde_json::from_str(&rubric_result).unwrap_or_default(),
        hints_used: r.try_get::<i64, _>("hints_used")? as u32,
        fluent: r.try_get::<i64, _>("fluent")? != 0,
        user_answer: r.try_get("user_answer")?,
        asked_prompt: r.try_get("asked_prompt")?,
        feedback: r.try_get("feedback")?,
        duration_ms: r.try_get("duration_ms")?,
    })
}

pub(crate) fn rating_from_row(r: &SqliteRow) -> Result<RatingRow> {
    Ok(RatingRow {
        id: r.try_get("id")?,
        question_id: r.try_get("question_id")?,
        review_id: r.try_get("review_id")?,
        rated_at: parse_ts(r.try_get("rated_at")?)?,
        usefulness: r.try_get::<i64, _>("usefulness")? as u8,
        feedback: r.try_get("feedback")?,
        classification: r
            .try_get::<Option<String>, _>("classification")?
            .as_deref()
            .map(RatingClass::parse)
            .transpose()?,
        processed_in_guide_version_id: r.try_get("processed_in_guide_version_id")?,
    })
}

pub async fn find(pool: &SqlitePool, id: i64) -> Result<Question> {
    let row = sqlx::query("SELECT * FROM questions WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(Error::NotFound { entity: "question", id })?;
    question_from_row(&row)
}

fn validate_content(level: u8, prompt: &str, answer: &str, rubric: &Rubric, pass_threshold: f32) -> Result<()> {
    validate_level(level)?;
    if prompt.trim().is_empty() {
        return Err(Error::Invalid("prompt must not be empty".into()));
    }
    if answer.trim().is_empty() {
        return Err(Error::Invalid("answer must not be empty".into()));
    }
    validate_rubric(rubric, pass_threshold)
}

pub fn forecast(new_active: u32, settings: &Settings) -> LoadForecast {
    LoadForecast {
        new_active,
        days_of_new_capacity: new_active.div_ceil(settings.new_per_day.max(1)),
        est_extra_reviews_per_day_first_month: new_active as f32 * REVIEWS_PER_NEW_FIRST_MONTH / 30.0,
    }
}

pub async fn add(
    pool: &SqlitePool,
    topic_id: i64,
    qs: Vec<NewQuestion>,
    status: QuestionStatus,
    settings: &Settings,
    now: DateTime<Utc>,
) -> Result<AddResult> {
    if !matches!(status, QuestionStatus::Active | QuestionStatus::Proposed) {
        return Err(Error::Invalid("new questions must be active or proposed".into()));
    }
    let topic = topics::find(pool, topic_id).await?;
    for (i, q) in qs.iter().enumerate() {
        validate_content(q.level, &q.prompt, &q.answer, &q.rubric, q.pass_threshold.unwrap_or(DEFAULT_PASS_THRESHOLD))
            .map_err(|e| match e {
                Error::Invalid(m) => Error::Invalid(format!("question {i}: {m}")),
                other => other,
            })?;
    }
    let stamp = ts(now);
    let mut tx = crate::db::begin_write(pool).await?;
    let mut next_priority: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(priority) + 1, 0) FROM questions WHERE topic_id = ?")
            .bind(topic_id)
            .fetch_one(&mut *tx)
            .await?;
    let mut ids = vec![];
    for q in qs {
        let priority = q.priority.unwrap_or_else(|| {
            next_priority += 1;
            next_priority - 1
        });
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO questions (topic_id, level, prompt, answer, rubric, pass_threshold, source_id, source_locator,
                 scope, project_id, status, priority, guide_version_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(topic_id)
        .bind(q.level as i64)
        .bind(q.prompt.trim())
        .bind(q.answer.trim())
        .bind(serde_json::to_string(&q.rubric).expect("rubric serializes"))
        .bind(q.pass_threshold.unwrap_or(DEFAULT_PASS_THRESHOLD) as f64)
        .bind(q.source_id)
        .bind(&q.source_locator)
        .bind(topic.scope.as_str())
        .bind(topic.project_id)
        .bind(status.as_str())
        .bind(priority)
        .bind(q.guide_version_id)
        .bind(&stamp)
        .bind(&stamp)
        .fetch_one(&mut *tx)
        .await?;
        for t in q.secondary_topic_ids {
            sqlx::query("INSERT OR IGNORE INTO question_topics (question_id, topic_id) VALUES (?, ?)")
                .bind(id)
                .bind(t)
                .execute(&mut *tx)
                .await?;
        }
        ids.push(id);
    }
    tx.commit().await?;
    let new_active = if status == QuestionStatus::Active { ids.len() as u32 } else { 0 };
    Ok(AddResult { ids, forecast: forecast(new_active, settings) })
}

pub async fn update(pool: &SqlitePool, id: i64, patch: QuestionPatch, now: DateTime<Utc>) -> Result<Question> {
    let old = find(pool, id).await?;
    if old.status == QuestionStatus::Retired {
        return Err(Error::Conflict(format!("question {id} is retired; update its newest revision instead")));
    }
    if let Some(t) = patch.topic_id {
        topics::find(pool, t).await?;
    }
    let rewording = patch.prompt.is_some()
        || patch.answer.is_some()
        || patch.rubric.is_some()
        || patch.pass_threshold.is_some()
        || patch.level.is_some();
    let mut q = old.clone();
    q.prompt = patch.prompt.unwrap_or(q.prompt);
    q.answer = patch.answer.unwrap_or(q.answer);
    q.rubric = patch.rubric.unwrap_or(q.rubric);
    q.pass_threshold = patch.pass_threshold.unwrap_or(q.pass_threshold);
    q.level = patch.level.unwrap_or(q.level);
    q.priority = patch.priority.unwrap_or(q.priority);
    q.topic_id = patch.topic_id.unwrap_or(q.topic_id);
    q.source_id = patch.source_id.or(q.source_id);
    q.source_locator = patch.source_locator.or(q.source_locator);
    validate_content(q.level, &q.prompt, &q.answer, &q.rubric, q.pass_threshold)?;
    let stamp = ts(now);
    let mut tx = crate::db::begin_write(pool).await?;
    let new_id = if rewording {
        let new_id: i64 = sqlx::query_scalar(
            "INSERT INTO questions (topic_id, level, prompt, answer, rubric, pass_threshold, source_id, source_locator,
                 scope, project_id, status, priority, guide_version_id, revision_of, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(q.topic_id)
        .bind(q.level as i64)
        .bind(q.prompt.trim())
        .bind(q.answer.trim())
        .bind(serde_json::to_string(&q.rubric).expect("rubric serializes"))
        .bind(q.pass_threshold as f64)
        .bind(q.source_id)
        .bind(&q.source_locator)
        .bind(q.scope.as_str())
        .bind(q.project_id)
        .bind(q.status.as_str())
        .bind(q.priority)
        .bind(q.guide_version_id)
        .bind(id)
        .bind(&stamp)
        .bind(&stamp)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("UPDATE questions SET status = 'retired', updated_at = ? WHERE id = ?")
            .bind(&stamp)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let mut state = scheduler::load_state_conn(&mut tx, id).await?;
        if state.reps > 0 {
            // Keep the schedule but give the new wording a fresh lapse count (leech detection).
            state.question_id = new_id;
            state.lapses = 0;
            scheduler::save_state(&mut tx, &state).await?;
        }
        sqlx::query("INSERT INTO question_topics (question_id, topic_id) SELECT ?, topic_id FROM question_topics WHERE question_id = ?")
            .bind(new_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        new_id
    } else {
        sqlx::query("UPDATE questions SET priority = ?, topic_id = ?, source_id = ?, source_locator = ?, updated_at = ? WHERE id = ?")
            .bind(q.priority)
            .bind(q.topic_id)
            .bind(q.source_id)
            .bind(&q.source_locator)
            .bind(&stamp)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        id
    };
    tx.commit().await?;
    find(pool, new_id).await
}

pub async fn set_status(pool: &SqlitePool, id: i64, status: QuestionStatus, now: DateTime<Utc>) -> Result<()> {
    find(pool, id).await?;
    sqlx::query("UPDATE questions SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status.as_str())
        .bind(ts(now))
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn link(
    pool: &SqlitePool,
    a: i64,
    b: i64,
    kind: LinkKind,
    weight: f32,
    note: String,
    now: DateTime<Utc>,
) -> Result<()> {
    validate_weight(weight)?;
    if a == b {
        return Err(Error::Invalid("a question cannot link to itself".into()));
    }
    find(pool, a).await?;
    find(pool, b).await?;
    let (a, b) = if kind == LinkKind::Related && a > b { (b, a) } else { (a, b) };
    sqlx::query(
        "INSERT INTO question_links (from_question_id, to_question_id, kind, weight, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT (from_question_id, to_question_id, kind) DO UPDATE SET weight = ?4, note = ?5, updated_at = ?6",
    )
    .bind(a)
    .bind(b)
    .bind(kind.as_str())
    .bind(weight as f64)
    .bind(note)
    .bind(ts(now))
    .execute(pool)
    .await?;
    Ok(())
}

/// Ids of the earlier wordings `id` revises, newest first.
pub(crate) async fn ancestors(pool: &SqlitePool, id: i64) -> Result<Vec<i64>> {
    let mut out = vec![];
    let mut cursor: Option<i64> = sqlx::query_scalar("SELECT revision_of FROM questions WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .flatten();
    while let Some(p) = cursor {
        if out.contains(&p) || p == id {
            break;
        }
        out.push(p);
        cursor = sqlx::query_scalar("SELECT revision_of FROM questions WHERE id = ?")
            .bind(p)
            .fetch_optional(pool)
            .await?
            .flatten();
    }
    Ok(out)
}

pub async fn get(pool: &SqlitePool, id: i64) -> Result<QuestionDetail> {
    let question = find(pool, id).await?;
    let topic_path = topics::get(pool, question.topic_id).await?.path.into_iter().map(|t| t.name).collect();
    let secondary_topic_ids =
        sqlx::query_scalar("SELECT topic_id FROM question_topics WHERE question_id = ? ORDER BY topic_id")
            .bind(id)
            .fetch_all(pool)
            .await?;
    let revisions = ancestors(pool, id).await?;
    let mut lineage = revisions.clone();
    lineage.push(id);
    let mut reviews = vec![];
    let mut ratings = vec![];
    for q in &lineage {
        for row in sqlx::query("SELECT * FROM reviews WHERE question_id = ?").bind(q).fetch_all(pool).await? {
            reviews.push(review_from_row(&row)?);
        }
        for row in sqlx::query("SELECT * FROM question_ratings WHERE question_id = ?").bind(q).fetch_all(pool).await? {
            ratings.push(rating_from_row(&row)?);
        }
    }
    reviews.sort_by_key(|r| (r.reviewed_at, r.id));
    ratings.sort_by_key(|r| (r.rated_at, r.id));
    Ok(QuestionDetail {
        state: scheduler::load_state(pool, id).await?,
        question,
        topic_path,
        secondary_topic_ids,
        reviews,
        ratings,
        revisions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::grading::{Grade, RubricPoint};
    use crate::scheduler::{self, SchedParams};
    use crate::testutil::{self, day, utc};
    use crate::Error;

    fn now() -> DateTime<Utc> {
        utc("2026-10-23T18:00:00Z")
    }
    fn nq(prompt: &str) -> NewQuestion {
        NewQuestion {
            level: 0,
            prompt: prompt.into(),
            answer: "answer".into(),
            rubric: Rubric(vec![RubricPoint { id: "r1".into(), point: "p".into(), weight: 1.0, essential: false }]),
            pass_threshold: None,
            source_id: None,
            source_locator: None,
            priority: None,
            guide_version_id: None,
            secondary_topic_ids: vec![],
        }
    }
    async fn setup() -> (SqlitePool, Settings, i64) {
        let p = test_pool().await;
        let s = Settings::load(&p).await.unwrap();
        let t = testutil::topic(&p, None, "T").await;
        (p, s, t)
    }

    #[tokio::test]
    async fn add_batch_assigns_priorities_and_forecast() {
        let (p, s, t) = setup().await;
        let r = add(&p, t, (0..5).map(|i| nq(&format!("q{i}"))).collect(), QuestionStatus::Active, &s, now()).await.unwrap();
        assert_eq!(r.ids.len(), 5);
        assert_eq!(r.forecast.new_active, 5);
        assert_eq!(r.forecast.days_of_new_capacity, 2);
        assert!((r.forecast.est_extra_reviews_per_day_first_month - 5.0 * 8.0 / 30.0).abs() < 1e-4);
        let more = add(&p, t, vec![nq("q5")], QuestionStatus::Active, &s, now()).await.unwrap();
        let q0 = get(&p, r.ids[0]).await.unwrap().question;
        let q4 = get(&p, r.ids[4]).await.unwrap().question;
        let q5 = get(&p, more.ids[0]).await.unwrap().question;
        assert!(q0.priority < q4.priority && q4.priority < q5.priority);
        assert_eq!(q0.pass_threshold, 0.6);
    }

    #[tokio::test]
    async fn add_proposed_not_counted_in_forecast() {
        let (p, s, t) = setup().await;
        let r = add(&p, t, vec![nq("a"), nq("b")], QuestionStatus::Proposed, &s, now()).await.unwrap();
        assert_eq!(r.forecast.new_active, 0);
        assert_eq!(get(&p, r.ids[0]).await.unwrap().question.status, QuestionStatus::Proposed);
        assert!(matches!(add(&p, t, vec![nq("c")], QuestionStatus::Retired, &s, now()).await, Err(Error::Invalid(_))));
    }

    #[tokio::test]
    async fn add_invalid_question_rolls_back_all() {
        let (p, s, t) = setup().await;
        let mut bad = nq("b");
        bad.answer = " ".into();
        let err = add(&p, t, vec![nq("a"), bad, nq("c")], QuestionStatus::Active, &s, now()).await.unwrap_err();
        assert!(matches!(&err, Error::Invalid(m) if m.contains("question 1")), "{err}");
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM questions").fetch_one(&p).await.unwrap();
        assert_eq!(n, 0);
        let mut lvl = nq("x");
        lvl.level = 4;
        assert!(add(&p, t, vec![lvl], QuestionStatus::Active, &s, now()).await.is_err());
        assert!(matches!(add(&p, 999, vec![nq("x")], QuestionStatus::Active, &s, now()).await, Err(Error::NotFound { .. })));
    }

    #[tokio::test]
    async fn add_inherits_topic_scope_and_secondary_topics() {
        let (p, s, t) = setup().await;
        let other = testutil::topic(&p, None, "Other").await;
        let mut q = nq("a");
        q.secondary_topic_ids = vec![other];
        let r = add(&p, t, vec![q], QuestionStatus::Active, &s, now()).await.unwrap();
        let d = get(&p, r.ids[0]).await.unwrap();
        assert_eq!(d.question.scope, Scope::Main);
        assert_eq!(d.secondary_topic_ids, vec![other]);
    }

    #[tokio::test]
    async fn rewording_creates_revision_and_carries_state() {
        let (p, s, t) = setup().await;
        let id = add(&p, t, vec![nq("old")], QuestionStatus::Active, &s, now()).await.unwrap().ids[0];
        let st = scheduler::next(&scheduler::QState::new(id), Grade::Good, day("2026-10-23"), now(), &SchedParams::main(&s)).unwrap();
        scheduler::save_state(&mut p.acquire().await.unwrap(), &st).await.unwrap();
        let new = update(&p, id, QuestionPatch { prompt: Some("new".into()), ..Default::default() }, now()).await.unwrap();
        assert_ne!(new.id, id);
        assert_eq!((new.prompt.as_str(), new.revision_of, new.status), ("new", Some(id), QuestionStatus::Active));
        assert_eq!(get(&p, id).await.unwrap().question.status, QuestionStatus::Retired);
        let carried = scheduler::load_state(&p, new.id).await.unwrap();
        assert_eq!((carried.due_on, carried.reps), (st.due_on, 1));
        assert!(matches!(update(&p, id, QuestionPatch { prompt: Some("again".into()), ..Default::default() }, now()).await, Err(Error::Conflict(_))));
    }

    #[tokio::test]
    async fn rebuild_carries_revision_state() {
        let (p, s, t) = setup().await;
        let id = add(&p, t, vec![nq("old")], QuestionStatus::Active, &s, now()).await.unwrap().ids[0];
        let sid = testutil::session(&p, "2026-10-23").await;
        testutil::review_row(&p, id, sid, "2026-10-23T18:00:00Z", 3).await;
        scheduler::rebuild_all(&p, &s).await.unwrap();
        let new = update(&p, id, QuestionPatch { answer: Some("better".into()), ..Default::default() }, now()).await.unwrap();
        let before = scheduler::load_state(&p, new.id).await.unwrap();
        scheduler::rebuild_all(&p, &s).await.unwrap();
        assert_eq!(scheduler::load_state(&p, new.id).await.unwrap(), before);
    }

    #[tokio::test]
    async fn revision_resets_lapses_now_and_after_rebuild() {
        let (p, s, t) = setup().await;
        let id = add(&p, t, vec![nq("old")], QuestionStatus::Active, &s, now()).await.unwrap().ids[0];
        for d in 1..=6 {
            let sid = testutil::session(&p, &format!("2026-10-{d:02}")).await;
            testutil::review_row(&p, id, sid, &format!("2026-10-{d:02}T18:00:00Z"), 1).await;
        }
        scheduler::rebuild_all(&p, &s).await.unwrap();
        assert_eq!(scheduler::load_state(&p, id).await.unwrap().lapses, 6);
        let new = update(&p, id, QuestionPatch { prompt: Some("simpler".into()), ..Default::default() }, now()).await.unwrap();
        let carried = scheduler::load_state(&p, new.id).await.unwrap();
        assert_eq!((carried.lapses, carried.reps), (0, 6));
        scheduler::rebuild_all(&p, &s).await.unwrap();
        assert_eq!(scheduler::load_state(&p, new.id).await.unwrap(), carried);
    }

    #[tokio::test]
    async fn priority_change_updates_in_place() {
        let (p, s, t) = setup().await;
        let id = add(&p, t, vec![nq("a")], QuestionStatus::Active, &s, now()).await.unwrap().ids[0];
        let q = update(&p, id, QuestionPatch { priority: Some(-5), ..Default::default() }, now()).await.unwrap();
        assert_eq!((q.id, q.priority), (id, -5));
    }

    #[tokio::test]
    async fn get_returns_history_and_revision_chain() {
        let (p, s, t) = setup().await;
        let id = add(&p, t, vec![nq("v1")], QuestionStatus::Active, &s, now()).await.unwrap().ids[0];
        let sid = testutil::session(&p, "2026-10-23").await;
        testutil::review_row(&p, id, sid, "2026-10-23T18:00:00Z", 3).await;
        let v2 = update(&p, id, QuestionPatch { prompt: Some("v2".into()), ..Default::default() }, now()).await.unwrap();
        let v3 = update(&p, v2.id, QuestionPatch { prompt: Some("v3".into()), ..Default::default() }, now()).await.unwrap();
        testutil::review_row(&p, v3.id, sid, "2026-10-24T18:00:00Z", 4).await;
        let d = get(&p, v3.id).await.unwrap();
        assert_eq!(d.revisions, vec![v2.id, id]);
        assert_eq!(d.reviews.len(), 2);
        assert_eq!(d.reviews[0].question_id, id);
        assert_eq!(d.reviews[1].grade, Grade::Easy);
        assert_eq!(d.topic_path, vec!["T".to_string()]);
    }

    #[tokio::test]
    async fn set_status_and_links() {
        let (p, s, t) = setup().await;
        let ids = add(&p, t, vec![nq("a"), nq("b")], QuestionStatus::Active, &s, now()).await.unwrap().ids;
        set_status(&p, ids[0], QuestionStatus::Suspended, now()).await.unwrap();
        assert_eq!(get(&p, ids[0]).await.unwrap().question.status, QuestionStatus::Suspended);
        link(&p, ids[1], ids[0], LinkKind::Related, 0.7, String::new(), now()).await.unwrap();
        link(&p, ids[0], ids[1], LinkKind::Related, 0.8, String::new(), now()).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM question_links").fetch_one(&p).await.unwrap();
        assert_eq!(n, 1);
        assert!(link(&p, ids[0], ids[0], LinkKind::Related, 0.5, String::new(), now()).await.is_err());
    }
}

#[cfg(test)]
mod concurrency_tests {
    use super::*;
    use crate::grading::RubricPoint;

    #[tokio::test]
    async fn concurrent_read_then_write_transactions_do_not_fail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.db");
        let a = crate::db::open(&path).await.unwrap();
        let b = crate::db::open(&path).await.unwrap();
        let s = Settings::load(&a).await.unwrap();
        let t = crate::testutil::topic(&a, None, "T").await;
        let q = || NewQuestion {
            level: 0, prompt: "p".into(), answer: "a".into(),
            rubric: Rubric(vec![RubricPoint { id: "r1".into(), point: "x".into(), weight: 1.0, essential: false }]),
            pass_threshold: None, source_id: None, source_locator: None, priority: None,
            guide_version_id: None, secondary_topic_ids: vec![],
        };
        let work = |pool: SqlitePool, s: Settings| async move {
            for _ in 0..40 {
                add(&pool, t, vec![q(), q()], QuestionStatus::Active, &s, Utc::now()).await?;
            }
            Ok::<_, Error>(())
        };
        let (ra, rb) = tokio::join!(tokio::spawn(work(a.clone(), s.clone())), tokio::spawn(work(b, s)));
        ra.unwrap().unwrap();
        rb.unwrap().unwrap();
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM questions").fetch_one(&a).await.unwrap();
        assert_eq!(n, 160);
    }
}
