mod prompt;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::app::{
    fetch_employee_by_id, fetch_project_by_id, fetch_task_automation_state_record,
    fetch_task_by_id, insert_activity_log, now_sqlite, parse_review_verdict_json,
    record_completion_metric, sqlite_pool, start_task_code_review_internal, TASK_STATUS_ARCHIVED,
};
use crate::codex::{
    extract_review_report, extract_review_verdict, load_codex_settings, start_codex_with_manager,
    stop_codex_for_automation_restart, CodexManager,
};
use crate::db::models::{
    CodexSessionRecord, Project, ReviewVerdict, Subtask, Task, TaskAttachment,
    TaskAutomationStateRecord,
};
use crate::git_workflow::{auto_commit_task_worktree, TaskGitAutoCommitOutcome};
use crate::notifications::{build_task_status_notification, publish_one_time_notification};

const AUTOMATION_MODE_REVIEW_FIX_LOOP_V1: &str = "review_fix_loop_v1";
const DEFAULT_MAX_FIX_ROUNDS: i32 = 3;
const FAILURE_STRATEGY_BLOCKED: &str = "blocked";
const FAILURE_STRATEGY_MANUAL_CONTROL: &str = "manual_control";
const PHASE_IDLE: &str = "idle";
const PHASE_LAUNCHING_REVIEW: &str = "launching_review";
const PHASE_WAITING_REVIEW: &str = "waiting_review";
const PHASE_LAUNCHING_FIX: &str = "launching_fix";
const PHASE_WAITING_EXECUTION: &str = "waiting_execution";
const PHASE_COMMITTING_CODE: &str = "committing_code";
const PHASE_REVIEW_LAUNCH_FAILED: &str = "review_launch_failed";
const PHASE_FIX_LAUNCH_FAILED: &str = "fix_launch_failed";
const PHASE_COMMIT_FAILED: &str = "commit_failed";
const PHASE_MANUAL_CONTROL: &str = "manual_control";
const PHASE_BLOCKED: &str = "blocked";
const PHASE_COMPLETED: &str = "completed";
const PENDING_ACTION_START_REVIEW: &str = "start_review";
const PENDING_ACTION_START_FIX: &str = "start_fix";
const SESSION_EVENT_AUTOMATION_RESTART_REQUESTED: &str = "automation_restart_requested";

#[derive(Clone, Debug)]
struct SessionExitFacts {
    session_id: String,
    session_kind: String,
    status: String,
    exit_code: Option<i32>,
    task_id: String,
    employee_id: Option<String>,
    has_stopping_requested: bool,
    has_restart_requested: bool,
    review_verdict: Option<ReviewVerdict>,
}

#[derive(Clone, Debug)]
struct TaskSessionIds {
    ids: HashSet<String>,
}

#[derive(Clone, Debug)]
struct TaskAutomationPolicy {
    max_fix_rounds: i32,
    failure_strategy: String,
}

#[derive(Clone, Debug)]
struct AutomationExecutionContext {
    working_dir: String,
    task_git_context_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AutomationRestartTarget {
    Review,
    Fix,
}

#[derive(Clone, Debug, serde::Serialize)]
struct TaskAutomationStateChangedEvent {
    task_id: String,
    project_id: String,
    phase: String,
}

impl TaskSessionIds {
    fn contains(&self, session_id: &str) -> bool {
        self.ids.contains(session_id)
    }
}

fn load_task_automation_policy(app: &AppHandle) -> TaskAutomationPolicy {
    let defaults = TaskAutomationPolicy {
        max_fix_rounds: DEFAULT_MAX_FIX_ROUNDS,
        failure_strategy: FAILURE_STRATEGY_BLOCKED.to_string(),
    };

    let Ok(settings) = load_codex_settings(app) else {
        return defaults;
    };

    TaskAutomationPolicy {
        max_fix_rounds: settings.task_automation_max_fix_rounds.max(1),
        failure_strategy: if settings.task_automation_failure_strategy
            == FAILURE_STRATEGY_MANUAL_CONTROL
        {
            FAILURE_STRATEGY_MANUAL_CONTROL.to_string()
        } else {
            FAILURE_STRATEGY_BLOCKED.to_string()
        },
    }
}

fn task_automation_enabled(task: &Task) -> bool {
    task.status != TASK_STATUS_ARCHIVED
        && task.automation_mode.as_deref() == Some(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1)
}

fn validate_task_automation_restart(task: &Task) -> Result<(), String> {
    if task.status == TASK_STATUS_ARCHIVED {
        return Err("已归档任务不能重启自动质控".to_string());
    }
    if task.automation_mode.as_deref() != Some(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1) {
        return Err("当前任务未开启自动质控".to_string());
    }

    Ok(())
}

fn emit_task_automation_state_changed<R: Runtime>(app: &AppHandle<R>, task: &Task, phase: &str) {
    let _ = app.emit(
        "task-automation-state-changed",
        TaskAutomationStateChangedEvent {
            task_id: task.id.clone(),
            project_id: task.project_id.clone(),
            phase: phase.to_string(),
        },
    );
}

pub(crate) async fn mark_task_automation_commit_completed<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    task: &Task,
    detail: &str,
) -> Result<(), String> {
    if task.automation_mode.as_deref() != Some(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1) {
        return Ok(());
    }

    let current = fetch_task_automation_state_record(pool, &task.id).await?;
    let Some(state_record) = current.as_ref() else {
        return Ok(());
    };

    if !matches!(
        state_record.phase.as_str(),
        PHASE_COMMITTING_CODE | PHASE_COMMIT_FAILED
    ) {
        return Ok(());
    }

    upsert_state_terminal(
        pool,
        &task.id,
        state_record.consumed_session_id.as_deref(),
        PHASE_COMPLETED,
        state_record.last_verdict_json.as_deref(),
        None,
        None,
        current.as_ref(),
    )
    .await?;
    insert_activity_log(
        pool,
        "task_automation_commit_completed",
        detail,
        None,
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    insert_activity_log(
        pool,
        "task_automation_completed",
        detail,
        None,
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    emit_task_automation_state_changed(app, task, PHASE_COMPLETED);
    Ok(())
}

async fn finalize_terminal_failure(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    state_record: Option<&TaskAutomationStateRecord>,
    facts: &SessionExitFacts,
    last_verdict_json: Option<&str>,
    message: &str,
) -> Result<(), String> {
    let policy = load_task_automation_policy(app);

    if policy.failure_strategy == FAILURE_STRATEGY_MANUAL_CONTROL {
        upsert_state_terminal(
            pool,
            &task.id,
            Some(&facts.session_id),
            PHASE_MANUAL_CONTROL,
            last_verdict_json,
            None,
            Some(message),
            state_record,
        )
        .await?;
        insert_activity_log(
            pool,
            "task_automation_manual_control",
            message,
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
        return Ok(());
    }

    upsert_state_terminal(
        pool,
        &task.id,
        Some(&facts.session_id),
        PHASE_BLOCKED,
        last_verdict_json,
        None,
        Some(message),
        state_record,
    )
    .await?;
    update_task_status_internal(app, pool, task, "blocked").await?;
    insert_activity_log(
        pool,
        "task_automation_blocked",
        message,
        facts.employee_id.as_deref(),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;

    Ok(())
}

pub fn spawn_resume_pending_automation(app: AppHandle) {
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(async move {
            if let Err(error) = resume_pending_automation(&app).await {
                eprintln!("[task-automation] 恢复 pending 自动化失败: {error}");
            }
        });
    });
}

pub async fn handle_session_exit_blocking(app: AppHandle, session_record_id: String) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(async move {
            if let Err(error) = handle_session_exit(&app, &session_record_id).await {
                eprintln!(
                    "[task-automation] 处理会话退出失败: session_record_id={}, error={}",
                    session_record_id, error
                );
            }
        });
    })
    .await;
}

async fn fetch_pending_automation_task_ids(pool: &SqlitePool) -> Result<Vec<String>, String> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT tas.task_id
        FROM task_automation_state tas
        INNER JOIN tasks t ON t.id = tas.task_id
        WHERE t.automation_mode = $1
          AND t.status != $2
          AND tas.phase IN ($3, $4, $5, $6, $7)
        "#,
    )
    .bind(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1)
    .bind(TASK_STATUS_ARCHIVED)
    .bind(PHASE_LAUNCHING_REVIEW)
    .bind(PHASE_REVIEW_LAUNCH_FAILED)
    .bind(PHASE_LAUNCHING_FIX)
    .bind(PHASE_FIX_LAUNCH_FAILED)
    .bind(PHASE_COMMITTING_CODE)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to list pending automation tasks: {}", error))
}

pub async fn resume_pending_automation(app: &AppHandle) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    let pending_task_ids = fetch_pending_automation_task_ids(&pool).await?;

    for task_id in pending_task_ids {
        let Some(state_record) = fetch_task_automation_state_record(&pool, &task_id).await? else {
            continue;
        };

        match state_record.phase.as_str() {
            PHASE_LAUNCHING_REVIEW | PHASE_REVIEW_LAUNCH_FAILED => {
                retry_pending_review(app, &pool, &task_id, &state_record).await?;
            }
            PHASE_LAUNCHING_FIX | PHASE_FIX_LAUNCH_FAILED => {
                retry_pending_fix(app, &pool, &task_id, &state_record).await?;
            }
            PHASE_COMMITTING_CODE => {
                let task = fetch_task_by_id(&pool, &task_id).await?;
                retry_pending_commit(app, &pool, &task, None).await?;
            }
            _ => {}
        }
    }

    Ok(())
}

pub async fn handle_session_exit(app: &AppHandle, session_record_id: &str) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    let Some(facts) = fetch_session_exit_facts(&pool, session_record_id).await? else {
        return Ok(());
    };

    let task = fetch_task_by_id(&pool, &facts.task_id).await?;
    let state_record = fetch_task_automation_state_record(&pool, &task.id).await?;

    if !task_automation_enabled(&task) {
        handle_disabled_mode_exit(&pool, &task, state_record.as_ref(), &facts).await?;
        return Ok(());
    }

    if let Some(state) = state_record.as_ref() {
        if state.consumed_session_id.as_deref() == Some(session_record_id) {
            return Ok(());
        }
    }

    match facts.session_kind.as_str() {
        "execution" => {
            handle_execution_exit(app, &pool, &task, state_record.as_ref(), &facts).await
        }
        "review" => handle_review_exit(app, &pool, &task, state_record.as_ref(), &facts).await,
        _ => Ok(()),
    }
}

async fn handle_execution_exit(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    state_record: Option<&TaskAutomationStateRecord>,
    facts: &SessionExitFacts,
) -> Result<(), String> {
    if facts.has_restart_requested {
        return Ok(());
    }

    if facts.has_stopping_requested {
        upsert_state_terminal(
            pool,
            &task.id,
            Some(&facts.session_id),
            PHASE_MANUAL_CONTROL,
            None,
            None,
            Some("执行已被人工停止，自动质控交由人工接管"),
            state_record,
        )
        .await?;
        insert_activity_log(
            pool,
            "task_automation_manual_control",
            "执行已被人工停止，自动质控交由人工接管",
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
        return Ok(());
    }

    if facts.status != "exited" || facts.exit_code != Some(0) {
        return finalize_terminal_failure(
            app,
            pool,
            task,
            state_record,
            facts,
            None,
            "自动修复执行异常失败，需人工接管",
        )
        .await;
    }

    let mut next_round_count = state_record.map(|item| item.round_count).unwrap_or(0);
    let mut next_last_error = None;
    let mut next_last_verdict_json = None;
    if matches!(
        state_record.map(|item| item.phase.as_str()),
        Some(PHASE_MANUAL_CONTROL | PHASE_BLOCKED | PHASE_IDLE)
    ) {
        next_round_count = 0;
    } else if state_record.is_none() {
        next_round_count = 0;
    } else {
        next_last_error = state_record.and_then(|item| item.last_error.clone());
        next_last_verdict_json = state_record.and_then(|item| item.last_verdict_json.clone());
    }

    reserve_pending_action(
        pool,
        &task.id,
        Some(&facts.session_id),
        PHASE_LAUNCHING_REVIEW,
        Some(PENDING_ACTION_START_REVIEW),
        None,
        next_round_count,
        next_last_error.as_deref(),
        next_last_verdict_json.as_deref(),
    )
    .await?;

    let reserved_state = fetch_task_automation_state_record(pool, &task.id)
        .await?
        .ok_or_else(|| "自动质控状态写入后丢失，无法发起审核".to_string())?;
    retry_pending_review(app, pool, &task.id, &reserved_state).await?;
    insert_activity_log(
        pool,
        "task_automation_review_started",
        "执行完成，已自动发起代码审核",
        facts.employee_id.as_deref(),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    Ok(())
}

async fn handle_review_exit(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    state_record: Option<&TaskAutomationStateRecord>,
    facts: &SessionExitFacts,
) -> Result<(), String> {
    if facts.has_restart_requested {
        return Ok(());
    }

    if facts.has_stopping_requested {
        upsert_state_terminal(
            pool,
            &task.id,
            Some(&facts.session_id),
            PHASE_MANUAL_CONTROL,
            None,
            None,
            Some("审核已被人工停止，自动质控交由人工接管"),
            state_record,
        )
        .await?;
        insert_activity_log(
            pool,
            "task_automation_manual_control",
            "审核已被人工停止，自动质控交由人工接管",
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
        return Ok(());
    }

    let Some(verdict) = facts.review_verdict.as_ref() else {
        return finalize_terminal_failure(
            app,
            pool,
            task,
            state_record,
            facts,
            None,
            "审核结果结构化输出无效，自动质控已停止，需人工接管",
        )
        .await;
    };

    let verdict_json = serde_json::to_string(verdict)
        .map_err(|error| format!("Failed to serialize review verdict: {}", error))?;

    if verdict.passed {
        update_task_status_internal(app, pool, task, "completed").await?;
        upsert_state_terminal(
            pool,
            &task.id,
            Some(&facts.session_id),
            PHASE_COMMITTING_CODE,
            Some(&verdict_json),
            None,
            None,
            state_record,
        )
        .await?;
        insert_activity_log(
            pool,
            "task_automation_commit_started",
            "审核已通过，正在自动提交代码",
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
        emit_task_automation_state_changed(app, task, PHASE_COMMITTING_CODE);
        return retry_pending_commit(app, pool, task, facts.employee_id.as_deref()).await;
    }

    let policy = load_task_automation_policy(app);
    let current_round_count = state_record.map(|item| item.round_count).unwrap_or(0);
    if current_round_count >= policy.max_fix_rounds {
        let final_message = if verdict.summary.trim().is_empty() {
            format!("自动修复 {} 轮后仍未通过审核", policy.max_fix_rounds)
        } else {
            format!(
                "自动修复 {} 轮后仍未通过审核：{}",
                policy.max_fix_rounds, verdict.summary
            )
        };
        return finalize_terminal_failure(
            app,
            pool,
            task,
            state_record,
            facts,
            Some(&verdict_json),
            &final_message,
        )
        .await;
    }

    let next_round_count = current_round_count + 1;
    reserve_pending_action(
        pool,
        &task.id,
        Some(&facts.session_id),
        PHASE_LAUNCHING_FIX,
        Some(PENDING_ACTION_START_FIX),
        Some(next_round_count),
        current_round_count,
        None,
        Some(&verdict_json),
    )
    .await?;

    let reserved_state = fetch_task_automation_state_record(pool, &task.id)
        .await?
        .ok_or_else(|| "自动质控状态写入后丢失，无法发起修复".to_string())?;
    retry_pending_fix(app, pool, &task.id, &reserved_state).await?;
    insert_activity_log(
        pool,
        "task_automation_fix_started",
        &format!("第 {} 轮自动修复已启动", next_round_count),
        facts.employee_id.as_deref(),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;

    Ok(())
}

async fn retry_pending_commit(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    employee_id: Option<&str>,
) -> Result<(), String> {
    let state_record = fetch_task_automation_state_record(pool, &task.id)
        .await?
        .ok_or_else(|| "自动质控状态不存在，无法继续自动提交".to_string())?;
    let verdict_summary = state_record
        .last_verdict_json
        .as_deref()
        .map(parse_review_verdict_json)
        .transpose()
        .map_err(|error| format!("解析自动质控审核结论失败: {}", error))?
        .map(|verdict| verdict.summary)
        .unwrap_or_else(|| "自动质控已完成".to_string());

    match auto_commit_task_worktree(app, &task.id).await {
        Ok(TaskGitAutoCommitOutcome::Committed { detail })
        | Ok(TaskGitAutoCommitOutcome::MergeReady { detail }) => {
            mark_task_automation_commit_completed(app, pool, task, &detail).await?;
            Ok(())
        }
        Ok(TaskGitAutoCommitOutcome::NoChanges { detail }) => {
            upsert_state_terminal(
                pool,
                &task.id,
                state_record.consumed_session_id.as_deref(),
                PHASE_COMPLETED,
                state_record.last_verdict_json.as_deref(),
                None,
                None,
                Some(&state_record),
            )
            .await?;
            insert_activity_log(
                pool,
                "task_automation_commit_completed",
                &detail,
                employee_id,
                Some(task.id.as_str()),
                Some(task.project_id.as_str()),
            )
            .await?;
            insert_activity_log(
                pool,
                "task_automation_completed",
                verdict_summary.as_str(),
                employee_id,
                Some(task.id.as_str()),
                Some(task.project_id.as_str()),
            )
            .await?;
            emit_task_automation_state_changed(app, task, PHASE_COMPLETED);
            Ok(())
        }
        Err(error) => {
            upsert_state_terminal(
                pool,
                &task.id,
                state_record.consumed_session_id.as_deref(),
                PHASE_COMMIT_FAILED,
                state_record.last_verdict_json.as_deref(),
                None,
                Some(&error),
                Some(&state_record),
            )
            .await?;
            insert_activity_log(
                pool,
                "task_automation_commit_failed",
                &error,
                employee_id,
                Some(task.id.as_str()),
                Some(task.project_id.as_str()),
            )
            .await?;
            emit_task_automation_state_changed(app, task, PHASE_COMMIT_FAILED);
            Ok(())
        }
    }
}

async fn retry_pending_review(
    app: &AppHandle,
    pool: &SqlitePool,
    task_id: &str,
    state_record: &TaskAutomationStateRecord,
) -> Result<(), String> {
    let task = fetch_task_by_id(pool, task_id).await?;
    if !task_automation_enabled(&task) {
        return Ok(());
    }

    let result = async {
        update_task_status_internal(app, pool, &task, "review").await?;
        let before_sessions = fetch_task_session_ids(pool, task_id, "review").await?;
        let manager = app.state::<Arc<Mutex<CodexManager>>>().inner().clone();
        start_task_code_review_internal(app.clone(), manager, task_id).await?;
        let new_session_id =
            resolve_new_task_session_id(pool, task_id, "review", &before_sessions).await?;
        finalize_launched_action(
            pool,
            task_id,
            PHASE_WAITING_REVIEW,
            Some(&new_session_id),
            None,
            state_record.round_count,
            state_record.last_verdict_json.as_deref(),
        )
        .await
    }
    .await;

    if let Err(error) = result {
        mark_launch_failure(pool, task_id, PHASE_REVIEW_LAUNCH_FAILED, &error).await?;
        return Err(error);
    }

    Ok(())
}

async fn retry_pending_fix(
    app: &AppHandle,
    pool: &SqlitePool,
    task_id: &str,
    state_record: &TaskAutomationStateRecord,
) -> Result<(), String> {
    let task = fetch_task_by_id(pool, task_id).await?;
    if !task_automation_enabled(&task) {
        return Ok(());
    }
    let result = async {
        let last_verdict_json = state_record
            .last_verdict_json
            .as_deref()
            .ok_or_else(|| "自动修复缺少最近审核结论，无法继续执行".to_string())?;
        let verdict = parse_review_verdict_json(last_verdict_json)?;
        let review_report = if let Some(session_id) = state_record.consumed_session_id.as_deref() {
            review_report_for_session(pool, session_id)
                .await?
                .unwrap_or_else(|| verdict.summary.clone())
        } else {
            verdict.summary.clone()
        };
        let before_sessions = fetch_task_session_ids(pool, task_id, "execution").await?;
        start_automation_fix_round(app, pool, &task, &review_report, &verdict).await?;
        let new_session_id =
            resolve_new_task_session_id(pool, task_id, "execution", &before_sessions).await?;
        finalize_launched_action(
            pool,
            task_id,
            PHASE_WAITING_EXECUTION,
            Some(&new_session_id),
            state_record.pending_round_count,
            state_record
                .pending_round_count
                .unwrap_or(state_record.round_count),
            Some(last_verdict_json),
        )
        .await
    }
    .await;

    if let Err(error) = result {
        mark_launch_failure(pool, task_id, PHASE_FIX_LAUNCH_FAILED, &error).await?;
        return Err(error);
    }

    Ok(())
}

async fn start_automation_fix_round(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    review_report: &str,
    verdict: &ReviewVerdict,
) -> Result<(), String> {
    let assignee_id = task
        .assignee_id
        .as_deref()
        .ok_or_else(|| "自动修复要求任务已指派开发负责人".to_string())?;
    let assignee = fetch_employee_by_id(pool, assignee_id).await?;
    let project = fetch_project_by_id(pool, &task.project_id).await?;
    let execution_context = resolve_automation_execution_context(pool, task, &project).await?;
    let attachments = fetch_task_attachments(pool, &task.id).await?;
    let subtasks = fetch_task_subtasks(pool, &task.id).await?;
    let execution_input =
        prompt::build_automation_fix_prompt(task, &subtasks, &attachments, review_report, verdict);

    update_task_status_internal(app, pool, task, "in_progress").await?;
    sqlx::query("UPDATE employees SET status = 'busy' WHERE id = $1")
        .bind(assignee_id)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to update assignee busy status: {}", error))?;

    let manager = app.state::<Arc<Mutex<CodexManager>>>().inner().clone();
    start_codex_with_manager(
        app.clone(),
        manager,
        assignee.id.clone(),
        execution_input.prompt,
        Some(assignee.model.clone()),
        Some(assignee.reasoning_effort.clone()),
        assignee.system_prompt.clone(),
        Some(execution_context.working_dir),
        Some(task.id.clone()),
        execution_context.task_git_context_id,
        None,
        Some(execution_input.image_paths),
        Some("execution".to_string()),
    )
    .await
}

async fn resolve_automation_execution_context(
    pool: &SqlitePool,
    task: &Task,
    _project: &Project,
) -> Result<AutomationExecutionContext, String> {
    let mut candidates = Vec::new();
    if let Some(last_session_id) = task.last_codex_session_id.as_deref() {
        if let Some(session) = sqlx::query_as::<_, CodexSessionRecord>(
            "SELECT * FROM codex_sessions WHERE id = $1 AND session_kind = 'execution' LIMIT 1",
        )
        .bind(last_session_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| format!("查询最近执行 Session 失败: {}", error))?
        {
            candidates.push(session);
        }
    }

    let latest_execution_session = sqlx::query_as::<_, CodexSessionRecord>(
        r#"
        SELECT *
        FROM codex_sessions
        WHERE task_id = $1
          AND session_kind = 'execution'
        ORDER BY started_at DESC, created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&task.id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("查询任务最近执行 Session 失败: {}", error))?;
    if let Some(session) = latest_execution_session {
        candidates.push(session);
    }

    let mut seen = HashSet::new();
    for session in candidates {
        let Some(working_dir) = session.working_dir.clone() else {
            continue;
        };
        if working_dir.trim().is_empty() || !seen.insert(working_dir.clone()) {
            continue;
        }
        return Ok(AutomationExecutionContext {
            working_dir,
            task_git_context_id: session.task_git_context_id.clone(),
        });
    }

    let context_row = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT worktree_path, id
        FROM task_git_contexts
        WHERE task_id = $1
        ORDER BY updated_at DESC, created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&task.id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("查询任务 Git 上下文失败: {}", error))?;
    if let Some((working_dir, context_id)) = context_row {
        if !working_dir.trim().is_empty() {
            return Ok(AutomationExecutionContext {
                working_dir,
                task_git_context_id: Some(context_id),
            });
        }
    }

    Err("当前任务缺少可复用的 Git worktree，上下文未准备好，无法自动修复".to_string())
}

async fn mark_launch_failure(
    pool: &SqlitePool,
    task_id: &str,
    phase: &str,
    message: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"
        UPDATE task_automation_state
        SET phase = $2,
            last_error = $3,
            updated_at = $4
        WHERE task_id = $1
        "#,
    )
    .bind(task_id)
    .bind(phase)
    .bind(message)
    .bind(now_sqlite())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to mark automation launch failure: {}", error))?;

    Ok(())
}

async fn handle_disabled_mode_exit(
    pool: &SqlitePool,
    task: &Task,
    state_record: Option<&TaskAutomationStateRecord>,
    facts: &SessionExitFacts,
) -> Result<(), String> {
    let Some(state_record) = state_record else {
        return Ok(());
    };

    let (phase, clear_verdict) = if matches!(
        state_record.phase.as_str(),
        PHASE_REVIEW_LAUNCH_FAILED | PHASE_FIX_LAUNCH_FAILED
    ) {
        (PHASE_IDLE, true)
    } else {
        (PHASE_MANUAL_CONTROL, false)
    };

    sqlx::query(
        r#"
        UPDATE task_automation_state
        SET phase = $2,
            consumed_session_id = $3,
            pending_action = NULL,
            pending_round_count = NULL,
            last_verdict_json = CASE WHEN $4 THEN NULL ELSE last_verdict_json END,
            updated_at = $5
        WHERE task_id = $1
        "#,
    )
    .bind(&task.id)
    .bind(phase)
    .bind(&facts.session_id)
    .bind(if clear_verdict { 1 } else { 0 })
    .bind(now_sqlite())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to update disabled automation state: {}", error))?;

    insert_activity_log(
        pool,
        "task_automation_skip_disabled",
        "自动质控已关闭，退出后不再触发后续动作",
        facts.employee_id.as_deref(),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await
}

async fn update_task_status_internal<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    current_task: &Task,
    next_status: &str,
) -> Result<(), String> {
    if current_task.status == next_status {
        return Ok(());
    }

    sqlx::query("UPDATE tasks SET status = $1 WHERE id = $2")
        .bind(next_status)
        .bind(&current_task.id)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to update task status internally: {}", error))?;

    insert_activity_log(
        pool,
        "task_status_changed",
        &format!("{} -> {}", current_task.title, next_status),
        None,
        Some(current_task.id.as_str()),
        Some(current_task.project_id.as_str()),
    )
    .await?;

    if current_task.status != "completed" && next_status == "completed" {
        let updated_task = fetch_task_by_id(pool, &current_task.id).await?;
        record_completion_metric(pool, &updated_task).await?;
    }

    if let Some(draft) =
        build_task_status_notification(current_task, current_task.status.as_str(), next_status)
    {
        let _ = publish_one_time_notification(app, draft).await?;
    }

    Ok(())
}

async fn reserve_pending_action(
    pool: &SqlitePool,
    task_id: &str,
    consumed_session_id: Option<&str>,
    phase: &str,
    pending_action: Option<&str>,
    pending_round_count: Option<i32>,
    round_count: i32,
    last_error: Option<&str>,
    last_verdict_json: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO task_automation_state (
            task_id,
            phase,
            round_count,
            consumed_session_id,
            last_trigger_session_id,
            pending_action,
            pending_round_count,
            last_error,
            last_verdict_json,
            updated_at
        ) VALUES ($1, $2, $3, $4, NULL, $5, $6, $7, $8, $9)
        ON CONFLICT(task_id) DO UPDATE SET
            phase = excluded.phase,
            round_count = excluded.round_count,
            consumed_session_id = excluded.consumed_session_id,
            pending_action = excluded.pending_action,
            pending_round_count = excluded.pending_round_count,
            last_error = excluded.last_error,
            last_verdict_json = excluded.last_verdict_json,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(task_id)
    .bind(phase)
    .bind(round_count)
    .bind(consumed_session_id)
    .bind(pending_action)
    .bind(pending_round_count)
    .bind(last_error)
    .bind(last_verdict_json)
    .bind(now_sqlite())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to reserve automation state: {}", error))?;

    Ok(())
}

async fn finalize_launched_action(
    pool: &SqlitePool,
    task_id: &str,
    phase: &str,
    last_trigger_session_id: Option<&str>,
    round_count: Option<i32>,
    fallback_round_count: i32,
    last_verdict_json: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        r#"
        UPDATE task_automation_state
        SET phase = $2,
            last_trigger_session_id = $3,
            round_count = $4,
            pending_action = NULL,
            pending_round_count = NULL,
            last_error = NULL,
            last_verdict_json = $5,
            updated_at = $6
        WHERE task_id = $1
        "#,
    )
    .bind(task_id)
    .bind(phase)
    .bind(last_trigger_session_id)
    .bind(round_count.unwrap_or(fallback_round_count))
    .bind(last_verdict_json)
    .bind(now_sqlite())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to finalize automation state: {}", error))?;

    Ok(())
}

async fn upsert_state_terminal(
    pool: &SqlitePool,
    task_id: &str,
    consumed_session_id: Option<&str>,
    phase: &str,
    last_verdict_json: Option<&str>,
    pending_round_count: Option<i32>,
    last_error: Option<&str>,
    current: Option<&TaskAutomationStateRecord>,
) -> Result<(), String> {
    let round_count = current.map(|item| item.round_count).unwrap_or(0);
    sqlx::query(
        r#"
        INSERT INTO task_automation_state (
            task_id,
            phase,
            round_count,
            consumed_session_id,
            last_trigger_session_id,
            pending_action,
            pending_round_count,
            last_error,
            last_verdict_json,
            updated_at
        ) VALUES ($1, $2, $3, $4, NULL, NULL, $5, $6, $7, $8)
        ON CONFLICT(task_id) DO UPDATE SET
            phase = excluded.phase,
            round_count = excluded.round_count,
            consumed_session_id = excluded.consumed_session_id,
            pending_action = NULL,
            pending_round_count = excluded.pending_round_count,
            last_error = excluded.last_error,
            last_verdict_json = excluded.last_verdict_json,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(task_id)
    .bind(phase)
    .bind(round_count)
    .bind(consumed_session_id)
    .bind(pending_round_count)
    .bind(last_error)
    .bind(last_verdict_json)
    .bind(now_sqlite())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to upsert terminal automation state: {}", error))?;

    Ok(())
}

async fn fetch_session_exit_facts(
    pool: &SqlitePool,
    session_record_id: &str,
) -> Result<Option<SessionExitFacts>, String> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<i32>,
            Option<String>,
            String,
            Option<String>,
        ),
    >(
        r#"
        SELECT id, session_kind, exit_code, employee_id, status, task_id
        FROM codex_sessions
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(session_record_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch session exit facts: {}", error))?;

    let Some((session_id, session_kind, exit_code, employee_id, status, task_id)) = row else {
        return Ok(None);
    };
    let Some(task_id) = task_id else {
        return Ok(None);
    };

    let has_stopping_requested = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM codex_session_events WHERE session_id = $1 AND event_type = 'stopping_requested'",
    )
    .bind(&session_id)
    .fetch_one(pool)
    .await
    .map_err(|error| format!("Failed to fetch stopping_requested event: {}", error))?
        > 0;
    let has_restart_requested = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM codex_session_events WHERE session_id = $1 AND event_type = $2",
    )
    .bind(&session_id)
    .bind(SESSION_EVENT_AUTOMATION_RESTART_REQUESTED)
    .fetch_one(pool)
    .await
    .map_err(|error| format!("Failed to fetch automation restart event: {}", error))?
        > 0;

    let review_verdict = sqlx::query_scalar::<_, Option<String>>(
        "SELECT message FROM codex_session_events WHERE session_id = $1 AND event_type = 'review_verdict' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&session_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch review verdict: {}", error))?
    .flatten()
    .as_deref()
    .map(parse_review_verdict_json)
    .transpose()
    .ok()
    .flatten();

    Ok(Some(SessionExitFacts {
        session_id,
        session_kind,
        status,
        exit_code,
        task_id,
        employee_id,
        has_stopping_requested,
        has_restart_requested,
        review_verdict,
    }))
}

async fn restart_review_step(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    state_record: &TaskAutomationStateRecord,
) -> Result<(), String> {
    let reviewer_id = task
        .reviewer_id
        .as_deref()
        .ok_or_else(|| "当前任务未指定审查员，无法重启自动审核".to_string())?;

    let _ = stop_codex_for_automation_restart(
        app,
        reviewer_id,
        state_record.last_trigger_session_id.as_deref(),
        "自动质控正在重启审核步骤",
    )
    .await?;

    reserve_pending_action(
        pool,
        &task.id,
        state_record.last_trigger_session_id.as_deref(),
        PHASE_LAUNCHING_REVIEW,
        Some(PENDING_ACTION_START_REVIEW),
        None,
        state_record.round_count,
        None,
        state_record.last_verdict_json.as_deref(),
    )
    .await?;

    let reserved_state = fetch_task_automation_state_record(pool, &task.id)
        .await?
        .ok_or_else(|| "自动质控状态不存在，无法重启审核步骤".to_string())?;
    retry_pending_review(app, pool, &task.id, &reserved_state).await?;
    insert_activity_log(
        pool,
        "task_automation_restart_requested",
        "已重启自动质控审核步骤",
        Some(reviewer_id),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    Ok(())
}

async fn restart_fix_step(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    state_record: &TaskAutomationStateRecord,
) -> Result<(), String> {
    let assignee_id = task
        .assignee_id
        .as_deref()
        .ok_or_else(|| "当前任务未指定开发负责人，无法重启自动修复".to_string())?;

    let _ = stop_codex_for_automation_restart(
        app,
        assignee_id,
        state_record.last_trigger_session_id.as_deref(),
        "自动质控正在重启修复步骤",
    )
    .await?;

    let last_verdict_json = if let Some(verdict_json) = state_record.last_verdict_json.clone() {
        Some(verdict_json)
    } else if let Some(session_id) = state_record.consumed_session_id.as_deref() {
        recover_review_verdict_json_for_session(pool, session_id).await?
    } else {
        None
    };
    let (pending_round_count, round_count) = restart_fix_round_state(state_record);

    reserve_pending_action(
        pool,
        &task.id,
        state_record.last_trigger_session_id.as_deref(),
        PHASE_LAUNCHING_FIX,
        Some(PENDING_ACTION_START_FIX),
        pending_round_count,
        round_count,
        None,
        last_verdict_json.as_deref(),
    )
    .await?;

    let reserved_state = fetch_task_automation_state_record(pool, &task.id)
        .await?
        .ok_or_else(|| "自动质控状态不存在，无法重启修复步骤".to_string())?;
    retry_pending_fix(app, pool, &task.id, &reserved_state).await?;
    insert_activity_log(
        pool,
        "task_automation_restart_requested",
        "已重启自动质控修复步骤",
        Some(assignee_id),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    Ok(())
}

fn restart_fix_round_state(state_record: &TaskAutomationStateRecord) -> (Option<i32>, i32) {
    if matches!(
        state_record.phase.as_str(),
        PHASE_BLOCKED | PHASE_MANUAL_CONTROL
    ) {
        (Some(0), 0)
    } else {
        (
            state_record
                .pending_round_count
                .or(Some(state_record.round_count)),
            state_record.round_count,
        )
    }
}

async fn resolve_restart_target(
    pool: &SqlitePool,
    state_record: &TaskAutomationStateRecord,
) -> Result<Option<AutomationRestartTarget>, String> {
    let target = match state_record.phase.as_str() {
        PHASE_WAITING_REVIEW | PHASE_LAUNCHING_REVIEW | PHASE_REVIEW_LAUNCH_FAILED => {
            Some(AutomationRestartTarget::Review)
        }
        PHASE_WAITING_EXECUTION | PHASE_LAUNCHING_FIX | PHASE_FIX_LAUNCH_FAILED => {
            Some(AutomationRestartTarget::Fix)
        }
        PHASE_BLOCKED | PHASE_MANUAL_CONTROL => {
            let Some(session_id) = state_record
                .consumed_session_id
                .as_deref()
                .or(state_record.last_trigger_session_id.as_deref())
            else {
                return Ok(None);
            };
            let session_kind = sqlx::query_scalar::<_, Option<String>>(
                "SELECT session_kind FROM codex_sessions WHERE id = $1 LIMIT 1",
            )
            .bind(session_id)
            .fetch_optional(pool)
            .await
            .map_err(|error| format!("Failed to resolve automation restart target: {}", error))?
            .flatten();

            match session_kind.as_deref() {
                Some("review") => {
                    let can_restart_fix = state_record.last_verdict_json.is_some()
                        || recover_review_verdict_json_for_session(pool, session_id)
                            .await?
                            .is_some();
                    if can_restart_fix {
                        Some(AutomationRestartTarget::Fix)
                    } else {
                        Some(AutomationRestartTarget::Review)
                    }
                }
                Some("execution") => Some(AutomationRestartTarget::Fix),
                _ => None,
            }
        }
        _ => None,
    };

    Ok(target)
}

pub async fn restart_task_automation_internal(
    app: &AppHandle,
    task_id: &str,
) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    let task = fetch_task_by_id(&pool, task_id).await?;
    validate_task_automation_restart(&task)?;

    let state_record = fetch_task_automation_state_record(&pool, task_id)
        .await?
        .ok_or_else(|| "当前任务没有可重启的自动质控状态".to_string())?;

    match resolve_restart_target(&pool, &state_record).await? {
        Some(AutomationRestartTarget::Review) => {
            restart_review_step(app, &pool, &task, &state_record).await
        }
        Some(AutomationRestartTarget::Fix) => {
            restart_fix_step(app, &pool, &task, &state_record).await
        }
        None => Err(format!(
            "当前自动质控阶段“{}”不支持重启，请在卡住或启动失败时使用",
            state_record.phase
        )),
    }
}

#[tauri::command]
pub async fn restart_task_automation(app: AppHandle, task_id: String) -> Result<(), String> {
    restart_task_automation_internal(&app, &task_id).await
}

#[cfg(test)]
mod automation_working_dir_tests {
    use sqlx::SqlitePool;

    use super::resolve_automation_execution_context;
    use crate::app::{build_current_migrator, PROJECT_TYPE_LOCAL, PROJECT_TYPE_SSH};
    use crate::db::models::{Project, Task};

    async fn setup_test_pool() -> SqlitePool {
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

    fn build_project(
        project_type: &str,
        repo_path: Option<&str>,
        remote_repo_path: Option<&str>,
    ) -> Project {
        Project {
            id: "project-1".to_string(),
            name: "demo".to_string(),
            description: None,
            status: "active".to_string(),
            repo_path: repo_path.map(str::to_string),
            project_type: project_type.to_string(),
            ssh_config_id: Some("ssh-1".to_string()),
            remote_repo_path: remote_repo_path.map(str::to_string),
            created_at: "2026-04-17 00:00:00".to_string(),
            updated_at: "2026-04-17 00:00:00".to_string(),
        }
    }

    fn build_task(project_id: &str) -> Task {
        Task {
            id: "task-1".to_string(),
            title: "demo task".to_string(),
            description: None,
            status: "review".to_string(),
            priority: "medium".to_string(),
            project_id: project_id.to_string(),
            use_worktree: true,
            assignee_id: Some("emp-1".to_string()),
            reviewer_id: Some("reviewer-1".to_string()),
            complexity: None,
            ai_suggestion: None,
            automation_mode: Some("review_fix_loop_v1".to_string()),
            last_codex_session_id: Some("exec-1".to_string()),
            last_review_session_id: None,
            created_at: "2026-04-17 00:00:00".to_string(),
            updated_at: "2026-04-17 00:00:00".to_string(),
        }
    }

    #[test]
    fn resolves_local_execution_worktree_for_local_project() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let project = build_project(PROJECT_TYPE_LOCAL, Some("/tmp/demo"), None);
            let task = build_task(&project.id);

            sqlx::query(
                r#"
                INSERT INTO projects (
                    id,
                    name,
                    description,
                    status,
                    repo_path,
                    project_type,
                    created_at,
                    updated_at
                ) VALUES ($1, $2, NULL, 'active', $3, 'local', '2026-04-17 00:00:00', '2026-04-17 00:00:00')
                "#,
            )
            .bind(&project.id)
            .bind(&project.name)
            .bind("/tmp/demo")
            .execute(&pool)
            .await
            .expect("insert project");

            sqlx::query(
                r#"
                INSERT INTO tasks (
                    id,
                    title,
                    description,
                    status,
                    priority,
                    project_id,
                    use_worktree,
                    assignee_id,
                    reviewer_id,
                    automation_mode,
                    last_codex_session_id,
                    last_review_session_id,
                    created_at,
                    updated_at
                ) VALUES ($1, $2, NULL, 'review', 'medium', $3, 1, NULL, NULL, 'review_fix_loop_v1', $4, NULL, '2026-04-17 00:00:00', '2026-04-17 00:00:00')
                "#,
            )
            .bind(&task.id)
            .bind(&task.title)
            .bind(&project.id)
            .bind(task.last_codex_session_id.as_deref())
            .execute(&pool)
            .await
            .expect("insert task");

            sqlx::query(
                r#"
                INSERT INTO task_git_contexts (
                    id,
                    task_id,
                    project_id,
                    base_branch,
                    task_branch,
                    target_branch,
                    worktree_path,
                    repo_head_commit_at_prepare,
                    state,
                    context_version,
                    created_at,
                    updated_at
                ) VALUES (
                    $1, $2, $3, 'main', 'codex/task-task-1', 'main', $4, NULL, 'merge_ready', 1, '2026-04-17 00:00:00', '2026-04-17 00:00:00'
                )
                "#,
            )
            .bind("ctx-1")
            .bind(&task.id)
            .bind(&project.id)
            .bind("/tmp/demo/.codex-ai-worktrees/task-1")
            .execute(&pool)
            .await
            .expect("insert task git context");

            sqlx::query(
                r#"
                INSERT INTO codex_sessions (
                    id,
                    task_id,
                    project_id,
                    task_git_context_id,
                    working_dir,
                    execution_target,
                    artifact_capture_mode,
                    session_kind,
                    status,
                    started_at,
                    created_at
                ) VALUES ($1, $2, $3, $4, $5, 'local', 'local_full', 'execution', 'exited', '2026-04-17 00:00:01', '2026-04-17 00:00:01')
                "#,
            )
            .bind("exec-1")
            .bind(&task.id)
            .bind(&project.id)
            .bind("ctx-1")
            .bind("/tmp/demo/.codex-ai-worktrees/task-1")
            .execute(&pool)
            .await
            .expect("insert execution session");

            let context = resolve_automation_execution_context(&pool, &task, &project)
                .await
                .expect("resolve automation execution context");

            assert_eq!(context.working_dir, "/tmp/demo/.codex-ai-worktrees/task-1");
            assert_eq!(context.task_git_context_id.as_deref(), Some("ctx-1"));

            pool.close().await;
        });
    }

    #[test]
    fn resolves_remote_execution_worktree_for_ssh_project() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let project = build_project(PROJECT_TYPE_SSH, None, Some("/srv/demo"));
            let task = build_task(&project.id);

            sqlx::query(
                r#"
                INSERT INTO ssh_configs (
                    id,
                    name,
                    host,
                    port,
                    username,
                    auth_type,
                    private_key_path,
                    known_hosts_mode,
                    password_ref,
                    passphrase_ref,
                    last_checked_at,
                    last_check_status,
                    last_check_message,
                    password_probe_checked_at,
                    password_probe_status,
                    password_probe_message,
                    created_at,
                    updated_at
                ) VALUES (
                    'ssh-1', 'SSH Demo', 'example.com', 22, 'demo', 'key', NULL, 'accept-new',
                    NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, '2026-04-17 00:00:00', '2026-04-17 00:00:00'
                )
                "#,
            )
            .execute(&pool)
            .await
            .expect("insert ssh config");

            sqlx::query(
                r#"
                INSERT INTO projects (
                    id,
                    name,
                    description,
                    status,
                    repo_path,
                    project_type,
                    ssh_config_id,
                    remote_repo_path,
                    created_at,
                    updated_at
                ) VALUES ($1, $2, NULL, 'active', NULL, 'ssh', 'ssh-1', $3, '2026-04-17 00:00:00', '2026-04-17 00:00:00')
                "#,
            )
            .bind(&project.id)
            .bind(&project.name)
            .bind("/srv/demo")
            .execute(&pool)
            .await
            .expect("insert ssh project");

            sqlx::query(
                r#"
                INSERT INTO tasks (
                    id,
                    title,
                    description,
                    status,
                    priority,
                    project_id,
                    use_worktree,
                    assignee_id,
                    reviewer_id,
                    automation_mode,
                    last_codex_session_id,
                    last_review_session_id,
                    created_at,
                    updated_at
                ) VALUES ($1, $2, NULL, 'review', 'medium', $3, 1, NULL, NULL, 'review_fix_loop_v1', $4, NULL, '2026-04-17 00:00:00', '2026-04-17 00:00:00')
                "#,
            )
            .bind(&task.id)
            .bind(&task.title)
            .bind(&project.id)
            .bind(task.last_codex_session_id.as_deref())
            .execute(&pool)
            .await
            .expect("insert ssh task");

            sqlx::query(
                r#"
                INSERT INTO task_git_contexts (
                    id,
                    task_id,
                    project_id,
                    base_branch,
                    task_branch,
                    target_branch,
                    worktree_path,
                    state,
                    context_version,
                    created_at,
                    updated_at
                ) VALUES (
                    $1, $2, $3, 'main', 'codex/task-task-1', 'main', $4, 'merge_ready', 1, '2026-04-17 00:00:00', '2026-04-17 00:00:00'
                )
                "#,
            )
            .bind("ctx-ssh-1")
            .bind(&task.id)
            .bind(&project.id)
            .bind("/srv/demo/.codex-ai-worktrees/task-1")
            .execute(&pool)
            .await
            .expect("insert ssh task git context");

            sqlx::query(
                r#"
                INSERT INTO codex_sessions (
                    id,
                    task_id,
                    project_id,
                    task_git_context_id,
                    working_dir,
                    execution_target,
                    artifact_capture_mode,
                    session_kind,
                    status,
                    started_at,
                    created_at
                ) VALUES ($1, $2, $3, $4, $5, 'ssh', 'ssh_full', 'execution', 'exited', '2026-04-17 00:00:01', '2026-04-17 00:00:01')
                "#,
            )
            .bind("exec-ssh-1")
            .bind(&task.id)
            .bind(&project.id)
            .bind("ctx-ssh-1")
            .bind("/srv/demo/.codex-ai-worktrees/task-1")
            .execute(&pool)
            .await
            .expect("insert ssh execution session");

            let context = resolve_automation_execution_context(&pool, &task, &project)
                .await
                .expect("resolve remote execution worktree");

            assert_eq!(context.working_dir, "/srv/demo/.codex-ai-worktrees/task-1");
            assert_eq!(context.task_git_context_id.as_deref(), Some("ctx-ssh-1"));

            pool.close().await;
        });
    }
}

async fn fetch_task_session_ids(
    pool: &SqlitePool,
    task_id: &str,
    session_kind: &str,
) -> Result<TaskSessionIds, String> {
    let ids = sqlx::query_scalar::<_, String>(
        "SELECT id FROM codex_sessions WHERE task_id = $1 AND session_kind = $2",
    )
    .bind(task_id)
    .bind(session_kind)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch task session ids: {}", error))?;
    Ok(TaskSessionIds {
        ids: ids.into_iter().collect(),
    })
}

async fn resolve_new_task_session_id(
    pool: &SqlitePool,
    task_id: &str,
    session_kind: &str,
    existing_ids: &TaskSessionIds,
) -> Result<String, String> {
    let rows = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM codex_sessions
        WHERE task_id = $1 AND session_kind = $2
        ORDER BY created_at DESC, started_at DESC, id DESC
        "#,
    )
    .bind(task_id)
    .bind(session_kind)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to resolve new task session id: {}", error))?;

    rows.into_iter()
        .find(|session_id| !existing_ids.contains(session_id))
        .ok_or_else(|| {
            format!(
                "Failed to resolve new {} session id for task {}",
                session_kind, task_id
            )
        })
}

async fn review_report_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    let stored_report = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT message
        FROM codex_session_events
        WHERE session_id = $1
          AND event_type = 'review_report'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch review report for session: {}", error))
    .map(|value| value.flatten())?;

    let recovered_report = recover_review_report_for_session(pool, session_id).await?;
    Ok(recovered_report.or(stored_report))
}

async fn review_raw_output_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    let lines = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT message
        FROM codex_session_events
        WHERE session_id = $1
          AND event_type IN ('stdout', 'stderr')
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch review raw output for session: {}", error))?;

    let lines = lines
        .into_iter()
        .flatten()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return Ok(None);
    }

    Ok(Some(lines.join("\n")))
}

async fn recover_review_verdict_json_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    let stored_verdict = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT message
        FROM codex_session_events
        WHERE session_id = $1
          AND event_type = 'review_verdict'
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch review verdict for session: {}", error))?
    .flatten();
    if let Some(verdict_json) = stored_verdict {
        if parse_review_verdict_json(&verdict_json).is_ok() {
            return Ok(Some(verdict_json));
        }
    }

    let raw_output = review_raw_output_for_session(pool, session_id).await?;
    let recovered = raw_output
        .as_deref()
        .and_then(extract_review_verdict)
        .filter(|value| parse_review_verdict_json(value).is_ok());
    Ok(recovered)
}

async fn recover_review_report_for_session(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<String>, String> {
    let raw_output = review_raw_output_for_session(pool, session_id).await?;
    Ok(raw_output.as_deref().and_then(extract_review_report))
}

async fn fetch_task_subtasks(pool: &SqlitePool, task_id: &str) -> Result<Vec<Subtask>, String> {
    sqlx::query_as::<_, Subtask>("SELECT * FROM subtasks WHERE task_id = $1 ORDER BY sort_order")
        .bind(task_id)
        .fetch_all(pool)
        .await
        .map_err(|error| format!("Failed to fetch task subtasks: {}", error))
}

async fn fetch_task_attachments(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Vec<TaskAttachment>, String> {
    sqlx::query_as::<_, TaskAttachment>(
        "SELECT * FROM task_attachments WHERE task_id = $1 ORDER BY sort_order, created_at",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch task attachments: {}", error))
}

#[cfg(test)]
mod automation_guard_tests {
    use sqlx::SqlitePool;

    use super::{
        fetch_pending_automation_task_ids, resolve_restart_target,
        validate_task_automation_restart, AutomationRestartTarget,
        AUTOMATION_MODE_REVIEW_FIX_LOOP_V1, PHASE_BLOCKED, PHASE_MANUAL_CONTROL,
        PHASE_REVIEW_LAUNCH_FAILED,
    };
    use crate::app::{build_current_migrator, TASK_STATUS_ARCHIVED};
    use crate::db::models::{Task, TaskAutomationStateRecord};

    async fn setup_test_pool() -> SqlitePool {
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

    fn build_task(task_id: &str, status: &str) -> Task {
        Task {
            id: task_id.to_string(),
            title: format!("task {task_id}"),
            description: None,
            status: status.to_string(),
            priority: "medium".to_string(),
            project_id: "project-1".to_string(),
            use_worktree: false,
            assignee_id: None,
            reviewer_id: None,
            complexity: None,
            ai_suggestion: None,
            automation_mode: Some(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1.to_string()),
            last_codex_session_id: None,
            last_review_session_id: None,
            created_at: "2026-04-21 00:00:00".to_string(),
            updated_at: "2026-04-21 00:00:00".to_string(),
        }
    }

    async fn insert_project(pool: &SqlitePool) {
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
            ) VALUES ('project-1', 'demo', NULL, 'active', NULL, '2026-04-21 00:00:00', '2026-04-21 00:00:00')
            "#,
        )
        .execute(pool)
        .await
        .expect("insert project");
    }

    async fn insert_task(pool: &SqlitePool, task: &Task) {
        sqlx::query(
            r#"
            INSERT INTO tasks (
                id,
                title,
                description,
                status,
                priority,
                project_id,
                use_worktree,
                assignee_id,
                reviewer_id,
                automation_mode,
                created_at,
                updated_at
            ) VALUES ($1, $2, NULL, $3, 'medium', $4, 0, NULL, NULL, $5, $6, $7)
            "#,
        )
        .bind(&task.id)
        .bind(&task.title)
        .bind(&task.status)
        .bind(&task.project_id)
        .bind(task.automation_mode.as_deref())
        .bind(&task.created_at)
        .bind(&task.updated_at)
        .execute(pool)
        .await
        .expect("insert task");
    }

    async fn insert_session(
        pool: &SqlitePool,
        session_id: &str,
        task_id: &str,
        session_kind: &str,
    ) {
        sqlx::query(
            r#"
            INSERT INTO codex_sessions (
                id,
                task_id,
                project_id,
                execution_target,
                artifact_capture_mode,
                session_kind,
                status,
                started_at,
                created_at
            ) VALUES ($1, $2, 'project-1', 'local', 'local_full', $3, 'exited', '2026-04-21 00:00:01', '2026-04-21 00:00:01')
            "#,
        )
        .bind(session_id)
        .bind(task_id)
        .bind(session_kind)
        .execute(pool)
        .await
        .expect("insert session");
    }

    async fn insert_session_event(
        pool: &SqlitePool,
        event_id: &str,
        session_id: &str,
        event_type: &str,
        message: &str,
    ) {
        sqlx::query(
            r#"
            INSERT INTO codex_session_events (id, session_id, event_type, message, created_at)
            VALUES ($1, $2, $3, $4, '2026-04-21 00:00:02')
            "#,
        )
        .bind(event_id)
        .bind(session_id)
        .bind(event_type)
        .bind(message)
        .execute(pool)
        .await
        .expect("insert session event");
    }

    #[test]
    fn archived_tasks_are_excluded_from_pending_automation_resume_queue() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
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
                ) VALUES ('project-1', 'demo', NULL, 'active', NULL, '2026-04-21 00:00:00', '2026-04-21 00:00:00')
                "#,
            )
            .execute(&pool)
            .await
            .expect("insert project");

            for task in [
                build_task("task-active", "review"),
                build_task("task-archived", TASK_STATUS_ARCHIVED),
            ] {
                sqlx::query(
                    r#"
                    INSERT INTO tasks (
                        id,
                        title,
                        description,
                        status,
                        priority,
                        project_id,
                        use_worktree,
                        assignee_id,
                        reviewer_id,
                        automation_mode,
                        created_at,
                        updated_at
                    ) VALUES ($1, $2, NULL, $3, 'medium', $4, 0, NULL, NULL, $5, $6, $7)
                    "#,
                )
                .bind(&task.id)
                .bind(&task.title)
                .bind(&task.status)
                .bind(&task.project_id)
                .bind(task.automation_mode.as_deref())
                .bind(&task.created_at)
                .bind(&task.updated_at)
                .execute(&pool)
                .await
                .expect("insert task");

                sqlx::query(
                    r#"
                    INSERT INTO task_automation_state (
                        task_id,
                        phase,
                        round_count,
                        consumed_session_id,
                        last_trigger_session_id,
                        pending_action,
                        pending_round_count,
                        last_error,
                        last_verdict_json,
                        updated_at
                    ) VALUES ($1, $2, 0, NULL, NULL, 'start_review', NULL, NULL, NULL, '2026-04-21 00:00:00')
                    "#,
                )
                .bind(&task.id)
                .bind(PHASE_REVIEW_LAUNCH_FAILED)
                .execute(&pool)
                .await
                .expect("insert task automation state");
            }

            let task_ids = fetch_pending_automation_task_ids(&pool)
                .await
                .expect("load pending automation task ids");
            assert_eq!(task_ids, vec!["task-active".to_string()]);

            pool.close().await;
        });
    }

    #[test]
    fn archived_task_restart_is_rejected() {
        let task = build_task("task-archived", TASK_STATUS_ARCHIVED);

        let error = validate_task_automation_restart(&task).expect_err("archived task restart");
        assert_eq!(error, "已归档任务不能重启自动质控");
    }

    #[test]
    fn blocked_review_state_with_recoverable_verdict_resolves_fix_restart_target() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let task = build_task("task-review", "blocked");
            insert_project(&pool).await;
            insert_task(&pool, &task).await;
            insert_session(&pool, "session-review", &task.id, "review").await;
            insert_session_event(
                &pool,
                "event-review-1",
                "session-review",
                "stdout",
                r#"<review_verdict>{"passed":false,"needs_human":false,"blocking_issue_count":1,"summary":"发现 1 个阻断问题。"}<\/review_verdict>"#,
            )
            .await;

            let state = TaskAutomationStateRecord {
                task_id: task.id.clone(),
                phase: PHASE_BLOCKED.to_string(),
                round_count: 0,
                consumed_session_id: Some("session-review".to_string()),
                last_trigger_session_id: None,
                pending_action: None,
                pending_round_count: None,
                last_error: Some("审核结果结构化输出无效".to_string()),
                last_verdict_json: None,
                updated_at: "2026-04-21 00:00:02".to_string(),
            };

            let target = resolve_restart_target(&pool, &state)
                .await
                .expect("resolve blocked review target");
            assert_eq!(target, Some(AutomationRestartTarget::Fix));

            pool.close().await;
        });
    }

    #[test]
    fn blocked_review_state_without_recoverable_verdict_resolves_review_restart_target() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let task = build_task("task-review-fallback", "blocked");
            insert_project(&pool).await;
            insert_task(&pool, &task).await;
            insert_session(&pool, "session-review-fallback", &task.id, "review").await;

            let state = TaskAutomationStateRecord {
                task_id: task.id.clone(),
                phase: PHASE_BLOCKED.to_string(),
                round_count: 0,
                consumed_session_id: Some("session-review-fallback".to_string()),
                last_trigger_session_id: None,
                pending_action: None,
                pending_round_count: None,
                last_error: Some("审核结果结构化输出无效".to_string()),
                last_verdict_json: None,
                updated_at: "2026-04-21 00:00:02".to_string(),
            };

            let target = resolve_restart_target(&pool, &state)
                .await
                .expect("resolve blocked review fallback target");
            assert_eq!(target, Some(AutomationRestartTarget::Review));

            pool.close().await;
        });
    }

    #[test]
    fn manual_control_execution_state_resolves_fix_restart_target() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let pool = setup_test_pool().await;
            let task = build_task("task-execution", "blocked");
            insert_project(&pool).await;
            insert_task(&pool, &task).await;
            insert_session(&pool, "session-execution", &task.id, "execution").await;

            let state = TaskAutomationStateRecord {
                task_id: task.id.clone(),
                phase: PHASE_MANUAL_CONTROL.to_string(),
                round_count: 1,
                consumed_session_id: Some("session-execution".to_string()),
                last_trigger_session_id: Some("session-execution".to_string()),
                pending_action: None,
                pending_round_count: None,
                last_error: Some("执行已被人工停止".to_string()),
                last_verdict_json: Some(
                    r#"{"passed":false,"needs_human":false,"blocking_issue_count":1,"summary":"发现 1 个阻断问题。"}"#
                        .to_string(),
                ),
                updated_at: "2026-04-21 00:00:02".to_string(),
            };

            let target = resolve_restart_target(&pool, &state)
                .await
                .expect("resolve manual control execution target");
            assert_eq!(target, Some(AutomationRestartTarget::Fix));

            pool.close().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{restart_fix_round_state, PHASE_BLOCKED};
    use crate::db::models::TaskAutomationStateRecord;

    #[test]
    fn blocked_phase_constant_kept_stable() {
        assert_eq!(PHASE_BLOCKED, "blocked");
    }

    #[test]
    fn blocked_fix_restart_resets_round_count() {
        let state = TaskAutomationStateRecord {
            task_id: "task-1".to_string(),
            phase: PHASE_BLOCKED.to_string(),
            round_count: 3,
            consumed_session_id: Some("session-1".to_string()),
            last_trigger_session_id: Some("session-1".to_string()),
            pending_action: None,
            pending_round_count: Some(4),
            last_error: Some("blocked".to_string()),
            last_verdict_json: None,
            updated_at: "2026-04-22 00:00:00".to_string(),
        };

        assert_eq!(restart_fix_round_state(&state), (Some(0), 0));
    }

    #[test]
    fn manual_control_fix_restart_resets_round_count() {
        let state = TaskAutomationStateRecord {
            task_id: "task-1".to_string(),
            phase: "manual_control".to_string(),
            round_count: 3,
            consumed_session_id: Some("session-1".to_string()),
            last_trigger_session_id: Some("session-1".to_string()),
            pending_action: None,
            pending_round_count: Some(4),
            last_error: Some("manual".to_string()),
            last_verdict_json: None,
            updated_at: "2026-04-22 00:00:00".to_string(),
        };

        assert_eq!(restart_fix_round_state(&state), (Some(0), 0));
    }

    #[test]
    fn non_terminal_fix_restart_keeps_existing_round_count() {
        let state = TaskAutomationStateRecord {
            task_id: "task-1".to_string(),
            phase: "launching_fix".to_string(),
            round_count: 3,
            consumed_session_id: Some("session-1".to_string()),
            last_trigger_session_id: Some("session-1".to_string()),
            pending_action: None,
            pending_round_count: Some(4),
            last_error: Some("launching".to_string()),
            last_verdict_json: None,
            updated_at: "2026-04-22 00:00:00".to_string(),
        };

        assert_eq!(restart_fix_round_state(&state), (Some(4), 3));
    }
}
