// Domain slice: task_commit (included into git_workflow)

#[derive(Debug, Clone)]
struct ResolvedTaskCommitTarget {
    task: Task,
    project: Project,
    runtime: GitProjectRuntimeContext,
    mode: String,
    working_dir: String,
    task_git_context: Option<TaskGitContextRecord>,
    worktree_missing: bool,
    warning: Option<String>,
}

fn change_is_stageable(change: &ProjectGitWorkingTreeChange) -> bool {
    matches!(
        change.stage_status.as_str(),
        "unstaged" | "untracked" | "partially_staged"
    )
}

fn change_is_staged(change: &ProjectGitWorkingTreeChange) -> bool {
    matches!(
        change.stage_status.as_str(),
        "staged" | "partially_staged"
    )
}

fn change_is_unmerged(change: &ProjectGitWorkingTreeChange) -> bool {
    change.stage_status == "unmerged"
}

fn summarize_working_tree_flags(changes: &[ProjectGitWorkingTreeChange]) -> (bool, bool, bool) {
    let has_stageable = changes.iter().any(change_is_stageable);
    let has_staged = changes.iter().any(change_is_staged);
    let has_unmerged = changes.iter().any(change_is_unmerged);
    (has_stageable, has_staged, has_unmerged)
}

fn collect_staged_change_prompt_lines(changes: &[ProjectGitWorkingTreeChange]) -> Vec<String> {
    changes
        .iter()
        .filter(|change| change_is_staged(change))
        .map(|change| {
            if change.change_type == "renamed" {
                if let Some(previous) = change.previous_path.as_deref() {
                    return format!("重命名 {previous} -> {}", change.path);
                }
            }
            let label = match change.change_type.as_str() {
                "added" => "新增",
                "deleted" => "删除",
                "renamed" => "重命名",
                _ => "修改",
            };
            format!("{label} {}", change.path)
        })
        .collect()
}

async fn resolve_task_commit_target<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
) -> Result<ResolvedTaskCommitTarget, String> {
    let pool = sqlite_pool(app).await?;
    let task = fetch_task_by_id(&pool, task_id).await?;
    let project = fetch_project_by_id(&pool, &task.project_id).await?;
    let runtime = resolve_project_runtime_context(&project)?;
    let context = fetch_task_git_context_by_task_id(&pool, task_id).await?;

    if task.use_worktree {
        if let Some(context) = context {
            let missing = !worktree_path_exists(app, &runtime, &context.worktree_path).await;
            if missing {
                return Ok(ResolvedTaskCommitTarget {
                    task,
                    project,
                    runtime,
                    mode: TASK_COMMIT_MODE_WORKTREE.to_string(),
                    working_dir: context.worktree_path.clone(),
                    task_git_context: Some(context),
                    worktree_missing: true,
                    warning: Some("任务 Worktree 路径不存在，无法提交".to_string()),
                });
            }
            return Ok(ResolvedTaskCommitTarget {
                task,
                project,
                runtime,
                mode: TASK_COMMIT_MODE_WORKTREE.to_string(),
                working_dir: context.worktree_path.clone(),
                task_git_context: Some(context),
                worktree_missing: false,
                warning: None,
            });
        }
        return Ok(ResolvedTaskCommitTarget {
            task,
            project,
            runtime: runtime.clone(),
            mode: TASK_COMMIT_MODE_PROJECT_REPO.to_string(),
            working_dir: runtime.repo_path.clone(),
            task_git_context: None,
            worktree_missing: false,
            warning: Some(
                "任务已开启 Worktree 但尚未准备上下文，将使用项目主仓库提交（可能包含其他改动）"
                    .to_string(),
            ),
        });
    }

    Ok(ResolvedTaskCommitTarget {
        task,
        project,
        runtime: runtime.clone(),
        mode: TASK_COMMIT_MODE_PROJECT_REPO.to_string(),
        working_dir: runtime.repo_path.clone(),
        task_git_context: None,
        worktree_missing: false,
        warning: Some("非 Worktree 任务将提交项目主仓库当前工作区全部改动".to_string()),
    })
}

async fn collect_task_commit_overview_from_target<R: Runtime>(
    app: &AppHandle<R>,
    target: &ResolvedTaskCommitTarget,
) -> Result<TaskCommitOverview, String> {
    if target.worktree_missing {
        return Err(target
            .warning
            .clone()
            .unwrap_or_else(|| "任务 Worktree 不可用".to_string()));
    }

    let overview = collect_git_overview_for_dir(app, &target.runtime, &target.working_dir, 1).await?;
    let working_tree_changes =
        collect_working_tree_changes(app, &target.runtime, &target.working_dir).await?;
    let has_unmerged = working_tree_changes.iter().any(change_is_unmerged);

    Ok(TaskCommitOverview {
        task_id: target.task.id.clone(),
        project_id: target.project.id.clone(),
        mode: target.mode.clone(),
        task_git_context_id: target
            .task_git_context
            .as_ref()
            .map(|context| context.id.clone()),
        working_dir: target.working_dir.clone(),
        execution_target: target.runtime.execution_target.clone(),
        current_branch: overview.current_branch,
        working_tree_summary: overview.working_tree_summary,
        working_tree_changes,
        has_unmerged,
        warning: target.warning.clone(),
        refreshed_at: now_sqlite(),
    })
}

fn compute_commit_action_flags(
    target: &ResolvedTaskCommitTarget,
    has_stageable: bool,
    has_staged: bool,
    has_unmerged: bool,
) -> (bool, bool, bool, Vec<String>) {
    let mut warnings = Vec::new();
    if let Some(warning) = target.warning.clone() {
        warnings.push(warning);
    }

    if target.worktree_missing {
        return (false, false, false, warnings);
    }

    let git_state = target
        .task_git_context
        .as_ref()
        .map(|context| context.state.as_str());

    let blocked_git_state = matches!(
        git_state,
        Some(TASK_GIT_STATE_FAILED)
            | Some(TASK_GIT_STATE_DRIFTED)
            | Some(TASK_GIT_STATE_ACTION_PENDING)
            | Some(TASK_GIT_STATE_COMPLETED)
    );

    let has_committable = has_stageable || has_staged || has_unmerged;
    let can_commit = has_committable && !blocked_git_state;
    let can_ai_commit = can_commit;
    let can_merge = target.mode == TASK_COMMIT_MODE_WORKTREE
        && !target.worktree_missing
        && target.task_git_context.is_some()
        && !matches!(
            git_state,
            Some(TASK_GIT_STATE_FAILED) | Some(TASK_GIT_STATE_DRIFTED) | Some(TASK_GIT_STATE_COMPLETED)
        );

    (can_commit, can_ai_commit, can_merge, warnings)
}

#[tauri::command]
pub async fn get_task_commit_action_state<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<TaskCommitActionState, String> {
    let target = match resolve_task_commit_target(&app, &task_id).await {
        Ok(target) => target,
        Err(error) => {
            return Ok(TaskCommitActionState {
                task_id,
                project_id: String::new(),
                mode: TASK_COMMIT_MODE_PROJECT_REPO.to_string(),
                task_git_context_id: None,
                working_dir: None,
                execution_target: None,
                current_branch: None,
                git_context_state: None,
                worktree_missing: false,
                has_stageable: false,
                has_staged: false,
                has_unmerged: false,
                can_commit: false,
                can_ai_commit: false,
                can_merge: false,
                warnings: Vec::new(),
                error: Some(error),
            });
        }
    };

    if target.worktree_missing {
        let (can_commit, can_ai_commit, can_merge, warnings) =
            compute_commit_action_flags(&target, false, false, false);
        return Ok(TaskCommitActionState {
            task_id: target.task.id,
            project_id: target.project.id,
            mode: target.mode,
            task_git_context_id: target.task_git_context.as_ref().map(|c| c.id.clone()),
            working_dir: Some(target.working_dir),
            execution_target: Some(target.runtime.execution_target),
            current_branch: None,
            git_context_state: target.task_git_context.as_ref().map(|c| c.state.clone()),
            worktree_missing: true,
            has_stageable: false,
            has_staged: false,
            has_unmerged: false,
            can_commit,
            can_ai_commit,
            can_merge,
            warnings,
            error: target.warning,
        });
    }

    let overview = match collect_task_commit_overview_from_target(&app, &target).await {
        Ok(overview) => overview,
        Err(error) => {
            return Ok(TaskCommitActionState {
                task_id: target.task.id,
                project_id: target.project.id,
                mode: target.mode,
                task_git_context_id: target.task_git_context.as_ref().map(|c| c.id.clone()),
                working_dir: Some(target.working_dir),
                execution_target: Some(target.runtime.execution_target),
                current_branch: None,
                git_context_state: target.task_git_context.as_ref().map(|c| c.state.clone()),
                worktree_missing: target.worktree_missing,
                has_stageable: false,
                has_staged: false,
                has_unmerged: false,
                can_commit: false,
                can_ai_commit: false,
                can_merge: false,
                warnings: target.warning.into_iter().collect(),
                error: Some(error),
            });
        }
    };

    let (has_stageable, has_staged, has_unmerged) =
        summarize_working_tree_flags(&overview.working_tree_changes);
    let (can_commit, can_ai_commit, can_merge, warnings) =
        compute_commit_action_flags(&target, has_stageable, has_staged, has_unmerged);

    Ok(TaskCommitActionState {
        task_id: target.task.id,
        project_id: target.project.id,
        mode: target.mode,
        task_git_context_id: overview.task_git_context_id,
        working_dir: Some(overview.working_dir),
        execution_target: Some(overview.execution_target),
        current_branch: overview.current_branch,
        git_context_state: target.task_git_context.as_ref().map(|c| c.state.clone()),
        worktree_missing: target.worktree_missing,
        has_stageable,
        has_staged,
        has_unmerged,
        can_commit,
        can_ai_commit,
        can_merge,
        warnings,
        error: None,
    })
}

#[tauri::command]
pub async fn get_task_commit_overview<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<TaskCommitOverview, String> {
    let target = resolve_task_commit_target(&app, &task_id).await?;
    collect_task_commit_overview_from_target(&app, &target).await
}

#[tauri::command]
pub async fn stage_all_task_commit_files<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<String, String> {
    let pool = sqlite_pool(&app).await?;
    let target = resolve_task_commit_target(&app, &task_id).await?;
    if target.worktree_missing {
        return Err(target
            .warning
            .unwrap_or_else(|| "任务 Worktree 不可用".to_string()));
    }

    let result = git_runtime::stage_all(
        &app,
        &target.runtime.execution_target,
        target.runtime.ssh_config_id.as_deref(),
        &target.working_dir,
    )
    .await?;

    let action = if target.mode == TASK_COMMIT_MODE_WORKTREE {
        "task_git_stage_all"
    } else {
        "task_project_git_stage_all"
    };
    insert_activity_log(
        &pool,
        action,
        &result,
        None,
        Some(target.task.id.as_str()),
        Some(target.project.id.as_str()),
    )
    .await?;
    Ok(result)
}

async fn commit_resolved_task_target<R: Runtime>(
    app: &AppHandle<R>,
    target: &mut ResolvedTaskCommitTarget,
    message: &str,
    recover_automation: bool,
) -> Result<String, String> {
    if target.worktree_missing {
        return Err(target
            .warning
            .clone()
            .unwrap_or_else(|| "任务 Worktree 不可用".to_string()));
    }

    let pool = sqlite_pool(app).await?;
    let result = git_runtime::commit_changes(
        app,
        &target.runtime.execution_target,
        target.runtime.ssh_config_id.as_deref(),
        &target.working_dir,
        message,
    )
    .await?;

    if target.mode == TASK_COMMIT_MODE_WORKTREE {
        if let Some(context) = target.task_git_context.as_mut() {
            insert_activity_log(
                &pool,
                "task_git_committed",
                &result,
                None,
                Some(target.task.id.as_str()),
                Some(target.project.id.as_str()),
            )
            .await?;
            update_task_git_context_merge_ready(
                &pool,
                context,
                "任务代码已提交，等待合并到目标分支",
            )
            .await?;
            if recover_automation {
                task_automation::mark_task_automation_commit_completed(
                    app,
                    &pool,
                    &target.task,
                    &result,
                )
                .await?;
            }
            return Ok(result);
        }
    }

    insert_activity_log(
        &pool,
        "task_project_git_committed",
        &result,
        None,
        Some(target.task.id.as_str()),
        Some(target.project.id.as_str()),
    )
    .await?;
    if recover_automation {
        let _ = task_automation::mark_task_automation_commit_completed(
            app,
            &pool,
            &target.task,
            &result,
        )
        .await;
    }
    Ok(result)
}

#[tauri::command]
pub async fn commit_task_changes<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
    message: String,
) -> Result<String, String> {
    let trimmed = message.trim().to_string();
    if trimmed.is_empty() {
        return Err("提交说明不能为空".to_string());
    }
    let mut target = resolve_task_commit_target(&app, &task_id).await?;
    commit_resolved_task_target(&app, &mut target, &trimmed, true).await
}

fn strip_code_fence(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        let mut lines = rest.lines();
        let _ = lines.next();
        let body: Vec<&str> = lines.collect();
        if let Some(last) = body.last() {
            if last.trim() == "```" {
                return body[..body.len().saturating_sub(1)].join("\n");
            }
        }
        return body.join("\n");
    }
    trimmed.to_string()
}

fn content_still_has_conflict_markers(content: &str) -> bool {
    content.contains("<<<<<<<") || content.contains(">>>>>>>")
}

async fn ai_resolve_conflicts_in_dir<R: Runtime>(
    app: &AppHandle<R>,
    task: &Task,
    project: &Project,
    runtime: &GitProjectRuntimeContext,
    working_dir: &str,
    phase: &str,
) -> Result<TaskAiConflictResolveResult, String> {
    let pool = sqlite_pool(app).await?;
    insert_activity_log(
        &pool,
        "task_ai_conflict_resolve_started",
        &format!("phase={phase}; dir={working_dir}"),
        None,
        Some(task.id.as_str()),
        Some(project.id.as_str()),
    )
    .await?;

    let mut resolved_files = Vec::new();
    for attempt in 0..2 {
        let changes = collect_working_tree_changes(app, runtime, working_dir).await?;
        let unmerged: Vec<String> = changes
            .into_iter()
            .filter(change_is_unmerged)
            .map(|change| change.path)
            .collect();
        if unmerged.is_empty() {
            break;
        }

        for relative_path in &unmerged {
            let snapshot = git_runtime::capture_worktree_text_snapshot(
                app,
                &runtime.execution_target,
                runtime.ssh_config_id.as_deref(),
                working_dir,
                relative_path,
            )
            .await?;
            if snapshot.status != "text" {
                let detail = format!(
                    "冲突文件 {} 无法以文本方式读取（status={}），请人工处理",
                    relative_path, snapshot.status
                );
                insert_activity_log(
                    &pool,
                    "task_ai_conflict_resolve_failed",
                    &detail,
                    None,
                    Some(task.id.as_str()),
                    Some(project.id.as_str()),
                )
                .await?;
                return Err(detail);
            }
            let content = snapshot.text.unwrap_or_default();
            let prompt = format!(
                "你是 Git 冲突解决助手。请解决下面文件中的合并冲突，输出完整文件内容。\n\
规则：\n\
1. 只输出最终文件正文，不要 markdown 代码围栏，不要解释。\n\
2. 删除所有冲突标记（<<<<<<< ======= >>>>>>>）。\n\
3. 保留合理业务逻辑，不要重构无关代码。\n\
4. 若双方改动都需要，做最小化合并。\n\
文件路径：{relative_path}\n\
当前内容：\n{content}\n"
            );

            let raw = crate::codex::process::run_ai_command(
                app,
                prompt,
                None,
                Some(task.id.clone()),
                Some(project.id.clone()),
                Some(working_dir.to_string()),
                None,
                None,
                None,
                None,
            )
            .await
            .map_err(|error| {
                format!("AI 解冲突失败（{relative_path}）：{error}")
            })?;

            let resolved = strip_code_fence(&raw);
            if content_still_has_conflict_markers(&resolved) {
                if attempt == 0 {
                    continue;
                }
                let detail = format!("AI 未能清除冲突标记：{relative_path}，请人工处理");
                insert_activity_log(
                    &pool,
                    "task_ai_conflict_resolve_failed",
                    &detail,
                    None,
                    Some(task.id.as_str()),
                    Some(project.id.as_str()),
                )
                .await?;
                return Err(detail);
            }

            git_runtime::write_text_file(
                app,
                &runtime.execution_target,
                runtime.ssh_config_id.as_deref(),
                working_dir,
                relative_path,
                &resolved,
            )
            .await?;
            if !resolved_files.iter().any(|path| path == relative_path) {
                resolved_files.push(relative_path.clone());
            }
        }

        if !resolved_files.is_empty() {
            git_runtime::stage_paths(
                app,
                &runtime.execution_target,
                runtime.ssh_config_id.as_deref(),
                working_dir,
                &resolved_files,
            )
            .await?;
        }

        let still_unmerged = collect_working_tree_changes(app, runtime, working_dir)
            .await?
            .into_iter()
            .any(|change| change_is_unmerged(&change));
        if !still_unmerged {
            break;
        }
        if attempt == 1 {
            let detail = "AI 解冲突后仍有未合并文件，请人工处理".to_string();
            insert_activity_log(
                &pool,
                "task_ai_conflict_resolve_failed",
                &detail,
                None,
                Some(task.id.as_str()),
                Some(project.id.as_str()),
            )
            .await?;
            return Err(detail);
        }
    }

    let remaining_unmerged = collect_working_tree_changes(app, runtime, working_dir)
        .await?
        .into_iter()
        .any(|change| change_is_unmerged(&change));
    if remaining_unmerged {
        let detail = "仍存在未合并冲突文件".to_string();
        insert_activity_log(
            &pool,
            "task_ai_conflict_resolve_failed",
            &detail,
            None,
            Some(task.id.as_str()),
            Some(project.id.as_str()),
        )
        .await?;
        return Err(detail);
    }

    let mut merge_completed = false;
    if phase == "post_merge"
        && git_runtime::complete_merge_commit(
            app,
            &runtime.execution_target,
            runtime.ssh_config_id.as_deref(),
            working_dir,
            Some("chore: resolve merge conflicts with AI"),
        )
        .await
        .is_ok()
    {
        merge_completed = true;
    }

    let detail = if resolved_files.is_empty() {
        "未发现冲突文件".to_string()
    } else {
        format!("已解决 {} 个冲突文件", resolved_files.len())
    };
    insert_activity_log(
        &pool,
        "task_ai_conflict_resolve_completed",
        &detail,
        None,
        Some(task.id.as_str()),
        Some(project.id.as_str()),
    )
    .await?;

    Ok(TaskAiConflictResolveResult {
        task_id: task.id.clone(),
        working_dir: working_dir.to_string(),
        resolved_files,
        detail,
        merge_completed,
    })
}

#[tauri::command]
pub async fn ai_resolve_task_git_conflicts<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
    phase: Option<String>,
    working_dir_override: Option<String>,
) -> Result<TaskAiConflictResolveResult, String> {
    let target = resolve_task_commit_target(&app, &task_id).await?;
    if target.worktree_missing {
        return Err(target
            .warning
            .unwrap_or_else(|| "任务 Worktree 不可用".to_string()));
    }
    let phase = phase
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("pre_commit");
    let working_dir = working_dir_override
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(target.working_dir.as_str());

    ai_resolve_conflicts_in_dir(
        &app,
        &target.task,
        &target.project,
        &target.runtime,
        working_dir,
        phase,
    )
    .await
}

#[tauri::command]
pub async fn ai_commit_task_changes<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<TaskAiCommitResult, String> {
    let pool = sqlite_pool(&app).await?;
    let mut target = resolve_task_commit_target(&app, &task_id).await?;
    if target.worktree_missing {
        return Err(target
            .warning
            .unwrap_or_else(|| "任务 Worktree 不可用".to_string()));
    }

    insert_activity_log(
        &pool,
        "task_ai_commit_started",
        &format!("mode={}", target.mode),
        None,
        Some(target.task.id.as_str()),
        Some(target.project.id.as_str()),
    )
    .await?;

    let mut conflict_resolved = false;
    let overview = collect_task_commit_overview_from_target(&app, &target).await?;
    if overview.has_unmerged {
        match ai_resolve_conflicts_in_dir(
            &app,
            &target.task,
            &target.project,
            &target.runtime,
            &target.working_dir,
            "pre_commit",
        )
        .await
        {
            Ok(resolve) => {
                conflict_resolved = !resolve.resolved_files.is_empty();
            }
            Err(error) => {
                insert_activity_log(
                    &pool,
                    "task_ai_commit_failed",
                    &error,
                    None,
                    Some(target.task.id.as_str()),
                    Some(target.project.id.as_str()),
                )
                .await?;
                return Err(error);
            }
        }
    }

    if let Err(error) = git_runtime::stage_all(
        &app,
        &target.runtime.execution_target,
        target.runtime.ssh_config_id.as_deref(),
        &target.working_dir,
    )
    .await
    {
        insert_activity_log(
            &pool,
            "task_ai_commit_failed",
            &error,
            None,
            Some(target.task.id.as_str()),
            Some(target.project.id.as_str()),
        )
        .await?;
        return Err(error);
    }

    let overview = collect_task_commit_overview_from_target(&app, &target).await?;
    let staged_prompts = collect_staged_change_prompt_lines(&overview.working_tree_changes);
    if staged_prompts.is_empty() {
        let detail = "没有可提交的已暂存改动".to_string();
        insert_activity_log(
            &pool,
            "task_ai_commit_failed",
            &detail,
            None,
            Some(target.task.id.as_str()),
            Some(target.project.id.as_str()),
        )
        .await?;
        return Err(detail);
    }

    let generated = match generate_commit_message_for_project(
        &app,
        &target.project.id,
        overview.current_branch.as_deref(),
        overview.working_tree_summary.as_deref(),
        &staged_prompts,
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            insert_activity_log(
                &pool,
                "task_ai_commit_failed",
                &error,
                None,
                Some(target.task.id.as_str()),
                Some(target.project.id.as_str()),
            )
            .await?;
            return Err(error);
        }
    };

    let detail = match commit_resolved_task_target(&app, &mut target, &generated.message, true).await
    {
        Ok(value) => value,
        Err(error) => {
            insert_activity_log(
                &pool,
                "task_ai_commit_failed",
                &error,
                None,
                Some(target.task.id.as_str()),
                Some(target.project.id.as_str()),
            )
            .await?;
            return Err(error);
        }
    };

    insert_activity_log(
        &pool,
        "task_ai_commit_completed",
        &detail,
        None,
        Some(target.task.id.as_str()),
        Some(target.project.id.as_str()),
    )
    .await?;

    Ok(TaskAiCommitResult {
        task_id: target.task.id,
        mode: target.mode.clone(),
        message: generated.message,
        detail,
        conflict_resolved,
        merge_ready: target.mode == TASK_COMMIT_MODE_WORKTREE,
    })
}
