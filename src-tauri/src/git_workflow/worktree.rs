// Domain slice: worktree (included into git_workflow)

fn normalize_worktree_path_key(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed == "/" {
        return "/".to_string();
    }
    trimmed.trim_end_matches('/').to_string()
}

fn parse_worktree_list_porcelain(output: &str) -> Result<Vec<RawWorktreeEntry>, String> {
    let mut entries = Vec::new();
    let mut current: Option<RawWorktreeEntry> = None;

    for line in output.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if let Some(entry) = current.take() {
                if !entry.path.is_empty() {
                    entries.push(entry);
                }
            }
            continue;
        }

        if let Some(path) = trimmed.strip_prefix("worktree ") {
            if let Some(entry) = current.take() {
                if !entry.path.is_empty() {
                    entries.push(entry);
                }
            }
            current = Some(RawWorktreeEntry {
                path: path.trim().to_string(),
                ..RawWorktreeEntry::default()
            });
            continue;
        }

        let entry = current
            .as_mut()
            .ok_or_else(|| format!("无法解析 git worktree list 输出：{}", trimmed))?;
        if let Some(head_sha) = trimmed.strip_prefix("HEAD ") {
            entry.head_sha = trim_optional(Some(head_sha.to_string()));
        } else if let Some(branch_ref) = trimmed.strip_prefix("branch ") {
            entry.branch_ref = trim_optional(Some(branch_ref.to_string()));
        } else if trimmed == "bare" {
            entry.is_bare = true;
        } else if trimmed == "detached" {
            entry.is_detached = true;
        } else if let Some(reason) = trimmed.strip_prefix("locked") {
            entry.is_locked = true;
            entry.lock_reason = trim_optional(Some(reason.to_string()));
        } else if let Some(reason) = trimmed.strip_prefix("prunable") {
            entry.is_prunable = true;
            entry.prunable_reason = trim_optional(Some(reason.to_string()));
        }
    }

    if let Some(entry) = current {
        if !entry.path.is_empty() {
            entries.push(entry);
        }
    }

    Ok(entries)
}

fn ensure_worktree_allows_file_operations(entry: &RawWorktreeEntry) -> Result<(), String> {
    if entry.is_bare {
        return Err("裸仓库 worktree 不支持文件级操作".to_string());
    }
    if entry.is_prunable {
        return Err("当前 worktree 已处于可清理状态，请先移除或修复后再操作".to_string());
    }
    Ok(())
}

fn resolve_branch_execution_worktree(
    entries: &[RawWorktreeEntry],
    fallback_repo_path: &str,
    branch_name: &str,
) -> String {
    entries
        .iter()
        .find(|entry| {
            !entry.is_bare
                && !entry.is_prunable
                && entry.branch_name().as_deref() == Some(branch_name)
        })
        .map(|entry| entry.path.clone())
        .unwrap_or_else(|| fallback_repo_path.to_string())
}

fn build_repo_child_worktree_path(
    git_common_dir_path: &str,
    task_slug: &str,
) -> Result<PathBuf, String> {
    let git_common_dir = Path::new(git_common_dir_path);
    if git_common_dir.as_os_str().is_empty() {
        return Err("无法解析仓库 Git 公共目录".to_string());
    }
    Ok(git_common_dir.join("codex-ai-worktrees").join(task_slug))
}

fn build_worktree_path(
    repo_path: &str,
    task_id: &str,
    git_preferences: &GitPreferences,
) -> Result<String, String> {
    let repo = Path::new(repo_path);
    let repo_name = repo
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "无法解析仓库目录名".to_string())?;
    let repo_slug = sanitize_git_fragment(&repo_name);
    let task_slug = sanitize_git_fragment(task_id);
    let path = match git_preferences.worktree_location_mode.as_str() {
        "repo_child_hidden" => build_repo_child_worktree_path(
            repo.join(".git").to_string_lossy().as_ref(),
            &task_slug,
        )?,
        "custom_root" => {
            let root = git_preferences
                .worktree_custom_root
                .as_deref()
                .ok_or_else(|| "当前 Git 偏好缺少自定义 Worktree 根目录".to_string())?;
            if root == "~" || root.starts_with("~/") {
                Path::new(root).join(&repo_slug).join(&task_slug)
            } else {
                Path::new(root).join(&repo_slug).join(&task_slug)
            }
        }
        _ => {
            let parent = repo
                .parent()
                .ok_or_else(|| "无法解析仓库父目录".to_string())?;
            parent
                .join(format!(".codex-ai-worktrees-{}", repo_slug))
                .join(&task_slug)
        }
    };
    Ok(path.to_string_lossy().to_string())
}

fn resolve_repo_child_worktree_root_local(repo_path: &str) -> Result<String, String> {
    let git_common_dir = run_git_text(repo_path, &["rev-parse", "--git-common-dir"])?;
    if git_common_dir.is_empty() {
        return Err("无法解析仓库 Git 公共目录".to_string());
    }
    let path = if Path::new(&git_common_dir).is_absolute() {
        PathBuf::from(git_common_dir)
    } else {
        Path::new(repo_path).join(git_common_dir)
    };
    Ok(path.to_string_lossy().to_string())
}

async fn resolve_repo_child_worktree_root<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
) -> Result<String, String> {
    if runtime.execution_target == EXECUTION_TARGET_LOCAL {
        resolve_repo_child_worktree_root_local(&runtime.repo_path)
    } else {
        git_runtime::git_common_dir(
            app,
            &runtime.execution_target,
            runtime.ssh_config_id.as_deref(),
            &runtime.repo_path,
        )
        .await
    }
}

async fn build_task_worktree_path_for_runtime<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    task_id: &str,
    git_preferences: &GitPreferences,
) -> Result<String, String> {
    if git_preferences.worktree_location_mode != "repo_child_hidden" {
        return build_worktree_path(&runtime.repo_path, task_id, git_preferences);
    }

    let task_slug = sanitize_git_fragment(task_id);
    let git_common_dir_path = resolve_repo_child_worktree_root(app, runtime).await?;
    let path = build_repo_child_worktree_path(&git_common_dir_path, &task_slug)?;
    Ok(path.to_string_lossy().to_string())
}

async fn ensure_task_worktree_for_runtime<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    worktree_path: &str,
    task_branch: &str,
    target_branch: &str,
) -> Result<(), String> {
    #[cfg(test)]
    {
        if runtime.execution_target == EXECUTION_TARGET_LOCAL {
            let worktree = Path::new(worktree_path);
            if worktree.join(".git").exists() {
                return Ok(());
            }
            if worktree.exists() {
                let is_empty = fs::read_dir(worktree)
                    .map_err(|error| format!("读取 worktree 目录失败: {}", error))?
                    .next()
                    .is_none();
                if !is_empty {
                    return Err(format!("worktree 目录已存在且非空：{}", worktree_path));
                }
            } else if let Some(parent) = worktree.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("创建 worktree 父目录失败: {}", error))?;
            }
            let full_ref = format!("refs/heads/{task_branch}");
            if git_ref_exists_local(&runtime.repo_path, &full_ref) {
                return run_git_command(
                    &runtime.repo_path,
                    &["worktree", "add", worktree_path, task_branch],
                );
            }
            return run_git_command(
                &runtime.repo_path,
                &[
                    "worktree",
                    "add",
                    "-b",
                    task_branch,
                    worktree_path,
                    target_branch,
                ],
            );
        }
    }

    git_runtime::ensure_task_worktree(
        app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        worktree_path,
        task_branch,
        target_branch,
    )
    .await
}

async fn worktree_path_exists<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    worktree_path: &str,
) -> bool {
    if runtime.execution_target == EXECUTION_TARGET_LOCAL {
        return Path::new(worktree_path).exists();
    }

    git_runtime::path_exists(
        app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        worktree_path,
    )
    .await
    .unwrap_or(false)
}

async fn list_worktrees_raw<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
) -> Result<Vec<RawWorktreeEntry>, String> {
    let output = git_runtime::list_worktrees_porcelain(
        app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
    )
    .await?;
    parse_worktree_list_porcelain(&output)
}

async fn lookup_task_contexts_for_worktrees(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<HashMap<String, TaskGitContextWorktreeRow>, String> {
    let rows = sqlx::query_as::<_, TaskGitContextWorktreeRow>(
        r#"
        SELECT
            c.id,
            c.task_id,
            c.worktree_path,
            t.title AS task_title
        FROM task_git_contexts c
        LEFT JOIN tasks t ON t.id = c.task_id
        WHERE c.project_id = $1
        ORDER BY c.updated_at DESC, c.created_at DESC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("查询项目 worktree 关联任务失败: {}", error))?;

    let mut contexts = HashMap::with_capacity(rows.len());
    for row in rows {
        contexts
            .entry(normalize_worktree_path_key(&row.worktree_path))
            .or_insert(row);
    }
    Ok(contexts)
}

async fn enrich_worktree_with_status<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    entry: &RawWorktreeEntry,
    task_context: Option<&TaskGitContextWorktreeRow>,
) -> ProjectGitWorktree {
    let is_main =
        normalize_worktree_path_key(&entry.path) == normalize_worktree_path_key(&runtime.repo_path);
    let mut working_tree_summary = None;
    let mut working_tree_changes = Vec::new();

    if !entry.is_bare && !entry.is_prunable {
        if let Ok(overview) = collect_git_overview_for_dir(app, runtime, &entry.path, 1).await {
            working_tree_summary = overview.working_tree_summary;
        }
        if let Ok(changes) = collect_working_tree_changes(app, runtime, &entry.path).await {
            working_tree_changes = changes;
        }
    }

    ProjectGitWorktree {
        path: entry.path.clone(),
        branch: entry.branch_name(),
        head_sha: entry.head_sha.clone(),
        short_head_sha: short_git_sha(entry.head_sha.as_deref()),
        is_main,
        is_bare: entry.is_bare,
        is_detached: entry.is_detached,
        is_locked: entry.is_locked,
        lock_reason: entry.lock_reason.clone(),
        is_prunable: entry.is_prunable,
        prunable_reason: entry.prunable_reason.clone(),
        task_git_context_id: task_context.map(|value| value.id.clone()),
        task_id: task_context.map(|value| value.task_id.clone()),
        task_title: task_context.and_then(|value| value.task_title.clone()),
        working_tree_summary,
        working_tree_changes,
    }
}

fn has_stageable_worktree_changes(changes: &[ProjectGitWorkingTreeChange]) -> bool {
    changes.iter().any(|change| {
        matches!(
            change.stage_status.as_str(),
            "unstaged" | "untracked" | "partially_staged"
        )
    })
}

pub(crate) async fn auto_commit_task_worktree<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
) -> Result<TaskGitAutoCommitOutcome, String> {
    let pool = sqlite_pool(app).await?;
    let task = fetch_task_by_id(&pool, task_id).await?;
    let mut context = fetch_task_git_context_by_task_id(&pool, task_id)
        .await?
        .ok_or_else(|| "当前任务缺少 Git worktree，上下文未准备好，无法自动提交".to_string())?;
    let project = fetch_project_by_id(&pool, &task.project_id).await?;
    let runtime = resolve_project_runtime_context(&project)?;

    if !context_is_healthy(app, &runtime, &context).await {
        return Err("task git context 不可用，当前状态异常".to_string());
    }

    let mut overview = collect_task_git_commit_overview(app, &runtime, &context).await?;
    if has_stageable_worktree_changes(&overview.working_tree_changes) {
        git_runtime::stage_all(
            app,
            &runtime.execution_target,
            runtime.ssh_config_id.as_deref(),
            &context.worktree_path,
        )
        .await?;
        overview = collect_task_git_commit_overview(app, &runtime, &context).await?;
    }

    let staged_change_prompts = collect_staged_change_prompts(&overview.working_tree_changes);
    if !staged_change_prompts.is_empty() {
        let commit_message = generate_commit_message_for_project(
            app,
            &project.id,
            overview.current_branch.as_deref(),
            overview.working_tree_summary.as_deref(),
            &staged_change_prompts,
        )
        .await?;
        let detail = commit_task_git_changes_internal(
            app,
            &pool,
            &task,
            &runtime,
            &mut context,
            &commit_message.message,
            false,
        )
        .await?;
        return Ok(TaskGitAutoCommitOutcome::Committed { detail });
    }

    if task_git_context_has_pending_merge(app, &runtime, &context).await? {
        update_task_git_context_merge_ready(
            &pool,
            &mut context,
            "任务代码已就绪，等待合并到目标分支",
        )
        .await?;
        return Ok(TaskGitAutoCommitOutcome::MergeReady {
            detail: "任务代码已就绪，等待合并到目标分支".to_string(),
        });
    }

    Ok(TaskGitAutoCommitOutcome::NoChanges {
        detail: "审核通过且没有可提交的代码改动".to_string(),
    })
}

#[tauri::command]
pub async fn list_project_git_worktrees<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<Vec<ProjectGitWorktree>, String> {
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;
    let entries = list_worktrees_raw(&app, &runtime).await?;
    let context_map = lookup_task_contexts_for_worktrees(&pool, &project_id).await?;
    let mut worktrees = Vec::with_capacity(entries.len());
    for entry in &entries {
        let key = normalize_worktree_path_key(&entry.path);
        worktrees
            .push(enrich_worktree_with_status(&app, &runtime, entry, context_map.get(&key)).await);
    }

    insert_activity_log(
        &pool,
        "project_git_worktrees_viewed",
        &format!("浏览 Git worktree 列表：{} 条", worktrees.len()),
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(worktrees)
}

#[tauri::command]
pub async fn get_project_worktree_file_preview<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    worktree_path: String,
    relative_path: String,
    previous_path: Option<String>,
    change_type: Option<String>,
) -> Result<ProjectGitFilePreview, String> {
    let (pool, project, runtime, entry) =
        resolve_project_worktree_target(&app, &project_id, &worktree_path).await?;
    ensure_worktree_allows_file_operations(&entry)?;

    let preview = file_preview_in_dir(
        &app,
        &project.id,
        &runtime,
        &entry.path,
        &relative_path,
        previous_path,
        change_type,
    )
    .await?;

    insert_activity_log(
        &pool,
        "project_git_worktree_file_previewed",
        &format!(
            "预览 worktree 文件：{} · {}",
            entry.path,
            relative_path.trim()
        ),
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(preview)
}

#[tauri::command]
pub async fn stage_project_worktree_file<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    worktree_path: String,
    relative_path: String,
) -> Result<String, String> {
    let trimmed = relative_path.trim().to_string();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }

    let (pool, project, runtime, entry) =
        resolve_project_worktree_target(&app, &project_id, &worktree_path).await?;
    ensure_worktree_allows_file_operations(&entry)?;
    stage_file_in_dir(&app, &runtime, &entry.path, &trimmed).await?;

    let details = format!("已暂存 worktree 文件：{} · {}", entry.path, trimmed);
    insert_activity_log(
        &pool,
        "project_git_worktree_file_staged",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;
    Ok(details)
}

#[tauri::command]
pub async fn unstage_project_worktree_file<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    worktree_path: String,
    relative_path: String,
) -> Result<String, String> {
    let trimmed = relative_path.trim().to_string();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }

    let (pool, project, runtime, entry) =
        resolve_project_worktree_target(&app, &project_id, &worktree_path).await?;
    ensure_worktree_allows_file_operations(&entry)?;
    unstage_file_in_dir(&app, &runtime, &entry.path, &trimmed).await?;

    let details = format!("已取消暂存 worktree 文件：{} · {}", entry.path, trimmed);
    insert_activity_log(
        &pool,
        "project_git_worktree_file_unstaged",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;
    Ok(details)
}

#[tauri::command]
pub async fn stage_all_project_worktree_files<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    worktree_path: String,
) -> Result<String, String> {
    let (pool, project, runtime, entry) =
        resolve_project_worktree_target(&app, &project_id, &worktree_path).await?;
    ensure_worktree_allows_file_operations(&entry)?;
    stage_all_in_dir(&app, &runtime, &entry.path).await?;

    let details = format!("已暂存 worktree 全部变更：{}", entry.path);
    insert_activity_log(
        &pool,
        "project_git_worktree_stage_all",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;
    Ok(details)
}

#[tauri::command]
pub async fn unstage_all_project_worktree_files<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    worktree_path: String,
) -> Result<String, String> {
    let (pool, project, runtime, entry) =
        resolve_project_worktree_target(&app, &project_id, &worktree_path).await?;
    ensure_worktree_allows_file_operations(&entry)?;
    unstage_all_in_dir(&app, &runtime, &entry.path).await?;

    let details = format!("已取消暂存 worktree 全部变更：{}", entry.path);
    insert_activity_log(
        &pool,
        "project_git_worktree_unstage_all",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;
    Ok(details)
}

#[tauri::command]
pub async fn rollback_project_worktree_files<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    worktree_path: String,
    relative_paths: Vec<String>,
) -> Result<String, String> {
    let paths: Vec<String> = relative_paths
        .into_iter()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect();
    if paths.is_empty() {
        return Err("至少需要指定一个文件路径".to_string());
    }

    let (pool, project, runtime, entry) =
        resolve_project_worktree_target(&app, &project_id, &worktree_path).await?;
    ensure_worktree_allows_file_operations(&entry)?;
    rollback_files_in_dir(&app, &runtime, &entry.path, &paths).await?;

    let details = format!("已回滚 worktree {} 中的 {} 个文件", entry.path, paths.len());
    insert_activity_log(
        &pool,
        "project_git_worktree_rollback_files",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;
    Ok(details)
}

#[tauri::command]
pub async fn rollback_all_project_worktree_changes<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    worktree_path: String,
) -> Result<String, String> {
    let (pool, project, runtime, entry) =
        resolve_project_worktree_target(&app, &project_id, &worktree_path).await?;
    ensure_worktree_allows_file_operations(&entry)?;
    rollback_all_in_dir(&app, &runtime, &entry.path).await?;

    let details = format!("已回滚 worktree 全部变更：{}", entry.path);
    insert_activity_log(
        &pool,
        "project_git_worktree_rollback_all",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;
    Ok(details)
}

#[tauri::command]
pub async fn commit_project_worktree_changes<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    worktree_path: String,
    message: String,
) -> Result<String, String> {
    let trimmed = message.trim().to_string();
    if trimmed.is_empty() {
        return Err("提交说明不能为空".to_string());
    }

    let (pool, project, runtime, entry) =
        resolve_project_worktree_target(&app, &project_id, &worktree_path).await?;
    ensure_worktree_allows_file_operations(&entry)?;
    let result = commit_in_dir(&app, &runtime, &entry.path, &trimmed).await?;

    insert_activity_log(
        &pool,
        "project_git_worktree_committed",
        &format!("{} · {}", entry.path, result),
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(result)
}

#[tauri::command]
pub async fn generate_project_worktree_commit_message<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    worktree_path: String,
) -> Result<String, String> {
    let (pool, project, runtime, entry) =
        resolve_project_worktree_target(&app, &project_id, &worktree_path).await?;
    ensure_worktree_allows_file_operations(&entry)?;
    let message = generate_commit_message_for_dir(&app, &project.id, &runtime, &entry.path).await?;

    insert_activity_log(
        &pool,
        "project_git_worktree_commit_message_generated",
        &format!(
            "Worktree：{}；结果：{}",
            entry.path,
            message.lines().next().unwrap_or("未命名提交")
        ),
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(message)
}

#[tauri::command]
pub async fn remove_project_git_worktree<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    worktree_path: String,
    force: bool,
) -> Result<String, String> {
    let (pool, project, runtime, entry) =
        resolve_project_worktree_target(&app, &project_id, &worktree_path).await?;

    if normalize_worktree_path_key(&entry.path) == normalize_worktree_path_key(&runtime.repo_path) {
        return Err("主仓库 worktree 不允许删除".to_string());
    }
    if entry.is_locked {
        return Err(entry
            .lock_reason
            .as_ref()
            .map(|reason| format!("当前 worktree 已锁定，请先解锁：{}", reason))
            .unwrap_or_else(|| "当前 worktree 已锁定，请先解锁后再删除".to_string()));
    }

    git_runtime::remove_worktree(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        &entry.path,
        force,
        true,
    )
    .await?;

    let details = format!("已删除 Git worktree：{}", entry.path);
    insert_activity_log(
        &pool,
        "project_git_worktree_removed",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(details)
}

#[tauri::command]
pub async fn merge_project_git_worktree<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    worktree_path: String,
    target_branch: String,
    auto_stash: Option<bool>,
    delete_worktree: Option<bool>,
    delete_branch: Option<bool>,
) -> Result<String, String> {
    let normalized_worktree_path = normalize_worktree_path_key(&worktree_path);
    if normalized_worktree_path.is_empty() {
        return Err("worktree 路径不能为空".to_string());
    }

    let target_branch =
        trim_optional(Some(target_branch)).ok_or_else(|| "目标分支不能为空".to_string())?;
    let auto_stash = auto_stash.unwrap_or(true);
    let delete_worktree = delete_worktree.unwrap_or(false);
    let delete_branch = delete_branch.unwrap_or(false);

    if delete_branch && !delete_worktree {
        return Err("删除分支前需要先删除 worktree，请先勾选“删除 worktree”".to_string());
    }

    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;
    let entries = list_worktrees_raw(&app, &runtime).await?;
    let entry = entries
        .iter()
        .find(|value| normalize_worktree_path_key(&value.path) == normalized_worktree_path)
        .cloned()
        .ok_or_else(|| "指定 worktree 不属于当前项目".to_string())?;

    if normalize_worktree_path_key(&entry.path) == normalize_worktree_path_key(&runtime.repo_path) {
        return Err("主仓库 worktree 请使用上方分支管理里的合并功能".to_string());
    }

    ensure_worktree_allows_file_operations(&entry)?;
    if entry.is_detached {
        return Err("detached HEAD worktree 无法直接作为合并来源".to_string());
    }
    let source_branch = entry
        .branch_name()
        .ok_or_else(|| "当前 worktree 未绑定本地分支，无法合并".to_string())?;
    if source_branch == target_branch {
        return Err("源分支和目标分支不能相同".to_string());
    }
    if delete_worktree && entry.is_locked {
        return Err(entry
            .lock_reason
            .as_ref()
            .map(|reason| format!("当前 worktree 已锁定，请先解锁：{}", reason))
            .unwrap_or_else(|| "当前 worktree 已锁定，请先解锁后再删除".to_string()));
    }

    let working_tree_changes = collect_working_tree_changes(&app, &runtime, &entry.path).await?;
    let has_working_tree_changes = !working_tree_changes.is_empty();
    if has_working_tree_changes && delete_worktree && !auto_stash {
        return Err(
            "当前 worktree 存在未提交改动；如需合并后删除，请勾选“自动暂存未提交的更改”"
                .to_string(),
        );
    }

    let mut detail_parts = Vec::new();

    if has_working_tree_changes && auto_stash {
        let stash_payload = serde_json::json!({
            "include_untracked": true,
            "message": format!("codex-ai merge worktree {} into {}", source_branch, target_branch),
        });
        let stash_payload_json = stash_payload.to_string();
        git_runtime::execute_action(
            &app,
            &runtime.execution_target,
            runtime.ssh_config_id.as_deref(),
            &runtime.repo_path,
            &entry.path,
            &source_branch,
            "stash",
            &stash_payload_json,
        )
        .await?;
        detail_parts.push("已自动暂存当前 worktree 的未提交改动".to_string());
    }

    let merge_repo_path =
        resolve_branch_execution_worktree(&entries, &runtime.repo_path, &target_branch);
    let merge_result = git_runtime::merge_branches(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &merge_repo_path,
        &source_branch,
        &target_branch,
        "ff",
        None,
    )
    .await?;
    detail_parts.push(merge_result);

    if delete_worktree {
        git_runtime::remove_worktree(
            &app,
            &runtime.execution_target,
            runtime.ssh_config_id.as_deref(),
            &runtime.repo_path,
            &entry.path,
            false,
            true,
        )
        .await?;
        detail_parts.push(format!("已删除 Git worktree：{}", entry.path));
    }

    if delete_branch {
        let delete_result = git_runtime::delete_branch(
            &app,
            &runtime.execution_target,
            runtime.ssh_config_id.as_deref(),
            &runtime.repo_path,
            &source_branch,
            false,
        )
        .await?;
        detail_parts.push(delete_result);
    }

    let details = detail_parts.join("；");
    insert_activity_log(
        &pool,
        "project_git_worktree_merged",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(details)
}

async fn resolve_project_worktree_target<R: Runtime>(
    app: &AppHandle<R>,
    project_id: &str,
    worktree_path: &str,
) -> Result<
    (
        SqlitePool,
        Project,
        GitProjectRuntimeContext,
        RawWorktreeEntry,
    ),
    String,
> {
    let normalized_worktree_path = normalize_worktree_path_key(worktree_path);
    if normalized_worktree_path.is_empty() {
        return Err("worktree 路径不能为空".to_string());
    }

    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(app, project_id).await?;
    let entries = list_worktrees_raw(app, &runtime).await?;
    let entry = entries
        .into_iter()
        .find(|value| normalize_worktree_path_key(&value.path) == normalized_worktree_path)
        .ok_or_else(|| "指定 worktree 不属于当前项目".to_string())?;
    Ok((pool, project, runtime, entry))
}

