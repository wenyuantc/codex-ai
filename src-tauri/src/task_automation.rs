mod prompt;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::app::{
    build_task_completion_timer_update, fetch_employee_by_id, fetch_project_by_id,
    fetch_task_automation_state_record, fetch_task_by_id, insert_activity_log,
    insert_codex_session_event, new_id, normalize_optional_text, now_sqlite,
    parse_review_verdict_json, record_completion_metric, sqlite_pool,
    start_task_code_review_internal, start_task_timer_internal, stop_task_timer_internal,
    TASK_STATUS_ARCHIVED,
};
use crate::claude::{start_claude_with_manager, stop_claude_for_automation_restart, ClaudeManager};
use crate::codex::{
    extract_review_report, extract_review_verdict, load_codex_settings, start_codex_with_manager,
    stop_codex_for_automation_restart, CodexManager,
};
use crate::db::models::{
    AbortTaskPipelinePayload, CodexSessionRecord, Project, ResumeTaskPipelinePayload,
    ReviewVerdict, RunTaskAcceptancePayload, RunTaskPipelineStepManualPayload,
    StartTaskPipelinePayload, Subtask, Task, TaskAcceptanceRun, TaskAttachment,
    TaskAutomationStateRecord, TaskPipelineStep, RetryTaskPipelineStepPayload,
    UpdateTaskAcceptanceChecklistPayload, UpdateTaskPipelineStepPayload,
};
use crate::git_workflow::{
    auto_commit_task_worktree, mark_task_git_context_session_finished, TaskGitAutoCommitOutcome,
};
use crate::grok::{start_grok_with_manager, stop_grok_for_automation_restart, GrokManager};
use crate::notifications::{build_task_status_notification, publish_one_time_notification};
use crate::opencode::{
    start_opencode_with_manager, stop_opencode_for_automation_restart, OpenCodeManager,
};


// File-split for navigation; items remain in this module namespace via include!.
include!("task_automation/state.rs");
include!("task_automation/session_exit.rs");
include!("task_automation/fix_loop.rs");
include!("task_automation/restart.rs");
include!("task_automation/review_data.rs");
include!("task_automation/pipeline.rs");
include!("task_automation/acceptance.rs");

#[cfg(test)]
include!("task_automation/tests_modules.rs");
