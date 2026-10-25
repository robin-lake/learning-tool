use chrono::{DateTime, Duration, NaiveDate, Utc};
use fsrs::{FSRS, MemoryState};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::grading::Grade;
use crate::settings::Settings;
use crate::types::{parse_day, parse_ts, str_enum, ts};
use crate::{Error, Result};

str_enum!(CardState { New => "new", Review => "review", Relearning => "relearning" });

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct MemState {
    pub stability: f32,
    pub difficulty: f32,
}

/// Scheduling state of one question, derived from its reviews.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct QState {
    pub question_id: i64,
    pub mem: Option<MemState>,
    pub due_on: Option<NaiveDate>,
    pub last_reviewed_at: Option<DateTime<Utc>>,
    pub last_study_day: Option<NaiveDate>,
    pub reps: u32,
    pub lapses: u32,
    pub state: CardState,
}

impl QState {
    pub fn new(question_id: i64) -> QState {
        QState {
            question_id,
            mem: None,
            due_on: None,
            last_reviewed_at: None,
            last_study_day: None,
            reps: 0,
            lapses: 0,
            state: CardState::New,
        }
    }

    /// Current recall probability, or `None` if never reviewed.
    pub fn retrievability(&self, now: DateTime<Utc>) -> Option<f32> {
        let (mem, last) = (self.mem?, self.last_reviewed_at?);
        let days = (now - last).num_seconds().max(0) as f32 / 86_400.0;
        Some(retrievability(mem.stability, days))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SchedParams {
    pub desired_retention: f32,
    pub max_interval_days: Option<u32>,
}

impl SchedParams {
    pub fn main(settings: &Settings) -> SchedParams {
        SchedParams { desired_retention: settings.desired_retention_main, max_interval_days: None }
    }
}

/// FSRS-6 forgetting curve with the default decay parameter.
pub fn retrievability(stability: f32, days_elapsed: f32) -> f32 {
    let decay = fsrs::DEFAULT_PARAMETERS[20];
    fsrs::current_retrievability(MemoryState { stability, difficulty: 5.0 }, days_elapsed, decay)
}

/// The state after reviewing with `grade` on `study_day`.
pub fn next(
    prev: &QState,
    grade: Grade,
    study_day: NaiveDate,
    reviewed_at: DateTime<Utc>,
    p: &SchedParams,
) -> Result<QState> {
    next_with(&FSRS::default(), prev, grade, study_day, reviewed_at, p)
}

/// `next` with a caller-provided model, for replaying many reviews (building the model isn't free).
pub fn next_with(
    fsrs: &FSRS,
    prev: &QState,
    grade: Grade,
    study_day: NaiveDate,
    reviewed_at: DateTime<Utc>,
    p: &SchedParams,
) -> Result<QState> {
    let days_elapsed = prev
        .last_study_day
        .map(|last| (study_day - last).num_days().max(0) as u32)
        .unwrap_or(0);
    let mem = prev.mem.map(|m| MemoryState { stability: m.stability, difficulty: m.difficulty });
    let states = fsrs
        .next_states(mem, p.desired_retention, days_elapsed)
        .map_err(|e| Error::Fsrs(e.to_string()))?;
    let item = match grade {
        Grade::Again => states.again,
        Grade::Hard => states.hard,
        Grade::Good => states.good,
        Grade::Easy => states.easy,
    };
    let mut interval = (item.interval.round() as i64).max(1);
    if let Some(cap) = p.max_interval_days {
        interval = interval.min(cap.max(1) as i64);
    }
    // A failed review is a lapse, except a re-ask later on the same study day.
    let (state, lapses) = match grade {
        Grade::Again if prev.last_study_day == Some(study_day) => (CardState::Relearning, prev.lapses),
        Grade::Again => (CardState::Relearning, prev.lapses + 1),
        _ => (CardState::Review, prev.lapses),
    };
    Ok(QState {
        question_id: prev.question_id,
        mem: Some(MemState { stability: item.memory.stability, difficulty: item.memory.difficulty }),
        due_on: Some(study_day + Duration::days(interval)),
        last_reviewed_at: Some(reviewed_at),
        last_study_day: Some(study_day),
        reps: prev.reps + 1,
        lapses,
        state,
    })
}

fn state_from_row(r: &SqliteRow) -> Result<QState> {
    let stability: Option<f64> = r.try_get("stability")?;
    let difficulty: Option<f64> = r.try_get("difficulty")?;
    let opt_day = |s: Option<String>| s.as_deref().map(parse_day).transpose();
    Ok(QState {
        question_id: r.try_get("question_id")?,
        mem: stability.zip(difficulty).map(|(s, d)| MemState { stability: s as f32, difficulty: d as f32 }),
        due_on: opt_day(r.try_get("due_on")?)?,
        last_reviewed_at: r.try_get::<Option<String>, _>("last_reviewed_at")?.as_deref().map(parse_ts).transpose()?,
        last_study_day: opt_day(r.try_get("last_study_day")?)?,
        reps: r.try_get::<i64, _>("reps")? as u32,
        lapses: r.try_get::<i64, _>("lapses")? as u32,
        state: CardState::parse(r.try_get("state")?)?,
    })
}

pub async fn load_state(pool: &SqlitePool, question_id: i64) -> Result<QState> {
    let mut conn = pool.acquire().await?;
    load_state_conn(&mut conn, question_id).await
}

pub async fn load_state_conn(conn: &mut SqliteConnection, question_id: i64) -> Result<QState> {
    match sqlx::query("SELECT * FROM question_state WHERE question_id = ?")
        .bind(question_id)
        .fetch_optional(&mut *conn)
        .await?
    {
        Some(row) => state_from_row(&row),
        None => Ok(QState::new(question_id)),
    }
}

pub async fn save_state(conn: &mut SqliteConnection, s: &QState) -> Result<()> {
    sqlx::query(
        "INSERT INTO question_state (question_id, stability, difficulty, due_on, last_reviewed_at, last_study_day, reps, lapses, state)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT (question_id) DO UPDATE SET stability = ?2, difficulty = ?3, due_on = ?4, last_reviewed_at = ?5,
             last_study_day = ?6, reps = ?7, lapses = ?8, state = ?9",
    )
    .bind(s.question_id)
    .bind(s.mem.map(|m| m.stability as f64))
    .bind(s.mem.map(|m| m.difficulty as f64))
    .bind(s.due_on.map(|d| d.to_string()))
    .bind(s.last_reviewed_at.map(ts))
    .bind(s.last_study_day.map(|d| d.to_string()))
    .bind(s.reps as i64)
    .bind(s.lapses as i64)
    .bind(s.state.as_str())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Recomputes `question_state` from the review log. A question's history includes the reviews of
/// the questions it revises (its `revision_of` chain), so rewording keeps the schedule.
/// Returns the number of questions with state.
pub async fn rebuild_all(pool: &SqlitePool, settings: &Settings) -> Result<u32> {
    let params = SchedParams::main(settings);
    let mut tx = crate::db::begin_write(pool).await?;
    let questions: Vec<(i64, Option<i64>)> =
        sqlx::query_as("SELECT id, revision_of FROM questions").fetch_all(&mut *tx).await?;
    let parent = |id: i64| questions.iter().find(|(q, _)| *q == id).and_then(|(_, p)| *p);
    let reviews: Vec<(i64, String, i64, String)> = sqlx::query_as(
        "SELECT r.question_id, r.reviewed_at, r.grade, s.study_day
         FROM reviews r JOIN sessions s ON s.id = r.session_id
         ORDER BY r.reviewed_at, r.id",
    )
    .fetch_all(&mut *tx)
    .await?;

        sqlx::query("DELETE FROM question_state").execute(&mut *tx).await?;
    let mut rebuilt = 0;
    for &(qid, _) in &questions {
        let mut lineage = vec![qid];
        while let Some(p) = parent(*lineage.last().unwrap()) {
            if lineage.contains(&p) {
                break;
            }
            lineage.push(p);
        }
        let mut state = QState::new(qid);
        let mut any = false;
        let mut own = false;
        for (rq, at, grade, day) in reviews.iter().filter(|r| lineage.contains(&r.0)) {
            // Lapses belong to a wording: a revision starts its count fresh.
            if *rq == qid && !own {
                own = true;
                state.lapses = 0;
            }
            state = next(&state, Grade::from_i64(*grade)?, parse_day(day)?, parse_ts(at)?, &params)?;
            any = true;
        }
        if !own {
            state.lapses = if lineage.len() > 1 { 0 } else { state.lapses };
        }
        if any {
            save_state(&mut tx, &state).await?;
            rebuilt += 1;
        }
    }
    tx.commit().await?;
    Ok(rebuilt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::settings::Settings;
    use crate::testutil::{self, day, utc};

    fn main_params() -> SchedParams {
        SchedParams { desired_retention: 0.9, max_interval_days: None }
    }
    fn fresh(id: i64) -> QState {
        QState::new(id)
    }

    #[test]
    fn retrievability_is_point_nine_at_stability() {
        assert!((retrievability(10.0, 10.0) - 0.9).abs() < 1e-4);
    }

    #[test]
    fn retrievability_decreases_with_time() {
        assert!(retrievability(10.0, 1.0) > retrievability(10.0, 5.0));
        assert_eq!(retrievability(10.0, 0.0), 1.0);
    }

    #[test]
    fn first_good_schedules_future_due() {
        let s = next(&fresh(1), Grade::Good, day("2026-10-23"), utc("2026-10-23T18:00:00Z"), &main_params()).unwrap();
        assert!(s.due_on.unwrap() > day("2026-10-23"));
        assert_eq!((s.reps, s.lapses, s.state), (1, 0, CardState::Review));
        assert!(s.mem.is_some());
        assert_eq!(s.last_study_day, Some(day("2026-10-23")));
    }

    #[test]
    fn again_on_review_is_lapse() {
        let p = main_params();
        let s = next(&fresh(1), Grade::Good, day("2026-10-01"), utc("2026-10-01T18:00:00Z"), &p).unwrap();
        let s = next(&s, Grade::Again, day("2026-10-10"), utc("2026-10-10T18:00:00Z"), &p).unwrap();
        assert_eq!((s.lapses, s.state), (1, CardState::Relearning));
        assert_eq!(s.due_on, Some(day("2026-10-11")));
    }

    #[test]
    fn same_day_again_is_not_another_lapse() {
        let p = main_params();
        let d = day("2026-10-01");
        let s = next(&fresh(1), Grade::Again, d, utc("2026-10-01T18:00:00Z"), &p).unwrap();
        assert_eq!((s.lapses, s.state), (1, CardState::Relearning));
        let s = next(&s, Grade::Again, d, utc("2026-10-01T18:10:00Z"), &p).unwrap();
        assert_eq!(s.lapses, 1);
    }

    #[test]
    fn failing_every_day_from_new_becomes_a_leech() {
        let p = main_params();
        let mut s = fresh(1);
        for d in 1..=6 {
            let day_ = day(&format!("2026-10-{d:02}"));
            s = next(&s, Grade::Again, day_, utc(&format!("2026-10-{d:02}T18:00:00Z")), &p).unwrap();
        }
        assert_eq!(s.lapses, 6);
    }

    #[test]
    fn easy_interval_ge_good_interval() {
        let p = main_params();
        let d = day("2026-10-01");
        let at = utc("2026-10-01T18:00:00Z");
        let good = next(&fresh(1), Grade::Good, d, at, &p).unwrap();
        let easy = next(&fresh(1), Grade::Easy, d, at, &p).unwrap();
        assert!(easy.due_on >= good.due_on);
    }

    #[test]
    fn max_interval_caps() {
        let p = SchedParams { desired_retention: 0.9, max_interval_days: Some(2) };
        let s = next(&fresh(1), Grade::Easy, day("2026-10-01"), utc("2026-10-01T18:00:00Z"), &p).unwrap();
        assert!(s.due_on.unwrap() <= day("2026-10-03"));
    }

    #[test]
    fn same_day_review_uses_zero_elapsed() {
        let p = main_params();
        let d = day("2026-10-01");
        let s = next(&fresh(1), Grade::Again, d, utc("2026-10-01T18:00:00Z"), &p).unwrap();
        let s = next(&s, Grade::Good, d, utc("2026-10-01T18:10:00Z"), &p).unwrap();
        assert_eq!(s.reps, 2);
    }

    #[tokio::test]
    async fn load_state_defaults_to_new() {
        let pool = test_pool().await;
        let s = load_state(&pool, 7).await.unwrap();
        assert_eq!((s.question_id, s.state, s.reps), (7, CardState::New, 0));
    }

    #[tokio::test]
    async fn rebuild_matches_incremental() {
        let pool = test_pool().await;
        let settings = Settings::load(&pool).await.unwrap();
        let t = testutil::topic(&pool, None, "T").await;
        let qs = [testutil::question(&pool, t).await, testutil::question(&pool, t).await, testutil::question(&pool, t).await];
        let plan = [
            (qs[0], "2026-10-01", "2026-10-01T18:00:00Z", Grade::Good),
            (qs[1], "2026-10-01", "2026-10-01T18:01:00Z", Grade::Again),
            (qs[1], "2026-10-01", "2026-10-01T18:05:00Z", Grade::Hard),
            (qs[2], "2026-10-02", "2026-10-02T18:00:00Z", Grade::Easy),
            (qs[0], "2026-10-05", "2026-10-05T18:00:00Z", Grade::Again),
            (qs[0], "2026-10-06", "2026-10-06T18:00:00Z", Grade::Good),
        ];
        for (q, d, at, g) in plan {
            let sid = testutil::session(&pool, d).await;
            testutil::review_row(&pool, q, sid, at, g as i64).await;
            let prev = load_state(&pool, q).await.unwrap();
            let s = next(&prev, g, day(d), utc(at), &main_params()).unwrap();
            let mut conn = pool.acquire().await.unwrap();
            save_state(&mut conn, &s).await.unwrap();
        }
        let snapshot = |pool: SqlitePool| async move {
            let mut out = vec![];
            for q in qs {
                out.push(load_state(&pool, q).await.unwrap());
            }
            out
        };
        let before = snapshot(pool.clone()).await;
        assert_eq!(rebuild_all(&pool, &settings).await.unwrap(), 3);
        assert_eq!(snapshot(pool.clone()).await, before);
    }

    #[tokio::test]
    async fn tree_avg_retrievability_present_after_review() {
        let pool = test_pool().await;
        let parent = testutil::topic(&pool, None, "P").await;
        let child = testutil::topic(&pool, Some(parent), "C").await;
        let q = testutil::question(&pool, child).await;
        let before = crate::topics::tree(&pool, None, None, utc("2026-10-02T18:00:00Z")).await.unwrap();
        assert_eq!(before[0].avg_retrievability, None);
        let s = next(&QState::new(q), Grade::Good, day("2026-10-01"), utc("2026-10-01T18:00:00Z"), &main_params()).unwrap();
        save_state(&mut pool.acquire().await.unwrap(), &s).await.unwrap();
        let after = crate::topics::tree(&pool, None, None, utc("2026-10-02T18:00:00Z")).await.unwrap();
        let r = after[0].avg_retrievability.unwrap();
        assert!(r > 0.5 && r < 1.0, "{r}");
        assert_eq!(after[0].children[0].avg_retrievability, Some(r));
    }
}
