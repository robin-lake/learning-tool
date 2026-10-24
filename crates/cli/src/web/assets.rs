use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

/// The built React app (`pnpm --dir web build`), compiled into release binaries.
#[derive(RustEmbed)]
#[folder = "../../web/dist/"]
#[exclude = "._*"]
#[exclude = "*/._*"]
struct WebDist;

fn file(path: &str) -> Option<Response> {
    let f = WebDist::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    Some(([(header::CONTENT_TYPE, mime.as_ref().to_string())], f.data).into_response())
}

/// Serves embedded files; any other path without a file extension gets `index.html` so client-side
/// routes survive a reload.
pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    if let Some(res) = file(path) {
        return res;
    }
    let looks_like_file = path.rsplit('/').next().is_some_and(|last| last.contains('.'));
    if !looks_like_file {
        if let Some(res) = file("index.html") {
            return res;
        }
    }
    (StatusCode::NOT_FOUND, "not found").into_response()
}
