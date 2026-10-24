//! The web app's server: a JSON API under `/api` plus the embedded React app.

mod assets;
mod error;
mod routes;

use std::net::IpAddr;

use axum::Router;
use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::response::Response;
use learning_core::db::DataDir;
use sqlx::SqlitePool;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub dirs: DataDir,
}

/// Where authentication would go if the app is ever deployed; local use needs none.
async fn auth_seam(req: Request, next: Next) -> Response {
    next.run(req).await
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .nest("/api", routes::api())
        .fallback(assets::static_handler)
        .layer(middleware::from_fn(auth_seam))
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Serves the app until Ctrl+C, optionally opening the browser.
pub async fn serve(pool: SqlitePool, dirs: DataDir, host: IpAddr, port: u16, open_browser: bool) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind((host, port))
        .await
        .map_err(|e| anyhow::anyhow!("port {port} is in use or unavailable ({e}); choose another with --port"))?;
    let shown_host = if host.is_loopback() { "localhost".to_string() } else { host.to_string() };
    let url = format!("http://{shown_host}:{port}/");
    println!("Learning tool running at {url} (Ctrl+C to stop)");
    if open_browser {
        let target = url.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = open::that(&target) {
                tracing::warn!("couldn't open a browser: {e}");
            }
        });
    }
    axum::serve(listener, router(AppState { pool, dirs }))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
