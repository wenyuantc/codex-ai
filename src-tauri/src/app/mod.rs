use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use chrono::{Duration, NaiveDateTime, Utc};
use sqlx::{
    migrate::{Migration as SqlxMigration, MigrationType as SqlxMigrationType, Migrator},
    QueryBuilder, Sqlite, SqlitePool,
};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_sql::{DbInstances, DbPool};
use tokio::io::AsyncWriteExt;
use tokio::process::Command as TokioCommand;
use uuid::Uuid;

use crate::claude::ClaudeManager;
use crate::codex::{
    delete_secret_value, determine_effective_provider, ensure_supported_node_version,
    inspect_sdk_runtime, load_codex_settings, load_remote_codex_settings, new_codex_command,
    new_ssh_command, resolve_secret_value, store_secret_value, sweep_orphan_secret_refs,
    CodexManager, CodexSessionKind, SDK_INSTALL_PACKAGE_SPECS,
};
use crate::db::models::{
    CodexHealthCheck, CodexOutput, CodexRuntimeStatus, CodexSessionFileChange,
    CodexSessionFileChangeDetail, CodexSessionFileChangeDetailRecord, CodexSessionFileChangeInput,
    CodexSessionListItem, CodexSessionLogLine, CodexSessionRecord, CodexSessionResumePreview,
    CodexSettings, Comment, CreateComment, CreateEmployee, CreateProject, CreateSshConfig,
    CreateSubtask, CreateTask, DatabaseBackupResult, DatabaseRestoreResult, Employee,
    EmployeeMetric, EmployeeRunningSession, EmployeeRuntimeStatus, GlobalSearchItem,
    GlobalSearchResponse, PasswordAuthProbeResult, Project, ReviewVerdict, SearchGlobalPayload,
    SetTaskAutomationModePayload, SshConfig, SshConfigRecord, Subtask, Task, TaskAttachment,
    TaskAutomationState, TaskAutomationStateRecord, TaskExecutionChangeHistoryItem, TaskFileRef,
    TaskLatestReview, TransientNotification, UpdateEmployee, UpdateProject, UpdateSshConfig,
    UpdateTask,
};
use crate::notifications::{
    build_task_status_notification, database_error_dedupe_key, emit_transient_notification,
    ensure_sticky_notification, publish_one_time_notification, resolve_sticky_notification,
    sdk_unavailable_dedupe_key, settings_route, ssh_health_check_dedupe_key,
    ssh_missing_selection_dedupe_key, ssh_password_probe_dedupe_key,
    ssh_selected_config_dedupe_key, transient_notification_id, NotificationDraft,
    NOTIFICATION_SEVERITY_CRITICAL, NOTIFICATION_SEVERITY_ERROR, NOTIFICATION_SEVERITY_SUCCESS,
    NOTIFICATION_SEVERITY_WARNING, NOTIFICATION_TYPE_DATABASE_ERROR,
    NOTIFICATION_TYPE_SDK_UNAVAILABLE, NOTIFICATION_TYPE_SSH_CONFIG_ERROR,
};
use crate::process_spawn::configure_std_command;

pub(crate) mod database;
pub(crate) mod delivery;
pub(crate) mod employees;
pub(crate) mod notification_sound;
pub(crate) mod projects;
pub(crate) mod remote;
pub(crate) mod review;
pub(crate) mod session_events_policy;
pub(crate) mod session_events_retention;
pub(crate) mod sessions;
pub(crate) mod shared;
pub(crate) mod tasks;
pub(crate) mod templates;

#[allow(unused_imports)]
pub(crate) use database::{
    build_current_migrator, ensure_statement_terminated, fetch_database_migration_status,
    parse_activity_date_bound, resolve_scoped_project_ids_for_stats, sanitize_sql_backup_script,
};
#[allow(unused_imports)]
pub(crate) use employees::fetch_employee_by_id;
#[allow(unused_imports)]
pub(crate) use projects::{ensure_project_exists, fetch_any_project_by_id, fetch_project_by_id};
#[allow(unused_imports)]
pub(crate) use remote::{
    build_remote_codex_runtime_health, build_remote_opencode_sdk_bridge_command,
    build_remote_shell_command, build_ssh_command, cleanup_ssh_mux_masters,
    default_remote_opencode_sdk_install_dir, ensure_remote_opencode_sdk_runtime_layout,
    ensure_remote_sdk_runtime_layout, ensure_ssh_config_exists, execute_ssh_command,
    execute_ssh_command_with_input, fetch_ssh_config_record_by_id, inspect_remote_codex_runtime,
    inspect_remote_opencode_runtime, normalize_ssh_auth_type, redact_secret_text,
    remote_opencode_sdk_bridge_path, remote_path_join, remote_sdk_bridge_path,
    remote_shell_path_expression, sdk_notification_unavailable, shell_escape_single_quoted,
    spawn_ssh_stdio_command, ssh_config_target_host_label, SshStdioProcess,
};
#[allow(unused_imports)]
pub(crate) use review::{
    build_task_attachments_from_sources, build_task_review_prompt, cleanup_empty_attachment_dir,
    cleanup_remote_task_attachment, cleanup_remote_task_attachment_paths,
    cleanup_remote_task_attachments_for_task, cleanup_task_attachment_files,
    collect_session_output_text, filter_image_attachments, parse_review_findings_json,
    parse_review_verdict_json, persist_review_session_events,
    persist_review_session_events_from_session_logs, record_task_review_requested_activity,
    remote_task_attachment_dir, remote_task_attachment_path, start_task_code_review_internal,
    sync_task_attachment_records_to_remote, sync_task_image_attachments_to_remote,
    task_attachment_dir, task_attachment_is_image, truncate_review_text,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use review::{
    build_task_review_context_from_git_outputs, collect_local_task_review_context_for_task,
    collect_task_review_context,
};
#[allow(unused_imports)]
pub(crate) use sessions::{
    apply_codex_session_usage, compare_global_search_items, fetch_codex_session_by_id,
    fetch_execution_change_history_item_by_session_id, fetch_task_latest_review,
    insert_activity_log, insert_codex_session_event, insert_codex_session_event_with_id,
    insert_codex_session_record, mark_codex_session_origin_pipeline, normalize_global_search_types,
    replace_codex_session_file_changes, resolve_running_conflict_message,
    resolve_session_resume_state, rewrite_file_change_diff_labels, update_codex_session_record,
};
#[allow(unused_imports)]
pub(crate) use shared::{
    database_path, new_id, normalize_optional_text, normalize_project_type,
    normalize_runtime_path_string, now_sqlite, parse_sqlite_datetime, path_to_runtime_string,
    resolve_existing_file_path, resolve_user_file_path, sqlite_pool, validate_project_repo_path,
    validate_remote_repo_path, validate_runtime_working_dir, DatabaseMigrationStatus,
    RemoteCodexRuntimeHealth, RemoteTaskAttachmentSyncResult, ARTIFACT_CAPTURE_MODE_LOCAL_FULL,
    ARTIFACT_CAPTURE_MODE_SSH_FULL, ARTIFACT_CAPTURE_MODE_SSH_GIT_STATUS,
    ARTIFACT_CAPTURE_MODE_SSH_NONE, DB_AUTO_IMPORT_BACKUP_PREFIX, EXECUTION_TARGET_LOCAL,
    EXECUTION_TARGET_SSH, FILE_CHANGE_DIFF_CHAR_LIMIT, GLOBAL_SEARCH_DEFAULT_LIMIT,
    GLOBAL_SEARCH_MAX_LIMIT, GLOBAL_SEARCH_MIN_QUERY_LENGTH, GLOBAL_SEARCH_TYPE_EMPLOYEE,
    GLOBAL_SEARCH_TYPE_PROJECT, GLOBAL_SEARCH_TYPE_SESSION, GLOBAL_SEARCH_TYPE_TASK,
    PROJECT_TYPE_LOCAL, PROJECT_TYPE_SSH, REMOTE_TASK_ATTACHMENT_ROOT_DIR, REVIEW_FINDINGS_END_TAG,
    REVIEW_FINDINGS_START_TAG, REVIEW_REPORT_END_TAG, REVIEW_REPORT_START_TAG,
    REVIEW_VERDICT_END_TAG, REVIEW_VERDICT_START_TAG, SDK_BRIDGE_FILE_NAME,
    SDK_RUNTIME_PACKAGE_JSON, SQLITE_DATETIME_FORMAT, TASK_AUTOMATION_MODE_REVIEW_FIX_LOOP_V1,
    TASK_AUTOMATION_PHASE_COMMITTING_CODE, TASK_AUTOMATION_PHASE_LAUNCHING_FIX,
    TASK_AUTOMATION_PHASE_LAUNCHING_REVIEW, TASK_AUTOMATION_PHASE_WAITING_EXECUTION,
    TASK_AUTOMATION_PHASE_WAITING_REVIEW, TASK_STATUS_ARCHIVED,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use shared::{
    REVIEW_DIFF_CHAR_LIMIT, REVIEW_UNTRACKED_FILE_LIMIT, REVIEW_UNTRACKED_FILE_SIZE_LIMIT,
    REVIEW_UNTRACKED_TOTAL_CHAR_LIMIT,
};
#[allow(unused_imports)]
pub(crate) use tasks::{
    build_task_completion_timer_update, clear_task_automation_state_for_disabled_mode,
    decode_task_automation_state, disable_task_automation_for_archived_task,
    ensure_task_dependencies_satisfied, export_tasks_json_with_pool, fetch_task_attachments,
    fetch_task_automation_state_record, fetch_task_by_id, fetch_task_file_refs,
    fetch_task_subtasks, format_task_file_refs_prompt_section, import_tasks_json_with_pool,
    insert_task_record, is_orchestration_awaiting_session_exit,
    is_task_automation_active_for_archival, list_tasks_with_pool, parse_tasks_json_envelope,
    record_completion_metric, resolve_await_session_followups,
    resolve_project_task_default_settings, should_await_session_followups,
    should_clear_task_completed_at, start_task_timer_internal, stop_task_timer_internal,
    tasks_json_payload_is_field_safe, validate_assignee_for_project,
    validate_coordinator_for_project, validate_reviewer_for_project, validate_task_archival_guard,
    validate_task_automation_mode_change, validate_tasks_json_task, TasksJsonEnvelope,
    TasksJsonSource, TasksJsonSubtask, TasksJsonTask, TASKS_JSON_FORMAT, TASKS_JSON_VERSION,
};

#[cfg(test)]
mod tests;
