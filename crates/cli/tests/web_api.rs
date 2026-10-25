//! In-process tests of the web API: build the router on a temp database and drive it with `oneshot`.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use learning_core::db::{self, DataDir};
use learning_tool::web::{AppState, router, serve};
use serde_json::{Value, json};
use tower::ServiceExt;

struct Env {
    _tmp: tempfile::TempDir,
    state: AppState,
}

async fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = DataDir { root: tmp.path().to_path_buf() };
    dirs.ensure().unwrap();
    let pool = db::open(&dirs.db_path()).await.unwrap();
    Env { _tmp: tmp, state: AppState { pool, dirs } }
}

async fn call(e: &Env, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value, String) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty))
        .unwrap();
    let res = router(e.state.clone()).oneshot(req).await.unwrap();
    let status = res.status();
    let ctype = res.headers().get("content-type").map(|v| v.to_str().unwrap().to_string()).unwrap_or_default();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, value, ctype)
}

/// A topic with two questions; returns (topic_id, question ids).
async fn seed(e: &Env) -> (i64, Vec<i64>) {
    let (_, topic, _) = call(e, "POST", "/api/topics", Some(json!({"name": "Impedance", "target_level": 1}))).await;
    let topic_id = topic["id"].as_i64().unwrap();
    let s = learning_core::settings::Settings::load(&e.state.pool).await.unwrap();
    let q = |p: &str| learning_core::questions::NewQuestion {
        level: 0,
        prompt: p.into(),
        answer: "a".into(),
        rubric: learning_core::grading::Rubric(vec![learning_core::grading::RubricPoint {
            id: "r1".into(),
            point: "p".into(),
            weight: 1.0,
            essential: false,
        }]),
        pass_threshold: None,
        source_id: None,
        source_locator: None,
        priority: None,
        guide_version_id: None,
        secondary_topic_ids: vec![],
    };
    let added = learning_core::questions::add(
        &e.state.pool,
        topic_id,
        vec![q("one?"), q("two?")],
        learning_core::questions::QuestionStatus::Active,
        &s,
        chrono::Utc::now(),
    )
    .await
    .unwrap();
    (topic_id, added.ids)
}

#[tokio::test]
async fn home_returns_summary() {
    let e = env().await;
    seed(&e).await;
    let (status, body, ctype) = call(&e, "GET", "/api/home", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.starts_with("application/json"));
    assert_eq!(body["new_waiting"], 2);
    assert!(body["today"]["streak"].is_number());
}

#[tokio::test]
async fn activity_defaults_to_last_year() {
    let e = env().await;
    let (status, body, _) = call(&e, "GET", "/api/activity", None).await;
    assert_eq!((status, body), (StatusCode::OK, json!([])));
    let (status, _, _) = call(&e, "GET", "/api/activity?from=2026-01-01&to=2026-12-31", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(&e, "GET", "/api/activity?from=garbage", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn root_topics_lists_cards() {
    let e = env().await;
    let (topic_id, _) = seed(&e).await;
    let (status, body, _) = call(&e, "GET", "/api/topics", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["topic"].is_null());
    assert_eq!(body["children"][0]["id"], topic_id);
    assert_eq!(body["children"][0]["active_questions"], 2);
}

#[tokio::test]
async fn topic_page_with_all_flag() {
    let e = env().await;
    let (topic_id, _) = seed(&e).await;
    let (_, sub, _) = call(&e, "POST", "/api/topics", Some(json!({"parent_id": topic_id, "name": "Sub", "target_level": 1}))).await;
    let (status, body, _) = call(&e, "GET", &format!("/api/topics/{topic_id}?all=true"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["questions"].as_array().unwrap().len(), 2);
    assert_eq!(body["children"][0]["id"], sub["id"]);
    assert_eq!(body["breadcrumbs"][0]["name"], "Impedance");
}

#[tokio::test]
async fn create_and_rename_and_archive_topic() {
    let e = env().await;
    let (topic_id, _) = seed(&e).await;
    let (status, body, _) = call(&e, "PATCH", &format!("/api/topics/{topic_id}"), Some(json!({"name": "Complex impedance"}))).await;
    assert_eq!((status, body["name"].clone()), (StatusCode::OK, json!("Complex impedance")));
    let (_, body, _) = call(&e, "PATCH", &format!("/api/topics/{topic_id}"), Some(json!({"status": "archived"}))).await;
    assert_eq!(body["status"], "archived");
    let (_, root, _) = call(&e, "GET", "/api/topics", None).await;
    assert!(root["children"].as_array().unwrap().is_empty());
    let (_, root, _) = call(&e, "GET", "/api/topics?archived=true", None).await;
    assert_eq!(root["children"].as_array().unwrap().len(), 1);
    let (status, body, _) = call(&e, "POST", "/api/topics", Some(json!({"name": "!!!", "target_level": 1}))).await;
    assert_eq!((status, body["error"]["kind"].clone()), (StatusCode::BAD_REQUEST, json!("invalid")));
}

#[tokio::test]
async fn question_page_includes_history() {
    let e = env().await;
    let (_, ids) = seed(&e).await;
    let (status, body, _) = call(&e, "GET", &format!("/api/questions/{}", ids[0]), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["detail"]["question"]["prompt"], "one?");
    assert!(body["detail"]["reviews"].as_array().unwrap().is_empty());
    assert_eq!(body["topic_refs"][0]["name"], "Impedance");
}

#[tokio::test]
async fn rewording_returns_new_revision() {
    let e = env().await;
    let (_, ids) = seed(&e).await;
    let (status, body, _) = call(&e, "PATCH", &format!("/api/questions/{}", ids[0]), Some(json!({"prompt": "One, reworded?"}))).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(body["id"], ids[0]);
    assert_eq!(body["revision_of"], ids[0]);
    let (_, old, _) = call(&e, "GET", &format!("/api/questions/{}", ids[0]), None).await;
    assert_eq!(old["newest_revision_id"], body["id"]);
}

#[tokio::test]
async fn status_rejects_retired_with_400() {
    let e = env().await;
    let (_, ids) = seed(&e).await;
    let uri = format!("/api/questions/{}/status", ids[0]);
    let (status, _, _) = call(&e, "POST", &uri, Some(json!({"status": "retired"}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(&e, "POST", &uri, Some(json!({"status": "suspended"}))).await;
    assert!(status.is_success());
    let (_, q, _) = call(&e, "GET", &format!("/api/questions/{}", ids[0]), None).await;
    assert_eq!(q["detail"]["question"]["status"], "suspended");
}

#[tokio::test]
async fn rating_validates_range_400() {
    let e = env().await;
    let (_, ids) = seed(&e).await;
    let uri = format!("/api/questions/{}/ratings", ids[0]);
    let (status, _, _) = call(&e, "POST", &uri, Some(json!({"usefulness": 9}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(&e, "POST", &uri, Some(json!({"usefulness": 4, "feedback": "useful"}))).await;
    assert!(status.is_success());
    let (_, q, _) = call(&e, "GET", &format!("/api/questions/{}", ids[0]), None).await;
    assert_eq!(q["detail"]["ratings"][0]["usefulness"], 4);
}

#[tokio::test]
async fn pin_put_and_delete_are_idempotent() {
    let e = env().await;
    let (_, ids) = seed(&e).await;
    let uri = format!("/api/questions/{}/pin", ids[1]);
    assert!(call(&e, "PUT", &uri, None).await.0.is_success());
    assert!(call(&e, "PUT", &uri, None).await.0.is_success());
    let (_, home, _) = call(&e, "GET", "/api/home", None).await;
    assert_eq!(home["pinned"].as_array().unwrap().len(), 1);
    assert!(call(&e, "DELETE", &uri, None).await.0.is_success());
    assert!(call(&e, "DELETE", &uri, None).await.0.is_success());
    let (_, q, _) = call(&e, "GET", &format!("/api/questions/{}", ids[1]), None).await;
    assert_eq!(q["pinned"], false);
}

#[tokio::test]
async fn unknown_api_path_is_json_404() {
    let e = env().await;
    let (status, body, _) = call(&e, "GET", "/api/nope", None).await;
    assert_eq!((status, body["error"]["kind"].clone()), (StatusCode::NOT_FOUND, json!("not_found")));
}

#[tokio::test]
async fn spa_fallback_serves_index_html() {
    let e = env().await;
    let (status, body, ctype) = call(&e, "GET", "/topics/3", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.starts_with("text/html"), "{ctype}");
    assert!(body.as_str().unwrap().contains("<html"));
}

#[tokio::test]
async fn missing_question_is_404_with_error_shape() {
    let e = env().await;
    let (status, body, _) = call(&e, "GET", "/api/questions/999", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["kind"], "not_found");
    assert!(body["error"]["message"].as_str().unwrap().contains("999"));
}

#[tokio::test]
async fn bind_error_mentions_port_flag() {
    let e = env().await;
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    let err = serve(e.state.pool.clone(), e.state.dirs.clone(), "127.0.0.1".parse().unwrap(), port, false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains(&format!("port {port} is in use")) && err.contains("--port"), "{err}");
}

#[tokio::test]
async fn map_timeline_shape() {
    let e = env().await;
    let (topic_id, ids) = seed(&e).await;
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO sessions (kind, study_day, started_at, created_at, updated_at) VALUES ('main', date('now', '-2 days'), datetime('now'), datetime('now'), datetime('now')) RETURNING id",
    )
    .fetch_one(&e.state.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO reviews (question_id, session_id, reviewed_at, grade, score, rubric_result, user_answer, asked_prompt, created_at, updated_at) VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 3, 1.0, '[]', 'a', 'q', 'x', 'x')")
        .bind(ids[0])
        .bind(sid)
        .execute(&e.state.pool)
        .await
        .unwrap();
    let (status, body, _) = call(&e, "GET", "/api/map/timeline", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let days = body["days"].as_array().unwrap().len();
    assert!(days >= 1);
    let nodes = body["nodes"].as_array().unwrap();
    let topic = nodes.iter().find(|n| n["id"] == format!("t{topic_id}")).unwrap();
    assert_eq!(topic["series"]["questions"].as_array().unwrap().len(), days);
    let q = nodes.iter().find(|n| n["id"] == format!("q{}", ids[0])).unwrap();
    assert_eq!(q["parent"], format!("t{topic_id}"));
    assert_eq!(body["stale_after_days"], 14);
}

#[tokio::test]
async fn responses_are_gzipped_when_asked() {
    let e = env().await;
    let req = axum::http::Request::builder()
        .uri("/api/map/timeline")
        .header("accept-encoding", "gzip")
        .header("host", "localhost")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = tower::ServiceExt::oneshot(router(e.state.clone()), req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers().get("content-encoding").map(|v| v.to_str().unwrap()), Some("gzip"));
}
