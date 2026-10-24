//! Page-shaped read models for the web app. List views never carry answers or rubrics.

use chrono::{DateTime, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::questions::{self, QuestionDetail, QuestionStatus};
use crate::references::{self, SourceKind};
use crate::scheduler::retrievability;
use crate::topics::{self, Topic, TopicNode, TopicStatus};
use crate::types::{LinkKind, parse_day, parse_ts};
use crate::{Result, pins};

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct TopicRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct QuestionRef {
    pub id: i64,
    pub prompt: String,
    pub topic_path: Vec<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct TopicCard {
    pub id: i64,
    pub name: String,
    pub target_level: u8,
    pub status: TopicStatus,
    pub questions_by_level: [u32; 4],
    pub active_questions: u32,
    pub studied: u32,
    pub avg_retrievability: Option<f32>,
}

impl From<&TopicNode> for TopicCard {
    fn from(n: &TopicNode) -> TopicCard {
        TopicCard {
            id: n.topic.id,
            name: n.topic.name.clone(),
            target_level: n.topic.target_level,
            status: n.topic.status,
            questions_by_level: n.questions_by_level,
            active_questions: n.active_questions,
            studied: n.studied,
            avg_retrievability: n.avg_retrievability,
        }
    }
}

/// A question in a list: everything except the answer and rubric.
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct QuestionRow {
    pub id: i64,
    pub topic_id: i64,
    pub level: u8,
    pub prompt: String,
    pub status: QuestionStatus,
    pub due_on: Option<NaiveDate>,
    pub pinned: bool,
    pub recall_now: Option<f32>,
    pub reviews: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct LinkView {
    pub topic: TopicRef,
    pub kind: LinkKind,
    pub weight: f32,
    /// True when this topic is the link's `from` side.
    pub outgoing: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct ReferenceRef {
    pub id: i64,
    pub title: String,
    pub kind: SourceKind,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TopicPage {
    /// `None` for the root (all top-level topics).
    pub topic: Option<Topic>,
    pub breadcrumbs: Vec<TopicRef>,
    pub children: Vec<TopicCard>,
    pub questions: Vec<QuestionRow>,
    pub links: Vec<LinkView>,
    pub references: Vec<ReferenceRef>,
}

fn visible(card: &TopicCard, include_archived: bool) -> bool {
    include_archived || card.status == TopicStatus::Active
}

type RowTuple = (i64, i64, i64, String, String, Option<String>, Option<f64>, Option<String>, i64, i64);

async fn question_rows(pool: &SqlitePool, topic_ids: &[i64], now: DateTime<Utc>) -> Result<Vec<QuestionRow>> {
    let rows: Vec<RowTuple> = sqlx::query_as(
        "SELECT q.id, q.topic_id, q.level, q.prompt, q.status, s.due_on, s.stability, s.last_reviewed_at,
                EXISTS (SELECT 1 FROM pins p WHERE p.question_id = q.id
                        AND p.unpinned_at IS NULL AND p.used_in_session_id IS NULL),
                (SELECT count(*) FROM reviews r WHERE r.question_id = q.id)
         FROM questions q LEFT JOIN question_state s ON s.question_id = q.id
         WHERE q.status <> 'retired' AND q.topic_id IN (SELECT value FROM json_each(?))
         ORDER BY q.level, q.priority, q.id",
    )
    .bind(serde_json::to_string(topic_ids).expect("serializes"))
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(id, topic_id, level, prompt, status, due_on, stability, last, pinned, reviews)| {
            let recall_now = match (stability, last) {
                (Some(st), Some(last)) => {
                    let days = (now - parse_ts(&last)?).num_seconds().max(0) as f32 / 86_400.0;
                    Some(retrievability(st as f32, days))
                }
                _ => None,
            };
            Ok(QuestionRow {
                id,
                topic_id,
                level: level as u8,
                prompt,
                status: QuestionStatus::parse(&status)?,
                due_on: due_on.as_deref().map(parse_day).transpose()?,
                pinned: pinned != 0,
                recall_now,
                reviews: reviews as u32,
            })
        })
        .collect()
}

/// A topic page (or the root when `id` is `None`): breadcrumbs, subtopic cards, questions, links, references.
pub async fn topic_page(
    pool: &SqlitePool,
    id: Option<i64>,
    all: bool,
    include_archived: bool,
    now: DateTime<Utc>,
) -> Result<TopicPage> {
    let Some(id) = id else {
        let roots = topics::tree(pool, None, Some(0), now).await?;
        return Ok(TopicPage {
            topic: None,
            breadcrumbs: vec![],
            children: roots.iter().map(TopicCard::from).filter(|c| visible(c, include_archived)).collect(),
            questions: vec![],
            links: vec![],
            references: vec![],
        });
    };
    let detail = topics::get(pool, id).await?;
    let node = topics::tree(pool, Some(id), Some(1), now).await?;
    let children = node
        .first()
        .map(|n| n.children.iter().map(TopicCard::from).filter(|c| visible(c, include_archived)).collect())
        .unwrap_or_default();
    let topic_ids = if all { topics::subtree_ids(pool, id).await? } else { vec![id] };
    let mut links = vec![];
    for l in &detail.links {
        let outgoing = l.from_topic_id == id;
        let other = topics::find(pool, if outgoing { l.to_topic_id } else { l.from_topic_id }).await?;
        links.push(LinkView { topic: TopicRef { id: other.id, name: other.name }, kind: l.kind, weight: l.weight, outgoing });
    }
    let mut refs = vec![];
    for sid in &detail.source_ids {
        let (src, _, _) = references::get(pool, *sid).await?;
        refs.push(ReferenceRef { id: src.id, title: src.title, kind: src.kind });
    }
    Ok(TopicPage {
        breadcrumbs: detail.path.iter().map(|t| TopicRef { id: t.id, name: t.name.clone() }).collect(),
        children,
        questions: question_rows(pool, &topic_ids, now).await?,
        links,
        references: refs,
        topic: Some(detail.topic),
    })
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct QuestionPage {
    pub detail: QuestionDetail,
    pub topic_refs: Vec<TopicRef>,
    pub pinned: bool,
    pub recall_now: Option<f32>,
    pub source_citation: Option<String>,
    /// Set when this question was reworded: the newest wording's id.
    pub newest_revision_id: Option<i64>,
}

pub async fn question_page(pool: &SqlitePool, id: i64, now: DateTime<Utc>) -> Result<QuestionPage> {
    let detail = questions::get(pool, id).await?;
    let topic_refs = topics::get(pool, detail.question.topic_id)
        .await?
        .path
        .into_iter()
        .map(|t| TopicRef { id: t.id, name: t.name })
        .collect();
    let mut newest = None;
    let mut cursor = id;
    while let Some(next) = sqlx::query_scalar::<_, i64>("SELECT id FROM questions WHERE revision_of = ? ORDER BY id DESC LIMIT 1")
        .bind(cursor)
        .fetch_optional(pool)
        .await?
    {
        newest = Some(next);
        cursor = next;
    }
    let source_citation = match detail.question.source_id {
        Some(sid) => {
            let (src, _, _) = references::get(pool, sid).await?;
            Some(match &detail.question.source_locator {
                Some(loc) => format!("{}, {loc}", src.title),
                None => src.title,
            })
        }
        None => None,
    };
    Ok(QuestionPage {
        pinned: pins::is_pinned(pool, id).await?,
        recall_now: detail.state.retrievability(now),
        topic_refs,
        source_citation,
        newest_revision_id: newest,
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::questions::{QuestionPatch, QuestionStatus};
    use crate::testutil::{self, utc};
    use crate::topics::{self, TopicPatch, TopicStatus};

    fn now() -> chrono::DateTime<chrono::Utc> {
        utc("2026-10-24T18:00:00Z")
    }

    async fn tree() -> (sqlx::SqlitePool, i64, i64, i64) {
        let p = test_pool().await;
        let root = testutil::topic(&p, None, "Circuits").await;
        let child = testutil::topic(&p, Some(root), "Impedance").await;
        let other = testutil::topic(&p, None, "Tools").await;
        (p, root, child, other)
    }

    #[tokio::test]
    async fn root_page_lists_top_level_cards() {
        let (p, root, _, other) = tree().await;
        let page = topic_page(&p, None, false, false, now()).await.unwrap();
        assert!(page.topic.is_none() && page.breadcrumbs.is_empty() && page.questions.is_empty());
        let ids: Vec<i64> = page.children.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![root, other]);
    }

    #[tokio::test]
    async fn topic_page_breadcrumbs_children_and_direct_questions() {
        let (p, root, child, _) = tree().await;
        let own = testutil::question(&p, root).await;
        testutil::question(&p, child).await;
        let page = topic_page(&p, Some(root), false, false, now()).await.unwrap();
        assert_eq!(page.breadcrumbs.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["Circuits"]);
        assert_eq!(page.children.iter().map(|c| c.id).collect::<Vec<_>>(), vec![child]);
        assert_eq!(page.children[0].active_questions, 1);
        assert_eq!(page.questions.iter().map(|q| q.id).collect::<Vec<_>>(), vec![own]);
        let deep = topic_page(&p, Some(child), false, false, now()).await.unwrap();
        assert_eq!(deep.breadcrumbs.iter().map(|t| t.id).collect::<Vec<_>>(), vec![root, child]);
    }

    #[tokio::test]
    async fn topic_page_all_includes_subtree() {
        let (p, root, child, _) = tree().await;
        testutil::question(&p, root).await;
        testutil::question(&p, child).await;
        let page = topic_page(&p, Some(root), true, false, now()).await.unwrap();
        assert_eq!(page.questions.len(), 2);
    }

    #[tokio::test]
    async fn archived_children_hidden_unless_requested() {
        let (p, root, child, _) = tree().await;
        topics::update(&p, child, TopicPatch { status: Some(TopicStatus::Archived), ..Default::default() }, now()).await.unwrap();
        assert!(topic_page(&p, Some(root), false, false, now()).await.unwrap().children.is_empty());
        assert_eq!(topic_page(&p, Some(root), false, true, now()).await.unwrap().children.len(), 1);
    }

    #[tokio::test]
    async fn rows_never_include_answer_or_rubric() {
        let (p, root, _, _) = tree().await;
        let q = testutil::question(&p, root).await;
        crate::pins::pin(&p, q, now()).await.unwrap();
        let page = topic_page(&p, Some(root), true, true, now()).await.unwrap();
        assert!(page.questions[0].pinned);
        let json = serde_json::to_string(&page).unwrap();
        assert!(!json.contains("\"answer\"") && !json.contains("\"rubric\""), "{json}");
    }

    #[tokio::test]
    async fn links_resolved_with_direction() {
        let (p, root, child, other) = tree().await;
        topics::link(&p, child, other, crate::types::LinkKind::Applies, 0.7, String::new(), now()).await.unwrap();
        let page = topic_page(&p, Some(child), false, false, now()).await.unwrap();
        assert_eq!(page.links.len(), 1);
        assert_eq!((page.links[0].topic.name.as_str(), page.links[0].outgoing), ("Tools", true));
        let back = topic_page(&p, Some(other), false, false, now()).await.unwrap();
        assert!(!back.links[0].outgoing);
        let _ = root;
    }

    #[tokio::test]
    async fn question_page_newest_revision_followed() {
        let (p, root, _, _) = tree().await;
        let v1 = testutil::question(&p, root).await;
        let v2 = crate::questions::update(&p, v1, QuestionPatch { prompt: Some("v2".into()), ..Default::default() }, now()).await.unwrap();
        let v3 = crate::questions::update(&p, v2.id, QuestionPatch { prompt: Some("v3".into()), ..Default::default() }, now()).await.unwrap();
        assert_eq!(question_page(&p, v1, now()).await.unwrap().newest_revision_id, Some(v3.id));
        assert_eq!(question_page(&p, v3.id, now()).await.unwrap().newest_revision_id, None);
        assert_eq!(question_page(&p, v1, now()).await.unwrap().detail.question.status, QuestionStatus::Retired);
    }

    #[tokio::test]
    async fn question_page_recall_and_pin() {
        let (p, root, _, _) = tree().await;
        let q = testutil::question(&p, root).await;
        let fresh = question_page(&p, q, now()).await.unwrap();
        assert_eq!((fresh.pinned, fresh.recall_now), (false, None));
        assert_eq!(fresh.topic_refs.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["Circuits"]);
        testutil::due_state(&p, q, "2026-10-24", 3.0).await;
        crate::pins::pin(&p, q, now()).await.unwrap();
        let page = question_page(&p, q, now()).await.unwrap();
        assert!(page.pinned);
        let r = page.recall_now.unwrap();
        assert!(r > 0.0 && r < 1.0, "{r}");
    }
}
