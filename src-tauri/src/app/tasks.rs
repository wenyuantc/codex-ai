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
        "INSERT INTO tasks (id, title, description, status, priority, project_id, use_worktree, assignee_id, reviewer_id, coordinator_id, ai_suggestion, plan_content, automation_mode, time_started_at, time_spent_seconds, completed_at, due_date, blocked_reason, milestone_id, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)",
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
    .bind(&task.due_date)
    .bind(&task.blocked_reason)
    .bind(&task.milestone_id)
    .bind(&task.created_at)
    .bind(&task.updated_at)
    .execute(&mut **tx)
    .await
    .map_err(|error| format!("Failed to create task: {}", error))?;

    Ok(())
}

pub(crate) async fn ensure_milestone_belongs_to_project(
    pool: &SqlitePool,
    milestone_id: Option<&str>,
    project_id: &str,
) -> Result<(), String> {
    let Some(milestone_id) = milestone_id else {
        return Ok(());
    };

    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM milestones WHERE id = $1 AND project_id = $2",
    )
    .bind(milestone_id)
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(|error| format!("校验里程碑失败: {}", error))?;

    if exists == 0 {
        return Err("里程碑不存在或不属于当前项目".to_string());
    }

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
        pipeline_active: record.pipeline_active,
        pipeline_step_index: record.pipeline_step_index,
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
            | "pipeline_launching_step"
            | "pipeline_waiting_step"
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
    let due_date = normalize_optional_text(payload.due_date.as_deref());
    let milestone_id = normalize_optional_text(payload.milestone_id.as_deref());
    ensure_milestone_belongs_to_project(&pool, milestone_id.as_deref(), &payload.project_id)
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
        due_date,
        blocked_reason: None,
        milestone_id,
        acceptance_checklist: None,
        last_acceptance_status: None,
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

    if let Some(due_date) = task.due_date.as_deref() {
        insert_activity_log(
            &pool,
            "task_due_date_set",
            &format!("{}（截止日期：{}）", task.title, due_date),
            None,
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
    }

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
    let entering_blocked = next_status == "blocked" && current.status != "blocked";
    let leaving_blocked = current.status == "blocked" && next_status != "blocked";

    let normalized_due_date = updates.due_date.as_ref().map(|value| {
        value
            .as_deref()
            .and_then(|due_date| normalize_optional_text(Some(due_date)))
    });
    let normalized_milestone_id = updates.milestone_id.as_ref().map(|value| {
        value
            .as_deref()
            .and_then(|milestone_id| normalize_optional_text(Some(milestone_id)))
    });
    let normalized_blocked_reason = updates.blocked_reason.as_ref().map(|value| {
        value
            .as_deref()
            .and_then(|blocked_reason| normalize_optional_text(Some(blocked_reason)))
    });

    if entering_blocked {
        match &normalized_blocked_reason {
            Some(Some(value)) if !value.trim().is_empty() => {}
            _ => return Err("转为阻塞状态时必须填写阻塞原因".to_string()),
        }
    }

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
    if let Some(milestone_id) = normalized_milestone_id.as_ref() {
        ensure_milestone_belongs_to_project(&pool, milestone_id.as_deref(), &current.project_id)
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
    let effective_blocked_reason = if normalized_blocked_reason.is_some() {
        normalized_blocked_reason.clone()
    } else if leaving_blocked {
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
    if let Some(due_date) = normalized_due_date.clone() {
        separated
            .push("due_date = ")
            .push_bind_unseparated(due_date);
        touched = true;
    }
    if let Some(blocked_reason) = effective_blocked_reason.clone() {
        separated
            .push("blocked_reason = ")
            .push_bind_unseparated(blocked_reason);
        touched = true;
    }
    if let Some(milestone_id) = normalized_milestone_id.clone() {
        separated
            .push("milestone_id = ")
            .push_bind_unseparated(milestone_id);
        touched = true;
    }
    if let Some(acceptance_checklist) = updates.acceptance_checklist {
        separated.push("acceptance_checklist = ").push_bind_unseparated(
            acceptance_checklist.and_then(|value| normalize_optional_text(Some(&value))),
        );
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

    if let Some(updated_due_date) = normalized_due_date {
        if current.due_date != updated_due_date {
            insert_activity_log(
                &pool,
                "task_due_date_set",
                &format!(
                    "{}（截止日期：{}）",
                    current.title,
                    updated_due_date.as_deref().unwrap_or("已清除")
                ),
                None,
                Some(&id),
                Some(&current.project_id),
            )
            .await?;
        }
    }

    if let Some(updated_blocked_reason) = effective_blocked_reason {
        if current.blocked_reason != updated_blocked_reason {
            if let Some(reason) = updated_blocked_reason.as_deref() {
                insert_activity_log(
                    &pool,
                    "task_blocked_reason_set",
                    &format!("{}（阻塞原因：{}）", current.title, reason),
                    None,
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
            due_date: None,
            blocked_reason: None,
            milestone_id: None,
            acceptance_checklist: None,
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
pub async fn batch_update_tasks<R: Runtime>(
    app: AppHandle<R>,
    payload: crate::db::models::BatchUpdateTasksPayload,
) -> Result<Vec<Task>, String> {
    if payload.task_ids.is_empty() {
        return Err("请至少选择一个任务".to_string());
    }
    if payload.task_ids.len() > 100 {
        return Err("单次批量操作最多 100 个任务".to_string());
    }

    let mut updated = Vec::new();
    for task_id in &payload.task_ids {
        if let Some(status) = payload.status.as_ref() {
            let task = update_task_status(app.clone(), task_id.clone(), status.clone()).await?;
            updated.push(task);
            continue;
        }

        let mut update = UpdateTask {
            title: None,
            description: None,
            status: None,
            priority: payload.priority.clone(),
            assignee_id: None,
            reviewer_id: None,
            coordinator_id: None,
            complexity: None,
            ai_suggestion: None,
            plan_content: None,
            last_codex_session_id: None,
            last_review_session_id: None,
            due_date: None,
            blocked_reason: None,
            milestone_id: None,
            acceptance_checklist: None,
        };

        if payload.clear_assignee == Some(true) {
            update.assignee_id = Some(None);
        } else if let Some(assignee) = payload.assignee_id.clone() {
            update.assignee_id = Some(Some(assignee));
        }

        let task = update_task(app.clone(), task_id.clone(), update).await?;
        updated.push(task);
    }

    let pool = sqlite_pool(&app).await?;
    insert_activity_log(
        &pool,
        "tasks_batch_updated",
        &format!("批量更新 {} 个任务", updated.len()),
        None,
        None,
        updated.first().map(|task| task.project_id.as_str()),
    )
    .await?;

    Ok(updated)
}

#[tauri::command]
pub async fn export_tasks_csv<R: Runtime>(
    app: AppHandle<R>,
    payload: crate::db::models::ExportTasksCsvPayload,
) -> Result<crate::db::models::ExportTasksCsvResult, String> {
    let pool = sqlite_pool(&app).await?;
    let mut builder = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT t.id, t.title, t.status, t.priority, t.project_id, p.name AS project_name,
               t.assignee_id, t.due_date, t.created_at, t.completed_at, t.blocked_reason
        FROM tasks t
        INNER JOIN projects p ON p.id = t.project_id
        WHERE t.deleted_at IS NULL AND p.deleted_at IS NULL
        "#,
    );

    if let Some(project_id) = payload.project_id.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        builder.push(" AND t.project_id = ");
        builder.push_bind(project_id);
    }

    if let Some(mode) = payload.environment_mode.as_deref() {
        match mode {
            "ssh" => {
                builder.push(" AND p.project_type = 'ssh'");
            }
            "local" => {
                builder.push(" AND p.project_type = 'local'");
            }
            _ => {}
        }
    }

    builder.push(" ORDER BY t.updated_at DESC LIMIT 5000");

    let rows = builder
        .build_query_as::<(
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
        )>()
        .fetch_all(&pool)
        .await
        .map_err(|error| format!("导出任务失败: {error}"))?;

    let mut csv = String::from(
        "id,title,status,priority,project_id,project_name,assignee_id,due_date,created_at,completed_at,blocked_reason\n",
    );
    for row in &rows {
        let fields = [
            escape_csv(&row.0),
            escape_csv(&row.1),
            escape_csv(&row.2),
            escape_csv(&row.3),
            escape_csv(&row.4),
            escape_csv(&row.5),
            escape_csv(row.6.as_deref().unwrap_or("")),
            escape_csv(row.7.as_deref().unwrap_or("")),
            escape_csv(&row.8),
            escape_csv(row.9.as_deref().unwrap_or("")),
            escape_csv(row.10.as_deref().unwrap_or("")),
        ];
        csv.push_str(&fields.join(","));
        csv.push('\n');
    }

    Ok(crate::db::models::ExportTasksCsvResult {
        csv,
        row_count: rows.len() as i64,
    })
}

fn escape_csv(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

// ========== Task domain JSON export / import ==========

use serde::{Deserialize, Serialize};

pub(crate) const TASKS_JSON_FORMAT: &str = "codex-ai.tasks";
pub(crate) const TASKS_JSON_VERSION: i32 = 1;
pub(crate) const TASKS_JSON_DEFAULT_LIMIT: i64 = 5000;
pub(crate) const TASKS_JSON_MAX_LIMIT: i64 = 5000;

const IMPORT_ALLOWED_STATUSES: &[&str] = &[
    "todo",
    "in_progress",
    "review",
    "completed",
    "blocked",
    "archived",
];
const IMPORT_ALLOWED_PRIORITIES: &[&str] = &["low", "medium", "high", "urgent"];
const IMPORT_ALLOWED_SUBTASK_STATUSES: &[&str] =
    &["todo", "in_progress", "completed", "blocked", "review"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TasksJsonEnvelope {
    pub format: String,
    pub version: i32,
    pub exported_at: String,
    pub source: TasksJsonSource,
    pub tasks: Vec<TasksJsonTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TasksJsonSource {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub environment_mode: Option<String>,
    pub app: String,
}

/// Whitelist-only task export shape — never serialize full Task / assignee / SSH fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TasksJsonTask {
    pub source_id: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub subtasks: Vec<TasksJsonSubtask>,
    #[serde(default)]
    pub depends_on_source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TasksJsonSubtask {
    pub title: String,
    #[serde(default = "default_subtask_status")]
    pub status: String,
    #[serde(default)]
    pub sort_order: i32,
}

fn default_subtask_status() -> String {
    "todo".to_string()
}

pub(crate) fn parse_tasks_json_envelope(json: &str) -> Result<TasksJsonEnvelope, String> {
    let envelope: TasksJsonEnvelope = serde_json::from_str(json)
        .map_err(|error| format!("任务 JSON 解析失败: {error}"))?;
    if envelope.format != TASKS_JSON_FORMAT {
        return Err(format!(
            "不支持的任务 JSON 格式: {}（期望 {}）",
            envelope.format, TASKS_JSON_FORMAT
        ));
    }
    if envelope.version != TASKS_JSON_VERSION {
        return Err(format!(
            "不支持的任务 JSON 版本: {}（当前仅支持 {}）",
            envelope.version, TASKS_JSON_VERSION
        ));
    }
    Ok(envelope)
}

fn normalize_import_status(raw: &str) -> Result<String, String> {
    let status = raw.trim().to_ascii_lowercase();
    if IMPORT_ALLOWED_STATUSES.contains(&status.as_str()) {
        Ok(status)
    } else {
        Err(format!("非法任务状态: {raw}"))
    }
}

fn normalize_import_priority(raw: &str) -> Result<String, String> {
    let priority = raw.trim().to_ascii_lowercase();
    if IMPORT_ALLOWED_PRIORITIES.contains(&priority.as_str()) {
        Ok(priority)
    } else {
        Err(format!("非法任务优先级: {raw}"))
    }
}

fn normalize_import_subtask_status(raw: &str) -> Result<String, String> {
    let status = raw.trim().to_ascii_lowercase();
    if IMPORT_ALLOWED_SUBTASK_STATUSES.contains(&status.as_str()) {
        Ok(status)
    } else {
        Err(format!("非法子任务状态: {raw}"))
    }
}

/// Validate one envelope task for import. Pure — no DB access.
pub(crate) fn validate_tasks_json_task(
    task: &TasksJsonTask,
    index: usize,
) -> Result<(), String> {
    let source_id = task.source_id.trim();
    if source_id.is_empty() {
        return Err(format!("第 {} 条任务缺少 source_id", index + 1));
    }
    if task.title.trim().is_empty() {
        return Err(format!("第 {} 条任务标题不能为空", index + 1));
    }
    normalize_import_status(&task.status)
        .map_err(|e| format!("第 {} 条任务: {e}", index + 1))?;
    normalize_import_priority(&task.priority)
        .map_err(|e| format!("第 {} 条任务: {e}", index + 1))?;
    for (sub_idx, subtask) in task.subtasks.iter().enumerate() {
        if subtask.title.trim().is_empty() {
            return Err(format!(
                "第 {} 条任务的第 {} 个子任务标题不能为空",
                index + 1,
                sub_idx + 1
            ));
        }
        normalize_import_subtask_status(&subtask.status).map_err(|e| {
            format!(
                "第 {} 条任务的第 {} 个子任务: {e}",
                index + 1,
                sub_idx + 1
            )
        })?;
    }
    Ok(())
}

/// Ensure serialized envelope never contains disallowed secret/employee fields.
/// Matches JSON object keys (key + colon) so free text in titles/descriptions is not blocked.
pub(crate) fn tasks_json_payload_is_field_safe(json: &str) -> bool {
    let forbidden_keys = [
        "\"assignee_id\":",
        "\"reviewer_id\":",
        "\"coordinator_id\":",
        "\"ssh_config\":",
        "\"ssh_config_id\":",
        "\"password\":",
        "\"private_key\":",
        "\"secret\":",
        "\"attachments\":",
        "\"stored_path\":",
    ];
    !forbidden_keys.iter().any(|key| json.contains(key))
}

pub(crate) async fn export_tasks_json_with_pool(
    pool: &SqlitePool,
    payload: &crate::db::models::ExportTasksJsonPayload,
) -> Result<crate::db::models::ExportTasksJsonResult, String> {
    let environment_mode = payload
        .environment_mode
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let selected_ssh = payload
        .selected_ssh_config_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let project_id = payload
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let limit = payload
        .limit
        .filter(|v| *v > 0)
        .unwrap_or(TASKS_JSON_DEFAULT_LIMIT)
        .clamp(1, TASKS_JSON_MAX_LIMIT);

    let scoped_ids = super::database::resolve_scoped_project_ids_for_stats(
        pool,
        environment_mode,
        selected_ssh,
        project_id,
    )
    .await?;

    let mut tasks_out: Vec<TasksJsonTask> = Vec::new();
    let mut truncated = false;

    if !scoped_ids.is_empty() {
        let fetch_limit = limit + 1;
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT t.id, t.title, t.status, t.priority, t.description, t.due_date,
                   t.blocked_reason, t.completed_at
            FROM tasks t
            INNER JOIN projects p ON p.id = t.project_id
            WHERE t.deleted_at IS NULL AND p.deleted_at IS NULL AND t.project_id IN (
            "#,
        );
        {
            let mut separated = builder.separated(", ");
            for id in &scoped_ids {
                separated.push_bind(id);
            }
        }
        builder.push(") ORDER BY t.updated_at DESC LIMIT ");
        builder.push_bind(fetch_limit);

        let rows = builder
            .build_query_as::<(
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            )>()
            .fetch_all(pool)
            .await
            .map_err(|error| format!("导出任务失败: {error}"))?;

        truncated = rows.len() as i64 > limit;
        let rows: Vec<_> = rows.into_iter().take(limit as usize).collect();
        let task_ids: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
        let id_set: std::collections::HashSet<String> =
            task_ids.iter().cloned().collect();

        // tags by task
        let mut tags_by_task: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        if !task_ids.is_empty() {
            let mut tag_builder = QueryBuilder::<Sqlite>::new(
                r#"
                SELECT tt.task_id, tags.name
                FROM task_tags tt
                INNER JOIN tags ON tags.id = tt.tag_id
                WHERE tt.task_id IN (
                "#,
            );
            {
                let mut separated = tag_builder.separated(", ");
                for id in &task_ids {
                    separated.push_bind(id);
                }
            }
            tag_builder.push(") ORDER BY tags.name COLLATE NOCASE");
            let tag_rows = tag_builder
                .build_query_as::<(String, String)>()
                .fetch_all(pool)
                .await
                .map_err(|error| format!("导出任务标签失败: {error}"))?;
            for (task_id, name) in tag_rows {
                tags_by_task.entry(task_id).or_default().push(name);
            }
        }

        // subtasks by task
        let mut subtasks_by_task: std::collections::HashMap<String, Vec<TasksJsonSubtask>> =
            std::collections::HashMap::new();
        if !task_ids.is_empty() {
            let mut st_builder = QueryBuilder::<Sqlite>::new(
                r#"
                SELECT task_id, title, status, sort_order
                FROM subtasks
                WHERE task_id IN (
                "#,
            );
            {
                let mut separated = st_builder.separated(", ");
                for id in &task_ids {
                    separated.push_bind(id);
                }
            }
            st_builder.push(") ORDER BY sort_order, created_at");
            let st_rows = st_builder
                .build_query_as::<(String, String, String, i32)>()
                .fetch_all(pool)
                .await
                .map_err(|error| format!("导出子任务失败: {error}"))?;
            for (task_id, title, status, sort_order) in st_rows {
                subtasks_by_task
                    .entry(task_id)
                    .or_default()
                    .push(TasksJsonSubtask {
                        title,
                        status,
                        sort_order,
                    });
            }
        }

        // deps: only edges whose depends_on is also in export set
        let mut deps_by_task: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        if !task_ids.is_empty() {
            let mut dep_builder = QueryBuilder::<Sqlite>::new(
                r#"
                SELECT task_id, depends_on_task_id
                FROM task_dependencies
                WHERE task_id IN (
                "#,
            );
            {
                let mut separated = dep_builder.separated(", ");
                for id in &task_ids {
                    separated.push_bind(id);
                }
            }
            dep_builder.push(")");
            let dep_rows = dep_builder
                .build_query_as::<(String, String)>()
                .fetch_all(pool)
                .await
                .map_err(|error| format!("导出任务依赖失败: {error}"))?;
            for (task_id, depends_on) in dep_rows {
                if id_set.contains(&depends_on) {
                    deps_by_task.entry(task_id).or_default().push(depends_on);
                }
            }
        }

        for row in rows {
            let id = row.0;
            tasks_out.push(TasksJsonTask {
                source_id: id.clone(),
                title: row.1,
                status: row.2,
                priority: row.3,
                description: row.4,
                due_date: row.5,
                blocked_reason: row.6,
                completed_at: row.7,
                tags: tags_by_task.remove(&id).unwrap_or_default(),
                subtasks: subtasks_by_task.remove(&id).unwrap_or_default(),
                depends_on_source_ids: deps_by_task.remove(&id).unwrap_or_default(),
            });
        }
    }

    let envelope = TasksJsonEnvelope {
        format: TASKS_JSON_FORMAT.to_string(),
        version: TASKS_JSON_VERSION,
        exported_at: Utc::now().to_rfc3339(),
        source: TasksJsonSource {
            project_id: project_id.map(|s| s.to_string()),
            environment_mode: environment_mode.map(|s| s.to_string()),
            app: "codex-ai".to_string(),
        },
        tasks: tasks_out,
    };

    let json = serde_json::to_string_pretty(&envelope)
        .map_err(|error| format!("序列化任务 JSON 失败: {error}"))?;
    if !tasks_json_payload_is_field_safe(&json) {
        return Err("导出内容包含不允许的敏感字段，已中止".to_string());
    }

    let task_count = envelope.tasks.len() as i64;
    insert_activity_log(
        pool,
        "tasks_json_exported",
        &format!(
            "导出任务 JSON {} 条{}",
            task_count,
            if truncated { "（已截断）" } else { "" }
        ),
        None,
        None,
        project_id,
    )
    .await?;

    Ok(crate::db::models::ExportTasksJsonResult {
        json,
        task_count,
        truncated,
    })
}

#[tauri::command]
pub async fn export_tasks_json<R: Runtime>(
    app: AppHandle<R>,
    payload: crate::db::models::ExportTasksJsonPayload,
) -> Result<crate::db::models::ExportTasksJsonResult, String> {
    let pool = sqlite_pool(&app).await?;
    export_tasks_json_with_pool(&pool, &payload).await
}

pub(crate) async fn import_tasks_json_with_pool(
    pool: &SqlitePool,
    payload: &crate::db::models::ImportTasksJsonPayload,
) -> Result<crate::db::models::ImportTasksJsonResult, String> {
    use crate::db::models::{ImportTaskError, ImportTasksJsonResult};

    let project_id = payload.project_id.trim();
    if project_id.is_empty() {
        return Err("目标项目 ID 不能为空".to_string());
    }
    ensure_project_exists(pool, project_id).await?;

    let strategy = payload
        .conflict_strategy
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("create_new");
    if strategy != "create_new" && strategy != "skip_existing" {
        return Err(format!(
            "不支持的冲突策略: {strategy}（支持 create_new / skip_existing）"
        ));
    }

    let envelope = parse_tasks_json_envelope(&payload.json)?;
    if envelope.tasks.len() as i64 > TASKS_JSON_MAX_LIMIT {
        return Err(format!(
            "导入任务数量超过上限 {} 条",
            TASKS_JSON_MAX_LIMIT
        ));
    }

    let mut errors: Vec<ImportTaskError> = Vec::new();
    for (index, task) in envelope.tasks.iter().enumerate() {
        if let Err(message) = validate_tasks_json_task(task, index) {
            errors.push(ImportTaskError {
                index: index as i64,
                message,
            });
        }
    }
    // Detect duplicate source_ids inside the package
    {
        let mut seen = std::collections::HashSet::new();
        for (index, task) in envelope.tasks.iter().enumerate() {
            let sid = task.source_id.trim();
            if !sid.is_empty() && !seen.insert(sid.to_string()) {
                errors.push(ImportTaskError {
                    index: index as i64,
                    message: format!("第 {} 条任务 source_id 在包内重复: {sid}", index + 1),
                });
            }
        }
    }

    if !errors.is_empty() {
        return Ok(ImportTasksJsonResult {
            created: 0,
            skipped: 0,
            failed: errors.len() as i64,
            errors,
            task_ids: Vec::new(),
        });
    }

    // Pre-resolve which source_ids already exist (for skip_existing)
    let mut existing_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    if strategy == "skip_existing" && !envelope.tasks.is_empty() {
        let source_ids: Vec<String> = envelope
            .tasks
            .iter()
            .map(|t| t.source_id.trim().to_string())
            .collect();
        let mut exist_builder = QueryBuilder::<Sqlite>::new(
            "SELECT id FROM tasks WHERE deleted_at IS NULL AND id IN (",
        );
        {
            let mut separated = exist_builder.separated(", ");
            for id in &source_ids {
                separated.push_bind(id);
            }
        }
        exist_builder.push(")");
        let found = exist_builder
            .build_query_scalar::<String>()
            .fetch_all(pool)
            .await
            .map_err(|error| format!("检查已有任务失败: {error}"))?;
        existing_ids.extend(found);
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|error| format!("开始导入事务失败: {error}"))?;

    let mut source_to_new: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut created = 0_i64;
    let mut skipped = 0_i64;
    let mut created_task_ids: Vec<String> = Vec::new();
    let now = now_sqlite();

    // Cache tags by name for this project
    let mut tag_id_by_name: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    {
        let existing_tags = sqlx::query_as::<_, (String, String)>(
            "SELECT id, name FROM tags WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|error| format!("加载项目标签失败: {error}"))?;
        for (id, name) in existing_tags {
            tag_id_by_name.insert(name, id);
        }
    }

    for task in &envelope.tasks {
        let source_id = task.source_id.trim().to_string();
        if strategy == "skip_existing" && existing_ids.contains(&source_id) {
            source_to_new.insert(source_id.clone(), source_id);
            skipped += 1;
            continue;
        }

        let created_task_id = new_id();
        let status = normalize_import_status(&task.status)?;
        let priority = normalize_import_priority(&task.priority)?;
        let title = task.title.trim().to_string();
        let description = normalize_optional_text(task.description.as_deref());
        let due_date = normalize_optional_text(task.due_date.as_deref());
        let blocked_reason = normalize_optional_text(task.blocked_reason.as_deref());
        let completed_at = normalize_optional_text(task.completed_at.as_deref());

        let record = Task {
            id: created_task_id.clone(),
            title: title.clone(),
            description,
            status,
            priority,
            project_id: project_id.to_string(),
            use_worktree: false,
            assignee_id: None,
            reviewer_id: None,
            coordinator_id: None,
            complexity: None,
            ai_suggestion: None,
            plan_content: None,
            automation_mode: None,
            last_codex_session_id: None,
            last_review_session_id: None,
            time_started_at: None,
            time_spent_seconds: 0,
            completed_at,
            deleted_at: None,
            due_date,
            blocked_reason,
            milestone_id: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        insert_task_record(&mut tx, &record).await?;

        // tags
        let mut seen_tag_names = std::collections::HashSet::new();
        for tag_name_raw in &task.tags {
            let tag_name = tag_name_raw.trim();
            if tag_name.is_empty() || !seen_tag_names.insert(tag_name.to_string()) {
                continue;
            }
            let tag_id = if let Some(existing) = tag_id_by_name.get(tag_name) {
                existing.clone()
            } else {
                let created_tag_id = new_id();
                sqlx::query(
                    "INSERT INTO tags (id, project_id, name, color, created_at) VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(&created_tag_id)
                .bind(project_id)
                .bind(tag_name)
                .bind(None::<String>)
                .bind(&now)
                .execute(&mut *tx)
                .await
                .map_err(|error| format!("创建标签失败: {error}"))?;
                tag_id_by_name.insert(tag_name.to_string(), created_tag_id.clone());
                created_tag_id
            };
            sqlx::query("INSERT INTO task_tags (task_id, tag_id) VALUES ($1, $2)")
                .bind(&created_task_id)
                .bind(&tag_id)
                .execute(&mut *tx)
                .await
                .map_err(|error| format!("关联任务标签失败: {error}"))?;
        }

        // subtasks
        for subtask in &task.subtasks {
            let st_title = subtask.title.trim().to_string();
            let st_status = normalize_import_subtask_status(&subtask.status)?;
            let st_id = new_id();
            sqlx::query(
                "INSERT INTO subtasks (id, task_id, title, status, sort_order, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(&st_id)
            .bind(&created_task_id)
            .bind(&st_title)
            .bind(&st_status)
            .bind(subtask.sort_order)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|error| format!("创建子任务失败: {error}"))?;
        }

        source_to_new.insert(source_id, created_task_id.clone());
        created_task_ids.push(created_task_id);
        created += 1;
    }

    // Remap dependencies for created tasks (and skipped endpoints already in map)
    for task in &envelope.tasks {
        let source_id = task.source_id.trim();
        let Some(mapped_task_id) = source_to_new.get(source_id) else {
            continue;
        };
        // Only create deps for newly created tasks (not for skipped source rows)
        if strategy == "skip_existing" && existing_ids.contains(source_id) {
            continue;
        }
        for dep_source in &task.depends_on_source_ids {
            let dep_source = dep_source.trim();
            if dep_source.is_empty() {
                continue;
            }
            let Some(depends_on_id) = source_to_new.get(dep_source) else {
                // dangling edge — warn only, do not fail
                continue;
            };
            if depends_on_id == mapped_task_id {
                continue;
            }
            let dep_row_id = new_id();
            let result = sqlx::query(
                "INSERT OR IGNORE INTO task_dependencies (id, task_id, depends_on_task_id, created_at) VALUES ($1, $2, $3, $4)",
            )
            .bind(&dep_row_id)
            .bind(mapped_task_id)
            .bind(depends_on_id)
            .bind(&now)
            .execute(&mut *tx)
            .await;
            if let Err(error) = result {
                // Unique constraint / self-check — treat as soft warning
                eprintln!("[import_tasks_json] 跳过依赖边 {source_id} -> {dep_source}: {error}");
            }
        }
    }

    insert_activity_log(
        &mut *tx,
        "tasks_json_imported",
        &format!("导入任务 JSON：新建 {created}，跳过 {skipped}"),
        None,
        None,
        Some(project_id),
    )
    .await?;

    tx.commit()
        .await
        .map_err(|error| format!("提交导入事务失败: {error}"))?;

    Ok(ImportTasksJsonResult {
        created,
        skipped,
        failed: 0,
        errors: Vec::new(),
        task_ids: created_task_ids,
    })
}

#[tauri::command]
pub async fn import_tasks_json<R: Runtime>(
    app: AppHandle<R>,
    payload: crate::db::models::ImportTasksJsonPayload,
) -> Result<crate::db::models::ImportTasksJsonResult, String> {
    let pool = sqlite_pool(&app).await?;
    import_tasks_json_with_pool(&pool, &payload).await
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

/// Default/max LIMIT for global task lists (no project_id / project_ids).
pub(crate) const LIST_TASKS_DEFAULT_LIMIT: i64 = 500;
pub(crate) const LIST_TASKS_MAX_LIMIT: i64 = 1000;

/// Resolve LIMIT for `list_tasks`.
/// - With `project_id` or non-empty `project_ids`: no LIMIT (returns `None`).
/// - Otherwise: clamp requested limit into 1..=1000, default 500.
pub(crate) fn resolve_list_tasks_limit(
    project_id: Option<&str>,
    project_ids: Option<&[String]>,
    limit: Option<i64>,
) -> Option<i64> {
    let has_project_id = project_id.map(str::trim).is_some_and(|v| !v.is_empty());
    let has_project_ids = project_ids.is_some_and(|ids| !ids.is_empty());
    if has_project_id || has_project_ids {
        return None;
    }
    let resolved = limit
        .filter(|v| *v > 0)
        .unwrap_or(LIST_TASKS_DEFAULT_LIMIT)
        .clamp(1, LIST_TASKS_MAX_LIMIT);
    Some(resolved)
}

pub(crate) async fn list_tasks_with_pool(
    pool: &SqlitePool,
    payload: &crate::db::models::ListTasksPayload,
) -> Result<Vec<Task>, String> {
    let project_id = payload
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let status = payload
        .status
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let project_ids = payload.project_ids.as_deref();
    let limit = resolve_list_tasks_limit(project_id, project_ids, payload.limit);
    let offset = payload.offset.unwrap_or(0).max(0);

    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT * FROM tasks WHERE deleted_at IS NULL",
    );

    if let Some(pid) = project_id {
        builder.push(" AND project_id = ");
        builder.push_bind(pid);
    }

    if let Some(st) = status {
        builder.push(" AND status = ");
        builder.push_bind(st);
    }

    if let Some(ids) = project_ids {
        if !ids.is_empty() {
            builder.push(" AND project_id IN (");
            let mut separated = builder.separated(", ");
            for id in ids {
                separated.push_bind(id);
            }
            separated.push_unseparated(")");
        }
    }

    builder.push(" ORDER BY updated_at DESC, id DESC");

    if let Some(lim) = limit {
        builder.push(" LIMIT ");
        builder.push_bind(lim);
        builder.push(" OFFSET ");
        builder.push_bind(offset);
    }

    builder
        .build_query_as::<Task>()
        .fetch_all(pool)
        .await
        .map_err(|error| format!("获取任务列表失败: {}", error))
}

#[tauri::command]
pub async fn list_tasks<R: Runtime>(
    app: AppHandle<R>,
    payload: Option<crate::db::models::ListTasksPayload>,
) -> Result<Vec<Task>, String> {
    let pool = sqlite_pool(&app).await?;
    let payload = payload.unwrap_or_default();
    list_tasks_with_pool(&pool, &payload).await
}

#[tauri::command]
pub async fn list_task_attachments<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<Vec<TaskAttachment>, String> {
    let pool = sqlite_pool(&app).await?;
    fetch_task_attachments(&pool, &task_id).await
}

#[tauri::command]
pub async fn list_task_subtasks<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<Vec<Subtask>, String> {
    let pool = sqlite_pool(&app).await?;
    fetch_task_subtasks(&pool, &task_id).await
}

pub(crate) async fn fetch_task_comments(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Vec<Comment>, String> {
    sqlx::query_as::<_, Comment>(
        "SELECT * FROM comments WHERE task_id = $1 ORDER BY created_at",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("获取任务评论失败: {}", error))
}

#[tauri::command]
pub async fn list_task_comments<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<Vec<Comment>, String> {
    let pool = sqlite_pool(&app).await?;
    fetch_task_comments(&pool, &task_id).await
}

#[cfg(test)]
mod list_tasks_limit_tests {
    use super::{
        resolve_list_tasks_limit, LIST_TASKS_DEFAULT_LIMIT, LIST_TASKS_MAX_LIMIT,
    };

    #[test]
    fn project_id_disables_limit() {
        assert_eq!(
            resolve_list_tasks_limit(Some("p1"), None, Some(10)),
            None
        );
    }

    #[test]
    fn project_ids_disables_limit() {
        let ids = vec!["p1".to_string(), "p2".to_string()];
        assert_eq!(
            resolve_list_tasks_limit(None, Some(&ids), Some(10)),
            None
        );
    }

    #[test]
    fn global_list_uses_default_limit() {
        assert_eq!(
            resolve_list_tasks_limit(None, None, None),
            Some(LIST_TASKS_DEFAULT_LIMIT)
        );
    }

    #[test]
    fn global_list_clamps_limit() {
        assert_eq!(
            resolve_list_tasks_limit(None, None, Some(0)),
            Some(LIST_TASKS_DEFAULT_LIMIT)
        );
        assert_eq!(
            resolve_list_tasks_limit(None, None, Some(-5)),
            Some(LIST_TASKS_DEFAULT_LIMIT)
        );
        assert_eq!(
            resolve_list_tasks_limit(None, None, Some(50)),
            Some(50)
        );
        assert_eq!(
            resolve_list_tasks_limit(None, None, Some(5000)),
            Some(LIST_TASKS_MAX_LIMIT)
        );
    }
}

#[cfg(test)]
mod tasks_json_tests {
    use super::{
        parse_tasks_json_envelope, tasks_json_payload_is_field_safe, validate_tasks_json_task,
        TasksJsonEnvelope, TasksJsonSource, TasksJsonSubtask, TasksJsonTask, TASKS_JSON_FORMAT,
        TASKS_JSON_VERSION,
    };

    fn sample_envelope() -> TasksJsonEnvelope {
        TasksJsonEnvelope {
            format: TASKS_JSON_FORMAT.to_string(),
            version: TASKS_JSON_VERSION,
            exported_at: "2026-08-05T12:00:00Z".to_string(),
            source: TasksJsonSource {
                project_id: Some("proj-1".to_string()),
                environment_mode: Some("local".to_string()),
                app: "codex-ai".to_string(),
            },
            tasks: vec![
                TasksJsonTask {
                    source_id: "task-a".to_string(),
                    title: "父任务".to_string(),
                    status: "todo".to_string(),
                    priority: "high".to_string(),
                    description: Some("desc".to_string()),
                    due_date: None,
                    blocked_reason: None,
                    completed_at: None,
                    tags: vec!["bug".to_string()],
                    subtasks: vec![TasksJsonSubtask {
                        title: "子任务".to_string(),
                        status: "todo".to_string(),
                        sort_order: 0,
                    }],
                    depends_on_source_ids: vec![],
                },
                TasksJsonTask {
                    source_id: "task-b".to_string(),
                    title: "依赖任务".to_string(),
                    status: "in_progress".to_string(),
                    priority: "medium".to_string(),
                    description: None,
                    due_date: None,
                    blocked_reason: None,
                    completed_at: None,
                    tags: vec![],
                    subtasks: vec![],
                    depends_on_source_ids: vec!["task-a".to_string()],
                },
            ],
        }
    }

    #[test]
    fn export_envelope_is_field_safe_without_assignee() {
        let json = serde_json::to_string_pretty(&sample_envelope()).expect("serialize");
        assert!(tasks_json_payload_is_field_safe(&json));
        assert!(!json.contains("assignee"));
        assert!(!json.contains("ssh_config"));
        assert!(!json.contains("password"));
        assert!(json.contains("source_id"));
        assert!(json.contains("depends_on_source_ids"));
        assert!(json.contains("subtasks"));
        assert!(json.contains("tags"));
    }

    #[test]
    fn parse_rejects_invalid_format_and_version() {
        let bad_format = r#"{"format":"other","version":1,"exported_at":"t","source":{"app":"x"},"tasks":[]}"#;
        let err = parse_tasks_json_envelope(bad_format).expect_err("format");
        assert!(err.contains("不支持的任务 JSON 格式"));

        let bad_version = r#"{"format":"codex-ai.tasks","version":99,"exported_at":"t","source":{"app":"x"},"tasks":[]}"#;
        let err = parse_tasks_json_envelope(bad_version).expect_err("version");
        assert!(err.contains("不支持的任务 JSON 版本"));
    }

    #[test]
    fn validate_task_rejects_unknown_status() {
        let mut task = sample_envelope().tasks.remove(0);
        task.status = "flying".to_string();
        let err = validate_tasks_json_task(&task, 0).expect_err("status");
        assert!(err.contains("非法任务状态"));
    }

    #[test]
    fn validate_task_accepts_valid_item() {
        let task = &sample_envelope().tasks[0];
        validate_tasks_json_task(task, 0).expect("valid");
    }
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
