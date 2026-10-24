use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::Result;
use crate::scheduler::retrievability;
use crate::settings::Settings;
use crate::types::{parse_ts, str_enum};

str_enum!(ItemKind { Review => "review", New => "new", Requeue => "requeue" });

/// Retrievability at or above which a review can open the session as an easy warm-up.
const WARMUP_MIN_R: f32 = 0.85;
const WARMUP_MAX: usize = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub question_id: i64,
    pub topic_id: i64,
    pub topic_depth: u32,
    pub topic_position: i64,
    pub level: u8,
    pub priority: i64,
    /// Current recall probability; `None` for new questions.
    pub retrievability: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueuePlan {
    pub items: Vec<(i64, ItemKind)>,
    pub due_total: u32,
    pub backlog_carried: u32,
    pub new_allowed: u32,
}

fn r(c: &Candidate) -> f32 {
    c.retrievability.unwrap_or(0.0)
}

/// Builds a session queue from due reviews and new candidates (see the design doc's session rules).
pub fn plan(mut due: Vec<Candidate>, mut new: Vec<Candidate>, new_introduced_today: u32, s: &Settings) -> QueuePlan {
    let due_total = due.len() as u32;
    due.sort_by(|a, b| r(a).total_cmp(&r(b)).then(a.question_id.cmp(&b.question_id)));
    let limit = s.backlog_daily_limit as usize;
    let backlog_carried = due.len().saturating_sub(limit) as u32;
    due.truncate(limit);

    let new_allowed =
        if due_total >= s.new_pause_review_threshold { 0 } else { s.new_per_day.saturating_sub(new_introduced_today) };
    new.sort_by_key(|c| (c.topic_depth, c.topic_position, c.level, c.priority, c.question_id));
    new.truncate(new_allowed as usize);

    // Warm-up: the most likely successes first, for an early win.
    let mut warmup: Vec<Candidate> = due.iter().rev().take_while(|c| r(c) >= WARMUP_MIN_R).take(WARMUP_MAX).cloned().collect();
    warmup.sort_by(|a, b| r(b).total_cmp(&r(a)));
    due.retain(|c| !warmup.iter().any(|w| w.question_id == c.question_id));

    // Body: remaining reviews (lowest retrievability first) with new items spread evenly.
    let reviews_len = due.len();
    let n = new.len();
    let mut body: Vec<(Candidate, ItemKind)> = due.into_iter().map(|c| (c, ItemKind::Review)).collect();
    for (k, c) in new.into_iter().enumerate() {
        let after_reviews = (((k + 1) * reviews_len) as f32 / (n + 1) as f32).round() as usize;
        body.insert(after_reviews + k, (c, ItemKind::New));
    }

    let mut items: Vec<(Candidate, ItemKind)> = warmup.into_iter().map(|c| (c, ItemKind::Review)).collect();
    items.extend(body);
    decluster(&mut items);

    QueuePlan {
        items: items.into_iter().map(|(c, k)| (c.question_id, k)).collect(),
        due_total,
        backlog_carried,
        new_allowed,
    }
}

/// Avoids two questions from the same topic in a row where a later item can be swapped in.
fn decluster(items: &mut [(Candidate, ItemKind)]) {
    let topic = |items: &[(Candidate, ItemKind)], i: usize| items[i].0.topic_id;
    for i in 1..items.len() {
        if topic(items, i) != topic(items, i - 1) {
            continue;
        }
        let prev = topic(items, i - 1);
        let after = (i + 1 < items.len()).then(|| topic(items, i + 1));
        if let Some(j) = (i + 1..items.len()).find(|&j| {
            let t = topic(items, j);
            t != prev && Some(t) != after
        }) {
            items.swap(i, j);
        }
    }
}

/// Due and new main-program candidates for `today`, plus how many new questions were introduced today.
/// Only active questions whose topic and every ancestor topic are active are included.
pub async fn load_candidates(
    pool: &SqlitePool,
    _s: &Settings,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<(Vec<Candidate>, Vec<Candidate>, u32)> {
    type Row = (i64, i64, i64, i64, i64, i64, Option<String>, Option<String>, Option<f64>, Option<String>);
    let rows: Vec<Row> = sqlx::query_as(
        "WITH RECURSIVE live(id, depth) AS (
             SELECT id, 0 FROM topics WHERE parent_id IS NULL AND status = 'active'
             UNION ALL
             SELECT t.id, live.depth + 1 FROM topics t JOIN live ON t.parent_id = live.id WHERE t.status = 'active'
         )
         SELECT q.id, q.topic_id, live.depth, t.position, q.level, q.priority,
                s.state, s.due_on, s.stability, s.last_reviewed_at
         FROM questions q
         JOIN live ON live.id = q.topic_id
         JOIN topics t ON t.id = q.topic_id
         LEFT JOIN question_state s ON s.question_id = q.id
         WHERE q.status = 'active' AND q.scope = 'main'",
    )
    .fetch_all(pool)
    .await?;
    let today_s = today.to_string();
    let (mut due, mut new) = (vec![], vec![]);
    for (id, topic_id, depth, position, level, priority, state, due_on, stability, last) in rows {
        let mut c = Candidate {
            question_id: id,
            topic_id,
            topic_depth: depth as u32,
            topic_position: position,
            level: level as u8,
            priority,
            retrievability: None,
        };
        match state.as_deref() {
            None | Some("new") => new.push(c),
            _ => {
                if due_on.as_deref().is_some_and(|d| d <= today_s.as_str()) {
                    c.retrievability = Some(match (stability, last) {
                        (Some(st), Some(last)) => {
                            let days = (now - parse_ts(&last)?).num_seconds().max(0) as f32 / 86_400.0;
                            retrievability(st as f32, days)
                        }
                        _ => 0.0,
                    });
                    due.push(c);
                }
            }
        }
    }
    let introduced: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM session_items i JOIN sessions s ON s.id = i.session_id
         WHERE i.kind = 'new' AND i.review_id IS NOT NULL AND s.kind = 'main' AND s.study_day = ?",
    )
    .bind(&today_s)
    .fetch_one(pool)
    .await?;
    Ok((due, new, introduced as u32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::testutil::{self, utc};

    fn settings() -> Settings {
        Settings {
            new_per_day: 3,
            new_pause_review_threshold: 20,
            backlog_daily_limit: 30,
            session_target: 10,
            desired_retention_main: 0.9,
            leech_lapses: 6,
            day_rollover_hour: 4,
            timezone: crate::settings::TzSetting::System,
            backup_dir: None,
            backups_keep: 2,
        }
    }
    fn rev(id: i64, topic: i64, r: f32) -> Candidate {
        Candidate { question_id: id, topic_id: topic, topic_depth: 0, topic_position: 0, level: 0, priority: 0, retrievability: Some(r) }
    }
    fn new(id: i64, topic: i64, depth: u32, level: u8, priority: i64) -> Candidate {
        Candidate { question_id: id, topic_id: topic, topic_depth: depth, topic_position: 0, level, priority, retrievability: None }
    }
    fn ids(p: &QueuePlan) -> Vec<i64> {
        p.items.iter().map(|(id, _)| *id).collect()
    }
    fn news(p: &QueuePlan) -> Vec<i64> {
        p.items.iter().filter(|(_, k)| *k == ItemKind::New).map(|(id, _)| *id).collect()
    }

    #[test]
    fn new_capped_by_per_day_minus_introduced() {
        let n = (1..=5).map(|i| new(i, i, 0, 0, i)).collect();
        let p = plan(vec![], n, 1, &settings());
        assert_eq!(p.new_allowed, 2);
        assert_eq!(news(&p), vec![1, 2]);
    }

    #[test]
    fn new_paused_when_due_at_threshold() {
        let due = (1..=20).map(|i| rev(i, i, 0.5)).collect();
        let p = plan(due, vec![new(100, 1, 0, 0, 0)], 0, &settings());
        assert_eq!((p.new_allowed, p.due_total), (0, 20));
        assert!(news(&p).is_empty());
    }

    #[test]
    fn backlog_limited_lowest_r_first() {
        let due = (1..=35).map(|i| rev(i, i, i as f32 / 100.0)).collect();
        let p = plan(due, vec![], 0, &settings());
        assert_eq!((p.items.len(), p.backlog_carried, p.due_total), (30, 5, 35));
        let mut kept = ids(&p);
        kept.sort();
        assert_eq!(kept, (1..=30).collect::<Vec<_>>());
    }

    #[test]
    fn new_ordered_broad_first() {
        let n = vec![new(1, 1, 1, 0, 0), new(2, 2, 0, 1, 0), new(3, 2, 0, 0, 5), new(4, 2, 0, 0, 1)];
        let p = plan(vec![], n, 0, &settings());
        // depth 0 first; within it L0 before L1, then priority
        assert_eq!(news(&p), vec![4, 3, 2]);
    }

    #[test]
    fn warmup_takes_up_to_two_high_r() {
        let due = vec![rev(1, 1, 0.3), rev(2, 2, 0.95), rev(3, 3, 0.9), rev(4, 4, 0.97), rev(5, 5, 0.4)];
        let p = plan(due, vec![], 0, &settings());
        assert_eq!(&ids(&p)[..2], &[4, 2]);
        assert_eq!(&ids(&p)[2..], &[1, 5, 3]);
    }

    #[test]
    fn new_spread_evenly() {
        let due = (1..=6).map(|i| rev(i, i, 0.1 * i as f32)).collect();
        let n = vec![new(101, 101, 0, 0, 0), new(102, 102, 0, 0, 1)];
        let p = plan(due, n, 0, &settings());
        let kinds: Vec<_> = p.items.iter().map(|(_, k)| *k).collect();
        assert_eq!(kinds.iter().filter(|k| **k == ItemKind::New).count(), 2);
        assert_eq!(kinds[2], ItemKind::New);
        assert_eq!(kinds[5], ItemKind::New);
    }

    #[test]
    fn adjacent_same_topic_declustered() {
        let due = vec![rev(1, 1, 0.1), rev(2, 1, 0.2), rev(3, 2, 0.3), rev(4, 3, 0.4)];
        let p = plan(due, vec![], 0, &settings());
        let topic_of = |id: i64| if id <= 2 { 1 } else { id - 1 };
        let order = ids(&p);
        for w in order.windows(2) {
            assert_ne!(topic_of(w[0]), topic_of(w[1]), "{order:?}");
        }
    }

    #[test]
    fn all_same_topic_left_alone() {
        let due = vec![rev(1, 1, 0.1), rev(2, 1, 0.2), rev(3, 1, 0.3)];
        let p = plan(due, vec![], 0, &settings());
        assert_eq!(ids(&p), vec![1, 2, 3]);
    }

    async fn reviewed_new_item(pool: &SqlitePool, q: i64, study_day: &str) {
        let s = testutil::session(pool, study_day).await;
        let r = testutil::review_row(pool, q, s, &format!("{study_day}T18:00:00Z"), 3).await;
        sqlx::query("INSERT INTO session_items (session_id, ordinal, question_id, kind, review_id) VALUES (?, 0, ?, 'new', ?)")
            .bind(s).bind(q).bind(r).execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn archived_topic_questions_are_not_queued() {
        let pool = test_pool().await;
        let s = Settings::load(&pool).await.unwrap();
        let parent = testutil::topic(&pool, None, "P").await;
        let child = testutil::topic(&pool, Some(parent), "C").await;
        let q = testutil::question(&pool, child).await;
        reviewed_new_item(&pool, q, "2026-10-01").await;
        sqlx::query("INSERT INTO question_state (question_id, stability, difficulty, due_on, last_reviewed_at, last_study_day, reps, state) VALUES (?, 2, 5, '2026-10-02', '2026-10-01T18:00:00+00:00', '2026-10-01', 1, 'review')")
            .bind(q).execute(&pool).await.unwrap();
        testutil::question(&pool, child).await;
        let today = testutil::day("2026-10-23");
        let now = utc("2026-10-23T18:00:00Z");
        let (due, new, _) = load_candidates(&pool, &s, today, now).await.unwrap();
        assert_eq!((due.len(), new.len()), (1, 1));
        assert_eq!(due[0].topic_depth, 1);
        assert!(due[0].retrievability.unwrap() < 1.0);
        sqlx::query("UPDATE topics SET status = 'archived' WHERE id = ?").bind(parent).execute(&pool).await.unwrap();
        let (due, new, _) = load_candidates(&pool, &s, today, now).await.unwrap();
        assert!(due.is_empty() && new.is_empty());
        assert_eq!(crate::questions::get(&pool, q).await.unwrap().reviews.len(), 1);
    }

    #[tokio::test]
    async fn suspended_and_proposed_not_queued() {
        let pool = test_pool().await;
        let s = Settings::load(&pool).await.unwrap();
        let t = testutil::topic(&pool, None, "T").await;
        for status in ["suspended", "proposed", "retired"] {
            let q = testutil::question(&pool, t).await;
            sqlx::query("UPDATE questions SET status = ? WHERE id = ?").bind(status).bind(q).execute(&pool).await.unwrap();
        }
        let (due, new, _) = load_candidates(&pool, &s, testutil::day("2026-10-23"), utc("2026-10-23T18:00:00Z")).await.unwrap();
        assert!(due.is_empty() && new.is_empty());
    }

    #[tokio::test]
    async fn new_introduced_today_counts_reviewed_new_items_only() {
        let pool = test_pool().await;
        let s = Settings::load(&pool).await.unwrap();
        let t = testutil::topic(&pool, None, "T").await;
        let a = testutil::question(&pool, t).await;
        let b = testutil::question(&pool, t).await;
        reviewed_new_item(&pool, a, "2026-10-23").await;
        reviewed_new_item(&pool, b, "2026-10-22").await;
        // a review (not a new item) today doesn't count
        let sess = testutil::session(&pool, "2026-10-23").await;
        testutil::review_row(&pool, b, sess, "2026-10-23T19:00:00Z", 3).await;
        let (_, _, n) = load_candidates(&pool, &s, testutil::day("2026-10-23"), utc("2026-10-23T20:00:00Z")).await.unwrap();
        assert_eq!(n, 1);
    }
}
