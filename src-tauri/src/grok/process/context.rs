use tauri::{AppHandle, Runtime};

pub(super) use crate::engine::ExecutionContext;

const ENGINE_LABEL: &str = "Grok";

pub(super) async fn resolve_session_execution_context<R: Runtime>(
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
