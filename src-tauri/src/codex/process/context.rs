use tauri::{AppHandle, Runtime};

pub(crate) use crate::engine::ExecutionContext;

const ENGINE_LABEL: &str = "Codex";

pub(super) async fn resolve_task_project_execution_context<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
) -> Result<ExecutionContext, String> {
    crate::engine::context::resolve_task_project_execution_context(app, task_id, ENGINE_LABEL).await
}

pub(super) async fn resolve_project_execution_context<R: Runtime>(
    app: &AppHandle<R>,
    project_id: &str,
) -> Result<ExecutionContext, String> {
    crate::engine::context::resolve_project_execution_context(app, project_id, ENGINE_LABEL).await
}

pub(super) async fn resolve_one_shot_working_dir<R: Runtime>(
    app: &AppHandle<R>,
    task_id: Option<&str>,
    project_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<Option<String>, String> {
    crate::engine::context::resolve_one_shot_working_dir(
        app,
        task_id,
        project_id,
        working_dir,
        ENGINE_LABEL,
    )
    .await
}

pub(crate) async fn resolve_session_execution_context<R: Runtime>(
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
