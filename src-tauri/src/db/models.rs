#![allow(dead_code)]

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};
use sqlx::FromRow;

fn deserialize_explicit_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

// ========== Table Models ==========

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub repo_path: Option<String>,
    pub project_type: String,
    pub ssh_config_id: Option<String>,
    pub remote_repo_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SshConfigRecord {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub auth_type: String,
    pub private_key_path: Option<String>,
    pub password_ref: Option<String>,
    pub passphrase_ref: Option<String>,
    pub known_hosts_mode: String,
    pub last_checked_at: Option<String>,
    pub last_check_status: Option<String>,
    pub last_check_message: Option<String>,
    pub password_probe_checked_at: Option<String>,
    pub password_probe_status: Option<String>,
    pub password_probe_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub auth_type: String,
    pub private_key_path: Option<String>,
    pub known_hosts_mode: String,
    pub last_checked_at: Option<String>,
    pub last_check_status: Option<String>,
    pub last_check_message: Option<String>,
    pub password_probe_checked_at: Option<String>,
    pub password_probe_status: Option<String>,
    pub password_probe_message: Option<String>,
    pub password_configured: bool,
    pub passphrase_configured: bool,
    pub password_auth_available: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<SshConfigRecord> for SshConfig {
    fn from(value: SshConfigRecord) -> Self {
        let password_probe_status = value.password_probe_status.clone();
        Self {
            id: value.id,
            name: value.name,
            host: value.host,
            port: value.port,
            username: value.username,
            auth_type: value.auth_type,
            private_key_path: value.private_key_path,
            known_hosts_mode: value.known_hosts_mode,
            last_checked_at: value.last_checked_at,
            last_check_status: value.last_check_status,
            last_check_message: value.last_check_message,
            password_probe_checked_at: value.password_probe_checked_at,
            password_probe_status,
            password_probe_message: value.password_probe_message,
            password_configured: value.password_ref.is_some(),
            passphrase_configured: value.passphrase_ref.is_some(),
            password_auth_available: matches!(
                value.password_probe_status.as_deref(),
                Some("passed" | "available")
            ),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Employee {
    pub id: String,
    pub name: String,
    pub role: String,
    pub model: String,
    pub reasoning_effort: String,
    pub status: String,
    pub specialization: Option<String>,
    pub system_prompt: Option<String>,
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub project_id: String,
    pub use_worktree: bool,
    pub assignee_id: Option<String>,
    pub reviewer_id: Option<String>,
    pub complexity: Option<i32>,
    pub ai_suggestion: Option<String>,
    pub automation_mode: Option<String>,
    pub last_codex_session_id: Option<String>,
    pub last_review_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskAttachment {
    pub id: String,
    pub task_id: String,
    pub original_name: String,
    pub stored_path: String,
    pub mime_type: String,
    pub file_size: i64,
    pub sort_order: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Subtask {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub status: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Comment {
    pub id: String,
    pub task_id: String,
    pub employee_id: Option<String>,
    pub content: String,
    pub is_ai_generated: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ActivityLog {
    pub id: String,
    pub employee_id: Option<String>,
    pub action: String,
    pub details: Option<String>,
    pub task_id: Option<String>,
    pub project_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EmployeeMetric {
    pub id: String,
    pub employee_id: String,
    pub tasks_completed: i32,
    pub average_completion_time: Option<f64>,
    pub success_rate: Option<f64>,
    pub period_start: String,
    pub period_end: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProjectEmployee {
    pub project_id: String,
    pub employee_id: String,
    pub role: String,
    pub joined_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CodexSessionRecord {
    pub id: String,
    pub employee_id: Option<String>,
    pub task_id: Option<String>,
    pub project_id: Option<String>,
    pub task_git_context_id: Option<String>,
    pub cli_session_id: Option<String>,
    pub working_dir: Option<String>,
    pub execution_target: String,
    pub ssh_config_id: Option<String>,
    pub target_host_label: Option<String>,
    pub artifact_capture_mode: String,
    pub session_kind: String,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub exit_code: Option<i32>,
    pub resume_session_id: Option<String>,
    pub created_at: String,
}

pub const TASK_GIT_CONTEXT_STATE_PROVISIONING: &str = "provisioning";
pub const TASK_GIT_CONTEXT_STATE_READY: &str = "ready";
pub const TASK_GIT_CONTEXT_STATE_RUNNING: &str = "running";
pub const TASK_GIT_CONTEXT_STATE_MERGE_READY: &str = "merge_ready";
pub const TASK_GIT_CONTEXT_STATE_ACTION_PENDING: &str = "action_pending";
pub const TASK_GIT_CONTEXT_STATE_COMPLETED: &str = "completed";
pub const TASK_GIT_CONTEXT_STATE_FAILED: &str = "failed";
pub const TASK_GIT_CONTEXT_STATE_DRIFTED: &str = "drifted";

pub const TASK_GIT_CONTEXT_STATES: &[&str] = &[
    TASK_GIT_CONTEXT_STATE_PROVISIONING,
    TASK_GIT_CONTEXT_STATE_READY,
    TASK_GIT_CONTEXT_STATE_RUNNING,
    TASK_GIT_CONTEXT_STATE_MERGE_READY,
    TASK_GIT_CONTEXT_STATE_ACTION_PENDING,
    TASK_GIT_CONTEXT_STATE_COMPLETED,
    TASK_GIT_CONTEXT_STATE_FAILED,
    TASK_GIT_CONTEXT_STATE_DRIFTED,
];

pub const TASK_GIT_ACTION_MERGE: &str = "merge";
pub const TASK_GIT_ACTION_PUSH: &str = "push";
pub const TASK_GIT_ACTION_REBASE: &str = "rebase";
pub const TASK_GIT_ACTION_CHERRY_PICK: &str = "cherry_pick";
pub const TASK_GIT_ACTION_STASH: &str = "stash";
pub const TASK_GIT_ACTION_UNSTASH: &str = "unstash";
pub const TASK_GIT_ACTION_CLEANUP_WORKTREE: &str = "cleanup_worktree";

pub const TASK_GIT_ACTION_TYPES: &[&str] = &[
    TASK_GIT_ACTION_MERGE,
    TASK_GIT_ACTION_PUSH,
    TASK_GIT_ACTION_REBASE,
    TASK_GIT_ACTION_CHERRY_PICK,
    TASK_GIT_ACTION_STASH,
    TASK_GIT_ACTION_UNSTASH,
    TASK_GIT_ACTION_CLEANUP_WORKTREE,
];

pub fn is_valid_task_git_context_state(value: &str) -> bool {
    TASK_GIT_CONTEXT_STATES.contains(&value)
}

pub fn is_valid_task_git_action_type(value: &str) -> bool {
    TASK_GIT_ACTION_TYPES.contains(&value)
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskGitContext {
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub base_branch: Option<String>,
    pub task_branch: Option<String>,
    pub target_branch: Option<String>,
    pub worktree_path: Option<String>,
    pub repo_head_commit_at_prepare: Option<String>,
    pub state: String,
    pub context_version: i32,
    pub pending_action_type: Option<String>,
    pub pending_action_token_hash: Option<String>,
    pub pending_action_payload_json: Option<String>,
    pub pending_action_nonce: Option<String>,
    pub pending_action_requested_at: Option<String>,
    pub pending_action_expires_at: Option<String>,
    pub pending_action_repo_revision: Option<String>,
    pub pending_action_bound_context_version: Option<i32>,
    pub last_reconciled_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CodexSessionEvent {
    pub id: String,
    pub session_id: String,
    pub event_type: String,
    pub message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CodexSessionFileChange {
    pub id: String,
    pub session_id: String,
    pub path: String,
    pub change_type: String,
    pub capture_mode: String,
    pub previous_path: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CodexSessionFileChangeDetailRecord {
    pub id: String,
    pub change_id: String,
    pub absolute_path: Option<String>,
    pub previous_absolute_path: Option<String>,
    pub before_status: String,
    pub before_text: Option<String>,
    pub before_truncated: i32,
    pub after_status: String,
    pub after_text: Option<String>,
    pub after_truncated: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexHealthCheck {
    pub execution_target: String,
    pub ssh_config_id: Option<String>,
    pub target_host_label: Option<String>,
    pub codex_available: bool,
    pub codex_version: Option<String>,
    pub node_available: bool,
    pub node_version: Option<String>,
    pub task_sdk_enabled: bool,
    pub one_shot_sdk_enabled: bool,
    pub sdk_installed: bool,
    pub sdk_version: Option<String>,
    pub sdk_install_dir: String,
    pub task_execution_effective_provider: String,
    pub one_shot_effective_provider: String,
    pub sdk_status_message: String,
    pub database_loaded: bool,
    pub database_path: Option<String>,
    pub database_current_version: Option<i64>,
    pub database_current_description: Option<String>,
    pub database_latest_version: i64,
    pub shell_available: bool,
    pub password_auth_available: bool,
    pub password_probe_status: Option<String>,
    pub last_session_error: Option<String>,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexRuntimeStatus {
    pub running: bool,
    pub session: Option<CodexSessionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSettings {
    pub task_sdk_enabled: bool,
    pub one_shot_sdk_enabled: bool,
    pub one_shot_model: String,
    pub one_shot_reasoning_effort: String,
    pub task_automation_default_enabled: bool,
    pub task_automation_max_fix_rounds: i32,
    pub task_automation_failure_strategy: String,
    pub node_path_override: Option<String>,
    pub sdk_install_dir: String,
    pub one_shot_preferred_provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSettingsDocument {
    pub local: CodexSettings,
    #[serde(default)]
    pub remote_profiles: HashMap<String, CodexSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSdkInstallResult {
    pub execution_target: String,
    pub ssh_config_id: Option<String>,
    pub target_host_label: Option<String>,
    pub sdk_installed: bool,
    pub sdk_version: Option<String>,
    pub install_dir: String,
    pub node_version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseBackupResult {
    pub source_path: String,
    pub destination_path: String,
    pub database_version: Option<i64>,
    pub created_at: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseRestoreResult {
    pub source_path: String,
    pub backup_path: String,
    pub database_version: Option<i64>,
    pub restored_at: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLatestReview {
    pub session: CodexSessionRecord,
    pub report: Option<String>,
    pub reviewer_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionChangeHistoryItem {
    pub session: CodexSessionRecord,
    pub capture_mode: String,
    pub changes: Vec<CodexSessionFileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewVerdict {
    pub passed: bool,
    pub needs_human: bool,
    pub blocking_issue_count: i32,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskAutomationStateRecord {
    pub task_id: String,
    pub phase: String,
    pub round_count: i32,
    pub consumed_session_id: Option<String>,
    pub last_trigger_session_id: Option<String>,
    pub pending_action: Option<String>,
    pub pending_round_count: Option<i32>,
    pub last_error: Option<String>,
    pub last_verdict_json: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAutomationState {
    pub task_id: String,
    pub phase: String,
    pub round_count: i32,
    pub consumed_session_id: Option<String>,
    pub last_trigger_session_id: Option<String>,
    pub pending_action: Option<String>,
    pub pending_round_count: Option<i32>,
    pub last_error: Option<String>,
    pub last_verdict: Option<ReviewVerdict>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSessionFileChangeDetail {
    pub change: CodexSessionFileChange,
    pub working_dir: Option<String>,
    pub absolute_path: Option<String>,
    pub previous_absolute_path: Option<String>,
    pub before_status: String,
    pub before_text: Option<String>,
    pub before_truncated: bool,
    pub after_status: String,
    pub after_text: Option<String>,
    pub after_truncated: bool,
    pub diff_text: Option<String>,
    pub diff_truncated: bool,
    pub snapshot_status: String,
    pub snapshot_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSessionLogLine {
    pub event_id: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CodexSessionListItem {
    pub session_record_id: String,
    pub session_id: String,
    pub cli_session_id: Option<String>,
    pub session_kind: String,
    pub status: String,
    pub last_updated_at: String,
    pub display_name: String,
    pub summary: Option<String>,
    pub content_preview: Option<String>,
    pub employee_id: Option<String>,
    pub employee_name: Option<String>,
    pub task_id: Option<String>,
    pub task_title: Option<String>,
    pub task_status: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub working_dir: Option<String>,
    pub execution_target: String,
    pub ssh_config_id: Option<String>,
    pub target_host_label: Option<String>,
    pub artifact_capture_mode: String,
    pub resume_status: String,
    pub resume_message: Option<String>,
    pub can_resume: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSessionResumePreview {
    pub requested_session_id: String,
    pub resolved_session_id: Option<String>,
    pub session_record_id: Option<String>,
    pub session_kind: Option<String>,
    pub session_status: Option<String>,
    pub display_name: Option<String>,
    pub summary: Option<String>,
    pub employee_id: Option<String>,
    pub employee_name: Option<String>,
    pub task_id: Option<String>,
    pub task_title: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub working_dir: Option<String>,
    pub execution_target: Option<String>,
    pub ssh_config_id: Option<String>,
    pub target_host_label: Option<String>,
    pub artifact_capture_mode: Option<String>,
    pub resume_status: String,
    pub resume_message: Option<String>,
    pub can_resume: bool,
}

#[derive(Debug, Clone)]
pub struct CodexSessionFileChangeInput {
    pub path: String,
    pub change_type: String,
    pub capture_mode: String,
    pub previous_path: Option<String>,
    pub detail: Option<CodexSessionFileChangeDetailInput>,
}

#[derive(Debug, Clone)]
pub struct CodexSessionFileChangeDetailInput {
    pub absolute_path: Option<String>,
    pub previous_absolute_path: Option<String>,
    pub before_status: String,
    pub before_text: Option<String>,
    pub before_truncated: bool,
    pub after_status: String,
    pub after_text: Option<String>,
    pub after_truncated: bool,
}

// ========== DTOs ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub description: Option<String>,
    pub project_type: Option<String>,
    pub repo_path: Option<String>,
    pub ssh_config_id: Option<String>,
    pub remote_repo_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    pub project_type: Option<String>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub repo_path: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub ssh_config_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub remote_repo_path: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSshConfig {
    pub name: String,
    pub host: String,
    pub port: Option<i64>,
    pub username: String,
    pub auth_type: String,
    pub private_key_path: Option<String>,
    pub password: Option<String>,
    pub passphrase: Option<String>,
    pub known_hosts_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSshConfig {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub username: Option<String>,
    pub auth_type: Option<String>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub private_key_path: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub password: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub passphrase: Option<Option<String>>,
    pub known_hosts_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEmployee {
    pub name: String,
    pub role: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub specialization: Option<String>,
    pub system_prompt: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEmployee {
    pub name: Option<String>,
    pub role: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub specialization: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub system_prompt: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub project_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTask {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub project_id: String,
    pub use_worktree: Option<bool>,
    pub assignee_id: Option<String>,
    pub reviewer_id: Option<String>,
    pub attachment_source_paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTask {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    pub priority: Option<String>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub assignee_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub reviewer_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub complexity: Option<Option<i32>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub ai_suggestion: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub last_codex_session_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub last_review_session_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTaskAutomationModePayload {
    pub task_id: String,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub automation_mode: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCodexSettings {
    pub task_sdk_enabled: Option<bool>,
    pub one_shot_sdk_enabled: Option<bool>,
    pub one_shot_model: Option<String>,
    pub one_shot_reasoning_effort: Option<String>,
    pub task_automation_default_enabled: Option<bool>,
    pub task_automation_max_fix_rounds: Option<i32>,
    pub task_automation_failure_strategy: Option<String>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub node_path_override: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub sdk_install_dir: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteCodexSettingsPayload {
    #[serde(alias = "sshConfigId")]
    pub ssh_config_id: String,
    pub updates: UpdateCodexSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordAuthProbeResult {
    pub ssh_config_id: String,
    pub target_host_label: String,
    pub supported: bool,
    pub status: String,
    pub message: String,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSubtask {
    pub task_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateComment {
    pub task_id: String,
    pub employee_id: Option<String>,
    pub content: String,
    pub is_ai_generated: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchGlobalPayload {
    pub query: String,
    pub types: Option<Vec<String>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub environment_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSearchItem {
    pub item_type: String,
    pub item_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub summary: Option<String>,
    pub navigation_path: String,
    pub score: i64,
    pub updated_at: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub employee_id: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSearchResponse {
    pub query: String,
    pub normalized_query: String,
    pub state: String,
    pub message: Option<String>,
    pub min_query_length: usize,
    pub total: usize,
    pub items: Vec<GlobalSearchItem>,
}

// ========== Event Payloads ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexOutput {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub session_kind: String,
    pub session_record_id: String,
    pub session_event_id: Option<String>,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexExit {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub session_kind: String,
    pub session_record_id: String,
    pub session_event_id: Option<String>,
    pub line: Option<String>,
    pub code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSession {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub session_kind: String,
    pub session_record_id: String,
    pub session_id: String,
}

#[cfg(test)]
mod tests {
    use super::{
        is_valid_task_git_action_type, is_valid_task_git_context_state, CreateTask, ReviewVerdict,
        TaskGitContext, UpdateEmployee, UpdateProject, UpdateSshConfig, UpdateTask,
        TASK_GIT_ACTION_TYPES, TASK_GIT_CONTEXT_STATES,
    };

    #[test]
    fn project_update_keeps_explicit_nulls() {
        let payload: UpdateProject = serde_json::from_str(
            r#"{"name":"项目A","description":null,"repo_path":"/tmp/repo","status":"active"}"#,
        )
        .expect("deserialize project update");

        assert_eq!(payload.name.as_deref(), Some("项目A"));
        assert_eq!(payload.description, Some(None));
        assert_eq!(payload.repo_path, Some(Some("/tmp/repo".to_string())));
    }

    #[test]
    fn project_update_keeps_ssh_nullable_fields() {
        let payload: UpdateProject = serde_json::from_str(
            r#"{"project_type":"ssh","ssh_config_id":null,"remote_repo_path":"/srv/repo"}"#,
        )
        .expect("deserialize ssh project update");

        assert_eq!(payload.project_type.as_deref(), Some("ssh"));
        assert_eq!(payload.ssh_config_id, Some(None));
        assert_eq!(
            payload.remote_repo_path,
            Some(Some("/srv/repo".to_string()))
        );
    }

    #[test]
    fn employee_update_keeps_nullable_fields() {
        let payload: UpdateEmployee =
            serde_json::from_str(r#"{"name":"Alice","specialization":null,"project_id":"proj-1"}"#)
                .expect("deserialize employee update");

        assert_eq!(payload.specialization, Some(None));
        assert_eq!(payload.project_id, Some(Some("proj-1".to_string())));
    }

    #[test]
    fn task_update_keeps_nullable_fields() {
        let payload: UpdateTask =
            serde_json::from_str(r#"{"description":null,"assignee_id":null,"complexity":3}"#)
                .expect("deserialize task update");

        assert_eq!(payload.description, Some(None));
        assert_eq!(payload.assignee_id, Some(None));
        assert_eq!(payload.complexity, Some(Some(3)));
    }

    #[test]
    fn task_update_keeps_review_fields() {
        let payload: UpdateTask = serde_json::from_str(
            r#"{"reviewer_id":null,"last_review_session_id":"review-session-1"}"#,
        )
        .expect("deserialize task review update");

        assert_eq!(payload.reviewer_id, Some(None));
        assert_eq!(
            payload.last_review_session_id,
            Some(Some("review-session-1".to_string()))
        );
    }

    #[test]
    fn create_task_accepts_attachment_source_paths() {
        let payload: CreateTask = serde_json::from_str(
            r#"{
                "title":"带图任务",
                "project_id":"proj-1",
                "attachment_source_paths":["/tmp/a.png","/tmp/b.jpg"]
            }"#,
        )
        .expect("deserialize create task");

        assert_eq!(
            payload.attachment_source_paths,
            Some(vec!["/tmp/a.png".to_string(), "/tmp/b.jpg".to_string()])
        );
    }

    #[test]
    fn create_task_accepts_use_worktree_flag() {
        let payload: CreateTask = serde_json::from_str(
            r#"{
                "title":"独立工作树任务",
                "project_id":"proj-1",
                "use_worktree":true
            }"#,
        )
        .expect("deserialize create task with worktree");

        assert_eq!(payload.use_worktree, Some(true));
    }

    #[test]
    fn review_verdict_deserializes() {
        let verdict: ReviewVerdict = serde_json::from_str(
            r#"{"passed":false,"needs_human":true,"blocking_issue_count":2,"summary":"需人工确认"}"#,
        )
        .expect("deserialize review verdict");

        assert!(!verdict.passed);
        assert!(verdict.needs_human);
        assert_eq!(verdict.blocking_issue_count, 2);
        assert_eq!(verdict.summary, "需人工确认");
    }

    #[test]
    fn ssh_config_update_keeps_secret_nullable_fields() {
        let payload: UpdateSshConfig = serde_json::from_str(
            r#"{"private_key_path":null,"password":"secret","passphrase":null}"#,
        )
        .expect("deserialize ssh config update");

        assert_eq!(payload.private_key_path, Some(None));
        assert_eq!(payload.password, Some(Some("secret".to_string())));
        assert_eq!(payload.passphrase, Some(None));
    }

    #[test]
    fn task_git_context_serializes_with_version_and_pending_payload() {
        let record = TaskGitContext {
            id: "ctx-1".to_string(),
            task_id: "task-1".to_string(),
            project_id: "proj-1".to_string(),
            base_branch: Some("main".to_string()),
            task_branch: Some("task/task-1".to_string()),
            target_branch: Some("main".to_string()),
            worktree_path: Some("/tmp/task-1".to_string()),
            repo_head_commit_at_prepare: Some("abc123".to_string()),
            state: "ready".to_string(),
            context_version: 1,
            pending_action_type: Some("merge".to_string()),
            pending_action_token_hash: Some("sha256:deadbeef".to_string()),
            pending_action_payload_json: Some(
                r#"{"allow_ff":true,"strategy":"squash","target_branch":"main"}"#.to_string(),
            ),
            pending_action_nonce: Some("nonce-1".to_string()),
            pending_action_requested_at: Some("2026-04-17 15:00:00".to_string()),
            pending_action_expires_at: Some("2026-04-17 15:05:00".to_string()),
            pending_action_repo_revision: Some("abc123".to_string()),
            pending_action_bound_context_version: Some(1),
            last_reconciled_at: Some("2026-04-17 15:01:00".to_string()),
            last_error: None,
            created_at: "2026-04-17 15:00:00".to_string(),
            updated_at: "2026-04-17 15:01:00".to_string(),
        };

        let json = serde_json::to_value(&record).expect("serialize task git context");
        assert_eq!(
            json.get("context_version").and_then(|value| value.as_i64()),
            Some(1)
        );
        assert_eq!(
            json.get("pending_action_payload_json")
                .and_then(|value| value.as_str()),
            Some(r#"{"allow_ff":true,"strategy":"squash","target_branch":"main"}"#)
        );
        assert_eq!(
            json.get("pending_action_token_hash")
                .and_then(|value| value.as_str()),
            Some("sha256:deadbeef")
        );
    }

    #[test]
    fn task_git_context_states_match_prd_contract() {
        for state in TASK_GIT_CONTEXT_STATES {
            assert!(
                is_valid_task_git_context_state(state),
                "missing state {state}"
            );
        }
        assert!(!is_valid_task_git_context_state("unknown"));
    }

    #[test]
    fn task_git_action_types_match_confirmation_gate_contract() {
        for action in TASK_GIT_ACTION_TYPES {
            assert!(
                is_valid_task_git_action_type(action),
                "missing action {action}"
            );
        }
        assert!(!is_valid_task_git_action_type("checkout"));
    }
}
