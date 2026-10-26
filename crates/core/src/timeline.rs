//! The knowledge timeline behind the map: per-topic and per-question state for every study day,
//! replayed from the review and status logs with the same scheduler `/study` uses.

use std::collections::HashMap;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::Result;
use crate::grading::Grade;
use crate::scheduler::{self, QState, SchedParams, retrievability};
use crate::settings::Settings;
use crate::status_log::{self, Entity, StatusEvent};
use crate::types::{parse_day, parse_ts, str_enum};

str_enum!(NodeKind { Topic => "topic", Question => "question" });

/// One value per entry of `Timeline::days`.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, Serialize, JsonSchema, PartialEq)]
pub struct Series {
    /// Questions existing (a topic's whole subtree; 0 or 1 for a question).
    pub questions: Vec<u32>,
    pub studied: Vec<u32>,
    /// Mean recall chance over studied, existing questions.
    pub recall: Vec<Option<f32>>,
    /// Mean stability in days over studied, existing questions.
    pub durability: Vec<Option<f32>>,
    /// Mean of each question's most recent rubric score.
    pub last_score: Vec<Option<f32>>,
    /// Days since the most recent review in the subtree.
    pub days_since: Vec<Option<u32>>,
    pub due: Vec<u32>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, Serialize, JsonSchema, PartialEq)]
pub struct Snapshot {
    pub questions: u32,
    pub studied: u32,
    pub recall: Option<f32>,
    pub durability: Option<f32>,
    pub last_score: Option<f32>,
    pub days_since: Option<u32>,
    pub due: u32,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct TimelineNode {
    /// "t12" for a topic, "q34" for a question.
    pub id: String,
    pub kind: NodeKind,
    pub ref_id: i64,
    pub parent: Option<String>,
    pub label: String,
    pub target_level: Option<u8>,
    pub created_on: NaiveDate,
    pub series: Series,
    /// State right now (equal to the last series entry when there is history).
    pub now: Snapshot,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct Timeline {
    /// Every calendar day from the first study day to today; empty before any review.
    pub days: Vec<NaiveDate>,
    pub today: NaiveDate,
    pub stale_after_days: u32,
    pub stale_full_days: u32,
    pub nodes: Vec<TimelineNode>,
}

/// One question's state at the end of one day.
#[derive(Clone, Copy, Default)]
struct Point {
    exists: bool,
    studied: bool,
    recall: Option<f32>,
    durability: Option<f32>,
    last_score: Option<f32>,
    days_since: Option<u32>,
    due: bool,
}

struct Review {
    day: NaiveDate,
    at: DateTime<Utc>,
    grade: Grade,
    score: f32,
}

struct QuestionChain {
    head: i64,
    topic: i64,
    label: String,
    created_on: NaiveDate,
    status: String,
    reviews: Vec<Review>,
}

/// Status on a past instant: the last event at or before it, else the first event's `from`, else now.
fn status_at<'a>(events: &[&'a StatusEvent], current: &'a str, instant: DateTime<Utc>) -> &'a str {
    if events.is_empty() {
        return current;
    }
    match events.iter().rev().find(|e| e.at <= instant) {
        Some(e) => &e.to_status,
        None => &events[0].from_status,
    }
}

/// Replays one chain's reviews and samples it at each day's end (and at `now` for today).
fn replay(
    fsrs: &fsrs::FSRS,
    chain: &QuestionChain,
    events: &[&StatusEvent],
    days: &[NaiveDate],
    instants: &[DateTime<Utc>],
    params: &SchedParams,
) -> Result<Vec<Point>> {
    let mut state = QState::new(chain.head);
    let (mut next, mut last_score) = (0, None);
    let mut points = Vec::with_capacity(days.len());
    for (&day, &instant) in days.iter().zip(instants) {
        while next < chain.reviews.len() && chain.reviews[next].day <= day {
            let r = &chain.reviews[next];
            state = scheduler::next_with(fsrs, &state, r.grade, r.day, r.at, params)?;
            last_score = Some(r.score);
            next += 1;
        }
        let status = status_at(events, &chain.status, instant);
        let exists = chain.created_on <= day && (status == "active" || status == "suspended");
        let studied = exists && state.reps > 0;
        let recall = match (studied, state.mem, state.last_reviewed_at) {
            (true, Some(mem), Some(last)) => {
                let elapsed = (instant - last).num_seconds().max(0) as f32 / 86_400.0;
                Some(retrievability(mem.stability, elapsed))
            }
            _ => None,
        };
        points.push(Point {
            exists,
            studied,
            recall,
            durability: if studied { state.mem.map(|m| m.stability) } else { None },
            last_score: if studied { last_score } else { None },
            days_since: if studied { state.last_study_day.map(|l| (day - l).num_days().max(0) as u32) } else { None },
            due: exists && status == "active" && state.due_on.is_some_and(|d| d <= day),
        });
    }
    Ok(points)
}

/// Totals and means over the given questions' points (means over studied ones).
fn aggregate<'a>(points: impl Iterator<Item = &'a Point>) -> Snapshot {
    let mut s = Snapshot::default();
    let (mut recall, mut durability, mut score) = ((0.0, 0), (0.0, 0), (0.0, 0));
    let add = |acc: &mut (f32, u32), v: Option<f32>| {
        if let Some(v) = v {
            acc.0 += v;
            acc.1 += 1;
        }
    };
    for p in points {
        s.questions += p.exists as u32;
        s.due += p.due as u32;
        if p.studied {
            s.studied += 1;
            add(&mut recall, p.recall);
            add(&mut durability, p.durability);
            add(&mut score, p.last_score);
            s.days_since = match (s.days_since, p.days_since) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
        }
    }
    // Three decimal places: the series are sent for every node and day, and JSON prints f32s at full length.
    let avg = |(sum, n): (f32, u32)| (n > 0).then(|| (sum / n as f32 * 1000.0).round() / 1000.0);
    s.recall = avg(recall);
    s.durability = avg(durability);
    s.last_score = avg(score);
    s
}

fn series_of(snapshots: &[Snapshot]) -> Series {
    Series {
        questions: snapshots.iter().map(|s| s.questions).collect(),
        studied: snapshots.iter().map(|s| s.studied).collect(),
        recall: snapshots.iter().map(|s| s.recall).collect(),
        durability: snapshots.iter().map(|s| s.durability).collect(),
        last_score: snapshots.iter().map(|s| s.last_score).collect(),
        days_since: snapshots.iter().map(|s| s.days_since).collect(),
        due: snapshots.iter().map(|s| s.due).collect(),
    }
}

/// The whole active knowledge tree with per-day state from the first study day to today.
pub async fn timeline(pool: &SqlitePool, s: &Settings, now: DateTime<Utc>) -> Result<Timeline> {
    let today = s.study_day(now);
    let params = SchedParams::main(s);

    // Active main-program topics whose ancestors are all active.
    type TopicRow = (i64, Option<i64>, String, String, i64, String);
    let topic_rows: Vec<TopicRow> = sqlx::query_as(
        "SELECT id, parent_id, name, status, target_level, created_at FROM topics WHERE scope = 'main' ORDER BY position, id",
    )
    .fetch_all(pool)
    .await?;
    let by_id: HashMap<i64, &TopicRow> = topic_rows.iter().map(|t| (t.0, t)).collect();
    let live = |mut id: i64| loop {
        match by_id.get(&id) {
            Some(t) if t.3 == "active" => match t.1 {
                Some(parent) => id = parent,
                None => return true,
            },
            _ => return false,
        }
    };
    let topics: Vec<&TopicRow> = topic_rows.iter().filter(|t| live(t.0)).collect();

    // Questions, collapsed into revision chains represented by their newest wording.
    type QuestionRow = (i64, i64, Option<i64>, String, String, String);
    let question_rows: Vec<QuestionRow> = sqlx::query_as(
        "SELECT id, topic_id, revision_of, created_at, status, prompt FROM questions WHERE scope = 'main'",
    )
    .fetch_all(pool)
    .await?;
    let q_by_id: HashMap<i64, &QuestionRow> = question_rows.iter().map(|q| (q.0, q)).collect();
    let replaced: std::collections::HashSet<i64> = question_rows.iter().filter_map(|q| q.2).collect();
    let mut head_of: HashMap<i64, i64> = HashMap::new();
    let mut chains: Vec<QuestionChain> = vec![];
    for q in &question_rows {
        // Heads only; proposed questions never existed on the map.
        if replaced.contains(&q.0) || q.4 == "proposed" || !topics.iter().any(|t| t.0 == q.1) {
            continue;
        }
        let mut first_created = parse_ts(&q.3)?;
        head_of.insert(q.0, q.0);
        let mut cursor = q.2;
        while let Some(prev) = cursor.and_then(|id| q_by_id.get(&id)) {
            if head_of.contains_key(&prev.0) {
                break;
            }
            head_of.insert(prev.0, q.0);
            first_created = first_created.min(parse_ts(&prev.3)?);
            cursor = prev.2;
        }
        chains.push(QuestionChain {
            head: q.0,
            topic: q.1,
            label: q.5.clone(),
            created_on: s.study_day(first_created),
            status: q.4.clone(),
            reviews: vec![],
        });
    }
    let chain_index: HashMap<i64, usize> = chains.iter().enumerate().map(|(i, c)| (c.head, i)).collect();

    let review_rows: Vec<(i64, String, i64, f64, String)> = sqlx::query_as(
        "SELECT r.question_id, r.reviewed_at, r.grade, r.score, s.study_day
         FROM reviews r JOIN sessions s ON s.id = r.session_id ORDER BY r.reviewed_at, r.id",
    )
    .fetch_all(pool)
    .await?;
    for (qid, at, grade, score, day) in review_rows {
        let Some(&i) = head_of.get(&qid).and_then(|h| chain_index.get(h)) else { continue };
        let day = parse_day(&day)?;
        if day > today {
            continue;
        }
        chains[i].reviews.push(Review { day, at: parse_ts(&at)?, grade: Grade::from_i64(grade)?, score: score as f32 });
    }

    let events = status_log::all(pool).await?;
    let mut events_by_question: HashMap<i64, Vec<&StatusEvent>> = HashMap::new();
    for e in events.iter().filter(|e| e.entity == Entity::Question) {
        events_by_question.entry(e.entity_id).or_default().push(e);
    }

    let first_day = chains.iter().filter_map(|c| c.reviews.first().map(|r| r.day)).min();
    let days: Vec<NaiveDate> = match first_day {
        Some(first) => (0..=(today - first).num_days()).map(|i| first + Duration::days(i)).collect(),
        None => vec![],
    };
    // Sample every day at its end; today at `now`. Without history, sample just today for `now`.
    let (sample_days, instants): (Vec<NaiveDate>, Vec<DateTime<Utc>>) = if days.is_empty() {
        (vec![today], vec![now])
    } else {
        days.iter().map(|&d| (d, if d == today { now } else { s.day_end(d) })).unzip()
    };

    let no_events = vec![];
    let fsrs = fsrs::FSRS::default();
    let mut points: Vec<Vec<Point>> = Vec::with_capacity(chains.len());
    for c in &chains {
        let ev = events_by_question.get(&c.head).unwrap_or(&no_events);
        points.push(replay(&fsrs, c, ev, &sample_days, &instants, &params)?);
    }

    // Each topic's question chains, including those of its subtopics.
    let mut subtree: HashMap<i64, Vec<usize>> = HashMap::new();
    for (i, c) in chains.iter().enumerate() {
        let mut t = Some(c.topic);
        while let Some(id) = t {
            subtree.entry(id).or_default().push(i);
            t = by_id.get(&id).and_then(|row| row.1);
        }
    }

    let n = sample_days.len();
    let has_history = !days.is_empty();
    let to_node_series = |snapshots: Vec<Snapshot>| -> (Series, Snapshot) {
        let now = snapshots.last().cloned().unwrap_or_default();
        (if has_history { series_of(&snapshots) } else { Series::default() }, now)
    };
    let mut nodes = vec![];
    for t in &topics {
        let members = subtree.get(&t.0).map(Vec::as_slice).unwrap_or_default();
        let snapshots: Vec<Snapshot> = (0..n).map(|d| aggregate(members.iter().map(|&i| &points[i][d]))).collect();
        let (series, now_snapshot) = to_node_series(snapshots);
        nodes.push(TimelineNode {
            id: format!("t{}", t.0),
            kind: NodeKind::Topic,
            ref_id: t.0,
            parent: t.1.map(|p| format!("t{p}")),
            label: t.2.clone(),
            target_level: Some(t.4 as u8),
            created_on: s.study_day(parse_ts(&t.5)?),
            series,
            now: now_snapshot,
        });
    }
    for (i, c) in chains.iter().enumerate() {
        let snapshots: Vec<Snapshot> = (0..n).map(|d| aggregate(std::iter::once(&points[i][d]))).collect();
        let (series, now_snapshot) = to_node_series(snapshots);
        nodes.push(TimelineNode {
            id: format!("q{}", c.head),
            kind: NodeKind::Question,
            ref_id: c.head,
            parent: Some(format!("t{}", c.topic)),
            label: c.label.clone(),
            target_level: None,
            created_on: c.created_on,
            series,
            now: now_snapshot,
        });
    }
    Ok(Timeline { days, today, stale_after_days: s.stale_after_days, stale_full_days: s.stale_full_days, nodes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::questions::{QuestionPatch, QuestionStatus};
    use crate::testutil::{self, day, utc};
    use crate::topics::{TopicPatch, TopicStatus};
    use sqlx::SqlitePool;

    async fn setup() -> (SqlitePool, Settings) {
        let p = test_pool().await;
        crate::settings::set(&p, "timezone", "UTC").await.unwrap();
        crate::settings::set(&p, "day_rollover_hour", "0").await.unwrap();
        let s = Settings::load(&p).await.unwrap();
        (p, s)
    }
    async fn created(p: &SqlitePool, q: i64, at: &str) {
        sqlx::query("UPDATE questions SET created_at = ? WHERE id = ?").bind(at).bind(q).execute(p).await.unwrap();
    }
    async fn review(p: &SqlitePool, q: i64, d: &str, grade: i64, score: f64) {
        let sid = testutil::session(p, d).await;
        let rid = testutil::review_row(p, q, sid, &format!("{d}T12:00:00Z"), grade).await;
        sqlx::query("UPDATE reviews SET score = ? WHERE id = ?").bind(score).bind(rid).execute(p).await.unwrap();
    }
    fn node<'a>(t: &'a Timeline, id: &str) -> &'a TimelineNode {
        t.nodes.iter().find(|n| n.id == id).unwrap_or_else(|| panic!("no node {id}"))
    }

    #[tokio::test]
    async fn empty_db_has_no_days_but_now_filled() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        let tl = timeline(&p, &s, utc("2026-10-24T18:00:00Z")).await.unwrap();
        assert!(tl.days.is_empty());
        assert_eq!((tl.stale_after_days, tl.stale_full_days), (14, 60));
        let topic = node(&tl, &format!("t{t}"));
        assert_eq!((topic.now.questions, topic.now.studied, topic.now.recall), (1, 0, None));
        assert!(topic.series.questions.is_empty());
        assert_eq!(node(&tl, &format!("q{q}")).parent.as_deref(), Some(format!("t{t}").as_str()));
    }

    #[tokio::test]
    async fn measures_are_rounded_to_keep_the_payload_small() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "T").await;
        let qa = testutil::question(&p, t).await;
        let qb = testutil::question(&p, t).await;
        review(&p, qa, "2026-10-01", 3, 1.0).await;
        review(&p, qb, "2026-10-02", 2, 0.7).await;
        let tl = timeline(&p, &s, utc("2026-10-09T18:00:00Z")).await.unwrap();
        let three_places = |v: f32| ((v as f64) * 1000.0 - ((v as f64) * 1000.0).round()).abs() < 1e-3;
        for n in &tl.nodes {
            for v in n.series.recall.iter().chain(&n.series.durability).chain(&n.series.last_score).flatten() {
                assert!(three_places(*v), "{} has {v}", n.id);
            }
        }
    }

    #[tokio::test]
    async fn recall_sags_between_reviews_and_jumps_on_review_day() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        review(&p, q, "2026-10-01", 3, 1.0).await;
        review(&p, q, "2026-10-05", 3, 1.0).await;
        let tl = timeline(&p, &s, utc("2026-10-06T18:00:00Z")).await.unwrap();
        assert_eq!((tl.days.first().copied(), tl.days.len()), (Some(day("2026-10-01")), 6));
        let r = &node(&tl, &format!("q{q}")).series.recall;
        let (d1, d4, d5) = (r[0].unwrap(), r[3].unwrap(), r[4].unwrap());
        assert!(d4 < d1 && d5 > d4, "{r:?}");
        assert_eq!(node(&tl, &format!("q{q}")).series.days_since[3], Some(3));
    }

    #[tokio::test]
    async fn end_of_day_respects_rollover() {
        let (p, _) = setup().await;
        crate::settings::set(&p, "timezone", "America/Los_Angeles").await.unwrap();
        crate::settings::set(&p, "day_rollover_hour", "4").await.unwrap();
        let s = Settings::load(&p).await.unwrap();
        assert_eq!(s.day_end(day("2026-10-01")), utc("2026-10-02T11:00:00Z")); // 04:00 PDT
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        // Reviewed at 02:00 PDT on 2 Oct, which belongs to study day 1 Oct.
        let sid = testutil::session(&p, "2026-10-01").await;
        testutil::review_row(&p, q, sid, "2026-10-02T09:00:00Z", 3).await;
        let tl = timeline(&p, &s, utc("2026-10-03T18:00:00Z")).await.unwrap();
        assert_eq!(tl.days[0], day("2026-10-01"));
        assert_eq!(node(&tl, &format!("q{q}")).series.studied[0], 1);
    }

    #[tokio::test]
    async fn revisions_collapse_into_newest_wording() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "T").await;
        let v1 = testutil::question(&p, t).await;
        review(&p, v1, "2026-10-01", 3, 0.5).await;
        let v2 = crate::questions::update(&p, v1, QuestionPatch { prompt: Some("v2".into()), ..Default::default() }, utc("2026-10-02T18:00:00Z")).await.unwrap();
        let tl = timeline(&p, &s, utc("2026-10-03T18:00:00Z")).await.unwrap();
        let qs: Vec<&TimelineNode> = tl.nodes.iter().filter(|n| n.kind == NodeKind::Question).collect();
        assert_eq!(qs.len(), 1);
        assert_eq!((qs[0].ref_id, qs[0].label.as_str()), (v2.id, "v2"));
        assert_eq!(qs[0].series.studied, vec![1, 1, 1]);
        assert_eq!(qs[0].series.questions, vec![1, 1, 1]);
        assert_eq!(qs[0].series.last_score[0], Some(0.5));
    }

    #[tokio::test]
    async fn growing_counts_follow_created_on() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "T").await;
        let a = testutil::question(&p, t).await;
        let b = testutil::question(&p, t).await;
        created(&p, a, "2026-10-01T10:00:00+00:00").await;
        created(&p, b, "2026-10-03T10:00:00+00:00").await;
        review(&p, a, "2026-10-01", 3, 1.0).await;
        let tl = timeline(&p, &s, utc("2026-10-04T18:00:00Z")).await.unwrap();
        assert_eq!(node(&tl, &format!("t{t}")).series.questions, vec![1, 1, 2, 2]);
        assert_eq!(node(&tl, &format!("q{b}")).created_on, day("2026-10-03"));
    }

    #[tokio::test]
    async fn retired_question_counts_before_not_after() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        review(&p, q, "2026-10-01", 3, 1.0).await;
        crate::questions::set_status(&p, q, QuestionStatus::Retired, utc("2026-10-03T18:00:00Z")).await.unwrap();
        let tl = timeline(&p, &s, utc("2026-10-05T18:00:00Z")).await.unwrap();
        assert_eq!(node(&tl, &format!("t{t}")).series.questions, vec![1, 1, 0, 0, 0]);
        assert_eq!(node(&tl, &format!("t{t}")).now.questions, 0);
    }

    #[tokio::test]
    async fn due_from_the_due_date_for_questions_and_topics() {
        // Found by cargo-mutants: nothing checked a non-zero due count.
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        review(&p, q, "2026-10-01", 1, 0.0).await; // again: due the next day
        let tl = timeline(&p, &s, utc("2026-10-03T18:00:00Z")).await.unwrap();
        assert_eq!(node(&tl, &format!("q{q}")).series.due, vec![0, 1, 1]);
        assert_eq!(node(&tl, &format!("t{t}")).series.due, vec![0, 1, 1]);
        assert_eq!(node(&tl, &format!("t{t}")).now.due, 1);
    }

    #[tokio::test]
    async fn suspended_exists_but_not_due() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        review(&p, q, "2026-10-01", 1, 0.0).await; // again: due the next day
        crate::questions::set_status(&p, q, QuestionStatus::Suspended, utc("2026-10-01T20:00:00Z")).await.unwrap();
        let tl = timeline(&p, &s, utc("2026-10-03T18:00:00Z")).await.unwrap();
        let n = node(&tl, &format!("q{q}"));
        assert_eq!((n.series.questions.clone(), n.series.due.clone()), (vec![1, 1, 1], vec![0, 0, 0]));
    }

    #[tokio::test]
    async fn topic_aggregates_sum_mean_min() {
        let (p, s) = setup().await;
        let root = testutil::topic(&p, None, "Root").await;
        let a = testutil::topic(&p, Some(root), "A").await;
        let b = testutil::topic(&p, Some(root), "B").await;
        let qa = testutil::question(&p, a).await;
        let qb = testutil::question(&p, b).await;
        testutil::question(&p, b).await; // never studied
        review(&p, qa, "2026-10-01", 3, 0.4).await;
        review(&p, qb, "2026-10-03", 3, 0.8).await;
        let tl = timeline(&p, &s, utc("2026-10-03T18:00:00Z")).await.unwrap();
        let r = node(&tl, &format!("t{root}"));
        assert_eq!((r.series.questions[2], r.series.studied[2]), (3, 2));
        assert!((r.series.last_score[2].unwrap() - 0.6).abs() < 1e-6);
        assert_eq!(r.series.days_since[2], Some(0));
        assert_eq!(r.series.days_since[1], Some(1));
        let mean = (node(&tl, &format!("q{qa}")).series.recall[2].unwrap() + node(&tl, &format!("q{qb}")).series.recall[2].unwrap()) / 2.0;
        assert!((r.series.recall[2].unwrap() - mean).abs() < 1e-3, "means agree to the 3 places sent");
    }

    #[tokio::test]
    async fn last_series_value_equals_now() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        review(&p, q, "2026-10-01", 3, 1.0).await;
        let tl = timeline(&p, &s, utc("2026-10-04T18:00:00Z")).await.unwrap();
        for n in &tl.nodes {
            let last = n.series.questions.len() - 1;
            assert_eq!(n.now.questions, n.series.questions[last]);
            assert_eq!(n.now.recall, n.series.recall[last]);
            assert_eq!(n.now.days_since, n.series.days_since[last]);
        }
    }

    #[tokio::test]
    async fn now_matches_today_stats_and_views() {
        let (p, s) = setup().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        testutil::question(&p, t).await;
        let now = utc("2026-10-24T18:00:00Z");
        review(&p, q, "2026-10-20", 3, 1.0).await;
        crate::scheduler::rebuild_all(&p, &s).await.unwrap();
        let tl = timeline(&p, &s, now).await.unwrap();
        let card = &crate::views::topic_page(&p, None, false, false, now).await.unwrap().children[0];
        let tn = node(&tl, &format!("t{t}"));
        assert_eq!((tn.now.questions, tn.now.studied), (card.active_questions, card.studied));
        assert!((tn.now.recall.unwrap() - card.avg_retrievability.unwrap()).abs() < 1e-3);
    }

    #[tokio::test]
    async fn archived_topics_excluded() {
        let (p, s) = setup().await;
        let parent = testutil::topic(&p, None, "P").await;
        let child = testutil::topic(&p, Some(parent), "C").await;
        testutil::question(&p, child).await;
        crate::topics::update(&p, parent, TopicPatch { status: Some(TopicStatus::Archived), ..Default::default() }, utc("2026-10-02T18:00:00Z")).await.unwrap();
        let tl = timeline(&p, &s, utc("2026-10-03T18:00:00Z")).await.unwrap();
        assert!(tl.nodes.is_empty());
    }

    #[tokio::test]
    #[ignore = "performance: run with --release -- --ignored perf_5000"]
    async fn perf_5000_questions_3_years() {
        let (p, s) = setup().await;
        let mut topics = vec![];
        for i in 0..50 {
            topics.push(testutil::topic(&p, None, &format!("T{i}")).await);
        }
        let mut tx = p.begin().await.unwrap();
        let sid: i64 = sqlx::query_scalar("INSERT INTO sessions (kind, study_day, started_at, created_at, updated_at) VALUES ('main', '2024-01-01', 'x', 'x', 'x') RETURNING id").fetch_one(&mut *tx).await.unwrap();
        for i in 0..5000i64 {
            let t = topics[(i % 50) as usize];
            let q: i64 = sqlx::query_scalar(r#"INSERT INTO questions (topic_id, level, prompt, answer, rubric, created_at, updated_at) VALUES (?, 0, 'q', 'a', '[]', '2024-01-01T00:00:00+00:00', 'x') RETURNING id"#).bind(t).fetch_one(&mut *tx).await.unwrap();
            for k in 0..6i64 {
                let d = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap() + chrono::Duration::days((i * 7 + k * 150) % 1095);
                sqlx::query("INSERT INTO reviews (question_id, session_id, reviewed_at, grade, score, rubric_result, user_answer, asked_prompt, created_at, updated_at) VALUES (?, ?, ?, 3, 1.0, '[]', '', '', 'x', 'x')")
                    .bind(q).bind(sid).bind(format!("{d}T12:00:00+00:00")).execute(&mut *tx).await.unwrap();
            }
        }
        tx.commit().await.unwrap();
        let start = std::time::Instant::now();
        let tl = timeline(&p, &s, utc("2026-12-31T18:00:00Z")).await.unwrap();
        let took = start.elapsed();
        assert!(!tl.days.is_empty());
        // Worst case: every question exists on every day. Real histories are far sparser.
        assert!(took < std::time::Duration::from_millis(500), "took {took:?}");
    }
}
