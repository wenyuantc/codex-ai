// Domain slice: branch (included into git_workflow)

#[cfg(test)]
fn merge_task_branch_into_target_local(
    repo_path: &str,
    context: &TaskGitContextRecord,
    target_branch: &str,
    strategy: &str,
    allow_ff: bool,
) -> Result<String, String> {
    let current_branch = run_git_text(repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if current_branch != target_branch {
        if !git_working_tree_is_clean(repo_path)? {
            let branch_label = if current_branch == "HEAD" {
                "detached HEAD"
            } else {
                current_branch.as_str()
            };
            return Err(format!(
                "项目主工作区当前在 {}，且存在未提交改动，无法切换到目标分支 {} 执行合并",
                branch_label, target_branch
            ));
        }
        run_git_command(repo_path, &["checkout", target_branch])?;
    }

    let mut args = vec!["merge"];
    if !allow_ff {
        args.push("--no-ff");
    }
    let strategy_arg = format!("--strategy={strategy}");
    args.push(strategy_arg.as_str());
    args.push(context.task_branch.as_str());
    run_git_command(repo_path, &args)?;

    Ok(format!(
        "已将任务分支 {} 合并到目标分支 {}",
        context.task_branch, target_branch
    ))
}

fn normalize_project_git_push_force_mode(value: Option<String>) -> Result<String, String> {
    match trim_optional(value).as_deref() {
        None | Some("none") => Ok("none".to_string()),
        Some("force") => Ok("force".to_string()),
        Some("force_with_lease") => Ok("force_with_lease".to_string()),
        Some(other) => Err(format!("不支持的推送模式：{}", other)),
    }
}

fn normalize_project_git_pull_mode(value: Option<String>) -> Result<String, String> {
    match trim_optional(value).as_deref() {
        None | Some("ff_only") => Ok("ff_only".to_string()),
        Some("rebase") => Ok("rebase".to_string()),
        Some(other) => Err(format!("不支持的拉取模式：{}", other)),
    }
}

fn normalize_project_git_merge_fast_forward(value: Option<String>) -> Result<String, String> {
    match trim_optional(value).as_deref() {
        None | Some("ff") => Ok("ff".to_string()),
        Some("no_ff") => Ok("no_ff".to_string()),
        Some("ff_only") => Ok("ff_only".to_string()),
        Some(other) => Err(format!("不支持的合并模式：{}", other)),
    }
}

fn normalize_project_git_merge_strategy(value: Option<String>) -> Result<Option<String>, String> {
    let trimmed = trim_optional(value);
    match trimmed.as_deref() {
        None => Ok(None),
        Some("ort") | Some("recursive") | Some("resolve") | Some("ours") | Some("subtree") => {
            Ok(trimmed)
        }
        Some(other) => Err(format!("不支持的合并策略：{}", other)),
    }
}

#[tauri::command]
pub async fn push_project_git_branch<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    remote_name: Option<String>,
    branch_name: Option<String>,
    force_mode: Option<String>,
) -> Result<String, String> {
    let remote_name = trim_optional(remote_name).unwrap_or_else(|| "origin".to_string());
    let branch_name = trim_optional(branch_name).unwrap_or_default();
    let force_mode = normalize_project_git_push_force_mode(force_mode)?;
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;

    let result = git_runtime::push_branch(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        &remote_name,
        &branch_name,
        &force_mode,
    )
    .await?;

    insert_activity_log(
        &pool,
        "project_git_pushed",
        &result,
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(result)
}

#[tauri::command]
pub async fn pull_project_git_branch<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    remote_name: Option<String>,
    branch_name: Option<String>,
    mode: Option<String>,
    auto_stash: Option<bool>,
) -> Result<String, String> {
    let remote_name = trim_optional(remote_name).unwrap_or_else(|| "origin".to_string());
    let branch_name = trim_optional(branch_name).unwrap_or_default();
    let pull_mode = normalize_project_git_pull_mode(mode)?;
    let auto_stash = auto_stash.unwrap_or(false);
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;

    let result = git_runtime::pull_branch(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        &remote_name,
        &branch_name,
        &pull_mode,
        auto_stash,
    )
    .await?;

    insert_activity_log(
        &pool,
        "project_git_pulled",
        &result,
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(result)
}

#[tauri::command]
pub async fn checkout_project_git_branch<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    branch_name: String,
) -> Result<String, String> {
    let branch_name =
        trim_optional(Some(branch_name)).ok_or_else(|| "分支名不能为空".to_string())?;
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;

    let result = git_runtime::checkout_branch(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        &branch_name,
    )
    .await?;

    insert_activity_log(
        &pool,
        "project_git_branch_checked_out",
        &result,
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(result)
}

#[tauri::command]
pub async fn create_project_git_branch<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    branch_name: String,
    base_branch: Option<String>,
    checkout: Option<bool>,
) -> Result<String, String> {
    let branch_name =
        trim_optional(Some(branch_name)).ok_or_else(|| "新分支名不能为空".to_string())?;
    let base_branch = trim_optional(base_branch);
    let checkout = checkout.unwrap_or(false);
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;

    let result = git_runtime::create_branch(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        &branch_name,
        base_branch.as_deref(),
        checkout,
    )
    .await?;

    insert_activity_log(
        &pool,
        "project_git_branch_created",
        &result,
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(result)
}

#[tauri::command]
pub async fn delete_project_git_branch<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    branch_name: String,
    force: Option<bool>,
) -> Result<String, String> {
    let branch_name =
        trim_optional(Some(branch_name)).ok_or_else(|| "待删除分支名不能为空".to_string())?;
    let force = force.unwrap_or(false);
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;

    let result = git_runtime::delete_branch(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        &branch_name,
        force,
    )
    .await?;

    insert_activity_log(
        &pool,
        "project_git_branch_deleted",
        &result,
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(result)
}

#[tauri::command]
pub async fn merge_project_git_branches<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    source_branch: String,
    target_branch: String,
    fast_forward: Option<String>,
    strategy: Option<String>,
) -> Result<String, String> {
    let source_branch =
        trim_optional(Some(source_branch)).ok_or_else(|| "源分支不能为空".to_string())?;
    let target_branch =
        trim_optional(Some(target_branch)).ok_or_else(|| "目标分支不能为空".to_string())?;
    if source_branch == target_branch {
        return Err("源分支和目标分支不能相同".to_string());
    }
    let fast_forward = normalize_project_git_merge_fast_forward(fast_forward)?;
    let strategy = normalize_project_git_merge_strategy(strategy)?;
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;

    let result = git_runtime::merge_branches(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        &source_branch,
        &target_branch,
        &fast_forward,
        strategy.as_deref(),
    )
    .await?;

    insert_activity_log(
        &pool,
        "project_git_branches_merged",
        &result,
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(result)
}

