use tauri::{AppHandle, Runtime};

pub(crate) use crate::engine::ExecutionContext;

const ENGINE_LABEL: &str = "OpenCode";

pub(crate) async fn resolve_opencode_session_context<R: Runtime>(
    app: &AppHandle<R>,
    task_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<ExecutionContext, String> {
    crate::engine::context::resolve_session_execution_context(
        app,
        task_id,
        working_dir,
        ENGINE_LABEL,
    )
    .await
}
