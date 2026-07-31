// Domain slice: session_exit (included into task_automation)

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
          AND tas.phase IN ($3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1)
    .bind(TASK_STATUS_ARCHIVED)
    .bind(PHASE_LAUNCHING_REVIEW)
    .bind(PHASE_REVIEW_LAUNCH_FAILED)
    .bind(PHASE_LAUNCHING_FIX)
    .bind(PHASE_FIX_LAUNCH_FAILED)
    .bind(PHASE_COMMITTING_CODE)
    .bind(PHASE_COMMIT_FAILED)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to list pending automation tasks: {}", error))
}

pub async fn resume_pending_automation(app: &AppHandle) -> Result<(), String> {
    let pool = sqlite_pool(app).await?;
    recover_orphaned_running_sessions(&pool).await?;
    replay_unconsumed_terminal_automation_exits(app, &pool).await?;

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
            PHASE_COMMITTING_CODE | PHASE_COMMIT_FAILED => {
                let task = fetch_task_by_id(&pool, &task_id).await?;
                retry_pending_commit(app, &pool, &task, None).await?;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn recover_orphaned_running_sessions(pool: &SqlitePool) -> Result<usize, String> {
    let sessions = sqlx::query_as::<_, (String, Option<String>)>(
        r#"
        SELECT id, task_git_context_id
        FROM codex_sessions
        WHERE status = 'running'
          AND ended_at IS NULL
        ORDER BY started_at ASC, created_at ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch orphaned running sessions: {}", error))?;

    let mut recovered = 0;
    for (session_id, task_git_context_id) in sessions {
        let result = sqlx::query(
            r#"
            UPDATE codex_sessions
            SET status = 'failed',
                exit_code = 1,
                ended_at = $2
            WHERE id = $1
              AND status = 'running'
              AND ended_at IS NULL
            "#,
        )
        .bind(&session_id)
        .bind(now_sqlite())
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to recover orphaned running session: {}", error))?;

        if result.rows_affected() == 0 {
            continue;
        }

        insert_codex_session_event(
            pool,
            &session_id,
            "session_failed",
            Some(ORPHANED_RUNNING_SESSION_MESSAGE),
        )
        .await?;

        if let Some(task_git_context_id) = task_git_context_id.as_deref() {
            mark_task_git_context_session_finished(
                pool,
                task_git_context_id,
                false,
                Some(ORPHANED_RUNNING_SESSION_MESSAGE),
            )
            .await?;
        }

        recovered += 1;
    }

    if recovered > 0 {
        eprintln!("[task-automation] recovered {recovered} orphaned running session(s)");
    }

    Ok(recovered)
}

async fn replay_unconsumed_terminal_automation_exits(
    app: &AppHandle,
    pool: &SqlitePool,
) -> Result<usize, String> {
    let session_ids = sqlx::query_scalar::<_, String>(
        r#"
        SELECT s.id
        FROM task_automation_state tas
        INNER JOIN tasks t ON t.id = tas.task_id
        INNER JOIN codex_sessions s ON s.id = tas.last_trigger_session_id
        WHERE t.automation_mode = $1
          AND t.status != $2
          AND tas.phase IN ($3, $4)
          AND s.status IN ('exited', 'failed')
          AND (
            tas.consumed_session_id IS NULL
            OR tas.consumed_session_id != s.id
          )
        ORDER BY s.started_at ASC, s.created_at ASC
        "#,
    )
    .bind(AUTOMATION_MODE_REVIEW_FIX_LOOP_V1)
    .bind(TASK_STATUS_ARCHIVED)
    .bind(PHASE_WAITING_REVIEW)
    .bind(PHASE_WAITING_EXECUTION)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to fetch unconsumed automation exits: {}", error))?;

    let mut replayed = 0;
    for session_id in session_ids {
        handle_session_exit(app, &session_id).await?;
        replayed += 1;
    }

    if replayed > 0 {
        eprintln!("[task-automation] replayed {replayed} unconsumed automation exit(s)");
    }

    Ok(replayed)
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
        stop_task_timer_internal(pool, &task.id, "自动质控人工停止").await?;
        insert_activity_log(
            pool,
            "task_automation_manual_control",
            "执行已被人工停止，自动质控交由人工接管",
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
        emit_task_automation_state_changed(app, task, PHASE_MANUAL_CONTROL);
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
    if state_record.is_none()
        || matches!(
            state_record.map(|item| item.phase.as_str()),
            Some(PHASE_MANUAL_CONTROL | PHASE_BLOCKED | PHASE_IDLE)
        )
    {
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
    let review_launched = retry_pending_review(app, pool, &task.id, &reserved_state).await?;
    if review_launched {
        insert_activity_log(
            pool,
            "task_automation_review_started",
            "执行完成，已自动发起代码审核",
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
    }
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
        stop_task_timer_internal(pool, &task.id, "自动质控人工停止").await?;
        insert_activity_log(
            pool,
            "task_automation_manual_control",
            "审核已被人工停止，自动质控交由人工接管",
            facts.employee_id.as_deref(),
            Some(task.id.as_str()),
            Some(task.project_id.as_str()),
        )
        .await?;
        emit_task_automation_state_changed(app, task, PHASE_MANUAL_CONTROL);
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
        if should_auto_commit_task_worktree(task) {
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

        return complete_automation_without_auto_commit(
            app,
            pool,
            task,
            Some(&facts.session_id),
            Some(&verdict_json),
            facts.employee_id.as_deref(),
            state_record,
        )
        .await;
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

