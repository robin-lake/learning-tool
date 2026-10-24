//! The MCP server Claude Code talks to over stdio.

mod params;
mod tools;

use learning_core::db::DataDir;
use learning_core::settings::Settings;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt, tool_handler};
use serde::Serialize;
use sqlx::SqlitePool;

const INSTRUCTIONS: &str = "Spaced-repetition study tools for the learner's local knowledge base. \
Use start_session/next_question/record_review/end_session to run study sessions, the topic and question \
tools to build topics, and the reference tools to store and search sources. Never reveal a question's \
rubric or answer before the learner has attempted it.";

#[derive(Clone)]
pub struct LearningServer {
    pool: SqlitePool,
    dirs: DataDir,
    tool_router: ToolRouter<Self>,
}

impl LearningServer {
    pub fn new(pool: SqlitePool, dirs: DataDir) -> Self {
        Self { pool, dirs, tool_router: Self::tool_router() }
    }

    async fn settings(&self) -> Result<Settings, McpError> {
        Settings::load(&self.pool).await.map_err(internal)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for LearningServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("learning-tool", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

fn internal(e: learning_core::Error) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

/// A one-line summary followed by the result as pretty JSON; input problems become tool errors
/// Claude can read and fix.
pub(crate) fn respond<T: Serialize>(
    result: learning_core::Result<T>,
    summary: impl FnOnce(&T) -> String,
) -> Result<CallToolResult, McpError> {
    use learning_core::Error;
    match result {
        Ok(value) => {
            let json = serde_json::to_string_pretty(&value).map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![ContentBlock::text(format!("{}\n\n{json}", summary(&value)))]))
        }
        Err(e @ (Error::Invalid(_) | Error::NotFound { .. } | Error::Conflict(_))) => {
            Ok(CallToolResult::error(vec![ContentBlock::text(e.to_string())]))
        }
        Err(e) => Err(internal(e)),
    }
}

pub async fn serve(pool: SqlitePool, dirs: DataDir) -> anyhow::Result<()> {
    let service = LearningServer::new(pool, dirs).serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
