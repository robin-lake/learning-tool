//! `/api` handlers: parse input, call `learning-core`, serialize.

use std::collections::HashMap;

use axum::extract::{FromRequest, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{Duration, NaiveDate, Utc};
use learning_core::pins::{self, Pin};
use learning_core::questions::{self, Question, QuestionPatch, QuestionStatus};
use learning_core::settings::Settings;
use learning_core::stats::{self, DayActivity, HomeSummary};
use learning_core::topics::{self, NewTopic, Topic, TopicPatch, TopicStatus};
use learning_core::views::{self, QuestionPage, TopicPage};
use learning_core::guide;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::AppState;
use super::error::ApiError;

type ApiResult<T> = Result<Json<T>, ApiError>;

/// A JSON request body whose parse errors are reported in the API's error shape.
pub struct Body<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for Body<T> {
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, ApiError> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(v)) => Ok(Body(v)),
            Err(rejection) => Err(ApiError::invalid(rejection.body_text())),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateTopicBody {
    pub parent_id: Option<i64>,
    pub name: String,
    pub target_level: u8,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTopicBody {
    pub name: Option<String>,
    pub description: Option<String>,
    pub target_level: Option<u8>,
    pub status: Option<TopicStatus>,
}

#[derive(Debug, Deserialize)]
pub struct StatusBody {
    pub status: QuestionStatus,
}

#[derive(Debug, Deserialize)]
pub struct RatingBody {
    pub usefulness: u8,
    pub feedback: Option<String>,
}

pub fn api() -> Router<AppState> {
    Router::new()
        .route("/home", get(home))
        .route("/activity", get(activity))
        .route("/topics", get(root_topics).post(create_topic))
        .route("/topics/{id}", get(topic).patch(update_topic))
        .route("/questions/{id}", get(question).patch(update_question))
        .route("/questions/{id}/status", post(set_status))
        .route("/questions/{id}/ratings", post(rate))
        .route("/questions/{id}/pin", put(pin).delete(unpin))
        .fallback(|| async { ApiError::not_found("no such API endpoint") })
}

async fn settings(st: &AppState) -> Result<Settings, ApiError> {
    Ok(Settings::load(&st.pool).await?)
}

fn flag(q: &HashMap<String, String>, key: &str) -> bool {
    q.get(key).is_some_and(|v| v == "true" || v == "1")
}

fn date(q: &HashMap<String, String>, key: &str) -> Result<Option<NaiveDate>, ApiError> {
    q.get(key)
        .map(|v| v.parse().map_err(|_| ApiError::invalid(format!("{key} must be a YYYY-MM-DD date, got {v:?}"))))
        .transpose()
}

async fn home(State(st): State<AppState>) -> ApiResult<HomeSummary> {
    let s = settings(&st).await?;
    Ok(Json(stats::overview(&st.pool, &s, Utc::now()).await?))
}

async fn activity(State(st): State<AppState>, Query(q): Query<HashMap<String, String>>) -> ApiResult<Vec<DayActivity>> {
    let s = settings(&st).await?;
    let to = date(&q, "to")?.unwrap_or_else(|| s.study_day(Utc::now()));
    let from = date(&q, "from")?.unwrap_or(to - Duration::days(364));
    Ok(Json(stats::activity(&st.pool, from, to).await?))
}

async fn root_topics(State(st): State<AppState>, Query(q): Query<HashMap<String, String>>) -> ApiResult<TopicPage> {
    Ok(Json(views::topic_page(&st.pool, None, false, flag(&q, "archived"), Utc::now()).await?))
}

async fn topic(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<TopicPage> {
    Ok(Json(views::topic_page(&st.pool, Some(id), flag(&q, "all"), flag(&q, "archived"), Utc::now()).await?))
}

async fn create_topic(State(st): State<AppState>, Body(b): Body<CreateTopicBody>) -> ApiResult<Topic> {
    let new = NewTopic {
        parent_id: b.parent_id,
        name: b.name,
        description: b.description.unwrap_or_default(),
        target_level: b.target_level,
        position: None,
    };
    Ok(Json(topics::create(&st.pool, new, Utc::now()).await?))
}

async fn update_topic(State(st): State<AppState>, Path(id): Path<i64>, Body(b): Body<UpdateTopicBody>) -> ApiResult<Topic> {
    let patch = TopicPatch {
        name: b.name,
        description: b.description,
        target_level: b.target_level,
        status: b.status,
        position: None,
        parent_id: None,
    };
    Ok(Json(topics::update(&st.pool, id, patch, Utc::now()).await?))
}

async fn question(State(st): State<AppState>, Path(id): Path<i64>) -> ApiResult<QuestionPage> {
    Ok(Json(views::question_page(&st.pool, id, Utc::now()).await?))
}

async fn update_question(State(st): State<AppState>, Path(id): Path<i64>, Body(p): Body<QuestionPatch>) -> ApiResult<Question> {
    Ok(Json(questions::update(&st.pool, id, p, Utc::now()).await?))
}

async fn set_status(State(st): State<AppState>, Path(id): Path<i64>, Body(b): Body<StatusBody>) -> Result<StatusCode, ApiError> {
    if !matches!(b.status, QuestionStatus::Active | QuestionStatus::Suspended) {
        return Err(ApiError::invalid(format!("the web app can only set active or suspended, not {}", b.status)));
    }
    questions::set_status(&st.pool, id, b.status, Utc::now()).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn rate(State(st): State<AppState>, Path(id): Path<i64>, Body(b): Body<RatingBody>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let rating = guide::rate(&st.pool, id, b.usefulness, b.feedback, None, None, Utc::now()).await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": rating }))))
}

async fn pin(State(st): State<AppState>, Path(id): Path<i64>) -> ApiResult<Pin> {
    Ok(Json(pins::pin(&st.pool, id, Utc::now()).await?))
}

async fn unpin(State(st): State<AppState>, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    pins::unpin(&st.pool, id, Utc::now()).await?;
    Ok(StatusCode::NO_CONTENT)
}
