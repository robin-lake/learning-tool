use std::collections::HashMap;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

pub use crate::types::{LinkKind, Scope};
use crate::types::{conflict_on_unique, str_enum, ts, validate_level, validate_weight};
use crate::{Error, Result};

str_enum!(TopicStatus { Active => "active", Archived => "archived" });

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Topic {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub scope: Scope,
    pub project_id: Option<i64>,
    pub target_level: u8,
    pub status: TopicStatus,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NewTopic {
    /// Parent topic id; omit for a top-level topic.
    #[serde(default)]
    pub parent_id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Long-term target mastery level, 0-3.
    pub target_level: u8,
    /// Order among siblings; defaults to after the last sibling.
    #[serde(default)]
    pub position: Option<i64>,
}

/// Fields to change; `None` leaves a field as is. `parent_id: Some(None)` moves to top level.
#[derive(Debug, Clone, Default)]
pub struct TopicPatch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub target_level: Option<u8>,
    pub status: Option<TopicStatus>,
    pub position: Option<i64>,
    pub parent_id: Option<Option<i64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TopicLink {
    pub id: i64,
    pub from_topic_id: i64,
    pub to_topic_id: i64,
    pub kind: LinkKind,
    pub weight: f32,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TopicDetail {
    pub topic: Topic,
    /// Root first, ending with this topic.
    pub path: Vec<Topic>,
    pub children: Vec<Topic>,
    pub links: Vec<TopicLink>,
    pub source_ids: Vec<i64>,
    pub question_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TopicNode {
    pub topic: Topic,
    pub depth: u32,
    /// Active questions in this topic's whole subtree, by level 0-3.
    pub questions_by_level: [u32; 4],
    pub active_questions: u32,
    /// Active questions in the subtree with at least one review.
    pub studied: u32,
    /// Mean current recall probability over reviewed active questions in the subtree.
    pub avg_retrievability: Option<f32>,
    pub children: Vec<TopicNode>,
}

pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    slug.trim_end_matches('-').to_string()
}

fn topic_from_row(r: &SqliteRow) -> Result<Topic> {
    Ok(Topic {
        id: r.try_get("id")?,
        parent_id: r.try_get("parent_id")?,
        name: r.try_get("name")?,
        slug: r.try_get("slug")?,
        description: r.try_get("description")?,
        scope: Scope::parse(r.try_get("scope")?)?,
        project_id: r.try_get("project_id")?,
        target_level: r.try_get::<i64, _>("target_level")? as u8,
        status: TopicStatus::parse(r.try_get("status")?)?,
        position: r.try_get("position")?,
    })
}

fn link_from_row(r: &SqliteRow) -> Result<TopicLink> {
    Ok(TopicLink {
        id: r.try_get("id")?,
        from_topic_id: r.try_get("from_topic_id")?,
        to_topic_id: r.try_get("to_topic_id")?,
        kind: LinkKind::parse(r.try_get("kind")?)?,
        weight: r.try_get::<f64, _>("weight")? as f32,
        note: r.try_get("note")?,
    })
}

pub async fn find(pool: &SqlitePool, id: i64) -> Result<Topic> {
    let row = sqlx::query("SELECT * FROM topics WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(Error::NotFound { entity: "topic", id })?;
    topic_from_row(&row)
}

async fn all_topics(pool: &SqlitePool) -> Result<Vec<Topic>> {
    let rows = sqlx::query("SELECT * FROM topics ORDER BY position, id")
        .fetch_all(pool)
        .await?;
    rows.iter().map(topic_from_row).collect()
}

fn valid_slug(name: &str) -> Result<String> {
    let slug = slugify(name);
    if slug.is_empty() {
        return Err(Error::Invalid(format!("topic name {name:?} needs at least one letter or digit")));
    }
    Ok(slug)
}

pub async fn create(pool: &SqlitePool, t: NewTopic, now: DateTime<Utc>) -> Result<Topic> {
    validate_level(t.target_level)?;
    let slug = valid_slug(&t.name)?;
    let (scope, project_id) = match t.parent_id {
        Some(pid) => {
            let parent = find(pool, pid).await?;
            (parent.scope, parent.project_id)
        }
        None => (Scope::Main, None),
    };
    let position = match t.position {
        Some(p) => p,
        None => sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM topics WHERE parent_id IS ?",
        )
        .bind(t.parent_id)
        .fetch_one(pool)
        .await?,
    };
    let stamp = ts(now);
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO topics (parent_id, name, slug, description, scope, project_id, target_level, position, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(t.parent_id)
    .bind(t.name.trim())
    .bind(&slug)
    .bind(&t.description)
    .bind(scope.as_str())
    .bind(project_id)
    .bind(t.target_level as i64)
    .bind(position)
    .bind(&stamp)
    .bind(&stamp)
    .fetch_one(pool)
    .await
    .map_err(|e| conflict_on_unique(e, || format!("a sibling topic with slug {slug:?} already exists")))?;
    find(pool, id).await
}

/// Ids of `id` and all its descendants.
pub(crate) async fn subtree_ids(pool: &SqlitePool, id: i64) -> Result<Vec<i64>> {
    Ok(sqlx::query_scalar(
        "WITH RECURSIVE sub(id) AS (SELECT ?1 UNION ALL SELECT t.id FROM topics t JOIN sub ON t.parent_id = sub.id)
         SELECT id FROM sub",
    )
    .bind(id)
    .fetch_all(pool)
    .await?)
}

pub async fn update(pool: &SqlitePool, id: i64, patch: TopicPatch, now: DateTime<Utc>) -> Result<Topic> {
    let mut t = find(pool, id).await?;
    if let Some(name) = patch.name {
        t.slug = valid_slug(&name)?;
        t.name = name.trim().to_string();
    }
    if let Some(d) = patch.description {
        t.description = d;
    }
    if let Some(level) = patch.target_level {
        validate_level(level)?;
        t.target_level = level;
    }
    if let Some(s) = patch.status {
        t.status = s;
    }
    if let Some(p) = patch.position {
        t.position = p;
    }
    if let Some(parent) = patch.parent_id {
        if let Some(pid) = parent {
            find(pool, pid).await?;
            if subtree_ids(pool, id).await?.contains(&pid) {
                return Err(Error::Invalid("a topic cannot be moved under itself or its descendants".into()));
            }
        }
        t.parent_id = parent;
    }
    let slug = t.slug.clone();
    sqlx::query(
        "UPDATE topics SET parent_id = ?, name = ?, slug = ?, description = ?, target_level = ?, status = ?, position = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(t.parent_id)
    .bind(&t.name)
    .bind(&t.slug)
    .bind(&t.description)
    .bind(t.target_level as i64)
    .bind(t.status.as_str())
    .bind(t.position)
    .bind(ts(now))
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| conflict_on_unique(e, || format!("a sibling topic with slug {slug:?} already exists")))?;
    find(pool, id).await
}

pub async fn get(pool: &SqlitePool, id: i64) -> Result<TopicDetail> {
    let topic = find(pool, id).await?;
    let mut path = vec![topic.clone()];
    let mut cursor = topic.parent_id;
    while let Some(pid) = cursor {
        let parent = find(pool, pid).await?;
        cursor = parent.parent_id;
        path.push(parent);
    }
    path.reverse();
    let children = sqlx::query("SELECT * FROM topics WHERE parent_id = ? ORDER BY position, id")
    .bind(id)
    .fetch_all(pool)
    .await?
    .iter()
    .map(topic_from_row)
    .collect::<Result<Vec<_>>>()?;
    let links = sqlx::query("SELECT * FROM topic_links WHERE from_topic_id = ?1 OR to_topic_id = ?1 ORDER BY id")
        .bind(id)
        .fetch_all(pool)
        .await?
        .iter()
        .map(link_from_row)
        .collect::<Result<Vec<_>>>()?;
    let source_ids = sqlx::query_scalar("SELECT source_id FROM reference_topics WHERE topic_id = ? ORDER BY source_id")
        .bind(id)
        .fetch_all(pool)
        .await?;
    let question_ids = sqlx::query_scalar(
        "SELECT id FROM questions WHERE topic_id = ? AND status <> 'retired' ORDER BY priority, id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(TopicDetail { topic, path, children, links, source_ids, question_ids })
}

pub async fn tree(
    pool: &SqlitePool,
    root: Option<i64>,
    depth: Option<u32>,
    now: DateTime<Utc>,
) -> Result<Vec<TopicNode>> {
    let topics = all_topics(pool).await?;
    let mut by_parent: HashMap<Option<i64>, Vec<&Topic>> = HashMap::new();
    for t in &topics {
        by_parent.entry(t.parent_id).or_default().push(t);
    }
    // Direct (not subtree) counts per topic, active questions only.
    let mut direct: HashMap<i64, [u32; 4]> = HashMap::new();
    let rows: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT topic_id, level, count(*) FROM questions WHERE status = 'active' GROUP BY topic_id, level",
    )
    .fetch_all(pool)
    .await?;
    for (topic, level, n) in rows {
        direct.entry(topic).or_default()[level as usize] = n as u32;
    }
    let studied_rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT q.topic_id, count(*) FROM questions q JOIN question_state s ON s.question_id = q.id
         WHERE q.status = 'active' AND s.reps > 0 GROUP BY q.topic_id",
    )
    .fetch_all(pool)
    .await?;
    let studied_direct: HashMap<i64, u32> = studied_rows.into_iter().map(|(t, n)| (t, n as u32)).collect();
    let retr = crate::topics::retrievability_by_topic(pool, now).await?;

    struct Ctx<'a> {
        by_parent: &'a HashMap<Option<i64>, Vec<&'a Topic>>,
        direct: &'a HashMap<i64, [u32; 4]>,
        studied: &'a HashMap<i64, u32>,
        retr: &'a HashMap<i64, Vec<f32>>,
        max_depth: Option<u32>,
    }
    // Returns the node plus the subtree's retrievability samples.
    fn build(ctx: &Ctx, t: &Topic, depth: u32) -> (TopicNode, Vec<f32>) {
        let mut counts = ctx.direct.get(&t.id).copied().unwrap_or_default();
        let mut studied = ctx.studied.get(&t.id).copied().unwrap_or_default();
        let mut samples = ctx.retr.get(&t.id).cloned().unwrap_or_default();
        let mut children = vec![];
        for child in ctx.by_parent.get(&Some(t.id)).map(Vec::as_slice).unwrap_or_default() {
            let (node, s) = build(ctx, child, depth + 1);
            for (c, n) in counts.iter_mut().zip(node.questions_by_level) {
                *c += n;
            }
            studied += node.studied;
            samples.extend(s);
            if ctx.max_depth.is_none_or(|m| depth < m) {
                children.push(node);
            }
        }
        let avg = (!samples.is_empty()).then(|| samples.iter().sum::<f32>() / samples.len() as f32);
        let node = TopicNode {
            topic: t.clone(),
            depth,
            questions_by_level: counts,
            active_questions: counts.iter().sum(),
            studied,
            avg_retrievability: avg,
            children,
        };
        (node, samples)
    }

    let ctx = Ctx { by_parent: &by_parent, direct: &direct, studied: &studied_direct, retr: &retr, max_depth: depth };
    let roots: Vec<(&Topic, u32)> = match root {
        Some(id) => {
            let detail = get(pool, id).await?;
            let t = topics.iter().find(|t| t.id == id).expect("topic exists");
            vec![(t, detail.path.len() as u32 - 1)]
        }
        None => by_parent.get(&None).map(|v| v.iter().map(|t| (*t, 0)).collect()).unwrap_or_default(),
    };
    Ok(roots
        .into_iter()
        .map(|(t, d)| {
            let ctx = Ctx { max_depth: depth.map(|m| m + d), ..ctx };
            build(&ctx, t, d).0
        })
        .collect())
}

/// Current retrievability of each reviewed active question, grouped by topic.
pub(crate) async fn retrievability_by_topic(
    pool: &SqlitePool,
    now: DateTime<Utc>,
) -> Result<HashMap<i64, Vec<f32>>> {
    let rows: Vec<(i64, f64, String)> = sqlx::query_as(
        "SELECT q.topic_id, s.stability, s.last_reviewed_at
         FROM questions q JOIN question_state s ON s.question_id = q.id
         WHERE q.status = 'active' AND s.stability IS NOT NULL AND s.last_reviewed_at IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;
    let mut out: HashMap<i64, Vec<f32>> = HashMap::new();
    for (topic, stability, last) in rows {
        let days = (now - crate::types::parse_ts(&last)?).num_seconds().max(0) as f32 / 86_400.0;
        out.entry(topic).or_default().push(crate::scheduler::retrievability(stability as f32, days));
    }
    Ok(out)
}

pub async fn link(
    pool: &SqlitePool,
    from: i64,
    to: i64,
    kind: LinkKind,
    weight: f32,
    note: String,
    now: DateTime<Utc>,
) -> Result<TopicLink> {
    validate_weight(weight)?;
    if from == to {
        return Err(Error::Invalid("a topic cannot link to itself".into()));
    }
    find(pool, from).await?;
    find(pool, to).await?;
    let (from, to) = if kind == LinkKind::Related && from > to { (to, from) } else { (from, to) };
    let stamp = ts(now);
    let row = sqlx::query(
        "INSERT INTO topic_links (from_topic_id, to_topic_id, kind, weight, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT (from_topic_id, to_topic_id, kind) DO UPDATE SET weight = ?4, note = ?5, updated_at = ?6
         RETURNING *",
    )
    .bind(from)
    .bind(to)
    .bind(kind.as_str())
    .bind(weight as f64)
    .bind(note)
    .bind(stamp)
    .fetch_one(pool)
    .await?;
    link_from_row(&row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::Error;
    use chrono::{DateTime, Utc};

    fn now() -> DateTime<Utc> {
        "2026-10-23T18:00:00Z".parse().unwrap()
    }
    fn nt(parent: Option<i64>, name: &str) -> NewTopic {
        NewTopic { parent_id: parent, name: name.into(), description: String::new(), target_level: 1, position: None }
    }

    #[tokio::test]
    async fn create_nested_and_get_path() {
        let p = test_pool().await;
        let a = create(&p, nt(None, "Physics"), now()).await.unwrap();
        let b = create(&p, nt(Some(a.id), "Acoustics & Waves"), now()).await.unwrap();
        let c = create(&p, nt(Some(b.id), "Horns"), now()).await.unwrap();
        assert_eq!(b.slug, "acoustics-waves");
        assert_eq!(c.scope, Scope::Main);
        let d = get(&p, c.id).await.unwrap();
        let names: Vec<_> = d.path.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Physics", "Acoustics & Waves", "Horns"]);
        let d = get(&p, a.id).await.unwrap();
        assert_eq!(d.children.len(), 1);
        assert!(matches!(get(&p, 999).await, Err(Error::NotFound { .. })));
    }

    #[tokio::test]
    async fn duplicate_sibling_slug_conflicts() {
        let p = test_pool().await;
        create(&p, nt(None, "Physics"), now()).await.unwrap();
        assert!(matches!(create(&p, nt(None, "physics"), now()).await, Err(Error::Conflict(_))));
    }

    #[tokio::test]
    async fn same_slug_under_different_parents_ok() {
        let p = test_pool().await;
        let a = create(&p, nt(None, "A"), now()).await.unwrap();
        let b = create(&p, nt(None, "B"), now()).await.unwrap();
        create(&p, nt(Some(a.id), "Basics"), now()).await.unwrap();
        create(&p, nt(Some(b.id), "Basics"), now()).await.unwrap();
    }

    #[tokio::test]
    async fn invalid_names_and_levels_rejected() {
        let p = test_pool().await;
        assert!(matches!(create(&p, nt(None, "!!!"), now()).await, Err(Error::Invalid(_))));
        let mut t = nt(None, "X");
        t.target_level = 4;
        assert!(matches!(create(&p, t, now()).await, Err(Error::Invalid(_))));
        assert!(matches!(create(&p, nt(Some(42), "Y"), now()).await, Err(Error::NotFound { .. })));
    }

    #[tokio::test]
    async fn reparent_into_descendant_rejected() {
        let p = test_pool().await;
        let a = create(&p, nt(None, "A"), now()).await.unwrap();
        let b = create(&p, nt(Some(a.id), "B"), now()).await.unwrap();
        let c = create(&p, nt(Some(b.id), "C"), now()).await.unwrap();
        let patch = TopicPatch { parent_id: Some(Some(c.id)), ..Default::default() };
        assert!(matches!(update(&p, a.id, patch, now()).await, Err(Error::Invalid(_))));
        let patch = TopicPatch { parent_id: Some(Some(a.id)), ..Default::default() };
        assert!(matches!(update(&p, a.id, patch, now()).await, Err(Error::Invalid(_))));
        let patch = TopicPatch { parent_id: Some(None), name: Some("C2".into()), ..Default::default() };
        let moved = update(&p, c.id, patch, now()).await.unwrap();
        assert_eq!((moved.parent_id, moved.slug.as_str()), (None, "c2"));
    }

    #[tokio::test]
    async fn related_link_normalized_and_upserted() {
        let p = test_pool().await;
        let ids: Vec<i64> = futures_ids(&p, 5).await;
        let (two, five) = (ids[1], ids[4]);
        link(&p, five, two, LinkKind::Related, 0.4, String::new(), now()).await.unwrap();
        let l = link(&p, two, five, LinkKind::Related, 0.9, "close".into(), now()).await.unwrap();
        assert_eq!((l.from_topic_id, l.to_topic_id, l.weight), (two, five, 0.9));
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM topic_links").fetch_one(&p).await.unwrap();
        assert_eq!(n, 1);
        // prerequisite is directed and kept as given
        let l = link(&p, five, two, LinkKind::Prerequisite, 0.5, String::new(), now()).await.unwrap();
        assert_eq!((l.from_topic_id, l.to_topic_id), (five, two));
        assert_eq!(get(&p, two).await.unwrap().links.len(), 2);
    }

    #[tokio::test]
    async fn self_link_and_bad_weight_rejected() {
        let p = test_pool().await;
        let ids = futures_ids(&p, 2).await;
        assert!(matches!(link(&p, ids[0], ids[0], LinkKind::Related, 0.5, String::new(), now()).await, Err(Error::Invalid(_))));
        assert!(matches!(link(&p, ids[0], ids[1], LinkKind::Related, 1.5, String::new(), now()).await, Err(Error::Invalid(_))));
        assert!(matches!(link(&p, ids[0], 99, LinkKind::Related, 0.5, String::new(), now()).await, Err(Error::NotFound { .. })));
    }

    #[tokio::test]
    async fn tree_counts_by_level_and_depth_limit() {
        let p = test_pool().await;
        let a = create(&p, nt(None, "A"), now()).await.unwrap();
        let b = create(&p, nt(Some(a.id), "B"), now()).await.unwrap();
        let ts = now().to_rfc3339();
        for (topic, level, status) in [(a.id, 0, "active"), (b.id, 1, "active"), (b.id, 1, "active"), (b.id, 2, "proposed")] {
            sqlx::query("INSERT INTO questions (topic_id, level, prompt, answer, rubric, status, created_at, updated_at) VALUES (?, ?, 'q', 'a', '[]', ?, ?, ?)")
                .bind(topic).bind(level).bind(status).bind(&ts).bind(&ts).execute(&p).await.unwrap();
        }
        let full = tree(&p, None, None, now()).await.unwrap();
        assert_eq!(full.len(), 1);
        assert_eq!(full[0].questions_by_level, [1, 2, 0, 0]); // subtree, active only
        assert_eq!(full[0].active_questions, 3);
        assert_eq!(full[0].children[0].questions_by_level, [0, 2, 0, 0]);
        assert_eq!(full[0].children[0].depth, 1);
        let shallow = tree(&p, None, Some(0), now()).await.unwrap();
        assert!(shallow[0].children.is_empty());
        assert_eq!(shallow[0].active_questions, 3);
        let sub = tree(&p, Some(b.id), None, now()).await.unwrap();
        assert_eq!((sub.len(), sub[0].topic.id, sub[0].depth), (1, b.id, 1));
    }

    async fn futures_ids(p: &SqlitePool, n: usize) -> Vec<i64> {
        let mut ids = vec![];
        for i in 0..n {
            ids.push(create(p, nt(None, &format!("T{i}")), now()).await.unwrap().id);
        }
        ids
    }

    #[tokio::test]
    async fn tree_counts_studied_questions() {
        let p = test_pool().await;
        let a = create(&p, nt(None, "A"), now()).await.unwrap();
        let b = create(&p, nt(Some(a.id), "B"), now()).await.unwrap();
        let q1 = crate::testutil::question(&p, b.id).await;
        crate::testutil::question(&p, b.id).await;
        crate::testutil::due_state(&p, q1, "2026-10-25", 3.0).await;
        let t = tree(&p, None, None, now()).await.unwrap();
        assert_eq!((t[0].studied, t[0].active_questions, t[0].children[0].studied), (1, 2, 1));
    }
}
