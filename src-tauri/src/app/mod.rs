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
    TaskAutomationState, TaskAutomationStateRecord, TaskExecutionChangeHistoryItem,
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

mod database;
mod employees;
mod projects;
mod remote;
mod review;
mod sessions;
mod shared;
mod tasks;

pub(crate) use database::*;
pub(crate) use employees::*;
pub(crate) use projects::*;
pub(crate) use remote::*;
pub(crate) use review::*;
pub(crate) use sessions::*;
pub(crate) use shared::*;
pub(crate) use tasks::*;

#[cfg(test)]
mod tests;
