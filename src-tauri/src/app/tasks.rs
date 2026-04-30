use super::*;

pub(crate) async fn fetch_task_by_id(pool: &SqlitePool, id: &str) -> Result<Task, String> {
    sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1 AND deleted_at IS NULL LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Task {} not found: {}", id, error))
}

async fn fetch_any_task_by_id(pool: &SqlitePool, id: &str) -> Result<Task, String> {
    sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Task {} not found: {}", id, error))
}

async fn fetch_task_attachment_by_id(
    pool: &SqlitePool,
    id: &str,
) -> Result<TaskAttachment, String> {
    sqlx::query_as::<_, TaskAttachment>("SELECT * FROM task_attachments WHERE id = $1 LIMIT 1")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| format!("Task attachment {} not found: {}", id, error))
}

pub(crate) async fn fetch_task_attachments(
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

pub(crate) async fn fetch_task_subtasks(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Vec<Subtask>, String> {
    sqlx::query_as::<_, Subtask>(
        "SELECT * FROM subtasks WHERE task_id = $1 ORDER BY sort_order, created_at",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch subtasks: {}", error))
}

async fn validate_assignee_for_project(
    pool: &SqlitePool,
    assignee_id: Option<&str>,
    _project_id: &str,
) -> Result<(), String> {
    let Some(assignee_id) = assignee_id else {
        return Ok(());
    };

    fetch_employee_by_id(pool, assignee_id).await?;
    Ok(())
}

pub(crate) async fn validate_reviewer_for_project(
    pool: &SqlitePool,
    reviewer_id: Option<&str>,
    _project_id: &str,
) -> Result<(), String> {
    let Some(reviewer_id) = reviewer_id else {
        return Ok(());
    };

    let reviewer = fetch_employee_by_id(pool, reviewer_id).await?;
    if reviewer.role != "reviewer" {
        return Err(format!("员工 {} 不是审查员角色", reviewer.name));
    }

    Ok(())
}

pub(crate) async fn validate_coordinator_for_project(
    pool: &SqlitePool,
    coordinator_id: Option<&str>,
    _project_id: &str,
) -> Result<(), String> {
    let Some(coordinator_id) = coordinator_id else {
        return Ok(());
    };

    let coordinator = fetch_employee_by_id(pool, coordinator_id).await?;
    if coordinator.role != "coordinator" {
        return Err(format!("员工 {} 不是协调员角色", coordinator.name));
    }

    Ok(())
}

pub(crate) async fn insert_task_record(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    task: &Task,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO tasks (id, title, description, status, priority, project_id, use_worktree, assignee_id, reviewer_id, coordinator_id, ai_suggestion, plan_content, automation_mode, time_started_at, time_spent_seconds, completed_at, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)",
    )
    .bind(&task.id)
    .bind(&task.title)
    .bind(&task.description)
    .bind(&task.status)
    .bind(&task.priority)
    .bind(&task.project_id)
    .bind(task.use_worktree)
    .bind(&task.assignee_id)
    .bind(&task.reviewer_id)
    .bind(&task.coordinator_id)
    .bind(&task.ai_suggestion)
    .bind(&task.plan_content)
    .bind(&task.automation_mode)
    .bind(&task.time_started_at)
    .bind(task.time_spent_seconds)
    .bind(&task.completed_at)
    .bind(&task.created_at)
    .bind(&task.updated_at)
    .execute(&mut **tx)
    .await
    .map_err(|error| format!("Failed to create task: {}", error))?;

    Ok(())
}

async fn resolve_employee_activity_label(pool: &SqlitePool, employee_id: Option<&str>) -> String {
    let Some(employee_id) = employee_id else {
        return "未指定".to_string();
    };

    match fetch_employee_by_id(pool, employee_id).await {
        Ok(employee) => format!("{}（{}）", employee.name, employee.id),
        Err(_) => employee_id.to_string(),
    }
}

fn format_task_coordinator_changed_details(
    task_title: &str,
    previous_label: &str,
    next_label: &str,
) -> String {
    format!(
        "{}（协调员：{} -> {}）",
        task_title, previous_label, next_label
    )
}

fn format_task_plan_saved_details(
    task_title: &str,
    coordinator_label: &str,
    plan_content: &str,
) -> String {
    format!(
        "{}（协调员：{}；计划长度：{} 字）",
        task_title,
        coordinator_label,
        plan_content.chars().count()
    )
}

fn format_task_duration_label(total_seconds: i64) -> String {
    let total_seconds = total_seconds.max(0);
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        if minutes > 0 {
            format!("{}小时{}分钟", hours, minutes)
        } else {
            format!("{}小时", hours)
        }
    } else if minutes > 0 {
        if seconds > 0 {
            format!("{}分钟{}秒", minutes, seconds)
        } else {
            format!("{}分钟", minutes)
        }
    } else {
        format!("{}秒", seconds)
    }
}

fn task_time_spent_seconds_at(task: &Task, now: NaiveDateTime) -> i64 {
    let tracked = task.time_spent_seconds.max(0);
    let Some(started_at) = task.time_started_at.as_deref() else {
        return tracked;
    };

    let Some(started_at) = parse_sqlite_datetime(started_at) else {
        return tracked;
    };

    tracked + (now - started_at).num_seconds().max(0)
}

pub(crate) fn build_task_completion_timer_update(
    task: &Task,
    completed_at: NaiveDateTime,
) -> (String, i64) {
    (
        completed_at.format(SQLITE_DATETIME_FORMAT).to_string(),
        task_time_spent_seconds_at(task, completed_at),
    )
}

pub(crate) fn should_clear_task_completed_at(task: &Task, next_status: &str) -> bool {
    task.status == "completed" && next_status != "completed"
}

fn build_task_timer_activity_details(task_title: &str, total_seconds: i64) -> String {
    format!(
        "{}（累计耗时：{}）",
        task_title,
        format_task_duration_label(total_seconds)
    )
}

fn build_task_timer_stopped_details(task_title: &str, total_seconds: i64, reason: &str) -> String {
    format!(
        "{}（{}；累计耗时：{}）",
        task_title,
        reason,
        format_task_duration_label(total_seconds)
    )
}

pub(crate) async fn start_task_timer_internal(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Task, String> {
    let task = fetch_task_by_id(pool, task_id).await?;

    if task.status == TASK_STATUS_ARCHIVED {
        return Err("已归档任务不可开始计时".to_string());
    }
    if task.status == "completed" {
        return Err("已完成任务不可直接开始计时，请先重新打开任务".to_string());
    }
    if task.time_started_at.is_some() {
        return Ok(task);
    }

    let started_at = now_sqlite();
    let result = sqlx::query(
        r#"
        UPDATE tasks
        SET time_started_at = $1,
            completed_at = NULL
        WHERE id = $2
          AND time_started_at IS NULL
        "#,
    )
    .bind(&started_at)
    .bind(task_id)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to start task timer: {}", error))?;

    if result.rows_affected() > 0 {
        insert_activity_log(
            pool,
            "task_timer_started",
            &task.title,
            task.assignee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
    }

    fetch_task_by_id(pool, task_id).await
}

pub(crate) async fn stop_task_timer_internal(
    pool: &SqlitePool,
    task_id: &str,
    reason: &str,
) -> Result<Task, String> {
    let task = fetch_task_by_id(pool, task_id).await?;
    if task.time_started_at.is_none() {
        return Ok(task);
    }

    let now = Utc::now().naive_utc();
    let total_seconds = task_time_spent_seconds_at(&task, now);

    let result = sqlx::query(
        r#"
        UPDATE tasks
        SET time_spent_seconds = $1,
            time_started_at = NULL
        WHERE id = $2
          AND time_started_at IS NOT NULL
        "#,
    )
    .bind(total_seconds)
    .bind(task_id)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to stop task timer: {}", error))?;

    if result.rows_affected() > 0 {
        insert_activity_log(
            pool,
            "task_timer_stopped",
            &build_task_timer_stopped_details(&task.title, total_seconds, reason),
            task.assignee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
    }

    fetch_task_by_id(pool, task_id).await
}

pub(crate) async fn fetch_task_automation_state_record(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Option<TaskAutomationStateRecord>, String> {
    sqlx::query_as::<_, TaskAutomationStateRecord>(
        "SELECT * FROM task_automation_state WHERE task_id = $1 LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch task automation state: {}", error))
}

pub(crate) fn decode_task_automation_state(
    record: TaskAutomationStateRecord,
) -> Result<TaskAutomationState, String> {
    let last_verdict = match record.last_verdict_json.as_deref() {
        Some(raw) => Some(parse_review_verdict_json(raw)?),
        None => None,
    };

    Ok(TaskAutomationState {
        task_id: record.task_id,
        phase: record.phase,
        round_count: record.round_count,
        consumed_session_id: record.consumed_session_id,
        last_trigger_session_id: record.last_trigger_session_id,
        pending_action: record.pending_action,
        pending_round_count: record.pending_round_count,
        last_error: record.last_error,
        last_verdict,
        updated_at: record.updated_at,
    })
}

async fn resolve_next_task_attachment_sort_order(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<i32, String> {
    let next = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM task_attachments WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to resolve attachment order: {}", error))?
    .flatten()
    .unwrap_or(1);

    Ok(next as i32)
}

async fn insert_task_attachments(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    attachments: &[TaskAttachment],
) -> Result<(), String> {
    for attachment in attachments {
        sqlx::query(
            "INSERT INTO task_attachments (id, task_id, original_name, stored_path, mime_type, file_size, sort_order, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(&attachment.id)
        .bind(&attachment.task_id)
        .bind(&attachment.original_name)
        .bind(&attachment.stored_path)
        .bind(&attachment.mime_type)
        .bind(attachment.file_size)
        .bind(attachment.sort_order)
        .bind(&attachment.created_at)
        .execute(&mut **tx)
        .await
        .map_err(|error| format!("Failed to insert task attachment: {}", error))?;
    }

    Ok(())
}

pub(crate) async fn record_completion_metric(pool: &SqlitePool, task: &Task) -> Result<(), String> {
    let Some(employee_id) = task.assignee_id.as_deref() else {
        return Ok(());
    };

    let now = Utc::now().naive_utc();
    let duration_secs = if task.time_spent_seconds > 0 {
        task.time_spent_seconds as f64
    } else {
        let task_created_at = parse_sqlite_datetime(&task.created_at)
            .ok_or_else(|| format!("Invalid task created_at: {}", task.created_at))?;
        (now - task_created_at).num_seconds().max(0) as f64
    };

    let day_start = now
        .date()
        .and_hms_opt(0, 0, 0)
        .expect("valid day start")
        .format(SQLITE_DATETIME_FORMAT)
        .to_string();
    let day_end = (now + Duration::days(1))
        .date()
        .and_hms_opt(0, 0, 0)
        .expect("valid day end")
        .format(SQLITE_DATETIME_FORMAT)
        .to_string();

    let existing = sqlx::query_as::<_, EmployeeMetric>(
        "SELECT * FROM employee_metrics WHERE employee_id = $1 AND period_start = $2 AND period_end = $3 LIMIT 1",
    )
    .bind(employee_id)
    .bind(&day_start)
    .bind(&day_end)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch employee metrics: {}", error))?;

    if let Some(existing) = existing {
        let previous_count = existing.tasks_completed.max(0) as f64;
        let new_count = existing.tasks_completed + 1;
        let avg_completion_time = if previous_count == 0.0 {
            duration_secs
        } else {
            ((existing.average_completion_time.unwrap_or(duration_secs) * previous_count)
                + duration_secs)
                / (previous_count + 1.0)
        };
        let success_rate = if previous_count == 0.0 {
            100.0
        } else {
            ((existing.success_rate.unwrap_or(100.0) * previous_count) + 100.0)
                / (previous_count + 1.0)
        };

        sqlx::query(
            "UPDATE employee_metrics SET tasks_completed = $1, average_completion_time = $2, success_rate = $3 WHERE id = $4",
        )
        .bind(new_count)
        .bind(avg_completion_time)
        .bind(success_rate)
        .bind(existing.id)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to update employee metrics: {}", error))?;
    } else {
        sqlx::query(
            "INSERT INTO employee_metrics (id, employee_id, tasks_completed, average_completion_time, success_rate, period_start, period_end) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(new_id())
        .bind(employee_id)
        .bind(1_i64)
        .bind(duration_secs)
        .bind(100.0_f64)
        .bind(day_start)
        .bind(day_end)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to insert employee metrics: {}", error))?;
    }

    Ok(())
}

pub(crate) async fn clear_task_automation_state_for_disabled_mode(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"
        UPDATE task_automation_state
        SET pending_action = NULL,
            pending_round_count = NULL,
            last_verdict_json = NULL,
            phase = CASE
                WHEN phase IN ('review_launch_failed', 'fix_launch_failed') THEN 'idle'
                ELSE phase
            END,
            updated_at = $2
        WHERE task_id = $1
        "#,
    )
    .bind(task_id)
    .bind(now_sqlite())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to clear pending automation state: {}", error))?;

    Ok(())
}

pub(crate) async fn disable_task_automation_for_archived_task(
    pool: &SqlitePool,
    task: &Task,
) -> Result<(), String> {
    if task.automation_mode.as_deref() != Some(TASK_AUTOMATION_MODE_REVIEW_FIX_LOOP_V1) {
        return Ok(());
    }

    clear_task_automation_state_for_disabled_mode(pool, &task.id).await?;
    insert_activity_log(
        pool,
        "task_automation_disabled",
        &format!("{}（任务归档时自动关闭）", task.title),
        None,
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;

    Ok(())
}

pub(crate) fn is_task_automation_active_for_archival(phase: &str) -> bool {
    matches!(
        phase,
        TASK_AUTOMATION_PHASE_LAUNCHING_REVIEW
            | TASK_AUTOMATION_PHASE_WAITING_REVIEW
            | TASK_AUTOMATION_PHASE_LAUNCHING_FIX
            | TASK_AUTOMATION_PHASE_WAITING_EXECUTION
            | TASK_AUTOMATION_PHASE_COMMITTING_CODE
    )
}

pub(crate) fn validate_task_archival_guard(
    has_running_execution: bool,
    has_running_review: bool,
    automation_phase: Option<&str>,
) -> Result<(), String> {
    let mut blockers = Vec::new();

    if has_running_execution {
        blockers.push("执行");
    }
    if has_running_review {
        blockers.push("审核");
    }
    if automation_phase.is_some_and(is_task_automation_active_for_archival) {
        blockers.push("自动质控");
    }

    if blockers.is_empty() {
        return Ok(());
    }

    Err(format!(
        "任务仍有进行中的{}流程，不能归档，请先停止相关会话或等待流程结束",
        blockers.join("、")
    ))
}

async fn ensure_task_can_be_archived<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    task: &Task,
) -> Result<(), String> {
    let manager = app.state::<Arc<Mutex<CodexManager>>>().inner().clone();
    let has_running_execution = crate::codex::get_live_task_process_by_task(
        app,
        &manager,
        &task.id,
        CodexSessionKind::Execution,
    )
    .await?
    .is_some();
    let has_running_review = crate::codex::get_live_task_process_by_task(
        app,
        &manager,
        &task.id,
        CodexSessionKind::Review,
    )
    .await?
    .is_some();
    let automation_phase = fetch_task_automation_state_record(pool, &task.id)
        .await?
        .map(|record| record.phase);

    validate_task_archival_guard(
        has_running_execution,
        has_running_review,
        automation_phase.as_deref(),
    )
}

pub(crate) fn validate_task_automation_mode_change(
    task: &Task,
    automation_mode: Option<&str>,
) -> Result<(), String> {
    if let Some(mode) = automation_mode {
        if mode != TASK_AUTOMATION_MODE_REVIEW_FIX_LOOP_V1 {
            return Err(format!("不支持的自动质控模式: {}", mode));
        }
        if task.status == TASK_STATUS_ARCHIVED {
            return Err("已归档任务不能开启自动质控".to_string());
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn set_task_automation_mode<R: Runtime>(
    app: AppHandle<R>,
    payload: SetTaskAutomationModePayload,
) -> Result<Task, String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &payload.task_id).await?;
    let normalized_mode = payload
        .automation_mode
        .and_then(|value| value)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    validate_task_automation_mode_change(&task, normalized_mode.as_deref())?;

    sqlx::query("UPDATE tasks SET automation_mode = $1 WHERE id = $2")
        .bind(&normalized_mode)
        .bind(&payload.task_id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update task automation mode: {}", error))?;

    if normalized_mode.is_some() {
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
            ) VALUES ($1, 'idle', 0, NULL, NULL, NULL, NULL, NULL, NULL, $2)
            ON CONFLICT(task_id) DO UPDATE SET
                phase = 'idle',
                round_count = 0,
                consumed_session_id = NULL,
                last_trigger_session_id = NULL,
                pending_action = NULL,
                pending_round_count = NULL,
                last_error = NULL,
                last_verdict_json = NULL,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&payload.task_id)
        .bind(now_sqlite())
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to upsert task automation state: {}", error))?;
    } else {
        clear_task_automation_state_for_disabled_mode(&pool, &payload.task_id).await?;
    }

    insert_activity_log(
        &pool,
        if normalized_mode.is_some() {
            "task_automation_enabled"
        } else {
            "task_automation_disabled"
        },
        &task.title,
        None,
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;

    fetch_task_by_id(&pool, &payload.task_id).await
}

#[tauri::command]
pub async fn get_task_automation_state<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<Option<TaskAutomationState>, String> {
    let pool = sqlite_pool(&app).await?;
    let Some(record) = fetch_task_automation_state_record(&pool, &task_id).await? else {
        return Ok(None);
    };

    Ok(Some(decode_task_automation_state(record)?))
}

#[tauri::command]
pub async fn start_task_timer<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<Task, String> {
    let pool = sqlite_pool(&app).await?;
    start_task_timer_internal(&pool, &task_id).await
}

#[tauri::command]
pub async fn create_task<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateTask,
) -> Result<Task, String> {
    let pool = sqlite_pool(&app).await?;
    ensure_project_exists(&pool, &payload.project_id).await?;
    validate_assignee_for_project(&pool, payload.assignee_id.as_deref(), &payload.project_id)
        .await?;
    validate_reviewer_for_project(&pool, payload.reviewer_id.as_deref(), &payload.project_id)
        .await?;
    validate_coordinator_for_project(
        &pool,
        payload.coordinator_id.as_deref(),
        &payload.project_id,
    )
    .await?;
    let project = fetch_project_by_id(&pool, &payload.project_id).await?;
    let settings = resolve_project_task_default_settings(
        &project.project_type,
        project.ssh_config_id.as_deref(),
        || load_codex_settings(&app),
        |ssh_config_id| load_remote_codex_settings(&app, ssh_config_id),
    );
    let automation_mode = settings
        .as_ref()
        .filter(|settings| settings.task_automation_default_enabled)
        .map(|_| "review_fix_loop_v1".to_string());
    let default_task_use_worktree = settings
        .as_ref()
        .map(|settings| settings.git_preferences.default_task_use_worktree)
        .unwrap_or(false);

    if automation_mode.is_some()
        && normalize_optional_text(payload.reviewer_id.as_deref()).is_none()
    {
        return Err("当前已开启“新建任务默认自动质控”，请先指定审查员。".to_string());
    }

    let task = Task {
        id: new_id(),
        title: payload.title.trim().to_string(),
        description: normalize_optional_text(payload.description.as_deref()),
        status: "todo".to_string(),
        priority: payload.priority.unwrap_or_else(|| "medium".to_string()),
        project_id: payload.project_id,
        use_worktree: payload.use_worktree.unwrap_or(default_task_use_worktree),
        assignee_id: normalize_optional_text(payload.assignee_id.as_deref()),
        reviewer_id: normalize_optional_text(payload.reviewer_id.as_deref()),
        coordinator_id: normalize_optional_text(payload.coordinator_id.as_deref()),
        complexity: None,
        ai_suggestion: None,
        plan_content: normalize_optional_text(payload.plan_content.as_deref()),
        automation_mode,
        last_codex_session_id: None,
        last_review_session_id: None,
        time_started_at: None,
        time_spent_seconds: 0,
        completed_at: None,
        deleted_at: None,
        created_at: now_sqlite(),
        updated_at: now_sqlite(),
    };

    if task.title.is_empty() {
        return Err("任务标题不能为空".to_string());
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start task transaction: {}", error))?;

    insert_task_record(&mut tx, &task).await?;

    if task.automation_mode.is_some() {
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
            ) VALUES ($1, 'idle', 0, NULL, NULL, NULL, NULL, NULL, NULL, $2)
            "#,
        )
        .bind(&task.id)
        .bind(now_sqlite())
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("Failed to initialize task automation state: {}", error))?;
    }

    let mut uploaded_remote_paths = Vec::new();
    let attachments = if let Some(source_paths) = payload.attachment_source_paths.as_ref() {
        let attachments = build_task_attachments_from_sources(&app, &task.id, source_paths, 1)?;
        let image_attachments = filter_image_attachments(&attachments);
        if project.project_type == PROJECT_TYPE_SSH && !image_attachments.is_empty() {
            let ssh_config_id = project
                .ssh_config_id
                .as_deref()
                .ok_or_else(|| "当前 SSH 项目未绑定 SSH 配置，无法同步图片到远程".to_string())?;
            match sync_task_attachment_records_to_remote(
                &app,
                ssh_config_id,
                &image_attachments,
                false,
            )
            .await
            {
                Ok(sync_result) => {
                    uploaded_remote_paths = sync_result.remote_paths;
                }
                Err(error) => {
                    cleanup_task_attachment_files(
                        &attachments
                            .iter()
                            .map(|attachment| attachment.stored_path.clone())
                            .collect::<Vec<_>>(),
                    );
                    cleanup_empty_attachment_dir(&app, &task.id);
                    tx.rollback().await.ok();
                    return Err(error);
                }
            }
        }
        if let Err(error) = insert_task_attachments(&mut tx, &attachments).await {
            cleanup_task_attachment_files(
                &attachments
                    .iter()
                    .map(|attachment| attachment.stored_path.clone())
                    .collect::<Vec<_>>(),
            );
            cleanup_empty_attachment_dir(&app, &task.id);
            if project.project_type == PROJECT_TYPE_SSH {
                if let Some(ssh_config_id) = project.ssh_config_id.as_deref() {
                    cleanup_remote_task_attachment_paths(
                        &app,
                        ssh_config_id,
                        &uploaded_remote_paths,
                    )
                    .await;
                }
            }
            tx.rollback().await.ok();
            return Err(error);
        }
        attachments
    } else {
        Vec::new()
    };

    if let Err(error) = tx.commit().await {
        cleanup_task_attachment_files(
            &attachments
                .iter()
                .map(|attachment| attachment.stored_path.clone())
                .collect::<Vec<_>>(),
        );
        cleanup_empty_attachment_dir(&app, &task.id);
        if project.project_type == PROJECT_TYPE_SSH {
            if let Some(ssh_config_id) = project.ssh_config_id.as_deref() {
                cleanup_remote_task_attachment_paths(&app, ssh_config_id, &uploaded_remote_paths)
                    .await;
            }
        }
        return Err(format!("Failed to commit task create: {}", error));
    }

    insert_activity_log(
        &pool,
        "task_created",
        &format!(
            "{}{}",
            task.title,
            if attachments.is_empty() {
                "".to_string()
            } else {
                format!("（含 {} 个附件）", attachments.len())
            }
        ),
        None,
        Some(&task.id),
        Some(&task.project_id),
    )
    .await?;

    if task.use_worktree {
        insert_activity_log(
            &pool,
            "task_worktree_enabled",
            &format!("{}（新建任务已开启独立 worktree）", task.title),
            None,
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
    }

    if task.coordinator_id.is_some() {
        let coordinator_label =
            resolve_employee_activity_label(&pool, task.coordinator_id.as_deref()).await;
        insert_activity_log(
            &pool,
            "task_coordinator_changed",
            &format_task_coordinator_changed_details(&task.title, "未指定", &coordinator_label),
            task.coordinator_id.as_deref(),
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
    }

    if let Some(plan_content) = task.plan_content.as_deref() {
        let coordinator_label =
            resolve_employee_activity_label(&pool, task.coordinator_id.as_deref()).await;
        insert_activity_log(
            &pool,
            "task_plan_saved",
            &format_task_plan_saved_details(&task.title, &coordinator_label, plan_content),
            task.coordinator_id.as_deref(),
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
    }

    if project.project_type == PROJECT_TYPE_SSH && !uploaded_remote_paths.is_empty() {
        insert_activity_log(
            &pool,
            "remote_task_attachments_synced",
            &format!(
                "{}（已同步 {} 张图片到远程）",
                task.title,
                uploaded_remote_paths.len()
            ),
            None,
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
    }

    if task.automation_mode.is_some() {
        insert_activity_log(
            &pool,
            "task_automation_enabled",
            &format!("{}（新建任务默认开启）", task.title),
            None,
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
    }

    fetch_task_by_id(&pool, &task.id).await
}

pub(crate) fn resolve_project_task_default_settings<T, LocalLoader, RemoteLoader>(
    project_type: &str,
    ssh_config_id: Option<&str>,
    load_local_settings: LocalLoader,
    load_remote_settings: RemoteLoader,
) -> Option<T>
where
    LocalLoader: FnOnce() -> Result<T, String>,
    RemoteLoader: FnOnce(&str) -> Result<T, String>,
{
    if project_type == PROJECT_TYPE_SSH {
        if let Some(ssh_config_id) = ssh_config_id {
            if let Ok(settings) = load_remote_settings(ssh_config_id) {
                return Some(settings);
            }
        }
    }

    load_local_settings().ok()
}

#[tauri::command]
pub async fn add_task_attachments<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
    source_paths: Vec<String>,
) -> Result<Vec<TaskAttachment>, String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &task_id).await?;
    let project = fetch_project_by_id(&pool, &task.project_id).await?;

    if source_paths.is_empty() {
        return Ok(Vec::new());
    }

    let start_sort_order = resolve_next_task_attachment_sort_order(&pool, &task_id).await?;
    let attachments =
        build_task_attachments_from_sources(&app, &task_id, &source_paths, start_sort_order)?;
    let mut uploaded_remote_paths = Vec::new();
    let image_attachments = filter_image_attachments(&attachments);

    if project.project_type == PROJECT_TYPE_SSH && !image_attachments.is_empty() {
        let ssh_config_id = project
            .ssh_config_id
            .as_deref()
            .ok_or_else(|| "当前 SSH 项目未绑定 SSH 配置，无法同步图片到远程".to_string())?;
        match sync_task_attachment_records_to_remote(&app, ssh_config_id, &image_attachments, false)
            .await
        {
            Ok(sync_result) => {
                uploaded_remote_paths = sync_result.remote_paths;
            }
            Err(error) => {
                cleanup_task_attachment_files(
                    &attachments
                        .iter()
                        .map(|attachment| attachment.stored_path.clone())
                        .collect::<Vec<_>>(),
                );
                cleanup_empty_attachment_dir(&app, &task_id);
                return Err(error);
            }
        }
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("Failed to start attachment transaction: {}", error))?;

    if let Err(error) = insert_task_attachments(&mut tx, &attachments).await {
        cleanup_task_attachment_files(
            &attachments
                .iter()
                .map(|attachment| attachment.stored_path.clone())
                .collect::<Vec<_>>(),
        );
        cleanup_empty_attachment_dir(&app, &task_id);
        if project.project_type == PROJECT_TYPE_SSH {
            if let Some(ssh_config_id) = project.ssh_config_id.as_deref() {
                cleanup_remote_task_attachment_paths(&app, ssh_config_id, &uploaded_remote_paths)
                    .await;
            }
        }
        tx.rollback().await.ok();
        return Err(error);
    }

    if let Err(error) = tx.commit().await {
        cleanup_task_attachment_files(
            &attachments
                .iter()
                .map(|attachment| attachment.stored_path.clone())
                .collect::<Vec<_>>(),
        );
        cleanup_empty_attachment_dir(&app, &task_id);
        if project.project_type == PROJECT_TYPE_SSH {
            if let Some(ssh_config_id) = project.ssh_config_id.as_deref() {
                cleanup_remote_task_attachment_paths(&app, ssh_config_id, &uploaded_remote_paths)
                    .await;
            }
        }
        return Err(format!("Failed to commit attachment create: {}", error));
    }

    if project.project_type == PROJECT_TYPE_SSH && !uploaded_remote_paths.is_empty() {
        insert_activity_log(
            &pool,
            "remote_task_attachments_synced",
            &format!(
                "{}（追加同步 {} 张图片到远程）",
                task.title,
                uploaded_remote_paths.len()
            ),
            None,
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
    }

    fetch_task_attachments(&pool, &task_id).await
}

#[tauri::command]
pub async fn delete_task_attachment<R: Runtime>(
    app: AppHandle<R>,
    id: String,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let attachment = fetch_task_attachment_by_id(&pool, &id).await?;
    let task = fetch_task_by_id(&pool, &attachment.task_id).await?;
    let project = fetch_project_by_id(&pool, &task.project_id).await?;
    let stored_path = Path::new(&attachment.stored_path);

    if stored_path.exists() {
        fs::remove_file(stored_path)
            .map_err(|error| format!("删除附件文件失败: {}: {}", stored_path.display(), error))?;
    }

    sqlx::query("DELETE FROM task_attachments WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to delete task attachment: {}", error))?;

    cleanup_empty_attachment_dir(&app, &attachment.task_id);
    if project.project_type == PROJECT_TYPE_SSH {
        if let Some(ssh_config_id) = project.ssh_config_id.as_deref() {
            if let Err(error) =
                cleanup_remote_task_attachment(&app, ssh_config_id, &attachment).await
            {
                eprintln!("[task-attachments] 删除远程附件失败: {}", error);
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn update_task<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    updates: UpdateTask,
) -> Result<Task, String> {
    let pool = sqlite_pool(&app).await?;
    let current = fetch_task_by_id(&pool, &id).await?;
    let next_status = updates
        .status
        .clone()
        .unwrap_or_else(|| current.status.clone());
    let is_archiving =
        next_status == TASK_STATUS_ARCHIVED && current.status != TASK_STATUS_ARCHIVED;
    let completion_time_update = if next_status == "completed" && current.status != "completed" {
        Some(build_task_completion_timer_update(
            &current,
            Utc::now().naive_utc(),
        ))
    } else {
        None
    };
    let is_reopening_completed_task = should_clear_task_completed_at(&current, &next_status);

    if let Some(assignee_id) = updates.assignee_id.as_ref() {
        validate_assignee_for_project(&pool, assignee_id.as_deref(), &current.project_id).await?;
    }
    if let Some(reviewer_id) = updates.reviewer_id.as_ref() {
        validate_reviewer_for_project(&pool, reviewer_id.as_deref(), &current.project_id).await?;
    }
    let normalized_coordinator_id = updates.coordinator_id.as_ref().map(|value| {
        value
            .as_deref()
            .and_then(|coordinator_id| normalize_optional_text(Some(coordinator_id)))
    });
    if let Some(coordinator_id) = normalized_coordinator_id.as_ref() {
        validate_coordinator_for_project(&pool, coordinator_id.as_deref(), &current.project_id)
            .await?;
    }
    let normalized_plan_content = updates.plan_content.as_ref().map(|value| {
        value
            .as_deref()
            .and_then(|plan_content| normalize_optional_text(Some(plan_content)))
    });
    let coordinator_changed = normalized_coordinator_id
        .as_ref()
        .map(|coordinator_id| current.coordinator_id.as_deref() != coordinator_id.as_deref())
        .unwrap_or(false);
    let effective_plan_content = if normalized_plan_content.is_some() {
        normalized_plan_content.clone()
    } else if coordinator_changed && current.plan_content.is_some() {
        Some(None)
    } else {
        None
    };
    if is_archiving {
        ensure_task_can_be_archived(&app, &pool, &current).await?;
    }

    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE tasks SET ");
    let mut separated = builder.separated(", ");
    let mut touched = false;

    if let Some(title) = updates.title {
        let trimmed = title.trim().to_string();
        if trimmed.is_empty() {
            return Err("任务标题不能为空".to_string());
        }
        separated.push("title = ").push_bind_unseparated(trimmed);
        touched = true;
    }
    if let Some(description) = updates.description {
        separated.push("description = ").push_bind_unseparated(
            description.and_then(|value| normalize_optional_text(Some(&value))),
        );
        touched = true;
    }
    if let Some(status) = updates.status.clone() {
        separated.push("status = ").push_bind_unseparated(status);
        touched = true;
    }
    if let Some(priority) = updates.priority {
        separated
            .push("priority = ")
            .push_bind_unseparated(priority);
        touched = true;
    }
    if let Some(assignee_id) = updates.assignee_id {
        separated
            .push("assignee_id = ")
            .push_bind_unseparated(assignee_id);
        touched = true;
    }
    if let Some(reviewer_id) = updates.reviewer_id {
        separated
            .push("reviewer_id = ")
            .push_bind_unseparated(reviewer_id);
        touched = true;
    }
    if let Some(coordinator_id) = normalized_coordinator_id.clone() {
        separated
            .push("coordinator_id = ")
            .push_bind_unseparated(coordinator_id);
        touched = true;
    }
    if let Some(complexity) = updates.complexity {
        separated
            .push("complexity = ")
            .push_bind_unseparated(complexity);
        touched = true;
    }
    if let Some(ai_suggestion) = updates.ai_suggestion {
        separated
            .push("ai_suggestion = ")
            .push_bind_unseparated(ai_suggestion);
        touched = true;
    }
    if let Some(plan_content) = effective_plan_content.clone() {
        separated
            .push("plan_content = ")
            .push_bind_unseparated(plan_content);
        touched = true;
    }
    if let Some(last_codex_session_id) = updates.last_codex_session_id {
        separated
            .push("last_codex_session_id = ")
            .push_bind_unseparated(last_codex_session_id);
        touched = true;
    }
    if let Some(last_review_session_id) = updates.last_review_session_id {
        separated
            .push("last_review_session_id = ")
            .push_bind_unseparated(last_review_session_id);
        touched = true;
    }
    if let Some((completed_at, time_spent_seconds)) = completion_time_update.clone() {
        separated
            .push("time_spent_seconds = ")
            .push_bind_unseparated(time_spent_seconds);
        separated
            .push("time_started_at = ")
            .push_bind_unseparated(Option::<String>::None);
        separated
            .push("completed_at = ")
            .push_bind_unseparated(Some(completed_at));
        touched = true;
    } else if is_reopening_completed_task {
        separated
            .push("completed_at = ")
            .push_bind_unseparated(Option::<String>::None);
        touched = true;
    }
    if is_archiving
        && current.automation_mode.as_deref() == Some(TASK_AUTOMATION_MODE_REVIEW_FIX_LOOP_V1)
    {
        separated
            .push("automation_mode = ")
            .push_bind_unseparated(Option::<String>::None);
        touched = true;
    }

    if !touched {
        return Ok(current);
    }

    builder.push(" WHERE id = ").push_bind(&id);
    builder
        .build()
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update task: {}", error))?;

    if is_archiving {
        disable_task_automation_for_archived_task(&pool, &current).await?;
    }

    let next_coordinator_id = normalized_coordinator_id
        .clone()
        .unwrap_or_else(|| current.coordinator_id.clone());
    if let Some(updated_coordinator_id) = normalized_coordinator_id {
        if current.coordinator_id != updated_coordinator_id {
            let previous_label =
                resolve_employee_activity_label(&pool, current.coordinator_id.as_deref()).await;
            let next_label =
                resolve_employee_activity_label(&pool, updated_coordinator_id.as_deref()).await;
            insert_activity_log(
                &pool,
                "task_coordinator_changed",
                &format_task_coordinator_changed_details(
                    &current.title,
                    &previous_label,
                    &next_label,
                ),
                updated_coordinator_id.as_deref(),
                Some(&id),
                Some(&current.project_id),
            )
            .await?;
        }
    }

    if let Some(updated_plan_content) = normalized_plan_content {
        if current.plan_content != updated_plan_content {
            if let Some(plan_content) = updated_plan_content.as_deref() {
                let coordinator_label =
                    resolve_employee_activity_label(&pool, next_coordinator_id.as_deref()).await;
                insert_activity_log(
                    &pool,
                    "task_plan_saved",
                    &format_task_plan_saved_details(
                        &current.title,
                        &coordinator_label,
                        plan_content,
                    ),
                    next_coordinator_id.as_deref(),
                    Some(&id),
                    Some(&current.project_id),
                )
                .await?;
            }
        }
    }

    if next_status != current.status {
        insert_activity_log(
            &pool,
            "task_status_changed",
            &format!("{} -> {}", current.title, next_status),
            None,
            Some(&id),
            Some(&current.project_id),
        )
        .await?;

        if current.status != "completed" && next_status == "completed" {
            let updated_task = fetch_task_by_id(&pool, &id).await?;
            if let Some((_, time_spent_seconds)) = completion_time_update {
                insert_activity_log(
                    &pool,
                    "task_timer_completed",
                    &build_task_timer_activity_details(&current.title, time_spent_seconds),
                    updated_task.assignee_id.as_deref(),
                    Some(&id),
                    Some(&current.project_id),
                )
                .await?;
            }
            record_completion_metric(&pool, &updated_task).await?;
        }
        if is_reopening_completed_task {
            insert_activity_log(
                &pool,
                "task_timer_reopened",
                &current.title,
                current.assignee_id.as_deref(),
                Some(&id),
                Some(&current.project_id),
            )
            .await?;
        }

        if let Some(draft) =
            build_task_status_notification(&current, current.status.as_str(), &next_status)
        {
            let _ = publish_one_time_notification(&app, draft).await?;
        }
    }

    fetch_task_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn update_task_status<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    status: String,
) -> Result<Task, String> {
    update_task(
        app,
        id,
        UpdateTask {
            title: None,
            description: None,
            status: Some(status),
            priority: None,
            assignee_id: None,
            reviewer_id: None,
            coordinator_id: None,
            complexity: None,
            ai_suggestion: None,
            plan_content: None,
            last_codex_session_id: None,
            last_review_session_id: None,
        },
    )
    .await
}

#[tauri::command]
pub async fn delete_task<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &id).await?;

    sqlx::query("UPDATE tasks SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("软删除任务失败: {}", error))?;

    insert_activity_log(
        &pool,
        "task_deleted",
        &task.title,
        None,
        None,
        Some(&task.project_id),
    )
    .await?;

    Ok(())
}

#[tauri::command]
pub async fn permanently_delete_task<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_any_task_by_id(&pool, &id).await?;
    let project = fetch_any_project_by_id(&pool, &task.project_id).await?;
    let attachment_dir = task_attachment_dir(&app, &id).ok();
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("开始任务事务失败: {}", error))?;

    sqlx::query("DELETE FROM activity_logs WHERE task_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("删除任务活动日志失败: {}", error))?;
    sqlx::query("DELETE FROM tasks WHERE id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("永久删除任务失败: {}", error))?;

    insert_activity_log(
        &mut *tx,
        "task_permanently_deleted",
        &task.title,
        None,
        None,
        Some(&task.project_id),
    )
    .await?;

    tx.commit()
        .await
        .map_err(|error| format!("提交任务永久删除失败: {}", error))?;

    if let Some(attachment_dir) = attachment_dir.filter(|path| path.exists()) {
        if let Err(error) = fs::remove_dir_all(&attachment_dir) {
            eprintln!(
                "[task-attachments] 永久删除任务附件目录失败: path={}, error={}",
                attachment_dir.display(),
                error
            );
        }
    }

    if project.project_type == PROJECT_TYPE_SSH {
        if let Some(ssh_config_id) = project.ssh_config_id.as_deref() {
            if let Err(error) =
                cleanup_remote_task_attachments_for_task(&app, ssh_config_id, &task.id).await
            {
                eprintln!("[task-attachments] 永久删除远程任务附件目录失败: {}", error);
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn restore_task<R: Runtime>(app: AppHandle<R>, id: String) -> Result<Task, String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_any_task_by_id(&pool, &id).await?;
    if task.deleted_at.is_none() {
        return Err("任务不在回收站中".to_string());
    }

    sqlx::query("UPDATE tasks SET deleted_at = NULL, updated_at = $1 WHERE id = $2")
        .bind(now_sqlite())
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("恢复任务失败: {}", error))?;

    fetch_task_by_id(&pool, &id).await
}

#[tauri::command]
pub async fn list_trashed_tasks<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Task>, String> {
    let pool = sqlite_pool(&app).await?;
    sqlx::query_as::<_, Task>(
        "SELECT * FROM tasks WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("获取回收站任务列表失败: {}", error))
}

#[tauri::command]
pub async fn create_subtask<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateSubtask,
) -> Result<Subtask, String> {
    let pool = sqlite_pool(&app).await?;
    let title = payload.title.trim().to_string();
    if title.is_empty() {
        return Err("子任务标题不能为空".to_string());
    }

    let sort_order = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM subtasks WHERE task_id = $1",
    )
    .bind(&payload.task_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to resolve subtask order: {}", error))?
    .flatten()
    .unwrap_or(1);

    let id = new_id();
    sqlx::query("INSERT INTO subtasks (id, task_id, title, sort_order) VALUES ($1, $2, $3, $4)")
        .bind(&id)
        .bind(&payload.task_id)
        .bind(title)
        .bind(sort_order)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to create subtask: {}", error))?;

    sqlx::query_as::<_, Subtask>("SELECT * FROM subtasks WHERE id = $1 LIMIT 1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Failed to fetch created subtask: {}", error))
}

#[tauri::command]
pub async fn update_subtask_status<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    status: String,
) -> Result<Subtask, String> {
    let pool = sqlite_pool(&app).await?;
    sqlx::query("UPDATE subtasks SET status = $1 WHERE id = $2")
        .bind(&status)
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to update subtask status: {}", error))?;

    sqlx::query_as::<_, Subtask>("SELECT * FROM subtasks WHERE id = $1 LIMIT 1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Failed to fetch subtask: {}", error))
}

#[tauri::command]
pub async fn delete_subtask<R: Runtime>(app: AppHandle<R>, id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    sqlx::query("DELETE FROM subtasks WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(|error| format!("Failed to delete subtask: {}", error))?;

    Ok(())
}

#[tauri::command]
pub async fn create_comment<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateComment,
) -> Result<Comment, String> {
    let pool = sqlite_pool(&app).await?;
    let content = payload.content.trim().to_string();
    if content.is_empty() {
        return Err("评论内容不能为空".to_string());
    }

    let id = new_id();
    sqlx::query(
        "INSERT INTO comments (id, task_id, employee_id, content, is_ai_generated) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(&payload.task_id)
    .bind(payload.employee_id)
    .bind(content)
    .bind(if payload.is_ai_generated.unwrap_or(false) {
        1_i64
    } else {
        0_i64
    })
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to create comment: {}", error))?;

    sqlx::query_as::<_, Comment>("SELECT * FROM comments WHERE id = $1 LIMIT 1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(|error| format!("Failed to fetch created comment: {}", error))
}
