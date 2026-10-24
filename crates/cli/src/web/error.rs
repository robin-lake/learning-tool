use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use learning_core::Error;
use serde_json::json;

/// An API error, sent as `{"error": {"kind", "message"}}` with a matching status.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    kind: &'static str,
    message: String,
}

impl ApiError {
    pub fn invalid(message: impl Into<String>) -> ApiError {
        ApiError { status: StatusCode::BAD_REQUEST, kind: "invalid", message: message.into() }
    }

    pub fn not_found(message: impl Into<String>) -> ApiError {
        ApiError { status: StatusCode::NOT_FOUND, kind: "not_found", message: message.into() }
    }
}

impl From<Error> for ApiError {
    fn from(e: Error) -> ApiError {
        match e {
            Error::Invalid(m) => ApiError::invalid(m),
            Error::NotFound { .. } => ApiError::not_found(e.to_string()),
            Error::Conflict(m) => ApiError { status: StatusCode::CONFLICT, kind: "conflict", message: m },
            other => {
                tracing::error!("internal error: {other}");
                ApiError {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    kind: "internal",
                    message: "internal error; see the learning-tool log".into(),
                }
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": { "kind": self.kind, "message": self.message } }))).into_response()
    }
}
