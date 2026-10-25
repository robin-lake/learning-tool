//! Append-only log of topic and question status changes, so past days know what existed.

use chrono::{DateTime, Utc};
use sqlx::{SqliteConnection, SqlitePool};

use crate::Result;
use crate::types::{parse_ts, str_enum, ts};

str_enum!(Entity { Topic => "topic", Question => "question" });

str_enum!(StatusReason {
    Archive => "archive",
    Unarchive => "unarchive",
    Suspend => "suspend",
    Unsuspend => "unsuspend",
    Retire => "retire",
    Reword => "reword",
    Reactivate => "reactivate",
});

#[derive(Debug, Clone, PartialEq)]
pub struct StatusEvent {
    pub entity: Entity,
    pub entity_id: i64,
    pub from_status: String,
    pub to_status: String,
    pub reason: StatusReason,
    pub at: DateTime<Utc>,
}

/// Records a change; call it inside the transaction that changes the status.
pub(crate) async fn record(conn: &mut SqliteConnection, e: &StatusEvent) -> Result<()> {
    let at = ts(e.at);
    sqlx::query(
        "INSERT INTO status_events (entity, entity_id, from_status, to_status, reason, at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
    )
    .bind(e.entity.as_str())
    .bind(e.entity_id)
    .bind(&e.from_status)
    .bind(&e.to_status)
    .bind(e.reason.as_str())
    .bind(at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Every event, oldest first.
pub async fn all(pool: &SqlitePool) -> Result<Vec<StatusEvent>> {
    let rows: Vec<(String, i64, String, String, String, String)> = sqlx::query_as(
        "SELECT entity, entity_id, from_status, to_status, reason, at FROM status_events ORDER BY at, id",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(entity, entity_id, from_status, to_status, reason, at)| {
            Ok(StatusEvent {
                entity: Entity::parse(&entity)?,
                entity_id,
                from_status,
                to_status,
                reason: StatusReason::parse(&reason)?,
                at: parse_ts(&at)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::questions::{self, QuestionPatch, QuestionStatus};
    use crate::testutil::{self, utc};
    use crate::topics::{self, TopicPatch, TopicStatus};

    fn now() -> DateTime<Utc> {
        utc("2026-10-24T18:00:00Z")
    }
    fn shape(events: &[StatusEvent]) -> Vec<(Entity, &str, &str, StatusReason)> {
        events.iter().map(|e| (e.entity, e.from_status.as_str(), e.to_status.as_str(), e.reason)).collect()
    }

    #[tokio::test]
    async fn archive_and_unarchive_topic_logged() {
        let p = test_pool().await;
        let t = testutil::topic(&p, None, "T").await;
        for status in [TopicStatus::Archived, TopicStatus::Active] {
            topics::update(&p, t, TopicPatch { status: Some(status), ..Default::default() }, now()).await.unwrap();
        }
        assert_eq!(
            shape(&all(&p).await.unwrap()),
            vec![
                (Entity::Topic, "active", "archived", StatusReason::Archive),
                (Entity::Topic, "archived", "active", StatusReason::Unarchive),
            ]
        );
    }

    #[tokio::test]
    async fn rename_only_topic_update_logs_nothing() {
        let p = test_pool().await;
        let t = testutil::topic(&p, None, "T").await;
        topics::update(&p, t, TopicPatch { name: Some("Renamed".into()), ..Default::default() }, now()).await.unwrap();
        assert!(all(&p).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn suspend_unsuspend_retire_logged() {
        let p = test_pool().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        for status in [QuestionStatus::Suspended, QuestionStatus::Active, QuestionStatus::Retired] {
            questions::set_status(&p, q, status, now()).await.unwrap();
        }
        let events = all(&p).await.unwrap();
        assert_eq!(
            shape(&events),
            vec![
                (Entity::Question, "active", "suspended", StatusReason::Suspend),
                (Entity::Question, "suspended", "active", StatusReason::Unsuspend),
                (Entity::Question, "active", "retired", StatusReason::Retire),
            ]
        );
        assert!(events.iter().all(|e| e.entity_id == q && e.at == now()));
    }

    #[tokio::test]
    async fn proposed_to_active_is_reactivate() {
        let p = test_pool().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        sqlx::query("UPDATE questions SET status = 'proposed' WHERE id = ?").bind(q).execute(&p).await.unwrap();
        questions::set_status(&p, q, QuestionStatus::Active, now()).await.unwrap();
        assert_eq!(all(&p).await.unwrap()[0].reason, StatusReason::Reactivate);
    }

    #[tokio::test]
    async fn reword_logs_retire_of_old_wording() {
        let p = test_pool().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        questions::update(&p, q, QuestionPatch { prompt: Some("v2".into()), ..Default::default() }, now()).await.unwrap();
        let events = all(&p).await.unwrap();
        assert_eq!(shape(&events), vec![(Entity::Question, "active", "retired", StatusReason::Reword)]);
        assert_eq!(events[0].entity_id, q);
    }

    #[tokio::test]
    async fn no_event_when_status_unchanged() {
        let p = test_pool().await;
        let t = testutil::topic(&p, None, "T").await;
        let q = testutil::question(&p, t).await;
        questions::set_status(&p, q, QuestionStatus::Active, now()).await.unwrap();
        topics::update(&p, t, TopicPatch { status: Some(TopicStatus::Active), ..Default::default() }, now()).await.unwrap();
        assert!(all(&p).await.unwrap().is_empty());
    }

    #[test]
    fn events_are_never_updated() {
        let src = include_str!("status_log.rs");
        let (upd, del) = (["UPDATE", "status_events"].join(" "), ["DELETE FROM", "status_events"].join(" "));
        assert!(!src.contains(&upd) && !src.contains(&del));
    }
}
