// Domain slice: runtime (included into git_workflow)

fn trim_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_git_branch_ref(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(branch) = trimmed.strip_prefix("refs/heads/") {
        return Some(branch.to_string());
    }
    Some(trimmed.to_string())
}

fn short_git_sha(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(7).collect::<String>())
}

fn sqlite_now_with_offset(minutes: i64) -> String {
    (Utc::now() + Duration::minutes(minutes))
        .format(SQLITE_DATETIME_FORMAT)
        .to_string()
}

fn hash_text(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn parse_token(token: &str) -> Result<(&str, &str), String> {
    let mut parts = token.splitn(2, '.');
    let nonce = parts.next().unwrap_or_default();
    let signature = parts.next().unwrap_or_default();
    if nonce.trim().is_empty() || signature.trim().is_empty() {
        return Err("确认 token 格式无效".to_string());
    }
    Ok((nonce, signature))
}

fn run_git_text(repo_path: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .map_err(|error| format!("执行 git {:?} 失败: {}", args, error))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("git {:?} 执行失败", args)
        } else {
            format!("git {:?} 执行失败: {}", args, stderr)
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
fn run_git_command(repo_path: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .map_err(|error| format!("执行 git {:?} 失败: {}", args, error))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("git {:?} 执行失败", args)
        } else {
            format!("git {:?} 执行失败: {}", args, stderr)
        });
    }
    Ok(())
}

#[cfg(test)]
fn git_ref_exists_local(repo_path: &str, full_ref: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["show-ref", "--verify", "--quiet", full_ref])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
fn ensure_git_repository(repo_path: &str) -> Result<(), String> {
    let git_dir = Path::new(repo_path).join(".git");
    if git_dir.exists() {
        Ok(())
    } else {
        Err(format!("工作目录 {} 不是 Git 仓库，缺少 .git", repo_path))
    }
}

fn resolve_project_runtime_context(project: &Project) -> Result<GitProjectRuntimeContext, String> {
    match project.project_type.as_str() {
        PROJECT_TYPE_SSH => {
            let repo_path = project
                .remote_repo_path
                .clone()
                .ok_or_else(|| "当前 SSH 项目未配置远程仓库目录".to_string())?;
            let ssh_config_id = project
                .ssh_config_id
                .clone()
                .ok_or_else(|| "当前 SSH 项目缺少 ssh_config_id".to_string())?;
            Ok(GitProjectRuntimeContext {
                repo_path,
                execution_target: EXECUTION_TARGET_SSH.to_string(),
                ssh_config_id: Some(ssh_config_id),
            })
        }
        _ => {
            let repo_path = project
                .repo_path
                .clone()
                .ok_or_else(|| "当前项目未配置本地仓库目录".to_string())?;
            #[cfg(test)]
            ensure_git_repository(&repo_path)?;
            Ok(GitProjectRuntimeContext {
                repo_path,
                execution_target: EXECUTION_TARGET_LOCAL.to_string(),
                ssh_config_id: None,
            })
        }
    }
}

async fn git_ref_exists<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    full_ref: &str,
) -> Result<bool, String> {
    #[cfg(test)]
    {
        if runtime.execution_target == EXECUTION_TARGET_LOCAL {
            return Ok(git_ref_exists_local(&runtime.repo_path, full_ref));
        }
    }

    git_runtime::git_ref_exists(
        app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        full_ref,
    )
    .await
}

#[cfg(test)]
fn determine_current_branch_local(repo_path: &str) -> Result<String, String> {
    let branch = run_git_text(repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let branch = branch.trim();
    if !branch.is_empty() && branch != "HEAD" {
        Ok(branch.to_string())
    } else {
        Err("无法解析当前分支".to_string())
    }
}

async fn determine_current_branch<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
) -> Result<String, String> {
    #[cfg(test)]
    {
        if runtime.execution_target == EXECUTION_TARGET_LOCAL {
            return determine_current_branch_local(&runtime.repo_path);
        }
    }

    git_runtime::collect_git_overview(
        app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        1,
    )
    .await?
    .current_branch
    .ok_or_else(|| "无法解析当前分支".to_string())
}

fn sanitize_git_fragment(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_' | '/') {
            output.push('-');
        }
    }
    let output = output.trim_matches('-').to_string();
    if output.is_empty() {
        "task".to_string()
    } else {
        output
    }
}

fn resolve_project_git_preferences<R: Runtime>(
    app: &AppHandle<R>,
    project: &Project,
) -> Result<GitPreferences, String> {
    if project.project_type == PROJECT_TYPE_SSH {
        project
            .ssh_config_id
            .as_deref()
            .map(|ssh_config_id| load_remote_codex_settings(app, ssh_config_id))
            .transpose()?
            .map(|settings| settings.git_preferences)
            .ok_or_else(|| "SSH 项目缺少对应的 SSH 配置，无法解析 Git 偏好".to_string())
    } else {
        Ok(load_codex_settings(app)?.git_preferences)
    }
}

fn parse_revision_comparison_output(
    output: &str,
) -> Result<git_runtime::GitRuntimeRevisionComparison, String> {
    let mut parts = output.split_whitespace();
    let behind_raw = parts
        .next()
        .ok_or_else(|| format!("无法解析 revision 比较结果: {}", output.trim()))?;
    let ahead_raw = parts
        .next()
        .ok_or_else(|| format!("无法解析 revision 比较结果: {}", output.trim()))?;
    let _behind_commits = behind_raw
        .parse::<u32>()
        .map_err(|error| format!("解析 behind commits 失败: {}", error))?;
    let ahead_commits = ahead_raw
        .parse::<u32>()
        .map_err(|error| format!("解析 ahead commits 失败: {}", error))?;

    Ok(git_runtime::GitRuntimeRevisionComparison { ahead_commits })
}

fn compare_revisions_local(
    repo_path: &str,
    base_revision: &str,
    target_revision: &str,
) -> Result<git_runtime::GitRuntimeRevisionComparison, String> {
    let range = format!("{base_revision}...{target_revision}");
    let output = run_git_text(repo_path, &["rev-list", "--left-right", "--count", &range])?;
    parse_revision_comparison_output(&output)
}

fn can_open_repo_file_locally(repo_path: &str, relative_path: &str) -> bool {
    let candidate = Path::new(repo_path).join(relative_path);
    candidate.is_file()
}

fn payload_object(payload: &Value) -> Result<&Map<String, Value>, String> {
    payload
        .as_object()
        .ok_or_else(|| "payload 必须是 JSON 对象".to_string())
}

fn payload_string(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn payload_bool(map: &Map<String, Value>, key: &str, default: bool) -> bool {
    map.get(key)
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

fn payload_string_array(map: &Map<String, Value>, key: &str) -> Result<Vec<String>, String> {
    let Some(value) = map.get(key) else {
        return Ok(Vec::new());
    };
    let array = value
        .as_array()
        .ok_or_else(|| format!("{key} 必须是字符串数组"))?;
    let items = array
        .iter()
        .filter_map(|item| item.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    Ok(items)
}

fn normalize_action_type(value: &str) -> Result<&'static str, String> {
    match value.trim() {
        "merge" => Ok("merge"),
        "push" => Ok("push"),
        "rebase" => Ok("rebase"),
        "cherry_pick" => Ok("cherry_pick"),
        "stash" => Ok("stash"),
        "unstash" => Ok("unstash"),
        "cleanup_worktree" => Ok("cleanup_worktree"),
        other => Err(format!("不支持的 git action: {}", other)),
    }
}

async fn resolve_project_runtime_for_git_overview<R: Runtime>(
    app: &AppHandle<R>,
    project_id: &str,
) -> Result<(SqlitePool, Project, GitProjectRuntimeContext), String> {
    let pool = sqlite_pool(app).await?;
    let project = fetch_project_by_id(&pool, project_id).await?;
    let runtime = resolve_project_runtime_context(&project)?;
    Ok((pool, project, runtime))
}

async fn stage_file_in_dir<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    working_dir: &str,
    relative_path: &str,
) -> Result<(), String> {
    let worktree_runtime = runtime.with_repo_path(working_dir);
    git_runtime::stage_path(
        app,
        &worktree_runtime.execution_target,
        worktree_runtime.ssh_config_id.as_deref(),
        &worktree_runtime.repo_path,
        relative_path,
    )
    .await?;
    Ok(())
}

async fn unstage_file_in_dir<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    working_dir: &str,
    relative_path: &str,
) -> Result<(), String> {
    let worktree_runtime = runtime.with_repo_path(working_dir);
    git_runtime::unstage_path(
        app,
        &worktree_runtime.execution_target,
        worktree_runtime.ssh_config_id.as_deref(),
        &worktree_runtime.repo_path,
        relative_path,
    )
    .await?;
    Ok(())
}

async fn stage_all_in_dir<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    working_dir: &str,
) -> Result<(), String> {
    let worktree_runtime = runtime.with_repo_path(working_dir);
    git_runtime::stage_all(
        app,
        &worktree_runtime.execution_target,
        worktree_runtime.ssh_config_id.as_deref(),
        &worktree_runtime.repo_path,
    )
    .await?;
    Ok(())
}

async fn unstage_all_in_dir<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    working_dir: &str,
) -> Result<(), String> {
    let worktree_runtime = runtime.with_repo_path(working_dir);
    git_runtime::unstage_all(
        app,
        &worktree_runtime.execution_target,
        worktree_runtime.ssh_config_id.as_deref(),
        &worktree_runtime.repo_path,
    )
    .await?;
    Ok(())
}

async fn rollback_files_in_dir<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    working_dir: &str,
    relative_paths: &[String],
) -> Result<(), String> {
    let worktree_runtime = runtime.with_repo_path(working_dir);
    for path in relative_paths {
        git_runtime::restore_path(
            app,
            &worktree_runtime.execution_target,
            worktree_runtime.ssh_config_id.as_deref(),
            &worktree_runtime.repo_path,
            path,
        )
        .await?;
    }
    Ok(())
}

async fn rollback_all_in_dir<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
    working_dir: &str,
) -> Result<(), String> {
    let worktree_runtime = runtime.with_repo_path(working_dir);
    git_runtime::restore_all(
        app,
        &worktree_runtime.execution_target,
        worktree_runtime.ssh_config_id.as_deref(),
        &worktree_runtime.repo_path,
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn rollback_all_project_git_changes<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<String, String> {
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(&app, &project_id).await?;
    rollback_all_in_dir(&app, &runtime, &runtime.repo_path).await?;
    let details = "已回滚当前项目全部工作区变更".to_string();
    insert_activity_log(
        &pool,
        "project_git_rollback_all",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;
    Ok(details)
}

