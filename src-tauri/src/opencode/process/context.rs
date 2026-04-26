use tauri::{AppHandle, Runtime};

use crate::codex::process::context::resolve_session_execution_context;
use crate::codex::process::context::ExecutionContext;

pub(crate) async fn resolve_opencode_session_context<R: Runtime>(
    app: &AppHandle<R>,
    task_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<ExecutionContext, String> {
    resolve_session_execution_context(app, task_id, working_dir).await
}
