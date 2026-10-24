use std::path::PathBuf;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::chunking::{Chunk, DEFAULT_MAX_CHARS, chunk};
pub use crate::db::DataDir;
use crate::topics;
use crate::types::{parse_ts, str_enum, ts};
use crate::{Error, Result};

str_enum!(SourceKind {
    Pdf => "pdf",
    Article => "article",
    Book => "book",
    Docs => "docs",
    Video => "video",
    Other => "other",
});

str_enum!(SourceStatus { Active => "active", Archived => "archived" });

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct Source {
    pub id: i64,
    pub title: String,
    pub authors: String,
    pub kind: SourceKind,
    pub url: Option<String>,
    /// Original file, relative to the data directory.
    pub file_path: Option<String>,
    /// Extracted text, relative to the data directory.
    pub text_path: Option<String>,
    pub summary: String,
    pub status: SourceStatus,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NewSource {
    pub title: String,
    #[serde(default)]
    pub authors: String,
    pub kind: SourceKind,
    #[serde(default)]
    pub url: Option<String>,
    /// Absolute path of the original file (PDF, HTML…) to copy into the library.
    #[serde(default)]
    pub source_file: Option<PathBuf>,
    /// Absolute path of the extracted text (pdftotext output or Markdown) to copy and index.
    #[serde(default)]
    pub text_file: Option<PathBuf>,
    /// Notes on the reference's essence and what it's good for.
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub topic_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq)]
pub struct Hit {
    pub source_id: i64,
    pub title: String,
    pub locator: String,
    /// Matching excerpt with matched terms in [brackets].
    pub snippet: String,
    pub rank: f64,
}

fn source_from_row(r: &SqliteRow) -> Result<Source> {
    Ok(Source {
        id: r.try_get("id")?,
        title: r.try_get("title")?,
        authors: r.try_get("authors")?,
        kind: SourceKind::parse(r.try_get("kind")?)?,
        url: r.try_get("url")?,
        file_path: r.try_get("file_path")?,
        text_path: r.try_get("text_path")?,
        summary: r.try_get("summary")?,
        status: SourceStatus::parse(r.try_get("status")?)?,
        added_at: parse_ts(r.try_get("added_at")?)?,
    })
}

async fn find(pool: &SqlitePool, id: i64) -> Result<Source> {
    let row = sqlx::query("SELECT * FROM sources WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(Error::NotFound { entity: "reference", id })?;
    source_from_row(&row)
}

fn require_file(path: &Option<PathBuf>) -> Result<()> {
    match path {
        Some(p) if !p.is_file() => Err(Error::Invalid(format!("file not found: {}", p.display()))),
        _ => Ok(()),
    }
}

/// Adds a reference: copies its files into `references/<id>/`, chunks and indexes the text,
/// and links it to topics. Nothing is written if any step fails.
pub async fn add(pool: &SqlitePool, dirs: &DataDir, n: NewSource, now: DateTime<Utc>) -> Result<(Source, u32)> {
    if n.title.trim().is_empty() {
        return Err(Error::Invalid("reference title must not be empty".into()));
    }
    require_file(&n.source_file)?;
    require_file(&n.text_file)?;
    for t in &n.topic_ids {
        topics::find(pool, *t).await?;
    }
    let text = match &n.text_file {
        Some(p) => Some(String::from_utf8_lossy(&std::fs::read(p)?).into_owned()),
        None => None,
    };
    let stamp = ts(now);
    let mut tx = pool.begin().await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO sources (title, authors, kind, url, summary, added_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?6) RETURNING id",
    )
    .bind(n.title.trim())
    .bind(&n.authors)
    .bind(n.kind.as_str())
    .bind(&n.url)
    .bind(&n.summary)
    .bind(&stamp)
    .fetch_one(&mut *tx)
    .await?;
    let dir = dirs.references_dir().join(id.to_string());
    let result: Result<u32> = async {
        let rel = |name: &str| format!("references/{id}/{name}");
        let mut file_path = None;
        let mut text_path = None;
        if n.source_file.is_some() || n.text_file.is_some() {
            std::fs::create_dir_all(&dir)?;
        }
        if let Some(src) = &n.source_file {
            let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("bin").to_lowercase();
            let name = format!("original.{ext}");
            std::fs::copy(src, dir.join(&name))?;
            file_path = Some(rel(&name));
        }
        let mut count = 0;
        if let (Some(src), Some(text)) = (&n.text_file, &text) {
            std::fs::copy(src, dir.join("text.md"))?;
            text_path = Some(rel("text.md"));
            for (ordinal, c) in chunk(text, DEFAULT_MAX_CHARS).into_iter().enumerate() {
                sqlx::query("INSERT INTO reference_chunks (source_id, locator, ordinal, text) VALUES (?, ?, ?, ?)")
                    .bind(id)
                    .bind(&c.locator)
                    .bind(ordinal as i64)
                    .bind(&c.text)
                    .execute(&mut *tx)
                    .await?;
                count += 1;
            }
        }
        sqlx::query("UPDATE sources SET file_path = ?, text_path = ? WHERE id = ?")
            .bind(&file_path)
            .bind(&text_path)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for t in &n.topic_ids {
            sqlx::query("INSERT OR IGNORE INTO reference_topics (source_id, topic_id) VALUES (?, ?)")
                .bind(id)
                .bind(t)
                .execute(&mut *tx)
                .await?;
        }
        Ok(count)
    }
    .await;
    let committed = match result {
        Ok(count) => tx.commit().await.map(|_| count).map_err(Error::from),
        Err(e) => Err(e),
    };
    match committed {
        Ok(count) => Ok((find(pool, id).await?, count)),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            Err(e)
        }
    }
}

/// Quotes each whitespace-separated term so user input can't break FTS5 query syntax.
fn fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

pub async fn search(pool: &SqlitePool, query: &str, topic_id: Option<i64>, limit: u32) -> Result<Vec<Hit>> {
    let q = fts_query(query);
    if q.is_empty() {
        return Ok(vec![]);
    }
    let topic_ids = match topic_id {
        Some(t) => Some(serde_json::to_string(&topics::subtree_ids(pool, t).await?).expect("serializes")),
        None => None,
    };
    let rows: Vec<(i64, String, String, String, f64)> = sqlx::query_as(
        "SELECT c.source_id, s.title, c.locator,
                snippet(reference_chunks_fts, 0, '[', ']', '…', 24), bm25(reference_chunks_fts)
         FROM reference_chunks_fts
         JOIN reference_chunks c ON c.id = reference_chunks_fts.rowid
         JOIN sources s ON s.id = c.source_id
         WHERE reference_chunks_fts MATCH ?1
           AND (?2 IS NULL OR c.source_id IN (
                SELECT source_id FROM reference_topics WHERE topic_id IN (SELECT value FROM json_each(?2))))
         ORDER BY bm25(reference_chunks_fts)
         LIMIT ?3",
    )
    .bind(&q)
    .bind(topic_ids)
    .bind(limit.max(1) as i64)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(source_id, title, locator, snippet, rank)| Hit { source_id, title, locator, snippet, rank })
        .collect())
}

/// The reference, its topic ids and its chunk count.
pub async fn get(pool: &SqlitePool, id: i64) -> Result<(Source, Vec<i64>, u32)> {
    let src = find(pool, id).await?;
    let topics = sqlx::query_scalar("SELECT topic_id FROM reference_topics WHERE source_id = ? ORDER BY topic_id")
        .bind(id)
        .fetch_all(pool)
        .await?;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM reference_chunks WHERE source_id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok((src, topics, n as u32))
}

/// Chunks in order, starting at `from_locator` (or the beginning).
pub async fn read(pool: &SqlitePool, id: i64, from_locator: Option<&str>, max_chunks: u32) -> Result<Vec<Chunk>> {
    find(pool, id).await?;
    let start: i64 = match from_locator {
        Some(loc) => sqlx::query_scalar("SELECT MIN(ordinal) FROM reference_chunks WHERE source_id = ? AND locator = ?")
            .bind(id)
            .bind(loc)
            .fetch_one(pool)
            .await
            .ok()
            .flatten()
            .ok_or_else(|| Error::Invalid(format!("reference {id} has no chunk at {loc:?}")))?,
        None => 0,
    };
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT locator, text FROM reference_chunks WHERE source_id = ? AND ordinal >= ? ORDER BY ordinal LIMIT ?",
    )
    .bind(id)
    .bind(start)
    .bind(max_chunks.max(1) as i64)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(locator, text)| Chunk { locator, text }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::testutil::{self, utc};
    use crate::Error;

    struct Env {
        _tmp: tempfile::TempDir,
        dirs: DataDir,
        src: std::path::PathBuf,
    }
    fn env() -> Env {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = DataDir { root: tmp.path().join("data") };
        dirs.ensure().unwrap();
        let src = tmp.path().join("in");
        std::fs::create_dir_all(&src).unwrap();
        Env { _tmp: tmp, dirs, src }
    }
    fn new_source(text_file: Option<std::path::PathBuf>, topic_ids: Vec<i64>) -> NewSource {
        NewSource {
            title: "Horn Book".into(),
            authors: "Someone".into(),
            kind: SourceKind::Pdf,
            url: None,
            source_file: None,
            text_file,
            summary: "about horns".into(),
            topic_ids,
        }
    }
    fn now() -> chrono::DateTime<chrono::Utc> {
        utc("2026-10-23T18:00:00Z")
    }

    #[tokio::test]
    async fn add_copies_files_and_indexes() {
        let e = env();
        let p = test_pool().await;
        std::fs::write(e.src.join("book.pdf"), b"%PDF fake").unwrap();
        std::fs::write(e.src.join("book.txt"), "horn mouth\x0cthroat area").unwrap();
        let mut n = new_source(Some(e.src.join("book.txt")), vec![]);
        n.source_file = Some(e.src.join("book.pdf"));
        let (src, chunks) = add(&p, &e.dirs, n, now()).await.unwrap();
        assert_eq!(chunks, 2);
        assert_eq!(src.file_path.as_deref(), Some(format!("references/{}/original.pdf", src.id).as_str()));
        assert_eq!(src.text_path.as_deref(), Some(format!("references/{}/text.md", src.id).as_str()));
        assert!(e.dirs.root.join(src.file_path.unwrap()).exists());
        let (_, topics, n) = get(&p, src.id).await.unwrap();
        assert_eq!((topics.len(), n), (0, 2));
    }

    #[tokio::test]
    async fn search_finds_term_with_locator_and_snippet() {
        let e = env();
        let p = test_pool().await;
        std::fs::write(e.src.join("t.txt"), "the mouth flares\x0cthe throat is narrow and small").unwrap();
        add(&p, &e.dirs, new_source(Some(e.src.join("t.txt")), vec![]), now()).await.unwrap();
        let hits = search(&p, "throat", None, 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].locator.as_str(), hits[0].title.as_str()), ("p. 2", "Horn Book"));
        assert!(hits[0].snippet.contains("[throat]"), "{}", hits[0].snippet);
        // porter stemming
        assert_eq!(search(&p, "flare", None, 10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn search_with_special_chars_does_not_error() {
        let e = env();
        let p = test_pool().await;
        std::fs::write(e.src.join("t.txt"), "C++ and horn* designs").unwrap();
        add(&p, &e.dirs, new_source(Some(e.src.join("t.txt")), vec![]), now()).await.unwrap();
        for q in ["C++", "\"mouth", "horn*", "AND", "(", ""] {
            search(&p, q, None, 10).await.unwrap();
        }
    }

    #[tokio::test]
    async fn search_filters_by_topic_subtree() {
        let e = env();
        let p = test_pool().await;
        let parent = testutil::topic(&p, None, "Acoustics").await;
        let child = testutil::topic(&p, Some(parent), "Horns").await;
        let other = testutil::topic(&p, None, "Other").await;
        std::fs::write(e.src.join("a.txt"), "throat one").unwrap();
        std::fs::write(e.src.join("b.txt"), "throat two").unwrap();
        add(&p, &e.dirs, new_source(Some(e.src.join("a.txt")), vec![child]), now()).await.unwrap();
        add(&p, &e.dirs, new_source(Some(e.src.join("b.txt")), vec![other]), now()).await.unwrap();
        assert_eq!(search(&p, "throat", None, 10).await.unwrap().len(), 2);
        let hits = search(&p, "throat", Some(parent), 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.contains("one"));
    }

    #[tokio::test]
    async fn missing_file_writes_nothing() {
        let e = env();
        let p = test_pool().await;
        let err = add(&p, &e.dirs, new_source(Some(e.src.join("nope.txt")), vec![]), now()).await.unwrap_err();
        assert!(matches!(&err, Error::Invalid(m) if m.contains("nope.txt")), "{err}");
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM sources").fetch_one(&p).await.unwrap();
        assert_eq!(n, 0);
        assert_eq!(std::fs::read_dir(e.dirs.references_dir()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn non_ascii_text_searchable() {
        let e = env();
        let p = test_pool().await;
        std::fs::write(e.src.join("t.txt"), "Schrödinger würde staunen").unwrap();
        add(&p, &e.dirs, new_source(Some(e.src.join("t.txt")), vec![]), now()).await.unwrap();
        assert_eq!(search(&p, "Schrödinger", None, 10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn read_from_locator() {
        let e = env();
        let p = test_pool().await;
        std::fs::write(e.src.join("t.txt"), "one\x0ctwo\x0cthree\x0cfour").unwrap();
        let (src, _) = add(&p, &e.dirs, new_source(Some(e.src.join("t.txt")), vec![]), now()).await.unwrap();
        let c = read(&p, src.id, Some("p. 2"), 2).await.unwrap();
        assert_eq!(c.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["two", "three"]);
        assert_eq!(read(&p, src.id, None, 5).await.unwrap().len(), 4);
        assert!(matches!(read(&p, src.id, Some("p. 99"), 2).await, Err(Error::Invalid(_))));
    }

    #[tokio::test]
    async fn url_only_reference() {
        let e = env();
        let p = test_pool().await;
        let mut n = new_source(None, vec![]);
        n.kind = SourceKind::Article;
        n.url = Some("https://example.com/horns".into());
        let (src, chunks) = add(&p, &e.dirs, n, now()).await.unwrap();
        assert_eq!((chunks, src.text_path), (0, None));
    }
}
