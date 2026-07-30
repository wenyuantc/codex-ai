// Domain slice: pending_action (included into git_workflow)

fn build_pending_action_signature(
    task_git_context_id: &str,
    action_type: &str,
    normalized_payload_json: &str,
    nonce: &str,
    expires_at: &str,
    context_version: i64,
) -> String {
    hash_text(&format!(
        "{task_git_context_id}\n{action_type}\n{normalized_payload_json}\n{nonce}\n{expires_at}\n{context_version}"
    ))
}

fn collect_staged_change_prompts(changes: &[ProjectGitWorkingTreeChange]) -> Vec<String> {
    changes
        .iter()
        .filter(|change| matches!(change.stage_status.as_str(), "staged" | "partially_staged"))
        .map(|change| match change.change_type.as_str() {
            "renamed" if change.previous_path.is_some() => format!(
                "重命名 {} -> {}",
                change.previous_path.as_deref().unwrap_or_default(),
                change.path
            ),
            "added" => format!("新增 {}", change.path),
            "deleted" => format!("删除 {}", change.path),
            "renamed" => format!("重命名 {}", change.path),
            _ => format!("修改 {}", change.path),
        })
        .collect()
}

fn summarize_prepared(context: &TaskGitContextRecord) -> PreparedTaskGitExecution {
    PreparedTaskGitExecution {
        task_git_context_id: context.id.clone(),
        working_dir: context.worktree_path.clone(),
        task_branch: context.task_branch.clone(),
        target_branch: context.target_branch.clone(),
        state: context.state.clone(),
        context_version: context.context_version,
    }
}

fn clear_pending_action_fields(context: &mut TaskGitContextRecord) {
    context.pending_action_type = None;
    context.pending_action_token_hash = None;
    context.pending_action_payload_json = None;
    context.pending_action_nonce = None;
    context.pending_action_requested_at = None;
    context.pending_action_expires_at = None;
    context.pending_action_repo_revision = None;
    context.pending_action_bound_context_version = None;
}

async fn reject_pending_action(
    pool: &SqlitePool,
    context: &mut TaskGitContextRecord,
    message: &str,
    drifted: bool,
) -> Result<(), String> {
    clear_pending_action_fields(context);
    context.context_version += 1;
    context.state = if drifted {
        TASK_GIT_STATE_DRIFTED.to_string()
    } else {
        TASK_GIT_STATE_MERGE_READY.to_string()
    };
    context.last_error = Some(message.to_string());
    context.updated_at = now_sqlite();
    let saved = save_task_git_context(pool, context).await?;
    *context = saved;
    insert_activity_log(
        pool,
        "git_action_rejected",
        message,
        None,
        Some(&context.task_id),
        Some(&context.project_id),
    )
    .await?;
    Ok(())
}

fn action_allows_drifted_context(action_type: &str) -> bool {
    action_type == "cleanup_worktree"
}

fn action_allows_completed_context(action_type: &str) -> bool {
    action_type == "cleanup_worktree"
}

fn action_allows_failed_context(action_type: &str, payload: &Value) -> Result<bool, String> {
    if action_type != "cleanup_worktree" {
        return Ok(false);
    }

    let map = payload_object(payload)?;
    Ok(payload_bool(map, "force_remove", true))
}

fn normalize_git_action_payload(
    action_type: &str,
    context: &TaskGitContextRecord,
    payload: &Value,
) -> Result<String, String> {
    let map = payload_object(payload)?;
    let normalized = match action_type {
        "merge" => serde_json::json!({
            "target_branch": payload_string(map, "target_branch").unwrap_or_else(|| context.target_branch.clone()),
            "strategy": payload_string(map, "strategy").unwrap_or_else(|| "ort".to_string()),
            "allow_ff": payload_bool(map, "allow_ff", true),
        }),
        "push" => serde_json::json!({
            "remote_name": payload_string(map, "remote_name").unwrap_or_else(|| "origin".to_string()),
            "source_branch": payload_string(map, "source_branch").unwrap_or_else(|| context.task_branch.clone()),
            "target_ref": payload_string(map, "target_ref").unwrap_or_else(|| context.task_branch.clone()),
            "force_mode": payload_string(map, "force_mode").unwrap_or_else(|| "none".to_string()),
        }),
        "rebase" => serde_json::json!({
            "onto_branch": payload_string(map, "onto_branch").unwrap_or_else(|| context.target_branch.clone()),
            "auto_stash": payload_bool(map, "auto_stash", false),
        }),
        "cherry_pick" => {
            let commit_ids = payload_string_array(map, "commit_ids")?;
            if commit_ids.is_empty() {
                return Err("cherry_pick 需要至少一个 commit_ids".to_string());
            }
            serde_json::json!({ "commit_ids": commit_ids })
        }
        "stash" => serde_json::json!({
            "include_untracked": payload_bool(map, "include_untracked", false),
            "message": payload_string(map, "message"),
        }),
        "unstash" => serde_json::json!({
            "stash_ref": payload_string(map, "stash_ref").unwrap_or_else(|| "stash@{0}".to_string()),
        }),
        "cleanup_worktree" => serde_json::json!({
            "delete_branch": payload_bool(map, "delete_branch", false),
            "prune_worktree": payload_bool(map, "prune_worktree", true),
            "force_remove": payload_bool(map, "force_remove", true),
        }),
        _ => return Err("不支持的 git action".to_string()),
    };

    serde_json::to_string(&normalized)
        .map_err(|error| format!("序列化规范化 payload 失败: {}", error))
}

async fn execute_normalized_action<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    repo_path: &str,
    context: &TaskGitContextRecord,
    action_type: &str,
    normalized_payload_json: &str,
) -> Result<String, String> {
    #[cfg(test)]
    {
        if runtime.execution_target == EXECUTION_TARGET_LOCAL {
            let payload: Value = serde_json::from_str(normalized_payload_json)
                .map_err(|error| format!("解析规范化 payload 失败: {}", error))?;
            let map = payload_object(&payload)?;
            return match action_type {
                "merge" => {
                    let target_branch = payload_string(map, "target_branch")
                        .ok_or_else(|| "merge 缺少 target_branch".to_string())?;
                    let strategy =
                        payload_string(map, "strategy").unwrap_or_else(|| "ort".to_string());
                    let allow_ff = payload_bool(map, "allow_ff", true);
                    merge_task_branch_into_target_local(
                        repo_path,
                        context,
                        target_branch.as_str(),
                        strategy.as_str(),
                        allow_ff,
                    )
                }
                "push" => {
                    let remote_name = payload_string(map, "remote_name")
                        .ok_or_else(|| "push 缺少 remote_name".to_string())?;
                    let source_branch = payload_string(map, "source_branch")
                        .ok_or_else(|| "push 缺少 source_branch".to_string())?;
                    let target_ref = payload_string(map, "target_ref")
                        .ok_or_else(|| "push 缺少 target_ref".to_string())?;
                    let force_mode =
                        payload_string(map, "force_mode").unwrap_or_else(|| "none".to_string());
                    let mut args = vec!["push"];
                    if force_mode == "force" {
                        args.push("--force");
                    } else if force_mode == "force_with_lease" {
                        args.push("--force-with-lease");
                    }
                    args.push(remote_name.as_str());
                    let refspec = format!("{source_branch}:{target_ref}");
                    args.push(refspec.as_str());
                    run_git_command(&context.worktree_path, &args)?;
                    Ok(format!("已推送 {} 到 {}", source_branch, target_ref))
                }
                "rebase" => {
                    let onto_branch = payload_string(map, "onto_branch")
                        .ok_or_else(|| "rebase 缺少 onto_branch".to_string())?;
                    let auto_stash = payload_bool(map, "auto_stash", false);
                    let mut args = vec!["rebase"];
                    if auto_stash {
                        args.push("--autostash");
                    }
                    args.push(onto_branch.as_str());
                    run_git_command(&context.worktree_path, &args)?;
                    Ok(format!("已将任务分支 rebase 到 {}", onto_branch))
                }
                "cherry_pick" => {
                    let commit_ids = payload_string_array(map, "commit_ids")?;
                    if commit_ids.is_empty() {
                        return Err("cherry_pick 缺少 commit_ids".to_string());
                    }
                    let mut args = vec!["cherry-pick"];
                    for commit_id in &commit_ids {
                        args.push(commit_id.as_str());
                    }
                    run_git_command(&context.worktree_path, &args)?;
                    Ok("已完成 cherry-pick".to_string())
                }
                "stash" => {
                    let include_untracked = payload_bool(map, "include_untracked", false);
                    let message = payload_string(map, "message");
                    let mut args = vec!["stash", "push"];
                    if include_untracked {
                        args.push("--include-untracked");
                    }
                    if let Some(message) = message.as_deref() {
                        args.push("-m");
                        args.push(message);
                    }
                    run_git_command(&context.worktree_path, &args)?;
                    Ok("已创建 stash".to_string())
                }
                "unstash" => {
                    let stash_ref =
                        payload_string(map, "stash_ref").unwrap_or_else(|| "stash@{0}".to_string());
                    run_git_command(
                        &context.worktree_path,
                        &["stash", "pop", stash_ref.as_str()],
                    )?;
                    Ok(format!("已恢复 {}", stash_ref))
                }
                "cleanup_worktree" => {
                    let delete_branch = payload_bool(map, "delete_branch", false);
                    let prune_worktree = payload_bool(map, "prune_worktree", true);
                    let force_remove = payload_bool(map, "force_remove", true);
                    let mut args = vec!["worktree", "remove", context.worktree_path.as_str()];
                    if force_remove {
                        args.push("--force");
                    }
                    run_git_command(repo_path, &args)?;
                    if delete_branch
                        && git_ref_exists_local(
                            repo_path,
                            &format!("refs/heads/{}", context.task_branch),
                        )
                    {
                        run_git_command(
                            repo_path,
                            &["branch", "-D", context.task_branch.as_str()],
                        )?;
                    }
                    if prune_worktree {
                        run_git_command(repo_path, &["worktree", "prune"])?;
                    }
                    Ok("已清理任务 worktree".to_string())
                }
                _ => Err("不支持的 git action".to_string()),
            };
        }
    }

    git_runtime::execute_action(
        app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        repo_path,
        &context.worktree_path,
        &context.task_branch,
        action_type,
        normalized_payload_json,
    )
    .await
}

#[tauri::command]
pub async fn request_git_action<R: Runtime>(
    app: AppHandle<R>,
    input: RequestGitActionInput,
) -> Result<GitActionRequestResult, String> {
    let pool = sqlite_pool(&app).await?;
    let mut context = fetch_task_git_context_by_id(&pool, &input.task_git_context_id).await?;
    let project = fetch_project_by_id(&pool, &context.project_id).await?;
    let runtime = resolve_project_runtime_context(&project)?;
    let action_type = normalize_action_type(&input.action_type)?;
    let allow_failed_context = action_allows_failed_context(action_type, &input.payload)?;
    if !context_is_healthy(&app, &runtime, &context).await {
        let refreshed = refresh_context_state(&app, &pool, &mut context, &runtime).await?;
        if action_allows_drifted_context(action_type) && refreshed.state == TASK_GIT_STATE_DRIFTED {
            context = fetch_task_git_context_by_id(&pool, &input.task_git_context_id).await?;
        } else {
            return Err(format!(
                "task git context 不可用，当前状态：{}",
                refreshed.state
            ));
        }
    }
    if context.state == TASK_GIT_STATE_PROVISIONING {
        return Err(format!(
            "当前状态不允许 request git action：{}",
            context.state
        ));
    }
    if context.state == TASK_GIT_STATE_FAILED && !allow_failed_context {
        return Err(format!(
            "当前状态不允许 request git action：{}",
            context.state
        ));
    }
    if context.state == TASK_GIT_STATE_COMPLETED && !action_allows_completed_context(action_type) {
        return Err(format!(
            "当前状态不允许 request git action：{}",
            context.state
        ));
    }
    if context.state == TASK_GIT_STATE_DRIFTED && !action_allows_drifted_context(action_type) {
        return Err(format!(
            "当前状态不允许 request git action：{}",
            context.state
        ));
    }

    let normalized_payload_json =
        normalize_git_action_payload(action_type, &context, &input.payload)?;
    let nonce = Uuid::new_v4().to_string();
    let expires_at = sqlite_now_with_offset(PENDING_ACTION_TTL_MINUTES);
    let next_version = context.context_version + 1;
    let repo_revision = if action_allows_drifted_context(action_type) {
        current_head_commit(&app, &runtime, &runtime.repo_path, "HEAD").await?
    } else {
        current_head_commit(&app, &runtime, &context.worktree_path, "HEAD").await?
    };
    let signature = build_pending_action_signature(
        &context.id,
        action_type,
        &normalized_payload_json,
        &nonce,
        &expires_at,
        next_version,
    );
    let token = format!("{}.{}", nonce, signature);

    context.state = TASK_GIT_STATE_ACTION_PENDING.to_string();
    context.context_version = next_version;
    context.pending_action_type = Some(action_type.to_string());
    context.pending_action_token_hash = Some(signature);
    context.pending_action_payload_json = Some(normalized_payload_json);
    context.pending_action_nonce = Some(nonce);
    context.pending_action_requested_at = Some(now_sqlite());
    context.pending_action_expires_at = Some(expires_at.clone());
    context.pending_action_repo_revision = Some(repo_revision);
    context.pending_action_bound_context_version = Some(next_version);
    context.last_error = None;
    context.updated_at = now_sqlite();

    let saved = save_task_git_context(&pool, &context).await?;
    insert_activity_log(
        &pool,
        "git_action_requested",
        &format!("已请求 {} 确认动作", action_type),
        None,
        Some(&saved.task_id),
        Some(&saved.project_id),
    )
    .await?;

    Ok(GitActionRequestResult {
        task_git_context_id: saved.id,
        action_type: action_type.to_string(),
        token,
        expires_at,
        state: saved.state,
        context_version: saved.context_version,
    })
}

#[tauri::command]
pub async fn confirm_git_action<R: Runtime>(
    app: AppHandle<R>,
    task_git_context_id: String,
    token: String,
) -> Result<ConfirmGitActionResult, String> {
    let pool = sqlite_pool(&app).await?;
    let mut context = fetch_task_git_context_by_id(&pool, &task_git_context_id).await?;
    let project = fetch_project_by_id(&pool, &context.project_id).await?;
    let runtime = resolve_project_runtime_context(&project)?;

    if context.pending_action_type.is_none() {
        insert_activity_log(
            &pool,
            "git_action_rejected",
            "未 request 即尝试 confirm git action",
            None,
            Some(&context.task_id),
            Some(&context.project_id),
        )
        .await?;
        return Err("当前没有待确认的 git action".to_string());
    }

    let action_type = context.pending_action_type.clone().unwrap_or_default();
    let payload_json = context
        .pending_action_payload_json
        .clone()
        .ok_or_else(|| "待确认 payload 丢失".to_string())?;
    let nonce = context
        .pending_action_nonce
        .clone()
        .ok_or_else(|| "待确认 nonce 丢失".to_string())?;
    let expires_at = context
        .pending_action_expires_at
        .clone()
        .ok_or_else(|| "待确认过期时间丢失".to_string())?;
    let bound_version = context
        .pending_action_bound_context_version
        .ok_or_else(|| "待确认上下文版本丢失".to_string())?;
    let stored_hash = context
        .pending_action_token_hash
        .clone()
        .ok_or_else(|| "待确认 token hash 丢失".to_string())?;

    let (token_nonce, token_signature) = parse_token(&token)?;
    if token_nonce != nonce || token_signature != stored_hash {
        insert_activity_log(
            &pool,
            "git_action_rejected",
            "git action token 不匹配",
            None,
            Some(&context.task_id),
            Some(&context.project_id),
        )
        .await?;
        return Err("确认 token 不匹配".to_string());
    }

    let expected_hash = build_pending_action_signature(
        &context.id,
        &action_type,
        &payload_json,
        &nonce,
        &expires_at,
        context.context_version,
    );
    if expected_hash != stored_hash {
        reject_pending_action(
            &pool,
            &mut context,
            "token 绑定信息失效，请重新 request",
            false,
        )
        .await?;
        return Err("token 绑定信息已失效".to_string());
    }
    if bound_version != context.context_version {
        reject_pending_action(
            &pool,
            &mut context,
            "context_version 已变化，旧 token 已失效",
            false,
        )
        .await?;
        return Err("context_version 已变化，旧 token 已失效".to_string());
    }
    if expires_at < now_sqlite() {
        reject_pending_action(&pool, &mut context, "git action token 已过期", false).await?;
        return Err("确认 token 已过期".to_string());
    }
    if !action_allows_drifted_context(&action_type)
        && !context_is_healthy(&app, &runtime, &context).await
    {
        reject_pending_action(
            &pool,
            &mut context,
            "任务工作树或任务分支状态异常，不能执行确认",
            true,
        )
        .await?;
        return Err("task git context 不可用，当前状态异常".to_string());
    }

    let current_revision = if action_allows_drifted_context(&action_type) {
        current_head_commit(&app, &runtime, &runtime.repo_path, "HEAD").await?
    } else {
        current_head_commit(&app, &runtime, &context.worktree_path, "HEAD").await?
    };
    if context.pending_action_repo_revision.as_deref() != Some(current_revision.as_str()) {
        reject_pending_action(
            &pool,
            &mut context,
            "仓库 revision 已变化，旧 token 已失效",
            false,
        )
        .await?;
        return Err("仓库 revision 已变化，请重新 request".to_string());
    }

    let execution_result = execute_normalized_action(
        &app,
        &runtime,
        &runtime.repo_path,
        &context,
        &action_type,
        &payload_json,
    )
    .await;
    match execution_result {
        Ok(message) => {
            let cleanup_completed = action_type == "cleanup_worktree";
            let result_message = if cleanup_completed {
                "已清理任务工作树，并移除失效上下文记录".to_string()
            } else {
                message.clone()
            };

            clear_pending_action_fields(&mut context);
            context.context_version += 1;
            context.state = if cleanup_completed || action_type == "merge" {
                TASK_GIT_STATE_COMPLETED.to_string()
            } else {
                TASK_GIT_STATE_MERGE_READY.to_string()
            };
            context.last_error = None;
            context.updated_at = now_sqlite();

            let result_context = if cleanup_completed {
                let summary = TaskGitContextSummary::from(context.clone());
                delete_task_git_context(&pool, &context.id).await?;
                summary
            } else {
                let saved = save_task_git_context(&pool, &context).await?;
                TaskGitContextSummary::from(saved)
            };

            insert_activity_log(
                &pool,
                "git_action_confirmed",
                &result_message,
                None,
                Some(&context.task_id),
                Some(&context.project_id),
            )
            .await?;
            if cleanup_completed {
                insert_activity_log(
                    &pool,
                    "task_worktree_cleanup_completed",
                    "任务工作树已清理，Git 上下文记录已移除",
                    None,
                    Some(&context.task_id),
                    Some(&context.project_id),
                )
                .await?;
            }
            Ok(ConfirmGitActionResult {
                context: result_context,
                action_type,
                message: result_message,
            })
        }
        Err(error) => {
            reject_pending_action(&pool, &mut context, &error, false).await?;
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn cancel_git_action<R: Runtime>(
    app: AppHandle<R>,
    task_git_context_id: String,
    token: Option<String>,
) -> Result<TaskGitContextSummary, String> {
    let pool = sqlite_pool(&app).await?;
    let mut context = fetch_task_git_context_by_id(&pool, &task_git_context_id).await?;
    if context.pending_action_type.is_none() {
        return Ok(TaskGitContextSummary::from(context));
    }
    if let Some(token) = token.as_deref() {
        let (nonce, signature) = parse_token(token)?;
        if context.pending_action_nonce.as_deref() != Some(nonce)
            || context.pending_action_token_hash.as_deref() != Some(signature)
        {
            return Err("取消 token 不匹配".to_string());
        }
    }

    clear_pending_action_fields(&mut context);
    context.context_version += 1;
    context.state = TASK_GIT_STATE_MERGE_READY.to_string();
    context.last_error = None;
    context.updated_at = now_sqlite();
    let saved = save_task_git_context(&pool, &context).await?;
    insert_activity_log(
        &pool,
        "git_action_cancelled",
        "已取消待确认 git action",
        None,
        Some(&saved.task_id),
        Some(&saved.project_id),
    )
    .await?;
    Ok(TaskGitContextSummary::from(saved))
}

