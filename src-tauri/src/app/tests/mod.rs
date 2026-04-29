pub(super) use std::collections::{HashMap, HashSet};
pub(super) use std::fs;
pub(super) use std::process::Command;

pub(super) use sqlx::{self, SqlitePool};

pub(super) use super::{
    build_current_migrator, build_remote_codex_runtime_health, build_remote_shell_command,
    build_task_completion_timer_update, build_task_review_context_from_git_outputs,
    build_task_review_prompt, clear_task_automation_state_for_disabled_mode,
    collect_local_task_review_context_for_task, compare_global_search_items,
    disable_task_automation_for_archived_task, ensure_statement_terminated,
    fetch_execution_change_history_item_by_session_id, fetch_task_automation_state_record,
    fetch_task_by_id, filter_image_attachments, insert_task_record,
    is_task_automation_active_for_archival, normalize_global_search_types,
    normalize_runtime_path_string, record_completion_metric, record_task_review_requested_activity,
    remote_shell_path_expression, remote_task_attachment_dir, remote_task_attachment_path,
    resolve_project_task_default_settings, resolve_running_conflict_message,
    resolve_session_resume_state, rewrite_file_change_diff_labels, sanitize_sql_backup_script,
    sdk_notification_unavailable, should_clear_task_completed_at, start_task_timer_internal,
    stop_task_timer_internal, task_attachment_is_image, validate_project_repo_path,
    validate_runtime_working_dir, validate_task_archival_guard,
    validate_task_automation_mode_change, CodexSettings, GlobalSearchItem, Project, Task,
    TaskAttachment, PROJECT_TYPE_LOCAL, PROJECT_TYPE_SSH, TASK_AUTOMATION_MODE_REVIEW_FIX_LOOP_V1,
    TASK_AUTOMATION_PHASE_COMMITTING_CODE, TASK_AUTOMATION_PHASE_LAUNCHING_FIX,
    TASK_AUTOMATION_PHASE_LAUNCHING_REVIEW, TASK_AUTOMATION_PHASE_WAITING_EXECUTION,
    TASK_AUTOMATION_PHASE_WAITING_REVIEW, TASK_STATUS_ARCHIVED,
};
pub(super) use crate::db::models::GitPreferences;

mod review_and_attachments;
mod runtime_and_paths;
mod sql_and_session;
mod task_lifecycle;

pub(super) async fn setup_test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("create sqlite memory pool");
    let migrator = build_current_migrator();
    let mut connection = pool.acquire().await.expect("acquire sqlite connection");
    migrator
        .run_direct(&mut *connection)
        .await
        .expect("run migrations");
    drop(connection);
    pool
}

pub(super) async fn insert_session(
    pool: &SqlitePool,
    session_id: &str,
    cli_session_id: Option<&str>,
    session_kind: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO codex_sessions (
            id,
            cli_session_id,
            session_kind,
            status,
            started_at,
            created_at
        ) VALUES ($1, $2, $3, 'exited', '2026-04-16 10:00:00', '2026-04-16 10:00:00')
        "#,
    )
    .bind(session_id)
    .bind(cli_session_id)
    .bind(session_kind)
    .execute(pool)
    .await
    .expect("insert session");
}

pub(super) async fn insert_session_started_event(
    pool: &SqlitePool,
    session_id: &str,
    message: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO codex_session_events (
            id,
            session_id,
            event_type,
            message,
            created_at
        ) VALUES ($1, $2, 'session_started', $3, '2026-04-16 10:00:01')
        "#,
    )
    .bind(format!("event-{session_id}"))
    .bind(session_id)
    .bind(message)
    .execute(pool)
    .await
    .expect("insert session started event");
}

pub(super) async fn insert_file_change(
    pool: &SqlitePool,
    change_id: &str,
    session_id: &str,
    path: &str,
    capture_mode: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO codex_session_file_changes (
            id,
            session_id,
            path,
            change_type,
            capture_mode,
            previous_path,
            created_at
        ) VALUES ($1, $2, $3, 'modified', $4, NULL, '2026-04-16 10:00:02')
        "#,
    )
    .bind(change_id)
    .bind(session_id)
    .bind(path)
    .bind(capture_mode)
    .execute(pool)
    .await
    .expect("insert file change");
}

pub(super) async fn insert_project(pool: &SqlitePool, project_id: &str) {
    sqlx::query(
        r#"
        INSERT INTO projects (
            id,
            name,
            description,
            status,
            repo_path,
            created_at,
            updated_at
        ) VALUES ($1, $2, NULL, 'active', NULL, '2026-04-16 10:00:00', '2026-04-16 10:00:00')
        "#,
    )
    .bind(project_id)
    .bind(format!("Project {project_id}"))
    .execute(pool)
    .await
    .expect("insert project");
}

pub(super) async fn insert_employee(pool: &SqlitePool, employee_id: &str, name: &str, role: &str) {
    sqlx::query(
        r#"
        INSERT INTO employees (
            id,
            name,
            role,
            model,
            reasoning_effort,
            status,
            specialization,
            system_prompt,
            project_id,
            created_at,
            updated_at
        ) VALUES ($1, $2, $3, 'gpt-5.4', 'high', 'offline', NULL, NULL, NULL, '2026-04-16 10:00:00', '2026-04-16 10:00:00')
        "#,
    )
    .bind(employee_id)
    .bind(name)
    .bind(role)
    .execute(pool)
    .await
    .expect("insert employee");
}
