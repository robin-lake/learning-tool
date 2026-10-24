use std::path::PathBuf;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection, SqlitePool};

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
    let mut tx = crate::db::begin_write(pool).await?;
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
            count = insert_chunks(&mut tx, id, text).await?;
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

/// Chunks `text` and indexes it for reference `id` (the FTS index follows via triggers).
async fn insert_chunks(conn: &mut SqliteConnection, id: i64, text: &str) -> Result<u32> {
    let mut count = 0;
    for (ordinal, c) in chunk(text, DEFAULT_MAX_CHARS).into_iter().enumerate() {
        sqlx::query("INSERT INTO reference_chunks (source_id, locator, ordinal, text) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(&c.locator)
            .bind(ordinal as i64)
            .bind(&c.text)
            .execute(&mut *conn)
            .await?;
        count += 1;
    }
    Ok(count)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct AttachFiles {
    /// Absolute path of the original file (e.g. a PDF of a book you now own).
    #[serde(default)]
    pub source_file: Option<PathBuf>,
    /// Absolute path of its extracted text (pdftotext output or Markdown); replaces the searchable text.
    #[serde(default)]
    pub text_file: Option<PathBuf>,
    /// Allow replacing a file the reference already has.
    #[serde(default)]
    pub replace: bool,
}

/// Adds files to an existing reference (e.g. a book stored as metadata only), keeping its id so
/// questions that cite it stay linked. New text replaces the reference's chunks.
pub async fn attach(
    pool: &SqlitePool,
    dirs: &DataDir,
    id: i64,
    files: AttachFiles,
    now: DateTime<Utc>,
) -> Result<(Source, u32)> {
    if files.source_file.is_none() && files.text_file.is_none() {
        return Err(Error::Invalid("attach needs a source_file, a text_file, or both".into()));
    }
    require_file(&files.source_file)?;
    require_file(&files.text_file)?;
    let current = find(pool, id).await?;
    if !files.replace {
        if files.source_file.is_some() && current.file_path.is_some() {
            return Err(Error::Conflict(format!(
                "reference {id} already has an original file; pass replace=true to swap it"
            )));
        }
        if files.text_file.is_some() && current.text_path.is_some() {
            return Err(Error::Conflict(format!(
                "reference {id} already has extracted text; pass replace=true to swap it"
            )));
        }
    }
    let text = match &files.text_file {
        Some(p) => Some(String::from_utf8_lossy(&std::fs::read(p)?).into_owned()),
        None => None,
    };
    let dir = dirs.references_dir().join(id.to_string());
    std::fs::create_dir_all(&dir)?;

    // Stage copies under temporary names; they only replace the real files after the commit.
    let original_name = files.source_file.as_ref().map(|p| {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("bin").to_lowercase();
        format!("original.{ext}")
    });
    let mut staged: Vec<(PathBuf, PathBuf)> = vec![];
    if let (Some(src), Some(name)) = (&files.source_file, &original_name) {
        staged.push((src.clone(), dir.join(name)));
    }
    if let Some(src) = &files.text_file {
        staged.push((src.clone(), dir.join("text.md")));
    }
    let incoming = |path: &PathBuf| path.with_extension("incoming");
    let result: Result<u32> = async {
        for (src, dest) in &staged {
            std::fs::copy(src, incoming(dest))?;
        }
        let mut tx = crate::db::begin_write(pool).await?;
        let file_path = original_name.as_ref().map(|n| format!("references/{id}/{n}"));
        let text_path = text.as_ref().map(|_| format!("references/{id}/text.md"));
        let count = match &text {
            Some(text) => {
                sqlx::query("DELETE FROM reference_chunks WHERE source_id = ?").bind(id).execute(&mut *tx).await?;
                insert_chunks(&mut tx, id, text).await?
            }
            None => sqlx::query_scalar::<_, i64>("SELECT count(*) FROM reference_chunks WHERE source_id = ?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await? as u32,
        };
        sqlx::query(
            "UPDATE sources SET file_path = COALESCE(?, file_path), text_path = COALESCE(?, text_path), updated_at = ?
             WHERE id = ?",
        )
        .bind(&file_path)
        .bind(&text_path)
        .bind(ts(now))
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(count)
    }
    .await;
    let count = match result {
        Ok(count) => count,
        Err(e) => {
            for (_, dest) in &staged {
                let _ = std::fs::remove_file(incoming(dest));
            }
            return Err(e);
        }
    };
    for (_, dest) in &staged {
        std::fs::rename(incoming(dest), dest)?;
    }
    // A replaced original with a different extension leaves the old file behind; remove it.
    if let (Some(old), Some(new)) = (&current.file_path, &original_name) {
        if !old.ends_with(new.as_str()) {
            let _ = std::fs::remove_file(dirs.root.join(old));
        }
    }
    Ok((find(pool, id).await?, count))
}

/// Links a reference to more topics (links are never removed; see DB-R4). Returns all its topic ids.
pub async fn link_topics(pool: &SqlitePool, id: i64, topic_ids: Vec<i64>) -> Result<Vec<i64>> {
    if topic_ids.is_empty() {
        return Err(Error::Invalid("give at least one topic id to link".into()));
    }
    find(pool, id).await?;
    for t in &topic_ids {
        topics::find(pool, *t).await?;
    }
    let mut tx = crate::db::begin_write(pool).await?;
    for t in &topic_ids {
        sqlx::query("INSERT OR IGNORE INTO reference_topics (source_id, topic_id) VALUES (?, ?)")
            .bind(id)
            .bind(t)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(get(pool, id).await?.1)
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

    fn book() -> NewSource {
        let mut n = new_source(None, vec![]);
        n.kind = SourceKind::Book;
        n.title = "Hornresp manual".into();
        n
    }

    #[tokio::test]
    async fn attach_adds_files_to_metadata_only_reference() {
        let e = env();
        let p = test_pool().await;
        let (src, _) = add(&p, &e.dirs, book(), now()).await.unwrap();
        std::fs::write(e.src.join("manual.pdf"), b"%PDF fake").unwrap();
        std::fs::write(e.src.join("manual.txt"), "throat chamber\x0cmouth area").unwrap();
        let files = AttachFiles { source_file: Some(e.src.join("manual.pdf")), text_file: Some(e.src.join("manual.txt")), replace: false };
        let (src2, chunks) = attach(&p, &e.dirs, src.id, files, now()).await.unwrap();
        assert_eq!((src2.id, chunks), (src.id, 2));
        assert_eq!(src2.kind, SourceKind::Book);
        assert!(e.dirs.root.join(src2.file_path.unwrap()).exists());
        assert_eq!(search(&p, "throat", None, 10).await.unwrap()[0].locator, "p. 1");
    }

    #[tokio::test]
    async fn attach_refuses_to_overwrite_without_replace() {
        let e = env();
        let p = test_pool().await;
        std::fs::write(e.src.join("old.txt"), "old words").unwrap();
        let (src, _) = add(&p, &e.dirs, new_source(Some(e.src.join("old.txt")), vec![]), now()).await.unwrap();
        std::fs::write(e.src.join("new.txt"), "fresh words").unwrap();
        let files = || AttachFiles { source_file: None, text_file: Some(e.src.join("new.txt")), replace: false };
        assert!(matches!(attach(&p, &e.dirs, src.id, files(), now()).await, Err(Error::Conflict(_))));
        assert_eq!(search(&p, "old", None, 10).await.unwrap().len(), 1);
        let (_, n) = attach(&p, &e.dirs, src.id, AttachFiles { replace: true, ..files() }, now()).await.unwrap();
        assert_eq!(n, 1);
        assert!(search(&p, "old", None, 10).await.unwrap().is_empty());
        assert_eq!(search(&p, "fresh", None, 10).await.unwrap().len(), 1);
        let text = std::fs::read_to_string(e.dirs.root.join(format!("references/{}/text.md", src.id))).unwrap();
        assert_eq!(text, "fresh words");
    }

    #[tokio::test]
    async fn attach_replacing_original_with_new_extension_removes_old_file() {
        let e = env();
        let p = test_pool().await;
        std::fs::write(e.src.join("page.html"), "<p>x</p>").unwrap();
        let mut n = new_source(None, vec![]);
        n.source_file = Some(e.src.join("page.html"));
        let (src, _) = add(&p, &e.dirs, n, now()).await.unwrap();
        std::fs::write(e.src.join("doc.pdf"), b"%PDF").unwrap();
        let files = AttachFiles { source_file: Some(e.src.join("doc.pdf")), text_file: None, replace: true };
        let (src2, _) = attach(&p, &e.dirs, src.id, files, now()).await.unwrap();
        let dir = e.dirs.references_dir().join(src.id.to_string());
        assert!(dir.join("original.pdf").exists());
        assert!(!dir.join("original.html").exists());
        assert_eq!(src2.file_path.as_deref(), Some(format!("references/{}/original.pdf", src.id).as_str()));
    }

    #[tokio::test]
    async fn attach_validates_input() {
        let e = env();
        let p = test_pool().await;
        let (src, _) = add(&p, &e.dirs, book(), now()).await.unwrap();
        let none = AttachFiles { source_file: None, text_file: None, replace: false };
        assert!(matches!(attach(&p, &e.dirs, src.id, none, now()).await, Err(Error::Invalid(_))));
        let missing = AttachFiles { source_file: Some(e.src.join("nope.pdf")), text_file: None, replace: false };
        let err = attach(&p, &e.dirs, src.id, missing, now()).await.unwrap_err();
        assert!(matches!(&err, Error::Invalid(m) if m.contains("nope.pdf")), "{err}");
        std::fs::write(e.src.join("a.pdf"), b"%PDF").unwrap();
        let ok = AttachFiles { source_file: Some(e.src.join("a.pdf")), text_file: None, replace: false };
        assert!(matches!(attach(&p, &e.dirs, 999, ok, now()).await, Err(Error::NotFound { .. })));
        let leftovers: Vec<_> = std::fs::read_dir(e.dirs.references_dir()).unwrap().flatten().collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[tokio::test]
    async fn link_topics_adds_links_idempotently() {
        let e = env();
        let p = test_pool().await;
        let a = testutil::topic(&p, None, "A").await;
        let b = testutil::topic(&p, None, "B").await;
        let (src, _) = add(&p, &e.dirs, new_source(None, vec![a]), now()).await.unwrap();
        assert_eq!(link_topics(&p, src.id, vec![b, a]).await.unwrap(), vec![a, b]);
        assert_eq!(link_topics(&p, src.id, vec![b]).await.unwrap(), vec![a, b]);
        assert_eq!(get(&p, src.id).await.unwrap().1, vec![a, b]);
    }

    #[tokio::test]
    async fn link_topics_validates_before_writing() {
        let e = env();
        let p = test_pool().await;
        let a = testutil::topic(&p, None, "A").await;
        let (src, _) = add(&p, &e.dirs, new_source(None, vec![]), now()).await.unwrap();
        assert!(matches!(link_topics(&p, src.id, vec![a, 999]).await, Err(Error::NotFound { .. })));
        assert!(get(&p, src.id).await.unwrap().1.is_empty());
        assert!(matches!(link_topics(&p, 999, vec![a]).await, Err(Error::NotFound { .. })));
        assert!(matches!(link_topics(&p, src.id, vec![]).await, Err(Error::Invalid(_))));
    }
}
