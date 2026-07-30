// Domain slice: context (included into git_workflow)

fn build_task_branch(task_id: &str) -> String {
    format!("codex/task-{}", sanitize_git_fragment(task_id))
}

async fn context_is_healthy<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    context: &TaskGitContextRecord,
) -> bool {
    let branch_exists =
        match git_ref_exists(app, runtime, &format!("refs/heads/{}", context.task_branch)).await {
            Ok(value) => value,
            Err(_) => false,
        };
    if !branch_exists {
        return false;
    }
    let worktree_runtime = GitProjectRuntimeContext {
        repo_path: context.worktree_path.clone(),
        execution_target: runtime.execution_target.clone(),
        ssh_config_id: runtime.ssh_config_id.clone(),
    };
    determine_current_branch(app, &worktree_runtime)
        .await
        .is_ok()
}

async fn ensure_task_branch_for_runtime<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    task_branch: &str,
    target_branch: &str,
) -> Result<(), String> {
    #[cfg(test)]
    {
        if runtime.execution_target == EXECUTION_TARGET_LOCAL {
            let full_ref = format!("refs/heads/{task_branch}");
            if git_ref_exists_local(&runtime.repo_path, &full_ref) {
                return Ok(());
            }
            return run_git_command(&runtime.repo_path, &["branch", task_branch, target_branch]);
        }
    }

    git_runtime::ensure_task_branch(
        app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        task_branch,
        target_branch,
    )
    .await
}

async fn current_head_commit<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    repo_path: &str,
    revision: &str,
) -> Result<String, String> {
    #[cfg(test)]
    {
        if runtime.execution_target == EXECUTION_TARGET_LOCAL {
            return run_git_text(repo_path, &["rev-parse", revision]);
        }
    }

    git_runtime::rev_parse(
        app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        repo_path,
        revision,
    )
    .await
}

fn task_git_context_has_pending_merge_local(
    context: &TaskGitContextRecord,
) -> Result<bool, String> {
    let comparison =
        compare_revisions_local(&context.worktree_path, &context.target_branch, "HEAD")?;
    Ok(comparison.ahead_commits > 0)
}

async fn fetch_task_git_context_by_id(
    pool: &SqlitePool,
    task_git_context_id: &str,
) -> Result<TaskGitContextRecord, String> {
    sqlx::query_as::<_, TaskGitContextRecord>(
        "SELECT * FROM task_git_contexts WHERE id = $1 LIMIT 1",
    )
    .bind(task_git_context_id)
    .fetch_one(pool)
    .await
    .map_err(|error| format!("Task git context {} 不存在: {}", task_git_context_id, error))
}

async fn fetch_task_git_context_by_task_id(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<Option<TaskGitContextRecord>, String> {
    sqlx::query_as::<_, TaskGitContextRecord>(
        "SELECT * FROM task_git_contexts WHERE task_id = $1 LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("查询 task git context 失败: {}", error))
}

async fn resolve_task_git_commit_target<R: Runtime>(
    app: &AppHandle<R>,
    task_git_context_id: &str,
) -> Result<
    (
        SqlitePool,
        Task,
        Project,
        GitProjectRuntimeContext,
        TaskGitContextRecord,
    ),
    String,
> {
    let pool = sqlite_pool(app).await?;
    let context = fetch_task_git_context_by_id(&pool, task_git_context_id).await?;
    let task = fetch_task_by_id(&pool, &context.task_id).await?;
    let project = fetch_project_by_id(&pool, &context.project_id).await?;
    let runtime = resolve_project_runtime_context(&project)?;
    Ok((pool, task, project, runtime, context))
}

async fn collect_task_git_commit_overview<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    context: &TaskGitContextRecord,
) -> Result<TaskGitCommitOverview, String> {
    let overview = collect_git_overview_for_dir(app, runtime, &context.worktree_path, 1).await?;
    let working_tree_changes =
        collect_working_tree_changes(app, runtime, &context.worktree_path).await?;

    Ok(TaskGitCommitOverview {
        task_git_context_id: context.id.clone(),
        project_id: context.project_id.clone(),
        worktree_path: context.worktree_path.clone(),
        execution_target: runtime.execution_target.clone(),
        current_branch: overview.current_branch,
        working_tree_summary: overview.working_tree_summary,
        working_tree_changes,
        refreshed_at: now_sqlite(),
    })
}

async fn update_task_git_context_merge_ready(
    pool: &SqlitePool,
    context: &mut TaskGitContextRecord,
    detail: &str,
) -> Result<TaskGitContextRecord, String> {
    context.context_version += 1;
    context.state = TASK_GIT_STATE_MERGE_READY.to_string();
    context.last_error = None;
    context.updated_at = now_sqlite();
    let saved = save_task_git_context(pool, context).await?;
    insert_activity_log(
        pool,
        "task_merge_ready",
        detail,
        None,
        Some(saved.task_id.as_str()),
        Some(saved.project_id.as_str()),
    )
    .await?;
    Ok(saved)
}

async fn task_git_context_has_pending_merge<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    context: &TaskGitContextRecord,
) -> Result<bool, String> {
    if runtime.execution_target == EXECUTION_TARGET_LOCAL {
        return task_git_context_has_pending_merge_local(context);
    }

    let comparison = git_runtime::compare_revisions(
        app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &context.worktree_path,
        &context.target_branch,
        "HEAD",
    )
    .await?;
    Ok(comparison.ahead_commits > 0)
}

async fn insert_task_git_context(
    pool: &SqlitePool,
    context: &TaskGitContextRecord,
) -> Result<TaskGitContextRecord, String> {
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
            pending_action_type,
            pending_action_token_hash,
            pending_action_payload_json,
            pending_action_nonce,
            pending_action_requested_at,
            pending_action_expires_at,
            pending_action_repo_revision,
            pending_action_bound_context_version,
            last_reconciled_at,
            last_error,
            created_at,
            updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22
        )
        "#,
    )
    .bind(&context.id)
    .bind(&context.task_id)
    .bind(&context.project_id)
    .bind(&context.base_branch)
    .bind(&context.task_branch)
    .bind(&context.target_branch)
    .bind(&context.worktree_path)
    .bind(&context.repo_head_commit_at_prepare)
    .bind(&context.state)
    .bind(context.context_version)
    .bind(&context.pending_action_type)
    .bind(&context.pending_action_token_hash)
    .bind(&context.pending_action_payload_json)
    .bind(&context.pending_action_nonce)
    .bind(&context.pending_action_requested_at)
    .bind(&context.pending_action_expires_at)
    .bind(&context.pending_action_repo_revision)
    .bind(context.pending_action_bound_context_version)
    .bind(&context.last_reconciled_at)
    .bind(&context.last_error)
    .bind(&context.created_at)
    .bind(&context.updated_at)
    .execute(pool)
    .await
    .map_err(|error| format!("写入 task git context 失败: {}", error))?;

    fetch_task_git_context_by_id(pool, &context.id).await
}

async fn save_task_git_context(
    pool: &SqlitePool,
    context: &TaskGitContextRecord,
) -> Result<TaskGitContextRecord, String> {
    sqlx::query(
        r#"
        UPDATE task_git_contexts
        SET base_branch = $2,
            task_branch = $3,
            target_branch = $4,
            worktree_path = $5,
            repo_head_commit_at_prepare = $6,
            state = $7,
            context_version = $8,
            pending_action_type = $9,
            pending_action_token_hash = $10,
            pending_action_payload_json = $11,
            pending_action_nonce = $12,
            pending_action_requested_at = $13,
            pending_action_expires_at = $14,
            pending_action_repo_revision = $15,
            pending_action_bound_context_version = $16,
            last_reconciled_at = $17,
            last_error = $18,
            updated_at = $19
        WHERE id = $1
        "#,
    )
    .bind(&context.id)
    .bind(&context.base_branch)
    .bind(&context.task_branch)
    .bind(&context.target_branch)
    .bind(&context.worktree_path)
    .bind(&context.repo_head_commit_at_prepare)
    .bind(&context.state)
    .bind(context.context_version)
    .bind(&context.pending_action_type)
    .bind(&context.pending_action_token_hash)
    .bind(&context.pending_action_payload_json)
    .bind(&context.pending_action_nonce)
    .bind(&context.pending_action_requested_at)
    .bind(&context.pending_action_expires_at)
    .bind(&context.pending_action_repo_revision)
    .bind(context.pending_action_bound_context_version)
    .bind(&context.last_reconciled_at)
    .bind(&context.last_error)
    .bind(&context.updated_at)
    .execute(pool)
    .await
    .map_err(|error| format!("更新 task git context 失败: {}", error))?;

    fetch_task_git_context_by_id(pool, &context.id).await
}

async fn delete_task_git_context(
    pool: &SqlitePool,
    task_git_context_id: &str,
) -> Result<(), String> {
    sqlx::query("DELETE FROM task_git_contexts WHERE id = $1")
        .bind(task_git_context_id)
        .execute(pool)
        .await
        .map_err(|error| {
            format!(
                "删除 Task git context {} 失败: {}",
                task_git_context_id, error
            )
        })?;
    Ok(())
}

async fn summarize_task_git_context<R: Runtime>(
    app: &AppHandle<R>,
    runtime: Option<&GitProjectRuntimeContext>,
    value: TaskGitContextRecord,
) -> TaskGitContextSummary {
    let mut summary = TaskGitContextSummary::from(value);
    if let Some(runtime) = runtime {
        summary.worktree_missing =
            !worktree_path_exists(app, runtime, &summary.worktree_path).await;
    }
    summary
}

async fn update_context_after_prepare<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    task: &Task,
    project: &Project,
    preferred_target_branch: Option<String>,
) -> Result<TaskGitContextRecord, String> {
    let runtime = resolve_project_runtime_context(project)?;
    let git_preferences = resolve_project_git_preferences(app, project)?;
    let target_branch = match preferred_target_branch {
        Some(branch) => branch,
        None => determine_current_branch(app, &runtime).await?,
    };
    let task_branch = build_task_branch(&task.id);
    let worktree_path =
        build_task_worktree_path_for_runtime(app, &runtime, &task.id, &git_preferences).await?;
    let head_commit =
        current_head_commit(app, &runtime, &runtime.repo_path, &target_branch).await?;

    if let Some(existing) = fetch_task_git_context_by_task_id(pool, &task.id).await? {
        if existing.target_branch != target_branch {
            return Err(format!(
                "当前任务已绑定目标分支 {}，不能切换到 {}",
                existing.target_branch, target_branch
            ));
        }
        if context_is_healthy(app, &runtime, &existing).await {
            if matches!(
                existing.state.as_str(),
                TASK_GIT_STATE_FAILED | TASK_GIT_STATE_DRIFTED
            ) {
                return mark_task_git_context_reconciled_after_prepare(
                    pool,
                    existing,
                    head_commit,
                    "任务 Git 上下文已恢复可用",
                )
                .await;
            }
            return Ok(existing);
        }

        ensure_task_branch_for_runtime(
            app,
            &runtime,
            &existing.task_branch,
            &existing.target_branch,
        )
        .await?;
        ensure_task_worktree_for_runtime(
            app,
            &runtime,
            &existing.worktree_path,
            &existing.task_branch,
            &existing.target_branch,
        )
        .await?;
        return mark_task_git_context_reconciled_after_prepare(
            pool,
            existing,
            head_commit,
            "任务 Git 上下文已恢复可用",
        )
        .await;
    }

    ensure_task_branch_for_runtime(app, &runtime, &task_branch, &target_branch).await?;
    ensure_task_worktree_for_runtime(app, &runtime, &worktree_path, &task_branch, &target_branch)
        .await?;

    let now = now_sqlite();
    let record = TaskGitContextRecord {
        id: Uuid::new_v4().to_string(),
        task_id: task.id.clone(),
        project_id: project.id.clone(),
        base_branch: target_branch.clone(),
        task_branch,
        target_branch,
        worktree_path,
        repo_head_commit_at_prepare: Some(head_commit),
        state: TASK_GIT_STATE_READY.to_string(),
        context_version: 1,
        pending_action_type: None,
        pending_action_token_hash: None,
        pending_action_payload_json: None,
        pending_action_nonce: None,
        pending_action_requested_at: None,
        pending_action_expires_at: None,
        pending_action_repo_revision: None,
        pending_action_bound_context_version: None,
        last_reconciled_at: None,
        last_error: None,
        created_at: now.clone(),
        updated_at: now,
    };

    match insert_task_git_context(pool, &record).await {
        Ok(saved) => {
            insert_activity_log(
                pool,
                "task_git_context_prepared",
                "任务 Git 隔离工作区已准备完成",
                None,
                Some(&task.id),
                Some(&project.id),
            )
            .await?;
            Ok(saved)
        }
        Err(error) => {
            if let Some(mut existing) = fetch_task_git_context_by_task_id(pool, &task.id).await? {
                if context_is_healthy(app, &runtime, &existing).await {
                    return Ok(existing);
                }
                existing.state = TASK_GIT_STATE_FAILED.to_string();
                existing.context_version += 1;
                existing.last_error = Some(error.clone());
                existing.updated_at = now_sqlite();
                let saved = save_task_git_context(pool, &existing).await?;
                insert_activity_log(
                    pool,
                    "task_git_context_prepare_failed",
                    &error,
                    None,
                    Some(&task.id),
                    Some(&project.id),
                )
                .await?;
                Ok(saved)
            } else {
                Err(error)
            }
        }
    }
}

async fn mark_task_git_context_reconciled_after_prepare(
    pool: &SqlitePool,
    mut context: TaskGitContextRecord,
    head_commit: String,
    message: &str,
) -> Result<TaskGitContextRecord, String> {
    context.base_branch = context.target_branch.clone();
    context.repo_head_commit_at_prepare = Some(head_commit);
    context.last_reconciled_at = Some(now_sqlite());
    context.last_error = None;
    clear_pending_action_fields(&mut context);
    context.state = TASK_GIT_STATE_READY.to_string();
    context.context_version += 1;
    context.updated_at = now_sqlite();
    let saved = save_task_git_context(pool, &context).await?;
    insert_activity_log(
        pool,
        "task_git_context_reconciled",
        message,
        None,
        Some(&saved.task_id),
        Some(&saved.project_id),
    )
    .await?;
    Ok(saved)
}

async fn refresh_context_state<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    context: &mut TaskGitContextRecord,
    runtime: &GitProjectRuntimeContext,
) -> Result<TaskGitContextRecord, String> {
    if context_is_healthy(app, runtime, context).await {
        return Ok(context.clone());
    }
    context.state = TASK_GIT_STATE_DRIFTED.to_string();
    context.context_version += 1;
    context.last_error = Some("检测到任务工作树或任务分支状态异常".to_string());
    clear_pending_action_fields(context);
    context.updated_at = now_sqlite();
    let saved = save_task_git_context(pool, context).await?;
    insert_activity_log(
        pool,
        "task_git_context_drift_detected",
        "检测到任务工作树或任务分支状态异常",
        None,
        Some(&saved.task_id),
        Some(&saved.project_id),
    )
    .await?;
    Ok(saved)
}

pub(crate) async fn validate_task_git_context_launch<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
    task_git_context_id: &str,
    working_dir: Option<&str>,
) -> Result<String, String> {
    let pool = sqlite_pool(app).await?;
    let context = fetch_task_git_context_by_id(&pool, task_git_context_id).await?;
    if context.task_id != task_id {
        return Err("taskGitContextId 与 taskId 不匹配".to_string());
    }
    let working_dir = trim_optional(working_dir.map(ToOwned::to_owned))
        .ok_or_else(|| "taskGitContextId 已提供时必须显式传入 workingDir".to_string())?;
    if working_dir != context.worktree_path {
        return Err("workingDir 与 task git context 绑定的 worktree 不一致".to_string());
    }
    if !matches!(
        context.state.as_str(),
        TASK_GIT_STATE_READY | TASK_GIT_STATE_RUNNING | TASK_GIT_STATE_MERGE_READY
    ) {
        return Err(format!(
            "当前 task git context 状态不允许启动执行：{}",
            context.state
        ));
    }
    Ok(context.worktree_path)
}

pub(crate) async fn mark_task_git_context_running(
    pool: &SqlitePool,
    task_git_context_id: &str,
) -> Result<(), String> {
    let mut context = fetch_task_git_context_by_id(pool, task_git_context_id).await?;
    if context.state == TASK_GIT_STATE_RUNNING {
        return Ok(());
    }
    context.state = TASK_GIT_STATE_RUNNING.to_string();
    context.context_version += 1;
    context.last_error = None;
    context.updated_at = now_sqlite();
    let saved = save_task_git_context(pool, &context).await?;
    insert_activity_log(
        pool,
        "task_execution_started",
        "任务 Git 上下文已进入运行中",
        None,
        Some(&saved.task_id),
        Some(&saved.project_id),
    )
    .await?;
    Ok(())
}

pub(crate) async fn mark_task_git_context_session_finished(
    pool: &SqlitePool,
    task_git_context_id: &str,
    success: bool,
    message: Option<&str>,
) -> Result<(), String> {
    let mut context = fetch_task_git_context_by_id(pool, task_git_context_id).await?;
    context.context_version += 1;
    context.updated_at = now_sqlite();
    if success {
        let task = fetch_task_by_id(pool, &context.task_id).await?;
        let is_automation_enabled = task.automation_mode.is_some();
        context.state = if is_automation_enabled {
            TASK_GIT_STATE_READY.to_string()
        } else {
            TASK_GIT_STATE_MERGE_READY.to_string()
        };
        context.last_error = None;
        let saved = save_task_git_context(pool, &context).await?;
        insert_activity_log(
            pool,
            if is_automation_enabled {
                "task_git_context_ready"
            } else {
                "task_merge_ready"
            },
            if is_automation_enabled {
                "任务执行完成，等待自动提交代码"
            } else {
                "任务执行完成，等待后续 Git 确认动作"
            },
            None,
            Some(&saved.task_id),
            Some(&saved.project_id),
        )
        .await?;
    } else {
        context.state = TASK_GIT_STATE_FAILED.to_string();
        context.last_error = message.map(ToOwned::to_owned);
        let saved = save_task_git_context(pool, &context).await?;
        insert_activity_log(
            pool,
            "task_git_context_prepare_failed",
            message.unwrap_or("任务执行失败"),
            None,
            Some(&saved.task_id),
            Some(&saved.project_id),
        )
        .await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_task_git_contexts<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<Vec<TaskGitContextSummary>, String> {
    let pool = sqlite_pool(&app).await?;
    let project = fetch_project_by_id(&pool, &project_id).await?;
    let runtime = resolve_project_runtime_context(&project).ok();
    let rows = sqlx::query_as::<_, TaskGitContextRecord>(
        "SELECT * FROM task_git_contexts WHERE project_id = $1 ORDER BY updated_at DESC, created_at DESC",
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("查询 task git contexts 失败: {}", error))?;
    let mut summaries = Vec::with_capacity(rows.len());
    for row in rows {
        summaries.push(summarize_task_git_context(&app, runtime.as_ref(), row).await);
    }
    Ok(summaries)
}

#[tauri::command]
pub async fn get_task_git_context<R: Runtime>(
    app: AppHandle<R>,
    task_git_context_id: String,
) -> Result<Option<TaskGitContextSummary>, String> {
    let pool = sqlite_pool(&app).await?;
    let row = sqlx::query_as::<_, TaskGitContextRecord>(
        "SELECT * FROM task_git_contexts WHERE id = $1 LIMIT 1",
    )
    .bind(task_git_context_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| format!("查询 task git context 失败: {}", error))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let project = fetch_project_by_id(&pool, &row.project_id).await?;
    let runtime = resolve_project_runtime_context(&project).ok();
    Ok(Some(
        summarize_task_git_context(&app, runtime.as_ref(), row).await,
    ))
}

#[tauri::command]
pub async fn prepare_task_git_execution<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
    preferred_target_branch: Option<String>,
) -> Result<PreparedTaskGitExecution, String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &task_id).await?;
    let project = fetch_project_by_id(&pool, &task.project_id).await?;
    let preferred_target_branch = trim_optional(preferred_target_branch);
    let context =
        update_context_after_prepare(&app, &pool, &task, &project, preferred_target_branch).await?;
    Ok(summarize_prepared(&context))
}

#[tauri::command]
pub async fn refresh_task_git_context<R: Runtime>(
    app: AppHandle<R>,
    task_git_context_id: String,
) -> Result<TaskGitContextSummary, String> {
    let pool = sqlite_pool(&app).await?;
    let mut context = fetch_task_git_context_by_id(&pool, &task_git_context_id).await?;
    let project = fetch_project_by_id(&pool, &context.project_id).await?;
    let runtime = resolve_project_runtime_context(&project)?;
    let refreshed = refresh_context_state(&app, &pool, &mut context, &runtime).await?;
    Ok(summarize_task_git_context(&app, Some(&runtime), refreshed).await)
}

#[tauri::command]
pub async fn reconcile_task_git_context<R: Runtime>(
    app: AppHandle<R>,
    task_git_context_id: String,
) -> Result<TaskGitContextSummary, String> {
    let pool = sqlite_pool(&app).await?;
    let context = fetch_task_git_context_by_id(&pool, &task_git_context_id).await?;
    let task = fetch_task_by_id(&pool, &context.task_id).await?;
    let project = fetch_project_by_id(&pool, &context.project_id).await?;
    let reconciled =
        update_context_after_prepare(&app, &pool, &task, &project, Some(context.target_branch))
            .await?;
    let runtime = resolve_project_runtime_context(&project)?;
    Ok(summarize_task_git_context(&app, Some(&runtime), reconciled).await)
}

async fn commit_task_git_changes_internal<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    task: &Task,
    runtime: &GitProjectRuntimeContext,
    context: &mut TaskGitContextRecord,
    message: &str,
    recover_automation: bool,
) -> Result<String, String> {
    let result = git_runtime::commit_changes(
        app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &context.worktree_path,
        message,
    )
    .await?;
    insert_activity_log(
        pool,
        "task_git_committed",
        &result,
        None,
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    update_task_git_context_merge_ready(pool, context, "任务代码已提交，等待合并到目标分支")
        .await?;
    if recover_automation {
        task_automation::mark_task_automation_commit_completed(app, pool, task, &result).await?;
    }
    Ok(result)
}

#[tauri::command]
pub async fn get_task_git_commit_overview<R: Runtime>(
    app: AppHandle<R>,
    task_git_context_id: String,
) -> Result<TaskGitCommitOverview, String> {
    let (_pool, _task, _project, runtime, context) =
        resolve_task_git_commit_target(&app, &task_git_context_id).await?;
    if !context_is_healthy(&app, &runtime, &context).await {
        return Err("task git context 不可用，当前状态异常".to_string());
    }
    collect_task_git_commit_overview(&app, &runtime, &context).await
}

#[tauri::command]
pub async fn stage_all_task_git_files<R: Runtime>(
    app: AppHandle<R>,
    task_git_context_id: String,
) -> Result<String, String> {
    let (pool, task, _project, runtime, context) =
        resolve_task_git_commit_target(&app, &task_git_context_id).await?;
    if !context_is_healthy(&app, &runtime, &context).await {
        return Err("task git context 不可用，当前状态异常".to_string());
    }
    let result = git_runtime::stage_all(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &context.worktree_path,
    )
    .await?;
    insert_activity_log(
        &pool,
        "task_git_stage_all",
        &result,
        None,
        Some(task.id.as_str()),
        Some(task.project_id.as_str()),
    )
    .await?;
    Ok(result)
}

#[tauri::command]
pub async fn commit_task_git_changes<R: Runtime>(
    app: AppHandle<R>,
    task_git_context_id: String,
    message: String,
) -> Result<String, String> {
    let trimmed = message.trim().to_string();
    if trimmed.is_empty() {
        return Err("提交说明不能为空".to_string());
    }

    let (pool, task, _project, runtime, mut context) =
        resolve_task_git_commit_target(&app, &task_git_context_id).await?;
    if !context_is_healthy(&app, &runtime, &context).await {
        return Err("task git context 不可用，当前状态异常".to_string());
    }

    commit_task_git_changes_internal(&app, &pool, &task, &runtime, &mut context, &trimmed, true)
        .await
}

#[tauri::command]
pub async fn delete_task_git_context_record<R: Runtime>(
    app: AppHandle<R>,
    task_git_context_id: String,
) -> Result<String, String> {
    let pool = sqlite_pool(&app).await?;
    let context = fetch_task_git_context_by_id(&pool, &task_git_context_id).await?;
    let project = fetch_project_by_id(&pool, &context.project_id).await?;
    let runtime = resolve_project_runtime_context(&project)?;
    if worktree_path_exists(&app, &runtime, &context.worktree_path).await {
        return Err("当前任务 worktree 仍存在，请先执行“清理任务工作树”".to_string());
    }

    delete_task_git_context(&pool, &context.id).await?;
    insert_activity_log(
        &pool,
        "task_worktree_cleanup_completed",
        "任务工作树已缺失，已直接移除 Git 上下文记录",
        None,
        Some(&context.task_id),
        Some(&context.project_id),
    )
    .await?;

    Ok("检测到任务 worktree 已不存在，已直接删除这条 Git 上下文记录".to_string())
}

