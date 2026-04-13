#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// ========== Table Models ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub repo_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub status: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub task_id: String,
    pub employee_id: Option<String>,
    pub content: String,
    pub is_ai_generated: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityLog {
    pub id: String,
    pub employee_id: Option<String>,
    pub action: String,
    pub details: Option<String>,
    pub task_id: Option<String>,
    pub project_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEmployee {
    pub project_id: String,
    pub employee_id: String,
    pub role: String,
    pub joined_at: String,
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
    pub description: Option<String>,
    pub status: Option<String>,
    pub repo_path: Option<String>,
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
    pub specialization: Option<String>,
    pub system_prompt: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTask {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub project_id: String,
    pub assignee_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assignee_id: Option<String>,
    pub complexity: Option<i32>,
    pub ai_suggestion: Option<String>,
    pub last_codex_session_id: Option<String>,
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
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexExit {
    pub employee_id: String,
    pub code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSession {
    pub employee_id: String,
    pub task_id: Option<String>,
    pub session_id: String,
}
