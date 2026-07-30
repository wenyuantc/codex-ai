// Domain slice: project_ops (included into git_workflow)

fn normalize_project_git_commit_history_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(PROJECT_GIT_COMMIT_HISTORY_PAGE_LIMIT_DEFAULT)
        .clamp(1, PROJECT_GIT_COMMIT_HISTORY_PAGE_LIMIT_MAX)
}

fn normalize_project_git_relative_path(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_project_git_change_type(value: Option<String>) -> String {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "modified".to_string())
}

fn build_project_git_file_preview_message(
    before_label: &str,
    after_label: &str,
    before_snapshot: &git_runtime::GitRuntimeTextSnapshot,
    after_snapshot: &git_runtime::GitRuntimeTextSnapshot,
) -> Option<String> {
    if before_snapshot.status == "binary" || after_snapshot.status == "binary" {
        Some("当前变更包含二进制文件，Diff 仅支持文本预览".to_string())
    } else if before_snapshot.status == "unavailable" || after_snapshot.status == "unavailable" {
        Some("当前目标不是普通文本文件，暂不支持完整 Diff 预览".to_string())
    } else if before_snapshot.truncated || after_snapshot.truncated {
        Some("文件内容较长，当前只展示截断后的 Diff 预览".to_string())
    } else if before_snapshot.status == "missing" && after_snapshot.status == "missing" {
        Some(format!(
            "当前文件在 {} 和 {} 中都不可用，无法生成 Diff",
            before_label, after_label
        ))
    } else {
        None
    }
}

fn build_project_git_file_preview(
    project_id: String,
    execution_target: String,
    repo_path: &str,
    relative_path: &str,
    previous_path: Option<String>,
    change_type: String,
    before_label: String,
    after_label: String,
    before_snapshot: git_runtime::GitRuntimeTextSnapshot,
    after_snapshot: git_runtime::GitRuntimeTextSnapshot,
) -> ProjectGitFilePreview {
    let absolute_path = Some(
        Path::new(repo_path)
            .join(relative_path)
            .to_string_lossy()
            .to_string(),
    );
    let previous_absolute_path = previous_path.as_ref().map(|path| {
        Path::new(repo_path)
            .join(path)
            .to_string_lossy()
            .to_string()
    });
    let message = build_project_git_file_preview_message(
        &before_label,
        &after_label,
        &before_snapshot,
        &after_snapshot,
    );

    ProjectGitFilePreview {
        project_id,
        relative_path: relative_path.to_string(),
        previous_path,
        absolute_path,
        previous_absolute_path,
        execution_target,
        change_type,
        before_label,
        before_status: before_snapshot.status,
        before_text: before_snapshot.text,
        before_truncated: before_snapshot.truncated,
        after_label,
        after_status: after_snapshot.status,
        after_text: after_snapshot.text,
        after_truncated: after_snapshot.truncated,
        message,
    }
}

fn missing_project_git_text_snapshot() -> git_runtime::GitRuntimeTextSnapshot {
    git_runtime::GitRuntimeTextSnapshot {
        status: "missing".to_string(),
        text: None,
        truncated: false,
    }
}

fn build_project_git_commit_preview_labels(commit_sha: &str) -> (String, String) {
    let short_sha = commit_sha.chars().take(7).collect::<String>();
    let commit_label = if short_sha.is_empty() {
        commit_sha.to_string()
    } else {
        short_sha
    };
    ("父提交".to_string(), format!("当前提交 {commit_label}"))
}

#[cfg(test)]
fn git_working_tree_is_clean(repo_path: &str) -> Result<bool, String> {
    Ok(run_git_text(repo_path, &["status", "--porcelain"])?
        .trim()
        .is_empty())
}

fn build_working_tree_changes(
    repo_path: &str,
    execution_target: &str,
    changes: Vec<git_runtime::GitRuntimeChange>,
) -> Vec<ProjectGitWorkingTreeChange> {
    changes
        .into_iter()
        .map(|change| ProjectGitWorkingTreeChange {
            can_open_file: change.change_type != "deleted"
                && (execution_target != EXECUTION_TARGET_LOCAL
                    || can_open_repo_file_locally(repo_path, &change.path)),
            path: change.path,
            previous_path: change.previous_path,
            change_type: change.change_type,
            stage_status: change.stage_status,
        })
        .collect()
}

async fn collect_working_tree_changes<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    working_dir: &str,
) -> Result<Vec<ProjectGitWorkingTreeChange>, String> {
    let worktree_runtime = runtime.with_repo_path(working_dir);
    let changes = git_runtime::collect_status_changes(
        app,
        &worktree_runtime.execution_target,
        worktree_runtime.ssh_config_id.as_deref(),
        &worktree_runtime.repo_path,
    )
    .await?;
    Ok(build_working_tree_changes(
        &worktree_runtime.repo_path,
        &worktree_runtime.execution_target,
        changes,
    ))
}

async fn collect_git_overview_for_dir<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    working_dir: &str,
    recent_commit_limit: usize,
) -> Result<git_runtime::GitRuntimeOverview, String> {
    let worktree_runtime = runtime.with_repo_path(working_dir);
    git_runtime::collect_git_overview(
        app,
        &worktree_runtime.execution_target,
        worktree_runtime.ssh_config_id.as_deref(),
        &worktree_runtime.repo_path,
        recent_commit_limit,
    )
    .await
}

#[tauri::command]
pub async fn get_project_git_overview<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<ProjectGitOverview, String> {
    let pool = sqlite_pool(&app).await?;
    let project = fetch_project_by_id(&pool, &project_id).await?;
    let rows = sqlx::query_as::<_, TaskGitContextRecord>(
        "SELECT * FROM task_git_contexts WHERE project_id = $1 ORDER BY updated_at DESC, created_at DESC",
    )
    .bind(&project_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("查询项目 Git 上下文失败: {}", error))?;

    let runtime = match resolve_project_runtime_context(&project) {
        Ok(runtime) => runtime,
        Err(error) => {
            let active_contexts = rows
                .iter()
                .filter(|row| row.state != TASK_GIT_STATE_COMPLETED)
                .cloned()
                .map(TaskGitContextSummary::from)
                .collect::<Vec<_>>();
            let pending_action_contexts = rows
                .iter()
                .filter(|row| {
                    row.pending_action_type.is_some() || row.state == TASK_GIT_STATE_ACTION_PENDING
                })
                .cloned()
                .map(TaskGitContextSummary::from)
                .collect::<Vec<_>>();
            return Ok(ProjectGitOverview {
                project_id: project.id,
                repo_path: project.repo_path.or(project.remote_repo_path),
                execution_target: if project.project_type == PROJECT_TYPE_SSH {
                    EXECUTION_TARGET_SSH.to_string()
                } else {
                    EXECUTION_TARGET_LOCAL.to_string()
                },
                git_runtime_provider: GIT_RUNTIME_PROVIDER_SIMPLE_GIT.to_string(),
                git_runtime_status: GIT_RUNTIME_STATUS_UNAVAILABLE.to_string(),
                git_runtime_message: Some(error),
                default_branch: None,
                current_branch: None,
                project_branches: Vec::new(),
                head_commit_sha: None,
                working_tree_summary: None,
                ahead_commits: None,
                behind_commits: None,
                working_tree_changes: Vec::new(),
                refreshed_at: now_sqlite(),
                recent_commits: Vec::new(),
                recent_commits_has_more: false,
                active_contexts,
                pending_action_contexts,
            });
        }
    };
    let mut active_contexts = Vec::new();
    let mut pending_action_contexts = Vec::new();
    for row in rows {
        let is_active = row.state != TASK_GIT_STATE_COMPLETED;
        let is_pending =
            row.pending_action_type.is_some() || row.state == TASK_GIT_STATE_ACTION_PENDING;
        let mut summary = TaskGitContextSummary::from(row);
        if runtime.execution_target == EXECUTION_TARGET_LOCAL {
            summary.worktree_missing = !Path::new(&summary.worktree_path).exists();
        }
        if is_active {
            active_contexts.push(summary.clone());
        }
        if is_pending {
            pending_action_contexts.push(summary);
        }
    }

    match git_runtime::collect_git_overview(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        PROJECT_GIT_RECENT_COMMIT_SUMMARY_LIMIT,
    )
    .await
    {
        Ok(overview) => {
            let working_tree_changes = build_working_tree_changes(
                &runtime.repo_path,
                &runtime.execution_target,
                overview.working_tree_changes,
            );

            Ok(ProjectGitOverview {
                project_id: project.id,
                repo_path: Some(runtime.repo_path),
                execution_target: runtime.execution_target,
                git_runtime_provider: GIT_RUNTIME_PROVIDER_SIMPLE_GIT.to_string(),
                git_runtime_status: GIT_RUNTIME_STATUS_READY.to_string(),
                git_runtime_message: None,
                default_branch: Some(overview.default_branch),
                current_branch: overview.current_branch,
                project_branches: overview.project_branches,
                head_commit_sha: Some(overview.head_commit_sha),
                working_tree_summary: overview.working_tree_summary,
                ahead_commits: overview.ahead_commits,
                behind_commits: overview.behind_commits,
                working_tree_changes,
                refreshed_at: now_sqlite(),
                recent_commits: overview
                    .recent_commits
                    .into_iter()
                    .map(|commit| ProjectGitCommit {
                        sha: commit.sha,
                        short_sha: commit.short_sha,
                        subject: commit.subject,
                        author_name: commit.author_name,
                        authored_at: commit.authored_at,
                    })
                    .collect(),
                recent_commits_has_more: overview.recent_commits_has_more,
                active_contexts,
                pending_action_contexts,
            })
        }
        Err(error) => Ok(ProjectGitOverview {
            project_id: project.id,
            repo_path: Some(runtime.repo_path),
            execution_target: runtime.execution_target,
            git_runtime_provider: GIT_RUNTIME_PROVIDER_SIMPLE_GIT.to_string(),
            git_runtime_status: GIT_RUNTIME_STATUS_UNAVAILABLE.to_string(),
            git_runtime_message: Some(error),
            default_branch: None,
            current_branch: None,
            project_branches: Vec::new(),
            head_commit_sha: None,
            working_tree_summary: None,
            ahead_commits: None,
            behind_commits: None,
            working_tree_changes: Vec::new(),
            refreshed_at: now_sqlite(),
            recent_commits: Vec::new(),
            recent_commits_has_more: false,
            active_contexts,
            pending_action_contexts,
        }),
    }
}

#[tauri::command]
pub async fn list_project_git_commits<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<ProjectGitCommitHistory, String> {
    let offset = offset.unwrap_or(0);
    let limit = normalize_project_git_commit_history_limit(limit);
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;
    let history = git_runtime::collect_commit_history(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        offset,
        limit,
    )
    .await?;

    if offset == 0 {
        insert_activity_log(
            &pool,
            "project_git_commit_history_viewed",
            &format!("浏览项目提交历史：最近 {} 条", history.commits.len()),
            None,
            None,
            Some(&project.id),
        )
        .await?;
    }

    Ok(ProjectGitCommitHistory {
        commits: history
            .commits
            .into_iter()
            .map(|commit| ProjectGitCommit {
                sha: commit.sha,
                short_sha: commit.short_sha,
                subject: commit.subject,
                author_name: commit.author_name,
                authored_at: commit.authored_at,
            })
            .collect(),
        has_more: history.has_more,
    })
}

#[tauri::command]
pub async fn get_project_git_commit_detail<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    commit_sha: String,
) -> Result<ProjectGitCommitDetail, String> {
    let trimmed_commit_sha = commit_sha.trim().to_string();
    if trimmed_commit_sha.is_empty() {
        return Err("提交 SHA 不能为空".to_string());
    }

    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;
    let detail = git_runtime::collect_commit_detail(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        &trimmed_commit_sha,
    )
    .await?;

    insert_activity_log(
        &pool,
        "project_git_commit_detail_viewed",
        &format!("查看提交详情：{} {}", detail.short_sha, detail.subject),
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(ProjectGitCommitDetail {
        project_id: project.id,
        execution_target: runtime.execution_target,
        sha: detail.sha,
        short_sha: detail.short_sha,
        subject: detail.subject,
        body: detail.body,
        author_name: detail.author_name,
        author_email: detail.author_email,
        authored_at: detail.authored_at,
        diff_text: detail.diff_text,
        diff_truncated: detail.diff_truncated,
        changed_files: detail
            .changed_files
            .into_iter()
            .map(|change| ProjectGitCommitFileChange {
                path: change.path,
                previous_path: change.previous_path,
                change_type: change.change_type,
            })
            .collect(),
    })
}

#[tauri::command]
pub async fn open_project_git_file<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    relative_path: String,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let project = fetch_project_by_id(&pool, &project_id).await?;
    let runtime = resolve_project_runtime_context(&project)?;
    if runtime.execution_target != EXECUTION_TARGET_LOCAL {
        return Err("SSH 项目暂不支持直接浏览远程文件".to_string());
    }

    let trimmed = relative_path.trim();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }

    let repo_root = Path::new(&runtime.repo_path)
        .canonicalize()
        .map_err(|error| format!("解析项目仓库路径失败: {}", error))?;
    let target = repo_root.join(trimmed);
    let canonical_target = target
        .canonicalize()
        .map_err(|error| format!("定位工作区文件失败: {}", error))?;
    if !canonical_target.starts_with(&repo_root) {
        return Err("文件路径超出当前仓库范围".to_string());
    }
    if !canonical_target.is_file() {
        return Err("当前文件不存在或不是普通文件".to_string());
    }

    app.opener()
        .open_path(canonical_target.to_string_lossy().to_string(), None::<&str>)
        .map_err(|error| format!("打开工作区文件失败: {}", error))?;

    insert_activity_log(
        &pool,
        "project_git_file_opened",
        &format!("浏览工作区文件：{}", trimmed),
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(())
}

#[tauri::command]
pub async fn get_project_git_file_preview<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    relative_path: String,
    previous_path: Option<String>,
    change_type: Option<String>,
) -> Result<ProjectGitFilePreview, String> {
    let pool = sqlite_pool(&app).await?;
    let project = fetch_project_by_id(&pool, &project_id).await?;
    let runtime = resolve_project_runtime_context(&project)?;
    let trimmed = relative_path.trim().to_string();
    let preview = file_preview_in_dir(
        &app,
        &project.id,
        &runtime,
        &runtime.repo_path,
        &trimmed,
        previous_path,
        change_type,
    )
    .await?;

    insert_activity_log(
        &pool,
        "project_git_file_previewed",
        &format!("预览工作区文件：{}", trimmed),
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(preview)
}

#[tauri::command]
pub async fn get_project_git_commit_file_preview<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    commit_sha: String,
    relative_path: String,
    previous_path: Option<String>,
    change_type: Option<String>,
) -> Result<ProjectGitFilePreview, String> {
    let trimmed_commit_sha = commit_sha.trim().to_string();
    if trimmed_commit_sha.is_empty() {
        return Err("提交 SHA 不能为空".to_string());
    }

    let pool = sqlite_pool(&app).await?;
    let project = fetch_project_by_id(&pool, &project_id).await?;
    let runtime = resolve_project_runtime_context(&project)?;
    let trimmed = relative_path.trim();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }

    let normalized_previous_path = normalize_project_git_relative_path(previous_path);
    let normalized_change_type = normalize_project_git_change_type(change_type);
    let parent_revision = format!("{trimmed_commit_sha}^1");
    let before_path = if normalized_change_type == "renamed" {
        normalized_previous_path.as_deref().unwrap_or(trimmed)
    } else {
        trimmed
    };
    let before_snapshot = if normalized_change_type == "added" {
        missing_project_git_text_snapshot()
    } else {
        git_runtime::capture_revision_text_snapshot(
            &app,
            &runtime.execution_target,
            runtime.ssh_config_id.as_deref(),
            &runtime.repo_path,
            &parent_revision,
            before_path,
        )
        .await?
    };
    let after_snapshot = if normalized_change_type == "deleted" {
        missing_project_git_text_snapshot()
    } else {
        git_runtime::capture_revision_text_snapshot(
            &app,
            &runtime.execution_target,
            runtime.ssh_config_id.as_deref(),
            &runtime.repo_path,
            &trimmed_commit_sha,
            trimmed,
        )
        .await?
    };
    let (before_label, after_label) = build_project_git_commit_preview_labels(&trimmed_commit_sha);

    insert_activity_log(
        &pool,
        "project_git_commit_file_previewed",
        &format!("预览提交文件对比：{} {}", after_label, trimmed),
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(build_project_git_file_preview(
        project.id,
        runtime.execution_target,
        &runtime.repo_path,
        trimmed,
        normalized_previous_path,
        normalized_change_type,
        before_label,
        after_label,
        before_snapshot,
        after_snapshot,
    ))
}

async fn commit_in_dir<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    working_dir: &str,
    message: &str,
) -> Result<String, String> {
    let worktree_runtime = runtime.with_repo_path(working_dir);
    git_runtime::commit_changes(
        app,
        &worktree_runtime.execution_target,
        worktree_runtime.ssh_config_id.as_deref(),
        &worktree_runtime.repo_path,
        message,
    )
    .await
}

async fn generate_commit_message_for_dir<R: Runtime>(
    app: &AppHandle<R>,
    project_id: &str,
    runtime: &GitProjectRuntimeContext,
    working_dir: &str,
) -> Result<String, String> {
    let overview = collect_git_overview_for_dir(app, runtime, working_dir, 1).await?;
    let staged_change_prompts = collect_staged_change_prompts(
        &collect_working_tree_changes(app, runtime, working_dir).await?,
    );
    if staged_change_prompts.is_empty() {
        return Err("当前没有可用于生成提交信息的已暂存文件".to_string());
    }

    Ok(generate_commit_message_for_project(
        app,
        project_id,
        overview.current_branch.as_deref(),
        overview.working_tree_summary.as_deref(),
        &staged_change_prompts,
    )
    .await?
    .message)
}

async fn file_preview_in_dir<R: Runtime>(
    app: &AppHandle<R>,
    project_id: &str,
    runtime: &GitProjectRuntimeContext,
    working_dir: &str,
    relative_path: &str,
    previous_path: Option<String>,
    change_type: Option<String>,
) -> Result<ProjectGitFilePreview, String> {
    let trimmed = relative_path.trim();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }

    let worktree_runtime = runtime.with_repo_path(working_dir);
    let normalized_previous_path = normalize_project_git_relative_path(previous_path);
    let normalized_change_type = normalize_project_git_change_type(change_type);

    let before_path = if normalized_change_type == "renamed" {
        normalized_previous_path.as_deref().unwrap_or(trimmed)
    } else {
        trimmed
    };
    let before_snapshot = if normalized_change_type == "added" {
        missing_project_git_text_snapshot()
    } else {
        git_runtime::capture_head_text_snapshot(
            app,
            &worktree_runtime.execution_target,
            worktree_runtime.ssh_config_id.as_deref(),
            &worktree_runtime.repo_path,
            before_path,
        )
        .await?
    };
    let after_snapshot = if normalized_change_type == "deleted" {
        missing_project_git_text_snapshot()
    } else {
        git_runtime::capture_worktree_text_snapshot(
            app,
            &worktree_runtime.execution_target,
            worktree_runtime.ssh_config_id.as_deref(),
            &worktree_runtime.repo_path,
            trimmed,
        )
        .await?
    };

    Ok(build_project_git_file_preview(
        project_id.to_string(),
        worktree_runtime.execution_target,
        &worktree_runtime.repo_path,
        trimmed,
        normalized_previous_path,
        normalized_change_type,
        "HEAD 基线".to_string(),
        "当前工作区".to_string(),
        before_snapshot,
        after_snapshot,
    ))
}

#[tauri::command]
pub async fn stage_project_git_file<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    relative_path: String,
) -> Result<String, String> {
    let trimmed = relative_path.trim().to_string();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;
    stage_file_in_dir(&app, &runtime, &runtime.repo_path, &trimmed).await?;

    let details = format!("已暂存工作区文件：{}", trimmed);
    insert_activity_log(
        &pool,
        "project_git_file_staged",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;
    Ok(details)
}

#[tauri::command]
pub async fn unstage_project_git_file<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    relative_path: String,
) -> Result<String, String> {
    let trimmed = relative_path.trim().to_string();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;
    unstage_file_in_dir(&app, &runtime, &runtime.repo_path, &trimmed).await?;

    let details = format!("已取消暂存工作区文件：{}", trimmed);
    insert_activity_log(
        &pool,
        "project_git_file_unstaged",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;
    Ok(details)
}

#[tauri::command]
pub async fn stage_all_project_git_files<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<String, String> {
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;
    stage_all_in_dir(&app, &runtime, &runtime.repo_path).await?;

    let details = "已暂存当前项目全部工作区变更".to_string();
    insert_activity_log(
        &pool,
        "project_git_stage_all",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;
    Ok(details)
}

#[tauri::command]
pub async fn unstage_all_project_git_files<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<String, String> {
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;
    unstage_all_in_dir(&app, &runtime, &runtime.repo_path).await?;

    let details = "已取消暂存当前项目全部工作区变更".to_string();
    insert_activity_log(
        &pool,
        "project_git_unstage_all",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;
    Ok(details)
}

#[tauri::command]
pub async fn rollback_project_git_files<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    relative_paths: Vec<String>,
) -> Result<String, String> {
    let paths: Vec<String> = relative_paths
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    if paths.is_empty() {
        return Err("至少需要指定一个文件路径".to_string());
    }
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;
    rollback_files_in_dir(&app, &runtime, &runtime.repo_path, &paths).await?;
    let details = format!("已回滚 {} 个工作区文件", paths.len());
    insert_activity_log(
        &pool,
        "project_git_rollback_files",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;
    Ok(details)
}

#[tauri::command]
pub async fn commit_project_git_changes<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    message: String,
) -> Result<String, String> {
    let trimmed = message.trim().to_string();
    if trimmed.is_empty() {
        return Err("提交说明不能为空".to_string());
    }

    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;
    let result = commit_in_dir(&app, &runtime, &runtime.repo_path, &trimmed).await?;

    insert_activity_log(
        &pool,
        "project_git_committed",
        &result,
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(result)
}

