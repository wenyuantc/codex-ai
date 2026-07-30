// Domain slice: state (included into task_automation)

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

const NO_REVIEWABLE_CODE_CHANGES_MESSAGE: &str =
    "执行完成但没有产生可审核的代码改动，自动质控无法进入审核，需人工补充任务或重新执行";

const WORKTREE_DISABLED_AUTO_COMMIT_SKIPPED_MESSAGE: &str =
    "审核已通过，任务未启用 Worktree，已跳过自动提交代码";

const ORPHANED_RUNNING_SESSION_MESSAGE: &str = "应用启动时发现会话上次未正常收尾，已标记为失败";

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
        stop_task_timer_internal(pool, &task.id, "自动质控转人工处理").await?;
        insert_activity_log(
            pool,
            "task_automation_manual_control",
            message,
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
        emit_task_automation_state_changed(app, task, PHASE_MANUAL_CONTROL);
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
    stop_task_timer_internal(pool, &task.id, "自动质控阻塞").await?;
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
    emit_task_automation_state_changed(app, task, PHASE_BLOCKED);

    Ok(())
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

    let completion_timer_update =
        if current_task.status != "completed" && next_status == "completed" {
            Some(build_task_completion_timer_update(
                current_task,
                chrono::Utc::now().naive_utc(),
            ))
        } else {
            None
        };
    let should_clear_completed_at =
        current_task.status == "completed" && next_status != "completed";

    if let Some((completed_at, time_spent_seconds)) = completion_timer_update.as_ref() {
        sqlx::query(
            r#"
            UPDATE tasks
            SET status = $1,
                time_spent_seconds = $2,
                time_started_at = NULL,
                completed_at = $3
            WHERE id = $4
            "#,
        )
        .bind(next_status)
        .bind(time_spent_seconds)
        .bind(completed_at)
        .bind(&current_task.id)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to update task status internally: {}", error))?;
    } else if should_clear_completed_at {
        sqlx::query("UPDATE tasks SET status = $1, completed_at = NULL WHERE id = $2")
            .bind(next_status)
            .bind(&current_task.id)
            .execute(pool)
            .await
            .map_err(|error| format!("Failed to update task status internally: {}", error))?;
    } else {
        sqlx::query("UPDATE tasks SET status = $1 WHERE id = $2")
            .bind(next_status)
            .bind(&current_task.id)
            .execute(pool)
            .await
            .map_err(|error| format!("Failed to update task status internally: {}", error))?;
    }

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
        if let Some((_, time_spent_seconds)) = completion_timer_update {
            insert_activity_log(
                pool,
                "task_timer_completed",
                &format!(
                    "{}（累计耗时：{}秒）",
                    current_task.title, time_spent_seconds
                ),
                updated_task.assignee_id.as_deref(),
                Some(current_task.id.as_str()),
                Some(current_task.project_id.as_str()),
            )
            .await?;
        }
        record_completion_metric(pool, &updated_task).await?;
    }
    if next_status == "blocked" {
        stop_task_timer_internal(pool, &current_task.id, "自动质控阻塞").await?;
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

