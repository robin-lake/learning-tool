//! End-to-end: drive `learning-tool mcp` over stdio with the rmcp client.

use rmcp::model::CallToolRequestParams;
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::TokioChildProcess;
use rmcp::ServiceExt;
use serde_json::{Value, json};

const TOOLS: [&str; 29] = [
    "start_session", "next_question", "skip_question", "record_review", "end_session", "rate_question", "pin_question", "unpin_question",
    "list_topics", "get_topic", "create_topic", "update_topic", "link_topics",
    "add_questions", "update_question", "set_question_status", "link_questions", "get_question",
    "today_stats", "add_reference", "attach_reference_file", "link_reference_topics", "search_references", "get_reference", "read_reference",
    "get_guide", "list_ratings", "classify_rating", "save_guide_version",
];

type Client = RunningService<RoleClient, ()>;

/// Calls a tool; returns (is_error, parsed JSON payload or the raw text).
async fn call(client: &Client, name: &'static str, args: Value) -> (bool, Value) {
    let params = CallToolRequestParams::new(name).with_arguments(args.as_object().cloned().unwrap_or_default());
    let res = client.call_tool(params).await.expect("tool call");
    let text = res.content.first().and_then(|c| c.as_text()).map(|t| t.text.clone()).unwrap_or_default();
    let payload = text.split_once("\n\n").map(|(_, j)| j).unwrap_or(&text);
    let value = serde_json::from_str(payload).unwrap_or(Value::String(text.clone()));
    (res.is_error.unwrap_or(false), value)
}

#[tokio::test]
async fn full_session_and_references_over_stdio() {
    let home = tempfile::tempdir().unwrap();
    let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_learning-tool"));
    cmd.arg("mcp").env("LEARNING_TOOL_HOME", home.path());
    let client = ().serve(TokioChildProcess::new(cmd).unwrap()).await.unwrap();

    let names: Vec<String> = client.list_all_tools().await.unwrap().into_iter().map(|t| t.name.to_string()).collect();
    for t in TOOLS {
        assert!(names.contains(&t.to_string()), "missing tool {t}");
    }

    let (err, topic) = call(&client, "create_topic", json!({"name": "Acoustics", "target_level": 1})).await;
    assert!(!err, "{topic}");
    let topic_id = topic["id"].as_i64().unwrap();
    let q = |p: &str| json!({"level": 0, "prompt": p, "answer": "a", "rubric": [{"id": "r1", "point": "p"}]});
    let (err, added) = call(&client, "add_questions", json!({"topic_id": topic_id, "questions": [q("one?"), q("two?")]})).await;
    assert!(!err, "{added}");
    assert_eq!(added["forecast"]["new_active"], 2);

    let pinned_id = added["ids"][1].as_i64().unwrap();
    let (err, pin) = call(&client, "pin_question", json!({"question_id": pinned_id})).await;
    assert!(!err, "{pin}");

    let (_, start) = call(&client, "start_session", json!({})).await;
    assert_eq!(start["planned"], 2);
    let sid = start["session_id"].as_i64().unwrap();
    for round in 0..2 {
        let (_, nq) = call(&client, "next_question", json!({"session_id": sid})).await;
        if round == 0 {
            assert_eq!((nq["question_id"].as_i64(), nq["pinned"].as_bool()), (Some(pinned_id), Some(true)), "{nq}");
        }
        let qid = nq["question_id"].as_i64().unwrap();
        let (err, out) = call(&client, "record_review", json!({
            "session_id": sid,
            "review": {"item_id": nq["item_id"], "question_id": qid, "rubric_result": [{"id": "r1", "hit": true}], "fluent": true,
                       "user_answer": "a", "asked_prompt": "q"}
        })).await;
        assert!(!err, "{out}");
        assert_eq!(out["grade"], "easy");
    }
    let (_, done) = call(&client, "next_question", json!({"session_id": sid})).await;
    assert!(done.is_null(), "{done}");
    let (_, summary) = call(&client, "end_session", json!({"session_id": sid})).await;
    assert_eq!(summary["answered"], 2);

    let text = home.path().join("book.txt");
    std::fs::write(&text, "horn mouth\x0cthroat").unwrap();
    let (err, r) = call(&client, "add_reference", json!({
        "title": "Horns", "kind": "pdf", "text_file": text, "topic_ids": [topic_id]
    })).await;
    assert!(!err, "{r}");
    let (_, hits) = call(&client, "search_references", json!({"query": "throat"})).await;
    assert_eq!(hits[0]["locator"], "p. 2");

    // A book stored as metadata only gets its text later, keeping its id.
    let (_, book) = call(&client, "add_reference", json!({"title": "Hornresp manual", "kind": "docs"})).await;
    let book_id = book[0]["id"].as_i64().unwrap();
    let manual = home.path().join("manual.txt");
    std::fs::write(&manual, "flare rate\x0cexponential horn").unwrap();
    let (err, attached) = call(&client, "attach_reference_file", json!({"id": book_id, "text_file": manual})).await;
    assert!(!err, "{attached}");
    assert_eq!(attached[0]["id"], book_id);
    let (_, hits) = call(&client, "search_references", json!({"query": "exponential"})).await;
    assert_eq!((hits[0]["source_id"].as_i64(), hits[0]["locator"].as_str()), (Some(book_id), Some("p. 2")));
    let (err, linked) = call(&client, "link_reference_topics", json!({"id": book_id, "topic_ids": [topic_id]})).await;
    assert!(!err, "{linked}");
    assert_eq!(linked, json!([topic_id]));
    let (err, msg) = call(&client, "attach_reference_file", json!({"id": book_id, "text_file": manual})).await;
    assert!(err && msg.to_string().contains("replace"), "{msg}");

    // A second session today has nothing left; an unknown rubric id is reported as a tool error.
    let (_, again) = call(&client, "start_session", json!({})).await;
    let sid2 = again["session_id"].as_i64().unwrap();
    let (err, msg) = call(&client, "record_review", json!({
        "session_id": sid2,
        "review": {"item_id": 1, "question_id": 1, "rubric_result": [{"id": "zz", "hit": true}], "user_answer": "a", "asked_prompt": "q"}
    })).await;
    assert!(err);
    assert!(msg.to_string().contains("zz"), "{msg}");

    let backups: Vec<_> = std::fs::read_dir(home.path().join("backups")).unwrap().flatten().collect();
    assert_eq!(backups.len(), 1);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn server_starts_when_backup_dir_is_unwritable() {
    let home = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_learning-tool");
    let blocker = home.path().join("not-a-dir");
    std::fs::write(&blocker, b"file").unwrap();
    let status = std::process::Command::new(bin)
        .args(["config", "set", "backup_dir", blocker.join("backups").to_str().unwrap()])
        .env("LEARNING_TOOL_HOME", home.path())
        .status()
        .unwrap();
    assert!(status.success());

    let mut cmd = tokio::process::Command::new(bin);
    cmd.arg("mcp").env("LEARNING_TOOL_HOME", home.path());
    let client = ().serve(TokioChildProcess::new(cmd).unwrap()).await.expect("server starts despite backup failure");
    let (err, stats) = call(&client, "today_stats", json!({})).await;
    assert!(!err, "{stats}");
    client.cancel().await.unwrap();
}
