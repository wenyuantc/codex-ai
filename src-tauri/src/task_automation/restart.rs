// Domain slice: restart (included into task_automation)

fn validate_task_automation_restart(task: &Task) -> Result<(), String> {
    if task.status == TASK_STATUS_ARCHIVED {
        return Err("已归档任务不能重启自动质控".to_string());
    }
    if task.automation_mode.as_deref() != Some(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1) {
        return Err("当前任务未开启自动质控".to_string());
    }

    Ok(())
}

async fn stop_running_session_for_automation_restart(
    app: &AppHandle,
    employee_id: &str,
    expected_session_record_id: Option<&str>,
    message: &str,
) -> Result<bool, String> {
    let Some(expected_session_record_id) = expected_session_record_id else {
        return Err("当前自动化步骤缺少会话标识，无法安全重启".to_string());
    };

    if stop_codex_for_automation_restart(
        app,
        employee_id,
        Some(expected_session_record_id),
        message,
    )
    .await?
    {
        return Ok(true);
    }

    if stop_claude_for_automation_restart(
        app,
        employee_id,
        Some(expected_session_record_id),
        message,
    )
    .await?
    {
        return Ok(true);
    }

    stop_grok_for_automation_restart(app, employee_id, Some(expected_session_record_id), message)
        .await
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

    let _ = stop_running_session_for_automation_restart(
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
    let review_launched = retry_pending_review(app, pool, &task.id, &reserved_state).await?;
    if review_launched {
        insert_activity_log(
            pool,
            "task_automation_restart_requested",
            "已重启自动质控审核步骤",
            Some(reviewer_id),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
    }
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

    let _ = stop_running_session_for_automation_restart(
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

