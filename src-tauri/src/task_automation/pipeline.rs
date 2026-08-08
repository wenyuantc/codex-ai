// Domain slice: coordinator pipeline (included into task_automation)

const PHASE_PIPELINE_LAUNCHING_STEP: &str = "pipeline_launching_step";
const PHASE_PIPELINE_WAITING_STEP: &str = "pipeline_waiting_step";
const PHASE_PIPELINE_STEP_FAILED: &str = "pipeline_step_failed";

const STEP_STATUS_PENDING: &str = "pending";
const STEP_STATUS_LAUNCHING: &str = "launching";
const STEP_STATUS_RUNNING: &str = "running";
const STEP_STATUS_SUCCEEDED: &str = "succeeded";
const STEP_STATUS_FAILED: &str = "failed";
const STEP_STATUS_CANCELLED: &str = "cancelled";

fn is_pipeline_phase(phase: &str) -> bool {
    matches!(
        phase,
        PHASE_PIPELINE_LAUNCHING_STEP | PHASE_PIPELINE_WAITING_STEP | PHASE_PIPELINE_STEP_FAILED
    )
}

fn state_pipeline_active(state: Option<&TaskAutomationStateRecord>) -> bool {
    state.is_some_and(|item| item.pipeline_active)
}

async fn fetch_task_pipeline_steps(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Vec<TaskPipelineStep>, String> {
    sqlx::query_as::<_, TaskPipelineStep>(
        r#"
        SELECT *
        FROM task_pipeline_steps
        WHERE task_id = $1
        ORDER BY step_index ASC, created_at ASC
        "#,
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to list task pipeline steps: {}", error))
}

async fn fetch_pipeline_step_by_index(
    pool: &SqlitePool,
    task_id: &str,
    step_index: i32,
) -> Result<TaskPipelineStep, String> {
    sqlx::query_as::<_, TaskPipelineStep>(
        r#"
        SELECT *
        FROM task_pipeline_steps
        WHERE task_id = $1 AND step_index = $2
        LIMIT 1
        "#,
    )
    .bind(task_id)
    .bind(step_index)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to fetch pipeline step: {}", error))?
    .ok_or_else(|| format!("任务编排步骤 {} 不存在", step_index + 1))
}

async fn resolve_pipeline_step_employee(
    pool: &SqlitePool,
    task: &Task,
    step: &TaskPipelineStep,
) -> Result<crate::db::models::Employee, String> {
    if let Some(employee_id) = step.employee_id.as_deref().map(str::trim).filter(|v| !v.is_empty())
    {
        let employee = fetch_employee_by_id(pool, employee_id).await?;
        if employee.project_id.as_deref() != Some(task.project_id.as_str())
            && employee.project_id.is_some()
        {
            return Err(format!(
                "步骤「{}」指派的员工 {} 不属于当前项目",
                step.title, employee.name
            ));
        }
        return Ok(employee);
    }

    let assignee_id = task
        .assignee_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "步骤「{}」未指定执行员工，且任务未设置负责人，无法启动编排",
                step.title
            )
        })?;
    fetch_employee_by_id(pool, assignee_id).await
}

fn build_pipeline_step_prompt(
    task: &Task,
    step: &TaskPipelineStep,
    previous_handoff: Option<&str>,
    total_steps: usize,
) -> String {
    let mut sections = Vec::new();
    sections.push(format!("任务标题：{}", task.title.trim()));
    if let Some(description) = task.description.as_deref().map(str::trim).filter(|v| !v.is_empty())
    {
        sections.push(format!("任务描述：\n{}", description));
    }
    sections.push(format!(
        "编排进度：第 {} / {} 步",
        step.step_index + 1,
        total_steps
    ));
    sections.push(format!("本步标题：{}", step.title.trim()));
    if let Some(goal) = step.goal.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        sections.push(format!("本步目标：\n{}", goal));
    }
    if let Some(criteria) = step
        .success_criteria
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        sections.push(format!("成功标准：\n{}", criteria));
    }
    if let Some(handoff) = previous_handoff.map(str::trim).filter(|v| !v.is_empty()) {
        sections.push(format!("上一步交接摘要：\n{}", handoff));
    }
    sections.push(
        "执行要求：\n- 仅完成本步目标，不要提前做后续步骤\n- 结束后用简短中文总结本步完成内容与风险（便于下一步交接）"
            .to_string(),
    );
    sections.join("\n\n")
}

async fn previous_step_handoff(
    pool: &SqlitePool,
    task_id: &str,
    step_index: i32,
) -> Result<Option<String>, String> {
    if step_index <= 0 {
        return Ok(None);
    }
    let prev = fetch_pipeline_step_by_index(pool, task_id, step_index - 1).await?;
    Ok(prev
        .handoff_summary
        .filter(|value| !value.trim().is_empty())
        .or(Some(format!("步骤 {}「{}」已完成", step_index, prev.title))))
}

async fn ensure_pipeline_automation_row(pool: &SqlitePool, task_id: &str) -> Result<(), String> {
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
            updated_at,
            pipeline_active,
            pipeline_step_index
        ) VALUES ($1, 'idle', 0, NULL, NULL, NULL, NULL, NULL, NULL, $2, 0, NULL)
        ON CONFLICT(task_id) DO NOTHING
        "#,
    )
    .bind(task_id)
    .bind(now_sqlite())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to ensure automation state for pipeline: {}", error))?;
    Ok(())
}

async fn set_pipeline_state(
    pool: &SqlitePool,
    task_id: &str,
    phase: &str,
    pipeline_active: bool,
    pipeline_step_index: Option<i32>,
    last_trigger_session_id: Option<&str>,
    consumed_session_id: Option<&str>,
    last_error: Option<&str>,
) -> Result<(), String> {
    ensure_pipeline_automation_row(pool, task_id).await?;
    sqlx::query(
        r#"
        UPDATE task_automation_state
        SET phase = $2,
            pipeline_active = $3,
            pipeline_step_index = $4,
            last_trigger_session_id = COALESCE($5, last_trigger_session_id),
            consumed_session_id = COALESCE($6, consumed_session_id),
            last_error = $7,
            pending_action = NULL,
            pending_round_count = NULL,
            updated_at = $8
        WHERE task_id = $1
        "#,
    )
    .bind(task_id)
    .bind(phase)
    .bind(pipeline_active)
    .bind(pipeline_step_index)
    .bind(last_trigger_session_id)
    .bind(consumed_session_id)
    .bind(last_error)
    .bind(now_sqlite())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to update pipeline automation state: {}", error))?;
    Ok(())
}

async fn update_pipeline_step_status(
    pool: &SqlitePool,
    step_id: &str,
    status: &str,
    session_id: Option<&str>,
    handoff_summary: Option<&str>,
    last_error: Option<&str>,
    set_started: bool,
    set_ended: bool,
) -> Result<(), String> {
    let now = now_sqlite();
    // When re-launching a step (set_started), clear ended_at so failure/success timing is fresh.
    sqlx::query(
        r#"
        UPDATE task_pipeline_steps
        SET status = $2,
            session_id = COALESCE($3, session_id),
            handoff_summary = COALESCE($4, handoff_summary),
            last_error = $5,
            started_at = CASE WHEN $6 THEN $7 ELSE started_at END,
            ended_at = CASE
                WHEN $8 THEN $7
                WHEN $6 THEN NULL
                ELSE ended_at
            END,
            updated_at = $7
        WHERE id = $1
        "#,
    )
    .bind(step_id)
    .bind(status)
    .bind(session_id)
    .bind(handoff_summary)
    .bind(last_error)
    .bind(set_started)
    .bind(&now)
    .bind(set_ended)
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to update pipeline step: {}", error))?;
    Ok(())
}

async fn latest_execution_session_id(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Option<String>, String> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM codex_sessions
        WHERE task_id = $1
          AND session_kind = 'execution'
        ORDER BY started_at DESC, created_at DESC
        LIMIT 1
        "#,
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("Failed to resolve latest execution session: {}", error))
}

async fn resolve_pipeline_working_dir(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    project: &Project,
) -> Result<AutomationExecutionContext, String> {
    if task.use_worktree {
        let prepared =
            crate::git_workflow::prepare_task_git_execution(app.clone(), task.id.clone(), None)
                .await?;
        return Ok(AutomationExecutionContext {
            working_dir: prepared.working_dir,
            task_git_context_id: Some(prepared.task_git_context_id),
        });
    }

    if let Ok(context) = resolve_automation_execution_context(pool, task, project).await {
        return Ok(context);
    }

    let working_dir = if project.project_type == "ssh" {
        project
            .remote_repo_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    } else {
        project
            .repo_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }
    .ok_or_else(|| "当前项目缺少可用工作目录，无法启动编排".to_string())?;

    Ok(AutomationExecutionContext {
        working_dir,
        task_git_context_id: None,
    })
}

async fn start_employee_execution_session(
    app: &AppHandle,
    employee: &crate::db::models::Employee,
    task: &Task,
    prompt: String,
    execution_context: &AutomationExecutionContext,
) -> Result<(), String> {
    let image_paths: Option<Vec<String>> = None;
    if employee.ai_provider == "claude" {
        let manager = app
            .state::<Arc<tokio::sync::Mutex<ClaudeManager>>>()
            .inner()
            .clone();
        start_claude_with_manager(
            app.clone(),
            manager,
            employee.id.clone(),
            prompt,
            Some(employee.model.clone()),
            Some(employee.reasoning_effort.clone()),
            employee.system_prompt.clone(),
            Some(execution_context.working_dir.clone()),
            Some(task.id.clone()),
            execution_context.task_git_context_id.clone(),
            None,
            image_paths,
            Some("execution".to_string()),
        )
        .await
    } else if employee.ai_provider == "opencode" {
        let manager = app
            .state::<Arc<tokio::sync::Mutex<OpenCodeManager>>>()
            .inner()
            .clone();
        start_opencode_with_manager(
            app.clone(),
            manager,
            employee.id.clone(),
            prompt,
            Some(employee.model.clone()),
            Some(execution_context.working_dir.clone()),
            Some(task.id.clone()),
            execution_context.task_git_context_id.clone(),
            None,
            image_paths,
        )
        .await
    } else if employee.ai_provider == "grok" {
        let manager = app
            .state::<Arc<tokio::sync::Mutex<GrokManager>>>()
            .inner()
            .clone();
        start_grok_with_manager(
            app.clone(),
            manager,
            employee.id.clone(),
            prompt,
            Some(employee.model.clone()),
            Some(employee.reasoning_effort.clone()),
            employee.system_prompt.clone(),
            Some(execution_context.working_dir.clone()),
            Some(task.id.clone()),
            execution_context.task_git_context_id.clone(),
            None,
            image_paths,
            Some("execution".to_string()),
        )
        .await
    } else {
        let manager = app.state::<Arc<Mutex<CodexManager>>>().inner().clone();
        start_codex_with_manager(
            app.clone(),
            manager,
            employee.id.clone(),
            prompt,
            Some(employee.model.clone()),
            Some(employee.reasoning_effort.clone()),
            employee.system_prompt.clone(),
            Some(execution_context.working_dir.clone()),
            Some(task.id.clone()),
            execution_context.task_git_context_id.clone(),
            None,
            image_paths,
            Some("execution".to_string()),
        )
        .await
    }
}

async fn launch_pipeline_step(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    step_index: i32,
) -> Result<(), String> {
    let steps = fetch_task_pipeline_steps(pool, &task.id).await?;
    if steps.is_empty() {
        return Err("当前任务没有可编排的工作包，请先生成协调员结构化计划".to_string());
    }
    let step = steps
        .iter()
        .find(|item| item.step_index == step_index)
        .cloned()
        .ok_or_else(|| format!("编排步骤 {} 不存在", step_index + 1))?;

    let employee = resolve_pipeline_step_employee(pool, task, &step).await?;
    let project = fetch_project_by_id(pool, &task.project_id).await?;
    let previous = previous_step_handoff(pool, &task.id, step_index).await?;
    let prompt = build_pipeline_step_prompt(task, &step, previous.as_deref(), steps.len());

    set_pipeline_state(
        pool,
        &task.id,
        PHASE_PIPELINE_LAUNCHING_STEP,
        true,
        Some(step_index),
        None,
        None,
        None,
    )
    .await?;
    update_pipeline_step_status(
        pool,
        &step.id,
        STEP_STATUS_LAUNCHING,
        None,
        None,
        None,
        true,
        false,
    )
    .await?;

    update_task_status_internal(app, pool, task, "in_progress").await?;
    // Align with single-run execution: start task timer when pipeline work begins.
    let _ = start_task_timer_internal(pool, &task.id).await?;
    sqlx::query("UPDATE employees SET status = 'busy' WHERE id = $1")
        .bind(&employee.id)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to mark employee busy: {}", error))?;

    let execution_context = match resolve_pipeline_working_dir(app, pool, task, &project).await {
        Ok(context) => context,
        Err(error) => {
            update_pipeline_step_status(
                pool,
                &step.id,
                STEP_STATUS_FAILED,
                None,
                None,
                Some(&error),
                false,
                true,
            )
            .await?;
            set_pipeline_state(
                pool,
                &task.id,
                PHASE_PIPELINE_STEP_FAILED,
                true,
                Some(step_index),
                None,
                None,
                Some(&error),
            )
            .await?;
            emit_task_automation_state_changed(app, task, PHASE_PIPELINE_STEP_FAILED);
            return Err(error);
        }
    };

    if let Err(error) =
        start_employee_execution_session(app, &employee, task, prompt, &execution_context).await
    {
        update_pipeline_step_status(
            pool,
            &step.id,
            STEP_STATUS_FAILED,
            None,
            None,
            Some(&error),
            false,
            true,
        )
        .await?;
        set_pipeline_state(
            pool,
            &task.id,
            PHASE_PIPELINE_STEP_FAILED,
            true,
            Some(step_index),
            None,
            None,
            Some(&error),
        )
        .await?;
        emit_task_automation_state_changed(app, task, PHASE_PIPELINE_STEP_FAILED);
        return Err(error);
    }

    let session_id = latest_execution_session_id(pool, &task.id).await?;
    update_pipeline_step_status(
        pool,
        &step.id,
        STEP_STATUS_RUNNING,
        session_id.as_deref(),
        None,
        None,
        false,
        false,
    )
    .await?;
    set_pipeline_state(
        pool,
        &task.id,
        PHASE_PIPELINE_WAITING_STEP,
        true,
        Some(step_index),
        session_id.as_deref(),
        None,
        None,
    )
    .await?;

    insert_activity_log(
        pool,
        "task_pipeline_step_started",
        &format!(
            "{}（步骤 {}：{}；员工：{}）",
            task.title,
            step_index + 1,
            step.title,
            employee.name
        ),
        Some(employee.id.as_str()),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    emit_task_automation_state_changed(app, task, PHASE_PIPELINE_WAITING_STEP);
    Ok(())
}

async fn handle_pipeline_execution_exit(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    state_record: &TaskAutomationStateRecord,
    facts: &SessionExitFacts,
) -> Result<(), String> {
    let step_index = state_record.pipeline_step_index.unwrap_or(0);
    let step = fetch_pipeline_step_by_index(pool, &task.id, step_index).await?;
    let steps = fetch_task_pipeline_steps(pool, &task.id).await?;
    let is_last = steps
        .last()
        .map(|item| item.step_index == step_index)
        .unwrap_or(true);

    if facts.has_stopping_requested {
        update_pipeline_step_status(
            pool,
            &step.id,
            STEP_STATUS_CANCELLED,
            Some(&facts.session_id),
            None,
            Some("步骤已被人工停止"),
            false,
            true,
        )
        .await?;
        set_pipeline_state(
            pool,
            &task.id,
            PHASE_MANUAL_CONTROL,
            false,
            Some(step_index),
            Some(&facts.session_id),
            Some(&facts.session_id),
            Some("编排已被人工停止"),
        )
        .await?;
        stop_task_timer_internal(pool, &task.id, "编排人工停止").await?;
        insert_activity_log(
            pool,
            "task_pipeline_aborted",
            &format!("{}（步骤 {} 被停止）", task.title, step_index + 1),
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
        emit_task_automation_state_changed(app, task, PHASE_MANUAL_CONTROL);
        return Ok(());
    }

    if facts.status != "exited" || facts.exit_code != Some(0) {
        let message = format!(
            "编排步骤 {}「{}」执行失败（status={} exit={:?}）",
            step_index + 1,
            step.title,
            facts.status,
            facts.exit_code
        );
        update_pipeline_step_status(
            pool,
            &step.id,
            STEP_STATUS_FAILED,
            Some(&facts.session_id),
            None,
            Some(&message),
            false,
            true,
        )
        .await?;
        set_pipeline_state(
            pool,
            &task.id,
            PHASE_PIPELINE_STEP_FAILED,
            true,
            Some(step_index),
            Some(&facts.session_id),
            Some(&facts.session_id),
            Some(&message),
        )
        .await?;
        insert_activity_log(
            pool,
            "task_pipeline_step_failed",
            &format!("{}（{}）", task.title, message),
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
        emit_task_automation_state_changed(app, task, PHASE_PIPELINE_STEP_FAILED);
        return Ok(());
    }

    let handoff = format!("步骤 {}「{}」已成功完成。", step_index + 1, step.title);
    update_pipeline_step_status(
        pool,
        &step.id,
        STEP_STATUS_SUCCEEDED,
        Some(&facts.session_id),
        Some(&handoff),
        None,
        false,
        true,
    )
    .await?;
    insert_activity_log(
        pool,
        "task_pipeline_step_completed",
        &format!(
            "{}（步骤 {}：{}）",
            task.title,
            step_index + 1,
            step.title
        ),
        facts.employee_id.as_deref(),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;

    if !is_last {
        let next_index = step_index + 1;
        // Consume current session before launching next so resume won't re-handle.
        set_pipeline_state(
            pool,
            &task.id,
            PHASE_PIPELINE_LAUNCHING_STEP,
            true,
            Some(next_index),
            Some(&facts.session_id),
            Some(&facts.session_id),
            None,
        )
        .await?;
        return launch_pipeline_step(app, pool, task, next_index).await;
    }

    // Pipeline finished successfully.
    set_pipeline_state(
        pool,
        &task.id,
        PHASE_IDLE,
        false,
        Some(step_index),
        Some(&facts.session_id),
        Some(&facts.session_id),
        None,
    )
    .await?;
    insert_activity_log(
        pool,
        "task_pipeline_completed",
        &format!("{}（全部编排步骤已完成）", task.title),
        facts.employee_id.as_deref(),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    emit_task_automation_state_changed(app, task, PHASE_IDLE);

    if task_automation_enabled(task) {
        let refreshed = fetch_task_automation_state_record(pool, &task.id).await?;
        reserve_pending_action(
            pool,
            &task.id,
            Some(&facts.session_id),
            PHASE_LAUNCHING_REVIEW,
            Some(PENDING_ACTION_START_REVIEW),
            None,
            0,
            None,
            refreshed
                .as_ref()
                .and_then(|item| item.last_verdict_json.as_deref()),
        )
        .await?;
        // Keep pipeline_active false after reserve (reserve doesn't clear it if not in SQL).
        sqlx::query(
            r#"
            UPDATE task_automation_state
            SET pipeline_active = 0
            WHERE task_id = $1
            "#,
        )
        .bind(&task.id)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to clear pipeline_active after pipeline: {}", error))?;

        let reserved_state = fetch_task_automation_state_record(pool, &task.id)
            .await?
            .ok_or_else(|| "自动质控状态写入后丢失，无法发起审核".to_string())?;
        let review_launched = retry_pending_review(app, pool, &task.id, &reserved_state).await?;
        if review_launched {
            insert_activity_log(
                pool,
                "task_automation_review_started",
                "编排完成，已自动发起代码审核",
                facts.employee_id.as_deref(),
                Some(task.id.as_str()),
                Some(task.project_id.as_str()),
            )
            .await?;
        }
        return Ok(());
    }

    // No automation: leave task in_progress / idle automation (align with manual completion path).
    stop_task_timer_internal(pool, &task.id, "编排完成").await?;
    Ok(())
}

#[tauri::command]
pub async fn list_task_pipeline_steps(
    app: AppHandle,
    task_id: String,
) -> Result<Vec<TaskPipelineStep>, String> {
    let pool = sqlite_pool(&app).await?;
    let _task = fetch_task_by_id(&pool, &task_id).await?;
    fetch_task_pipeline_steps(&pool, &task_id).await
}

#[tauri::command]
pub async fn update_task_pipeline_step(
    app: AppHandle,
    payload: UpdateTaskPipelineStepPayload,
) -> Result<TaskPipelineStep, String> {
    let pool = sqlite_pool(&app).await?;
    let step = sqlx::query_as::<_, TaskPipelineStep>(
        "SELECT * FROM task_pipeline_steps WHERE id = $1 LIMIT 1",
    )
    .bind(&payload.step_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to fetch pipeline step: {}", error))?
    .ok_or_else(|| "编排步骤不存在".to_string())?;

    let task = fetch_task_by_id(&pool, &step.task_id).await?;
    let state = fetch_task_automation_state_record(&pool, &task.id).await?;
    if state_pipeline_active(state.as_ref())
        && matches!(
            state.as_ref().map(|item| item.phase.as_str()),
            Some(PHASE_PIPELINE_LAUNCHING_STEP | PHASE_PIPELINE_WAITING_STEP)
        )
    {
        return Err("编排运行中，不能修改步骤".to_string());
    }

    let mut employee_id = step.employee_id.clone();
    if let Some(update) = payload.employee_id {
        employee_id = update
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if let Some(id) = employee_id.as_deref() {
            let employee = fetch_employee_by_id(&pool, id).await?;
            if employee.project_id.as_deref() != Some(task.project_id.as_str())
                && employee.project_id.is_some()
            {
                return Err(format!("员工 {} 不属于当前项目", employee.name));
            }
        }
    }

    let title = payload
        .title
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(step.title.clone());
    let goal = match payload.goal {
        Some(value) => value
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty()),
        None => step.goal.clone(),
    };
    let success_criteria = match payload.success_criteria {
        Some(value) => value
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty()),
        None => step.success_criteria.clone(),
    };

    sqlx::query(
        r#"
        UPDATE task_pipeline_steps
        SET employee_id = $2,
            title = $3,
            goal = $4,
            success_criteria = $5,
            updated_at = $6
        WHERE id = $1
        "#,
    )
    .bind(&payload.step_id)
    .bind(&employee_id)
    .bind(&title)
    .bind(&goal)
    .bind(&success_criteria)
    .bind(now_sqlite())
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to update pipeline step: {}", error))?;

    sqlx::query_as::<_, TaskPipelineStep>(
        "SELECT * FROM task_pipeline_steps WHERE id = $1 LIMIT 1",
    )
    .bind(&payload.step_id)
    .fetch_one(&pool)
    .await
    .map_err(|error| format!("Failed to reload pipeline step: {}", error))
}

#[tauri::command]
pub async fn start_task_pipeline(
    app: AppHandle,
    payload: StartTaskPipelinePayload,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &payload.task_id).await?;
    if task.status == TASK_STATUS_ARCHIVED {
        return Err("已归档任务不能启动编排".to_string());
    }

    let steps = fetch_task_pipeline_steps(&pool, &task.id).await?;
    if steps.is_empty() {
        return Err("当前任务没有编排步骤，请先生成协调员结构化计划".to_string());
    }

    let state = fetch_task_automation_state_record(&pool, &task.id).await?;
    if state_pipeline_active(state.as_ref())
        && matches!(
            state.as_ref().map(|item| item.phase.as_str()),
            Some(PHASE_PIPELINE_LAUNCHING_STEP | PHASE_PIPELINE_WAITING_STEP)
        )
    {
        return Err("编排已在运行中".to_string());
    }

    // Reset non-terminal steps to pending for a fresh run.
    sqlx::query(
        r#"
        UPDATE task_pipeline_steps
        SET status = $2,
            session_id = NULL,
            handoff_summary = NULL,
            last_error = NULL,
            started_at = NULL,
            ended_at = NULL,
            updated_at = $3
        WHERE task_id = $1
        "#,
    )
    .bind(&task.id)
    .bind(STEP_STATUS_PENDING)
    .bind(now_sqlite())
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to reset pipeline steps: {}", error))?;

    for step in &steps {
        let _ = resolve_pipeline_step_employee(&pool, &task, step).await?;
    }

    insert_activity_log(
        &pool,
        "task_pipeline_started",
        &format!("{}（共 {} 步）", task.title, steps.len()),
        task.assignee_id.as_deref(),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;

    let first_index = steps[0].step_index;
    launch_pipeline_step(&app, &pool, &task, first_index).await
}

fn resolve_failed_pipeline_step_index(
    state: &TaskAutomationStateRecord,
    steps: &[TaskPipelineStep],
) -> Option<i32> {
    if state.phase == PHASE_PIPELINE_STEP_FAILED {
        if let Some(index) = state.pipeline_step_index {
            if steps.iter().any(|step| {
                step.step_index == index && step.status == STEP_STATUS_FAILED
            }) {
                return Some(index);
            }
        }
    }

    steps
        .iter()
        .find(|step| step.status == STEP_STATUS_FAILED)
        .map(|step| step.step_index)
}

fn is_pipeline_mid_flight(state: Option<&TaskAutomationStateRecord>) -> bool {
    state.is_some_and(|item| {
        item.pipeline_active
            && matches!(
                item.phase.as_str(),
                PHASE_PIPELINE_LAUNCHING_STEP | PHASE_PIPELINE_WAITING_STEP
            )
    })
}

fn is_manual_runnable_step_status(status: &str) -> bool {
    matches!(
        status,
        STEP_STATUS_PENDING | STEP_STATUS_FAILED | STEP_STATUS_CANCELLED
    )
}

#[tauri::command]
pub async fn retry_task_pipeline_step(
    app: AppHandle,
    payload: RetryTaskPipelineStepPayload,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &payload.task_id).await?;
    if task.status == TASK_STATUS_ARCHIVED {
        return Err("已归档任务不能重试编排步骤".to_string());
    }

    let state = fetch_task_automation_state_record(&pool, &task.id)
        .await?
        .ok_or_else(|| "编排状态不存在，无法重试".to_string())?;
    if is_pipeline_mid_flight(Some(&state)) {
        return Err("编排仍在执行中，请等待当前步骤结束后再重试".to_string());
    }

    let steps = fetch_task_pipeline_steps(&pool, &task.id).await?;
    let step_index = resolve_failed_pipeline_step_index(&state, &steps)
        .ok_or_else(|| "当前没有失败的编排步骤可重试".to_string())?;
    launch_pipeline_step(&app, &pool, &task, step_index).await
}

/// Manually run a single incomplete pipeline step (after 转人工 / idle recovery).
#[tauri::command]
pub async fn run_task_pipeline_step_manual(
    app: AppHandle,
    payload: RunTaskPipelineStepManualPayload,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &payload.task_id).await?;
    if task.status == TASK_STATUS_ARCHIVED {
        return Err("已归档任务不能手动运行编排步骤".to_string());
    }

    let state = fetch_task_automation_state_record(&pool, &task.id).await?;
    if is_pipeline_mid_flight(state.as_ref()) {
        return Err("编排仍在执行中，不能手动启动其他步骤".to_string());
    }

    let step = sqlx::query_as::<_, TaskPipelineStep>(
        "SELECT * FROM task_pipeline_steps WHERE id = $1 LIMIT 1",
    )
    .bind(&payload.step_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("Failed to fetch pipeline step: {}", error))?
    .ok_or_else(|| "编排步骤不存在".to_string())?;

    if step.task_id != task.id {
        return Err("编排步骤不属于当前任务".to_string());
    }
    if !is_manual_runnable_step_status(&step.status) {
        return Err(format!(
            "步骤「{}」当前状态为「{}」，不能手动运行",
            step.title, step.status
        ));
    }

    insert_activity_log(
        &pool,
        "task_pipeline_step_manual_run",
        &format!(
            "{}（手动运行步骤 {}：{}）",
            task.title,
            step.step_index + 1,
            step.title
        ),
        task.assignee_id.as_deref(),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;

    launch_pipeline_step(&app, &pool, &task, step.step_index).await
}

#[tauri::command]
pub async fn abort_task_pipeline(
    app: AppHandle,
    payload: AbortTaskPipelinePayload,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &payload.task_id).await?;
    let state = fetch_task_automation_state_record(&pool, &task.id).await?;
    let steps = fetch_task_pipeline_steps(&pool, &task.id).await?;
    let has_incomplete_steps = steps.iter().any(|step| {
        matches!(
            step.status.as_str(),
            STEP_STATUS_PENDING
                | STEP_STATUS_LAUNCHING
                | STEP_STATUS_RUNNING
                | STEP_STATUS_FAILED
                | STEP_STATUS_CANCELLED
        )
    });
    let can_abort = state_pipeline_active(state.as_ref())
        || state
            .as_ref()
            .is_some_and(|item| is_pipeline_phase(&item.phase))
        || has_incomplete_steps;
    if !can_abort {
        return Err("当前任务没有进行中的编排".to_string());
    }

    if let Some(step_index) = state.as_ref().and_then(|item| item.pipeline_step_index) {
        if let Ok(step) = fetch_pipeline_step_by_index(&pool, &task.id, step_index).await {
            if matches!(
                step.status.as_str(),
                STEP_STATUS_RUNNING | STEP_STATUS_LAUNCHING | STEP_STATUS_PENDING
            ) {
                let _ = update_pipeline_step_status(
                    &pool,
                    &step.id,
                    STEP_STATUS_CANCELLED,
                    None,
                    None,
                    Some("编排已转人工"),
                    false,
                    true,
                )
                .await;
            }
        }
    }

    set_pipeline_state(
        &pool,
        &task.id,
        PHASE_MANUAL_CONTROL,
        false,
        state.as_ref().and_then(|item| item.pipeline_step_index),
        None,
        None,
        Some("编排已转人工"),
    )
    .await?;
    stop_task_timer_internal(&pool, &task.id, "编排转人工").await?;
    insert_activity_log(
        &pool,
        "task_pipeline_aborted",
        &format!("{}（转人工）", task.title),
        task.assignee_id.as_deref(),
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    emit_task_automation_state_changed(&app, &task, PHASE_MANUAL_CONTROL);
    Ok(())
}

/// Resume only interrupted mid-launch steps. Failed steps stay failed for manual retry.
async fn retry_pending_pipeline(
    app: &AppHandle,
    pool: &SqlitePool,
    task_id: &str,
    state_record: &TaskAutomationStateRecord,
) -> Result<(), String> {
    let task = fetch_task_by_id(pool, task_id).await?;
    let step_index = state_record.pipeline_step_index.unwrap_or(0);
    match state_record.phase.as_str() {
        // Do NOT auto-retry PHASE_PIPELINE_STEP_FAILED: user must click 重试失败步骤.
        PHASE_PIPELINE_LAUNCHING_STEP => {
            // If the step already reached a terminal status, leave it for manual action.
            if let Ok(step) = fetch_pipeline_step_by_index(pool, task_id, step_index).await {
                if matches!(
                    step.status.as_str(),
                    STEP_STATUS_FAILED | STEP_STATUS_SUCCEEDED | STEP_STATUS_CANCELLED
                ) {
                    return Ok(());
                }
            }
            launch_pipeline_step(app, pool, &task, step_index).await
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod pipeline_unit_tests {
    use super::{
        build_pipeline_step_prompt, is_manual_runnable_step_status, is_pipeline_mid_flight,
        is_pipeline_phase, resolve_failed_pipeline_step_index, PHASE_PIPELINE_STEP_FAILED,
        PHASE_PIPELINE_WAITING_STEP, STEP_STATUS_FAILED, STEP_STATUS_PENDING,
    };
    use crate::db::models::{Task, TaskAutomationStateRecord, TaskPipelineStep};

    fn sample_task() -> Task {
        Task {
            id: "t1".into(),
            title: "示例任务".into(),
            description: Some("描述".into()),
            status: "todo".into(),
            priority: "medium".into(),
            project_id: "p1".into(),
            use_worktree: true,
            assignee_id: Some("e1".into()),
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
            completed_at: None,
            deleted_at: None,
            due_date: None,
            blocked_reason: None,
            milestone_id: None,
            acceptance_checklist: None,
            last_acceptance_status: None,
            mcp_server_ids: None,
            created_at: "2026-08-03".into(),
            updated_at: "2026-08-03".into(),
        }
    }

    fn sample_step() -> TaskPipelineStep {
        TaskPipelineStep {
            id: "s1".into(),
            task_id: "t1".into(),
            step_index: 0,
            title: "实现接口".into(),
            goal: Some("写 API".into()),
            success_criteria: Some("可编译".into()),
            employee_id: None,
            status: "pending".into(),
            session_id: None,
            handoff_summary: None,
            last_error: None,
            started_at: None,
            ended_at: None,
            created_at: "2026-08-03".into(),
            updated_at: "2026-08-03".into(),
        }
    }

    fn sample_state(phase: &str, pipeline_active: bool, step_index: Option<i32>) -> TaskAutomationStateRecord {
        TaskAutomationStateRecord {
            task_id: "t1".into(),
            phase: phase.into(),
            round_count: 0,
            consumed_session_id: None,
            last_trigger_session_id: None,
            pending_action: None,
            pending_round_count: None,
            last_error: None,
            last_verdict_json: None,
            updated_at: "2026-08-03".into(),
            pipeline_active,
            pipeline_step_index: step_index,
        }
    }

    #[test]
    fn pipeline_phase_helper_recognizes_pipeline_phases() {
        assert!(is_pipeline_phase(PHASE_PIPELINE_WAITING_STEP));
        assert!(!is_pipeline_phase("waiting_review"));
    }

    #[test]
    fn step_prompt_includes_handoff_and_focus() {
        let prompt = build_pipeline_step_prompt(
            &sample_task(),
            &sample_step(),
            Some("上一步完成了数据模型"),
            3,
        );
        assert!(prompt.contains("本步标题：实现接口"));
        assert!(prompt.contains("上一步交接摘要"));
        assert!(prompt.contains("仅完成本步目标"));
        assert!(prompt.contains("第 1 / 3 步"));
    }

    #[test]
    fn resolve_failed_step_prefers_cursor_then_falls_back_to_step_status() {
        let mut failed = sample_step();
        failed.status = STEP_STATUS_FAILED.into();
        failed.step_index = 1;
        let pending = sample_step();
        let steps = vec![pending, failed.clone()];

        let state = sample_state(PHASE_PIPELINE_STEP_FAILED, true, Some(1));
        assert_eq!(resolve_failed_pipeline_step_index(&state, &steps), Some(1));

        // Phase drifted after restart, but step still failed — retry must still work.
        let drifted = sample_state("idle", false, None);
        assert_eq!(resolve_failed_pipeline_step_index(&drifted, &steps), Some(1));

        let clean = sample_state("idle", false, None);
        let only_pending = vec![sample_step()];
        assert_eq!(resolve_failed_pipeline_step_index(&clean, &only_pending), None);
    }

    #[test]
    fn mid_flight_and_manual_runnable_helpers() {
        let launching = sample_state("pipeline_launching_step", true, Some(0));
        assert!(is_pipeline_mid_flight(Some(&launching)));
        let failed = sample_state(PHASE_PIPELINE_STEP_FAILED, true, Some(0));
        assert!(!is_pipeline_mid_flight(Some(&failed)));
        assert!(is_manual_runnable_step_status(STEP_STATUS_PENDING));
        assert!(is_manual_runnable_step_status(STEP_STATUS_FAILED));
        assert!(!is_manual_runnable_step_status("running"));
    }
}
