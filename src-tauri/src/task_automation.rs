mod prompt;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use crate::app::{
    fetch_employee_by_id, fetch_project_by_id, fetch_task_automation_state_record,
    fetch_task_by_id, insert_activity_log, now_sqlite, parse_review_verdict_json,
    record_completion_metric, sqlite_pool, start_task_code_review_internal,
};
use crate::codex::{
    load_codex_settings, start_codex_with_manager, stop_codex_for_automation_restart, CodexManager,
};
use crate::db::models::{ReviewVerdict, Subtask, Task, TaskAttachment, TaskAutomationStateRecord};

const AUTOMATION_MODE_REVIEW_FIX_LOOP_V1: &str = "review_fix_loop_v1";
const DEFAULT_MAX_FIX_ROUNDS: i32 = 3;
const FAILURE_STRATEGY_BLOCKED: &str = "blocked";
const FAILURE_STRATEGY_MANUAL_CONTROL: &str = "manual_control";
const PHASE_IDLE: &str = "idle";
const PHASE_LAUNCHING_REVIEW: &str = "launching_review";
const PHASE_WAITING_REVIEW: &str = "waiting_review";
const PHASE_LAUNCHING_FIX: &str = "launching_fix";
const PHASE_WAITING_EXECUTION: &str = "waiting_execution";
const PHASE_REVIEW_LAUNCH_FAILED: &str = "review_launch_failed";
const PHASE_FIX_LAUNCH_FAILED: &str = "fix_launch_failed";
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
    update_task_status_internal(pool, task, "blocked").await?;
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

pub async fn resume_pending_automation(app: &AppHandle) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    let pending_task_ids = sqlx::query_scalar::<_, String>(
        r#"
        SELECT tas.task_id
        FROM task_automation_state tas
        INNER JOIN tasks t ON t.id = tas.task_id
        WHERE t.automation_mode = $1
          AND tas.phase IN ($2, $3, $4, $5)
        "#,
    )
    .bind(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1)
    .bind(PHASE_LAUNCHING_REVIEW)
    .bind(PHASE_REVIEW_LAUNCH_FAILED)
    .bind(PHASE_LAUNCHING_FIX)
    .bind(PHASE_FIX_LAUNCH_FAILED)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to list pending automation tasks: {}", error))?;

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

    if task.automation_mode.as_deref() != Some(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1) {
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
        update_task_status_internal(pool, task, "completed").await?;
        upsert_state_terminal(
            pool,
            &task.id,
            Some(&facts.session_id),
            PHASE_COMPLETED,
            Some(&verdict_json),
            None,
            None,
            state_record,
        )
        .await?;
        insert_activity_log(
            pool,
            "task_automation_completed",
            verdict.summary.as_str(),
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
        return Ok(());
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

async fn retry_pending_review(
    app: &AppHandle,
    pool: &SqlitePool,
    task_id: &str,
    state_record: &TaskAutomationStateRecord,
) -> Result<(), String> {
    let task = fetch_task_by_id(pool, task_id).await?;
    if task.automation_mode.as_deref() != Some(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1) {
        return Ok(());
    }

    let result = async {
        update_task_status_internal(pool, &task, "review").await?;
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
    if task.automation_mode.as_deref() != Some(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1) {
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
    let repo_path = project
        .repo_path
        .clone()
        .ok_or_else(|| "当前项目未配置仓库路径，无法自动修复".to_string())?;
    let attachments = fetch_task_attachments(pool, &task.id).await?;
    let subtasks = fetch_task_subtasks(pool, &task.id).await?;
    let execution_input =
        prompt::build_automation_fix_prompt(task, &subtasks, &attachments, review_report, verdict);

    update_task_status_internal(pool, task, "in_progress").await?;
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
        Some(repo_path),
        Some(task.id.clone()),
        None,
        Some(execution_input.image_paths),
        Some("execution".to_string()),
    )
    .await
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

async fn update_task_status_internal(
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

    reserve_pending_action(
        pool,
        &task.id,
        state_record.last_trigger_session_id.as_deref(),
        PHASE_LAUNCHING_FIX,
        Some(PENDING_ACTION_START_FIX),
        state_record
            .pending_round_count
            .or(Some(state_record.round_count)),
        state_record.round_count,
        None,
        state_record.last_verdict_json.as_deref(),
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

pub async fn restart_task_automation_internal(
    app: &AppHandle,
    task_id: &str,
) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    let task = fetch_task_by_id(&pool, task_id).await?;
    if task.automation_mode.as_deref() != Some(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1) {
        return Err("当前任务未开启自动质控".to_string());
    }

    let state_record = fetch_task_automation_state_record(&pool, task_id)
        .await?
        .ok_or_else(|| "当前任务没有可重启的自动质控状态".to_string())?;

    match state_record.phase.as_str() {
        PHASE_WAITING_REVIEW | PHASE_LAUNCHING_REVIEW | PHASE_REVIEW_LAUNCH_FAILED => {
            restart_review_step(app, &pool, &task, &state_record).await
        }
        PHASE_WAITING_EXECUTION | PHASE_LAUNCHING_FIX | PHASE_FIX_LAUNCH_FAILED => {
            restart_fix_step(app, &pool, &task, &state_record).await
        }
        _ => Err(format!(
            "当前自动质控阶段“{}”不支持重启，请在卡住或启动失败时使用",
            state_record.phase
        )),
    }
}

#[tauri::command]
pub async fn restart_task_automation(app: AppHandle, task_id: String) -> Result<(), String> {
    restart_task_automation_internal(&app, &task_id).await
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
    sqlx::query_scalar::<_, Option<String>>(
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
    .map(|value| value.flatten())
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
mod tests {
    use super::PHASE_BLOCKED;

    #[test]
    fn blocked_phase_constant_kept_stable() {
        assert_eq!(PHASE_BLOCKED, "blocked");
    }
}
