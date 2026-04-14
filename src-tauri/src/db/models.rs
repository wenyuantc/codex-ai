#![allow(dead_code)]

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
    pub created_at: String,
    pub updated_at: String,
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
    pub assignee_id: Option<String>,
    pub complexity: Option<i32>,
    pub ai_suggestion: Option<String>,
    pub last_codex_session_id: Option<String>,
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
    pub cli_session_id: Option<String>,
    pub working_dir: Option<String>,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub exit_code: Option<i32>,
    pub resume_session_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CodexSessionEvent {
    pub id: String,
    pub session_id: String,
    pub event_type: String,
    pub message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexHealthCheck {
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
    pub node_path_override: Option<String>,
    pub sdk_install_dir: String,
    pub one_shot_preferred_provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSdkInstallResult {
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

// ========== DTOs ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub description: Option<String>,
    pub repo_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub repo_path: Option<Option<String>>,
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
    pub assignee_id: Option<String>,
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
    pub complexity: Option<Option<i32>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub ai_suggestion: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub last_codex_session_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCodexSettings {
    pub task_sdk_enabled: Option<bool>,
    pub one_shot_sdk_enabled: Option<bool>,
    pub one_shot_model: Option<String>,
    pub one_shot_reasoning_effort: Option<String>,
    #[serde(default, deserialize_with = "deserialize_explicit_nullable")]
    pub node_path_override: Option<Option<String>>,
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

// ========== Event Payloads ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexOutput {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexExit {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSession {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub session_id: String,
}

#[cfg(test)]
mod tests {
    use super::{CreateTask, UpdateEmployee, UpdateProject, UpdateTask};

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
}
