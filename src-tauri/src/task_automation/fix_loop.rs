// Domain slice: fix_loop (included into task_automation)

fn should_auto_commit_task_worktree(task: &Task) -> bool {
    task.use_worktree
}

fn is_no_reviewable_code_changes_error(error: &str) -> bool {
    matches!(
        error,
        "当前工作区没有可审核的代码改动" | "当前工作区没有可审核的代码 diff"
    )
}

async fn retry_pending_commit<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    task: &Task,
    employee_id: Option<&str>,
) -> Result<(), String> {
    let state_record = fetch_task_automation_state_record(pool, &task.id)
        .await?
        .ok_or_else(|| "自动质控状态不存在，无法继续自动提交".to_string())?;

    if !should_auto_commit_task_worktree(task) {
        return complete_automation_without_auto_commit(
            app,
            pool,
            task,
            state_record.consumed_session_id.as_deref(),
            state_record.last_verdict_json.as_deref(),
            employee_id,
            Some(&state_record),
        )
        .await;
    }

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
            stop_task_timer_internal(pool, &task.id, "自动质控提交失败").await?;
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

async fn complete_automation_without_auto_commit<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    task: &Task,
    consumed_session_id: Option<&str>,
    last_verdict_json: Option<&str>,
    employee_id: Option<&str>,
    state_record: Option<&TaskAutomationStateRecord>,
) -> Result<(), String> {
    update_task_status_internal(app, pool, task, "completed").await?;
    record_automation_completed_without_auto_commit(
        pool,
        task,
        consumed_session_id,
        last_verdict_json,
        employee_id,
        state_record,
    )
    .await?;
    emit_task_automation_state_changed(app, task, PHASE_COMPLETED);
    Ok(())
}

async fn record_automation_completed_without_auto_commit(
    pool: &SqlitePool,
    task: &Task,
    consumed_session_id: Option<&str>,
    last_verdict_json: Option<&str>,
    employee_id: Option<&str>,
    state_record: Option<&TaskAutomationStateRecord>,
) -> Result<(), String> {
    upsert_state_terminal(
        pool,
        &task.id,
        consumed_session_id,
        PHASE_COMPLETED,
        last_verdict_json,
        None,
        None,
        state_record,
    )
    .await?;
    insert_activity_log(
        pool,
        "task_automation_completed",
        WORKTREE_DISABLED_AUTO_COMMIT_SKIPPED_MESSAGE,
        employee_id,
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
) -> Result<bool, String> {
    let task = fetch_task_by_id(pool, task_id).await?;
    if !task_automation_enabled(&task) {
        return Ok(false);
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
        if is_no_reviewable_code_changes_error(&error) {
            finalize_no_reviewable_changes(app, pool, &task, state_record).await?;
            return Ok(false);
        }
        mark_launch_failure(pool, &task, PHASE_REVIEW_LAUNCH_FAILED, &error).await?;
        emit_task_automation_state_changed(app, &task, PHASE_REVIEW_LAUNCH_FAILED);
        return Err(error);
    }

    // Notify frontend after status=review + waiting_review are committed so kanban can refresh.
    emit_task_automation_state_changed(app, &task, PHASE_WAITING_REVIEW);
    Ok(true)
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
        mark_launch_failure(pool, &task, PHASE_FIX_LAUNCH_FAILED, &error).await?;
        emit_task_automation_state_changed(app, &task, PHASE_FIX_LAUNCH_FAILED);
        return Err(error);
    }

    // Notify frontend after status=in_progress + waiting_execution are committed.
    emit_task_automation_state_changed(app, &task, PHASE_WAITING_EXECUTION);
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

    if assignee.ai_provider == "claude" {
        let manager = app
            .state::<Arc<tokio::sync::Mutex<ClaudeManager>>>()
            .inner()
            .clone();
        start_claude_with_manager(
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
    } else if assignee.ai_provider == "opencode" {
        let manager = app
            .state::<Arc<tokio::sync::Mutex<OpenCodeManager>>>()
            .inner()
            .clone();
        start_opencode_with_manager(
            app.clone(),
            manager,
            assignee.id.clone(),
            execution_input.prompt,
            Some(assignee.model.clone()),
            Some(execution_context.working_dir),
            Some(task.id.clone()),
            execution_context.task_git_context_id,
            None,
            Some(execution_input.image_paths),
        )
        .await
    } else if assignee.ai_provider == "grok" {
        let manager = app
            .state::<Arc<tokio::sync::Mutex<GrokManager>>>()
            .inner()
            .clone();
        start_grok_with_manager(
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
    } else {
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
    task: &Task,
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
    .bind(&task.id)
    .bind(phase)
    .bind(message)
    .bind(now_sqlite())
    .execute(pool)
    .await
    .map_err(|error| format!("Failed to mark automation launch failure: {}", error))?;

    stop_task_timer_internal(pool, &task.id, "自动质控启动失败").await?;

    Ok(())
}

async fn finalize_no_reviewable_changes(
    app: &AppHandle,
    pool: &SqlitePool,
    task: &Task,
    state_record: &TaskAutomationStateRecord,
) -> Result<(), String> {
    let policy = load_task_automation_policy(app);
    let phase = if policy.failure_strategy == FAILURE_STRATEGY_MANUAL_CONTROL {
        PHASE_MANUAL_CONTROL
    } else {
        PHASE_BLOCKED
    };

    upsert_state_terminal(
        pool,
        &task.id,
        state_record.consumed_session_id.as_deref(),
        phase,
        state_record.last_verdict_json.as_deref(),
        None,
        Some(NO_REVIEWABLE_CODE_CHANGES_MESSAGE),
        Some(state_record),
    )
    .await?;

    let current_task = fetch_task_by_id(pool, &task.id).await?;
    stop_task_timer_internal(pool, &task.id, "自动质控无可审核改动").await?;
    if phase == PHASE_BLOCKED {
        update_task_status_internal(app, pool, &current_task, "blocked").await?;
    } else if current_task.status == "review" && task.status != "review" {
        update_task_status_internal(app, pool, &current_task, &task.status).await?;
    }

    insert_activity_log(
        pool,
        if phase == PHASE_MANUAL_CONTROL {
            "task_automation_manual_control"
        } else {
            "task_automation_blocked"
        },
        NO_REVIEWABLE_CODE_CHANGES_MESSAGE,
        None,
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    emit_task_automation_state_changed(app, task, phase);

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

