use super::*;

pub(super) async fn record_failed_session(
    app: &AppHandle,
    employee_id: &str,
    task_id: Option<&str>,
    working_dir: Option<&str>,
    resume_session_id: Option<&str>,
    session_kind: CodexSessionKind,
    message: &str,
) {
    if let Ok(record) = insert_codex_session_record(
        app,
        Some(employee_id),
        task_id,
        None,
        working_dir,
        resume_session_id,
        session_kind.as_str(),
        "failed",
        EXECUTION_TARGET_LOCAL,
        None,
        None,
        ARTIFACT_CAPTURE_MODE_LOCAL_FULL,
        None,
        None,
    )
    .await
    {
        if let Ok(pool) = sqlite_pool(app).await {
            let _ =
                insert_codex_session_event(&pool, &record.id, "validation_failed", Some(message))
                    .await;
        }
    }
}

pub(super) async fn bind_cli_session_id(
    app: &AppHandle,
    employee_id: &str,
    task_id: Option<&String>,
    session_record_id: &str,
    session_kind: CodexSessionKind,
    cli_session_id: String,
) {
    let _ = update_codex_session_record(
        app,
        session_record_id,
        None,
        Some(Some(cli_session_id.as_str())),
        None,
        None,
    )
    .await;
    if let Ok(pool) = sqlite_pool(app).await {
        let _ = insert_codex_session_event(
            &pool,
            session_record_id,
            "cli_session_bound",
            Some(&format!("CLI 会话已绑定: {}", cli_session_id)),
        )
        .await;
    }
    let _ = app.emit(
        "codex-session",
        CodexSession {
            employee_id: employee_id.to_string(),
            task_id: task_id.cloned(),
            session_kind: session_kind.as_str().to_string(),
            session_record_id: session_record_id.to_string(),
            session_id: cli_session_id,
        },
    );
}

pub(super) async fn fetch_task_activity_context(
    pool: &sqlx::SqlitePool,
    task_id: &str,
) -> Result<(String, String), String> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT title, project_id FROM tasks WHERE id = $1 AND deleted_at IS NULL LIMIT 1",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        format!(
            "Failed to resolve task {} for activity log: {}",
            task_id, error
        )
    })
}

pub(super) async fn write_task_session_activity(
    app: &AppHandle,
    pool: &sqlx::SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: CodexSessionKind,
    resume_session_id: Option<&str>,
) {
    let Some(task_id) = task_id else {
        return;
    };

    let result = async {
        let (task_title, project_id) = fetch_task_activity_context(pool, task_id).await?;
        let session = fetch_codex_session_by_id(app, session_record_id).await?;
        let action = if session.execution_target == EXECUTION_TARGET_SSH
            && session_kind == CodexSessionKind::Execution
        {
            "remote_task_session_started"
        } else {
            session_kind.activity_start_action(resume_session_id.is_some())
        };

        insert_activity_log(
            pool,
            action,
            &task_title,
            Some(employee_id),
            Some(task_id),
            Some(project_id.as_str()),
        )
        .await
    }
    .await;

    if let Err(error) = result {
        let _ = insert_codex_session_event(
            pool,
            session_record_id,
            "activity_log_failed",
            Some(&error),
        )
        .await;
        let _ = app.emit(
            "codex-stdout",
            CodexOutput {
                employee_id: employee_id.to_string(),
                task_id: Some(task_id.to_string()),
                session_kind: session_kind.as_str().to_string(),
                session_record_id: session_record_id.to_string(),
                session_event_id: None,
                line: format!("[WARN] 活动日志写入失败: {}", error),
            },
        );
    }
}
