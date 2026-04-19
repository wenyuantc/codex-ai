use std::collections::hash_map::DefaultHasher;
#[cfg(test)]
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::Command;

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{FromRow, SqlitePool};
use tauri::{AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

use crate::app::{
    fetch_project_by_id, fetch_task_by_id, insert_activity_log, now_sqlite, sqlite_pool,
    EXECUTION_TARGET_LOCAL, EXECUTION_TARGET_SSH, PROJECT_TYPE_SSH,
};
use crate::codex::generate_commit_message_for_project;
use crate::db::models::{Project, Task};
use crate::git_runtime::{
    self, GIT_RUNTIME_PROVIDER_SIMPLE_GIT, GIT_RUNTIME_STATUS_READY, GIT_RUNTIME_STATUS_UNAVAILABLE,
};
use crate::task_automation;

const SQLITE_DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
const TASK_GIT_STATE_PROVISIONING: &str = "provisioning";
const TASK_GIT_STATE_READY: &str = "ready";
const TASK_GIT_STATE_RUNNING: &str = "running";
const TASK_GIT_STATE_MERGE_READY: &str = "merge_ready";
const TASK_GIT_STATE_ACTION_PENDING: &str = "action_pending";
const TASK_GIT_STATE_COMPLETED: &str = "completed";
const TASK_GIT_STATE_FAILED: &str = "failed";
const TASK_GIT_STATE_DRIFTED: &str = "drifted";
const PENDING_ACTION_TTL_MINUTES: i64 = 15;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskGitContextRecord {
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub base_branch: String,
    pub task_branch: String,
    pub target_branch: String,
    pub worktree_path: String,
    pub repo_head_commit_at_prepare: Option<String>,
    pub state: String,
    pub context_version: i64,
    pub pending_action_type: Option<String>,
    pub pending_action_token_hash: Option<String>,
    pub pending_action_payload_json: Option<String>,
    pub pending_action_nonce: Option<String>,
    pub pending_action_requested_at: Option<String>,
    pub pending_action_expires_at: Option<String>,
    pub pending_action_repo_revision: Option<String>,
    pub pending_action_bound_context_version: Option<i64>,
    pub last_reconciled_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGitContextSummary {
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub base_branch: String,
    pub task_branch: String,
    pub target_branch: String,
    pub worktree_path: String,
    pub repo_head_commit_at_prepare: Option<String>,
    pub state: String,
    pub context_version: i64,
    pub pending_action_type: Option<String>,
    pub pending_action_requested_at: Option<String>,
    pub pending_action_expires_at: Option<String>,
    pub last_reconciled_at: Option<String>,
    pub last_error: Option<String>,
    pub worktree_missing: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<TaskGitContextRecord> for TaskGitContextSummary {
    fn from(value: TaskGitContextRecord) -> Self {
        Self {
            id: value.id,
            task_id: value.task_id,
            project_id: value.project_id,
            base_branch: value.base_branch,
            task_branch: value.task_branch,
            target_branch: value.target_branch,
            worktree_path: value.worktree_path,
            repo_head_commit_at_prepare: value.repo_head_commit_at_prepare,
            state: value.state,
            context_version: value.context_version,
            pending_action_type: value.pending_action_type,
            pending_action_requested_at: value.pending_action_requested_at,
            pending_action_expires_at: value.pending_action_expires_at,
            last_reconciled_at: value.last_reconciled_at,
            last_error: value.last_error,
            worktree_missing: false,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedTaskGitExecution {
    pub task_git_context_id: String,
    pub working_dir: String,
    pub task_branch: String,
    pub target_branch: String,
    pub state: String,
    pub context_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitActionRequestResult {
    pub task_git_context_id: String,
    pub action_type: String,
    pub token: String,
    pub expires_at: String,
    pub state: String,
    pub context_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmGitActionResult {
    pub context: TaskGitContextSummary,
    pub action_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitCommit {
    pub sha: String,
    pub short_sha: String,
    pub subject: String,
    pub author_name: String,
    pub authored_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitWorkingTreeChange {
    pub path: String,
    pub previous_path: Option<String>,
    pub change_type: String,
    pub stage_status: String,
    pub can_open_file: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitFilePreview {
    pub project_id: String,
    pub relative_path: String,
    pub previous_path: Option<String>,
    pub absolute_path: Option<String>,
    pub previous_absolute_path: Option<String>,
    pub execution_target: String,
    pub change_type: String,
    pub before_status: String,
    pub before_text: Option<String>,
    pub before_truncated: bool,
    pub after_status: String,
    pub after_text: Option<String>,
    pub after_truncated: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitOverview {
    pub project_id: String,
    pub repo_path: Option<String>,
    pub execution_target: String,
    pub git_runtime_provider: String,
    pub git_runtime_status: String,
    pub git_runtime_message: Option<String>,
    pub default_branch: Option<String>,
    pub current_branch: Option<String>,
    pub project_branches: Vec<String>,
    pub head_commit_sha: Option<String>,
    pub working_tree_summary: Option<String>,
    pub ahead_commits: Option<u32>,
    pub behind_commits: Option<u32>,
    pub working_tree_changes: Vec<ProjectGitWorkingTreeChange>,
    pub refreshed_at: String,
    pub recent_commits: Vec<ProjectGitCommit>,
    pub active_contexts: Vec<TaskGitContextSummary>,
    pub pending_action_contexts: Vec<TaskGitContextSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGitCommitOverview {
    pub task_git_context_id: String,
    pub project_id: String,
    pub worktree_path: String,
    pub execution_target: String,
    pub current_branch: Option<String>,
    pub working_tree_summary: Option<String>,
    pub working_tree_changes: Vec<ProjectGitWorkingTreeChange>,
    pub refreshed_at: String,
}

#[derive(Debug, Clone)]
pub(crate) enum TaskGitAutoCommitOutcome {
    Committed { detail: String },
    MergeReady { detail: String },
    NoChanges { detail: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestGitActionInput {
    pub task_git_context_id: String,
    pub action_type: String,
    pub payload: Value,
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

fn parse_token(token: &str) -> Result<(&str, &str), String> {
    let mut parts = token.splitn(2, '.');
    let nonce = parts.next().unwrap_or_default();
    let signature = parts.next().unwrap_or_default();
    if nonce.trim().is_empty() || signature.trim().is_empty() {
        return Err("确认 token 格式无效".to_string());
    }
    Ok((nonce, signature))
}

#[derive(Clone, Debug)]
struct GitProjectRuntimeContext {
    repo_path: String,
    execution_target: String,
    ssh_config_id: Option<String>,
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
fn git_working_tree_is_clean(repo_path: &str) -> Result<bool, String> {
    Ok(run_git_text(repo_path, &["status", "--porcelain"])?
        .trim()
        .is_empty())
}

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

async fn determine_default_branch<R: Runtime>(
    app: &AppHandle<R>,
    runtime: &GitProjectRuntimeContext,
) -> Result<String, String> {
    #[cfg(test)]
    {
        if runtime.execution_target == EXECUTION_TARGET_LOCAL {
            if let Ok(value) = run_git_text(
                &runtime.repo_path,
                &["symbolic-ref", "refs/remotes/origin/HEAD", "--short"],
            ) {
                let branch = value.rsplit('/').next().unwrap_or(value.as_str()).trim();
                if !branch.is_empty() {
                    return Ok(branch.to_string());
                }
            }

            if let Ok(branch) =
                run_git_text(&runtime.repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])
            {
                let branch = branch.trim();
                if !branch.is_empty() && branch != "HEAD" {
                    return Ok(branch.to_string());
                }
            }

            if git_ref_exists_local(&runtime.repo_path, "refs/heads/main") {
                return Ok("main".to_string());
            }
            if git_ref_exists_local(&runtime.repo_path, "refs/heads/master") {
                return Ok("master".to_string());
            }

            return Err("无法解析默认目标分支".to_string());
        }
    }

    Ok(git_runtime::collect_git_overview(
        app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        1,
    )
    .await?
    .default_branch)
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

fn build_task_branch(task_id: &str) -> String {
    format!("codex/task-{}", sanitize_git_fragment(task_id))
}

fn build_worktree_path(repo_path: &str, task_id: &str) -> Result<String, String> {
    let repo = Path::new(repo_path);
    let repo_name = repo
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "无法解析仓库目录名".to_string())?;
    let parent = repo
        .parent()
        .ok_or_else(|| "无法解析仓库父目录".to_string())?;
    let path = parent
        .join(format!(
            ".codex-ai-worktrees-{}",
            sanitize_git_fragment(&repo_name)
        ))
        .join(sanitize_git_fragment(task_id));
    Ok(path.to_string_lossy().to_string())
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
    determine_default_branch(app, &worktree_runtime)
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
    let behind_commits = behind_raw
        .parse::<u32>()
        .map_err(|error| format!("解析 behind commits 失败: {}", error))?;
    let ahead_commits = ahead_raw
        .parse::<u32>()
        .map_err(|error| format!("解析 ahead commits 失败: {}", error))?;

    Ok(git_runtime::GitRuntimeRevisionComparison {
        ahead_commits,
        behind_commits,
    })
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
    let overview = git_runtime::collect_git_overview(
        app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &context.worktree_path,
        1,
    )
    .await?;
    let working_tree_changes = build_working_tree_changes(
        &context.worktree_path,
        &runtime.execution_target,
        git_runtime::collect_status_changes(
            app,
            &runtime.execution_target,
            runtime.ssh_config_id.as_deref(),
            &context.worktree_path,
        )
        .await?,
    );

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

fn can_open_repo_file_locally(repo_path: &str, relative_path: &str) -> bool {
    let candidate = Path::new(repo_path).join(relative_path);
    candidate.is_file()
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

fn has_stageable_worktree_changes(changes: &[ProjectGitWorkingTreeChange]) -> bool {
    changes.iter().any(|change| {
        matches!(
            change.stage_status.as_str(),
            "unstaged" | "untracked" | "partially_staged"
        )
    })
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

fn action_allows_drifted_context(action_type: &str) -> bool {
    action_type == "cleanup_worktree"
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
                    run_git_command(
                        repo_path,
                        &[
                            "worktree",
                            "remove",
                            context.worktree_path.as_str(),
                            "--force",
                        ],
                    )?;
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

async fn update_context_after_prepare<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    task: &Task,
    project: &Project,
    preferred_target_branch: Option<String>,
) -> Result<TaskGitContextRecord, String> {
    let runtime = resolve_project_runtime_context(project)?;
    let target_branch = match preferred_target_branch {
        Some(branch) => branch,
        None => determine_default_branch(app, &runtime).await?,
    };
    let task_branch = build_task_branch(&task.id);
    let worktree_path = build_worktree_path(&runtime.repo_path, &task.id)?;
    let head_commit =
        current_head_commit(app, &runtime, &runtime.repo_path, &target_branch).await?;

    if let Some(mut existing) = fetch_task_git_context_by_task_id(pool, &task.id).await? {
        if existing.target_branch != target_branch {
            return Err(format!(
                "当前任务已绑定目标分支 {}，不能切换到 {}",
                existing.target_branch, target_branch
            ));
        }
        if context_is_healthy(app, &runtime, &existing).await {
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
        existing.base_branch = existing.target_branch.clone();
        existing.repo_head_commit_at_prepare = Some(head_commit);
        existing.last_reconciled_at = Some(now_sqlite());
        existing.last_error = None;
        clear_pending_action_fields(&mut existing);
        existing.state = TASK_GIT_STATE_READY.to_string();
        existing.context_version += 1;
        existing.updated_at = now_sqlite();
        let saved = save_task_git_context(pool, &existing).await?;
        insert_activity_log(
            pool,
            "task_git_context_reconciled",
            "任务 Git 上下文已恢复可用",
            None,
            Some(&task.id),
            Some(&project.id),
        )
        .await?;
        return Ok(saved);
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
        let summary = summarize_task_git_context(&app, Some(&runtime), row).await;
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
        5,
    )
    .await
    {
        Ok(overview) => {
            let working_tree_changes = git_runtime::collect_status_changes(
                &app,
                &runtime.execution_target,
                runtime.ssh_config_id.as_deref(),
                &runtime.repo_path,
            )
            .await
            .map(|changes| {
                build_working_tree_changes(&runtime.repo_path, &runtime.execution_target, changes)
            })
            .unwrap_or_default();

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
            active_contexts,
            pending_action_contexts,
        }),
    }
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
            &commit_message,
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
    let trimmed = relative_path.trim();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }

    let normalized_previous_path = previous_path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let normalized_change_type = change_type
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "modified".to_string());

    let before_path = if normalized_change_type == "renamed" {
        normalized_previous_path.as_deref().unwrap_or(trimmed)
    } else {
        trimmed
    };
    let before_snapshot = if normalized_change_type == "added" {
        git_runtime::GitRuntimeTextSnapshot {
            status: "missing".to_string(),
            text: None,
            truncated: false,
        }
    } else {
        git_runtime::capture_head_text_snapshot(
            &app,
            &runtime.execution_target,
            runtime.ssh_config_id.as_deref(),
            &runtime.repo_path,
            before_path,
        )
        .await?
    };
    let after_snapshot = if normalized_change_type == "deleted" {
        git_runtime::GitRuntimeTextSnapshot {
            status: "missing".to_string(),
            text: None,
            truncated: false,
        }
    } else {
        git_runtime::capture_worktree_text_snapshot(
            &app,
            &runtime.execution_target,
            runtime.ssh_config_id.as_deref(),
            &runtime.repo_path,
            trimmed,
        )
        .await?
    };

    let absolute_path = Some(
        Path::new(&runtime.repo_path)
            .join(trimmed)
            .to_string_lossy()
            .to_string(),
    );
    let previous_absolute_path = normalized_previous_path.as_ref().map(|path| {
        Path::new(&runtime.repo_path)
            .join(path)
            .to_string_lossy()
            .to_string()
    });
    let message = if before_snapshot.status == "binary" || after_snapshot.status == "binary" {
        Some("当前变更包含二进制文件，Diff 仅支持文本预览".to_string())
    } else if before_snapshot.status == "unavailable" || after_snapshot.status == "unavailable" {
        Some("当前目标不是普通文本文件，暂不支持完整 Diff 预览".to_string())
    } else if before_snapshot.truncated || after_snapshot.truncated {
        Some("文件内容较长，当前只展示截断后的 Diff 预览".to_string())
    } else if before_snapshot.status == "missing" && after_snapshot.status == "missing" {
        Some("当前文件在基线和工作区中都不可用，无法生成 Diff".to_string())
    } else {
        None
    };

    insert_activity_log(
        &pool,
        "project_git_file_previewed",
        &format!("预览工作区文件：{}", trimmed),
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(ProjectGitFilePreview {
        project_id: project.id,
        relative_path: trimmed.to_string(),
        previous_path: normalized_previous_path,
        absolute_path,
        previous_absolute_path,
        execution_target: runtime.execution_target,
        change_type: normalized_change_type,
        before_status: before_snapshot.status,
        before_text: before_snapshot.text,
        before_truncated: before_snapshot.truncated,
        after_status: after_snapshot.status,
        after_text: after_snapshot.text,
        after_truncated: after_snapshot.truncated,
        message,
    })
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

async fn mutate_project_git_stage<R: Runtime>(
    app: &AppHandle<R>,
    project_id: &str,
    target_path: Option<&str>,
    stage: bool,
) -> Result<String, String> {
    let (pool, project, runtime) =
        resolve_project_runtime_for_git_overview(app, project_id).await?;
    let message = match (stage, target_path) {
        (true, Some(path)) => {
            git_runtime::stage_path(
                app,
                &runtime.execution_target,
                runtime.ssh_config_id.as_deref(),
                &runtime.repo_path,
                path,
            )
            .await?;
            let details = format!("已暂存工作区文件：{}", path);
            insert_activity_log(
                &pool,
                "project_git_file_staged",
                &details,
                None,
                None,
                Some(&project.id),
            )
            .await?;
            details
        }
        (false, Some(path)) => {
            git_runtime::unstage_path(
                app,
                &runtime.execution_target,
                runtime.ssh_config_id.as_deref(),
                &runtime.repo_path,
                path,
            )
            .await?;
            let details = format!("已取消暂存工作区文件：{}", path);
            insert_activity_log(
                &pool,
                "project_git_file_unstaged",
                &details,
                None,
                None,
                Some(&project.id),
            )
            .await?;
            details
        }
        (true, None) => {
            git_runtime::stage_all(
                app,
                &runtime.execution_target,
                runtime.ssh_config_id.as_deref(),
                &runtime.repo_path,
            )
            .await?;
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
            details
        }
        (false, None) => {
            git_runtime::unstage_all(
                app,
                &runtime.execution_target,
                runtime.ssh_config_id.as_deref(),
                &runtime.repo_path,
            )
            .await?;
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
            details
        }
    };
    Ok(message)
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
    mutate_project_git_stage(&app, &project_id, Some(&trimmed), true).await
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
    mutate_project_git_stage(&app, &project_id, Some(&trimmed), false).await
}

#[tauri::command]
pub async fn stage_all_project_git_files<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<String, String> {
    mutate_project_git_stage(&app, &project_id, None, true).await
}

#[tauri::command]
pub async fn unstage_all_project_git_files<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<String, String> {
    mutate_project_git_stage(&app, &project_id, None, false).await
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
    let result = git_runtime::commit_changes(
        &app,
        &runtime.execution_target,
        runtime.ssh_config_id.as_deref(),
        &runtime.repo_path,
        &trimmed,
    )
    .await?;

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
    if matches!(
        context.state.as_str(),
        TASK_GIT_STATE_PROVISIONING | TASK_GIT_STATE_FAILED | TASK_GIT_STATE_COMPLETED
    ) {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sqlx::SqlitePool;

    use super::*;
    use crate::app::build_current_migrator;

    async fn setup_test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");
        let migrator = build_current_migrator();
        let mut connection: sqlx::pool::PoolConnection<sqlx::Sqlite> =
            pool.acquire().await.expect("acquire sqlite connection");
        migrator
            .run_direct(&mut *connection)
            .await
            .expect("run migrations");
        drop(connection);
        pool
    }

    async fn insert_project_and_task(pool: &SqlitePool, repo_path: &str) -> (Project, Task) {
        let project = Project {
            id: "proj-1".to_string(),
            name: "Demo".to_string(),
            description: None,
            status: "active".to_string(),
            repo_path: Some(repo_path.to_string()),
            project_type: EXECUTION_TARGET_LOCAL.to_string(),
            ssh_config_id: None,
            remote_repo_path: None,
            created_at: now_sqlite(),
            updated_at: now_sqlite(),
        };
        sqlx::query(
            r#"INSERT INTO projects (id, name, description, status, repo_path, project_type, ssh_config_id, remote_repo_path, created_at, updated_at)
               VALUES ($1, $2, NULL, $3, $4, $5, NULL, NULL, $6, $7)"#,
        )
        .bind(&project.id)
        .bind(&project.name)
        .bind(&project.status)
        .bind(project.repo_path.as_deref())
        .bind(&project.project_type)
        .bind(&project.created_at)
        .bind(&project.updated_at)
        .execute(pool)
        .await
        .expect("insert project");

        let task = Task {
            id: "task-1".to_string(),
            title: "Prepare".to_string(),
            description: Some("demo".to_string()),
            status: "todo".to_string(),
            priority: "high".to_string(),
            project_id: project.id.clone(),
            use_worktree: true,
            assignee_id: None,
            reviewer_id: None,
            complexity: None,
            ai_suggestion: None,
            automation_mode: None,
            last_codex_session_id: None,
            last_review_session_id: None,
            created_at: now_sqlite(),
            updated_at: now_sqlite(),
        };
        sqlx::query(
            r#"INSERT INTO tasks (id, title, description, status, priority, project_id, use_worktree, assignee_id, reviewer_id, complexity, ai_suggestion, automation_mode, last_codex_session_id, last_review_session_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, NULL, NULL, NULL, NULL, NULL, NULL, $8, $9)"#,
        )
        .bind(&task.id)
        .bind(&task.title)
        .bind(task.description.as_deref())
        .bind(&task.status)
        .bind(&task.priority)
        .bind(&task.project_id)
        .bind(task.use_worktree)
        .bind(&task.created_at)
        .bind(&task.updated_at)
        .execute(pool)
        .await
        .expect("insert task");

        (project, task)
    }

    fn init_git_repo() -> String {
        let repo_root = std::env::temp_dir().join(format!(
            "codex-git-workflow-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        fs::create_dir_all(repo_root.join("src")).expect("create repo dir");
        fs::write(repo_root.join("src/main.ts"), "console.log('hello');\n").expect("write file");
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(&repo_root)
                .args(args)
                .status()
                .expect("run git");
            assert!(status.success(), "git {:?} should succeed", args);
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "codex@example.com"]);
        run(&["config", "user.name", "Codex"]);
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "init"]);
        repo_root.to_string_lossy().to_string()
    }

    async fn update_context_after_prepare_for_test(
        pool: &SqlitePool,
        task: &Task,
        project: &Project,
        preferred_target_branch: Option<String>,
    ) -> Result<TaskGitContextRecord, String> {
        let repo_path = project
            .repo_path
            .clone()
            .ok_or_else(|| "当前项目未配置本地仓库目录".to_string())?;
        ensure_git_repository(&repo_path)?;
        let target_branch = match preferred_target_branch {
            Some(branch) => branch,
            None => {
                if let Ok(value) = run_git_text(
                    &repo_path,
                    &["symbolic-ref", "refs/remotes/origin/HEAD", "--short"],
                ) {
                    let branch = value.rsplit('/').next().unwrap_or(value.as_str()).trim();
                    if !branch.is_empty() {
                        branch.to_string()
                    } else {
                        run_git_text(&repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])?
                    }
                } else {
                    run_git_text(&repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])?
                }
            }
        };
        let task_branch = build_task_branch(&task.id);
        let worktree_path = build_worktree_path(&repo_path, &task.id)?;
        let head_commit = run_git_text(&repo_path, &["rev-parse", &target_branch])?;

        if let Some(mut existing) = fetch_task_git_context_by_task_id(pool, &task.id).await? {
            if existing.target_branch != target_branch {
                return Err(format!(
                    "当前任务已绑定目标分支 {}，不能切换到 {}",
                    existing.target_branch, target_branch
                ));
            }
            if git_ref_exists_local(&repo_path, &format!("refs/heads/{}", existing.task_branch))
                && Path::new(&existing.worktree_path).join(".git").exists()
            {
                return Ok(existing);
            }

            let full_ref = format!("refs/heads/{}", existing.task_branch);
            if !git_ref_exists_local(&repo_path, &full_ref) {
                run_git_command(
                    &repo_path,
                    &["branch", &existing.task_branch, &existing.target_branch],
                )?;
            }
            let worktree = Path::new(&existing.worktree_path);
            if !worktree.join(".git").exists() {
                if worktree.exists() {
                    let is_empty = fs::read_dir(worktree)
                        .map_err(|error| format!("读取 worktree 目录失败: {}", error))?
                        .next()
                        .is_none();
                    if !is_empty {
                        return Err(format!(
                            "worktree 目录已存在且非空：{}",
                            existing.worktree_path
                        ));
                    }
                } else if let Some(parent) = worktree.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| format!("创建 worktree 父目录失败: {}", error))?;
                }
                run_git_command(
                    &repo_path,
                    &[
                        "worktree",
                        "add",
                        &existing.worktree_path,
                        &existing.task_branch,
                    ],
                )?;
            }
            existing.base_branch = existing.target_branch.clone();
            existing.repo_head_commit_at_prepare = Some(head_commit);
            existing.last_reconciled_at = Some(now_sqlite());
            existing.last_error = None;
            clear_pending_action_fields(&mut existing);
            existing.state = TASK_GIT_STATE_READY.to_string();
            existing.context_version += 1;
            existing.updated_at = now_sqlite();
            return save_task_git_context(pool, &existing).await;
        }

        let full_ref = format!("refs/heads/{task_branch}");
        if !git_ref_exists_local(&repo_path, &full_ref) {
            run_git_command(&repo_path, &["branch", &task_branch, &target_branch])?;
        }
        let worktree = Path::new(&worktree_path);
        if !worktree.join(".git").exists() {
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
            run_git_command(
                &repo_path,
                &["worktree", "add", &worktree_path, &task_branch],
            )?;
        }

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
        insert_task_git_context(pool, &record).await
    }

    #[test]
    fn list_local_branches_returns_local_heads() {
        let repo_path = init_git_repo();
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(&repo_path)
                .args(args)
                .status()
                .expect("run git");
            assert!(status.success(), "git {:?} should succeed", args);
        };
        run(&["branch", "release/1.0"]);
        run(&["branch", "feature/git-panel"]);

        let output = run_git_text(
            &repo_path,
            &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
        )
        .expect("list local branches");
        let branches = output
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();

        assert_eq!(
            branches,
            vec![
                "feature/git-panel".to_string(),
                "main".to_string(),
                "release/1.0".to_string(),
            ]
        );

        let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
    }

    #[test]
    fn prepare_task_git_execution_is_idempotent() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;

            let first = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("first prepare");
            let second = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("second prepare");

            assert_eq!(first.id, second.id);
            assert_eq!(first.worktree_path, second.worktree_path);
            assert!(Path::new(&first.worktree_path).join(".git").exists());

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&first.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            pool.close().await;
        });
    }

    #[test]
    fn pending_merge_requires_task_branch_to_be_ahead() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let repo_root = PathBuf::from(&repo_path);
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("prepare context");

            assert!(
                !task_git_context_has_pending_merge_local(&context)
                    .expect("initial pending merge state"),
                "fresh task branch should not require merge action"
            );

            fs::write(repo_root.join("README.md"), "# demo repo\n").expect("write repo update");
            run_git_command(&repo_path, &["add", "README.md"]).expect("stage repo update");
            run_git_command(&repo_path, &["commit", "-q", "-m", "repo update"])
                .expect("commit repo update");

            assert!(
                !task_git_context_has_pending_merge_local(&context)
                    .expect("target-ahead pending merge state"),
                "target branch moving ahead alone should not mark task branch merge-ready"
            );

            fs::write(
                PathBuf::from(&context.worktree_path).join("src/main.ts"),
                "console.log('task change');\n",
            )
            .expect("write task branch update");
            run_git_command(&context.worktree_path, &["add", "src/main.ts"])
                .expect("stage task branch update");
            run_git_command(
                &context.worktree_path,
                &["commit", "-q", "-m", "task branch update"],
            )
            .expect("commit task branch update");

            assert!(
                task_git_context_has_pending_merge_local(&context)
                    .expect("task-ahead pending merge state"),
                "task branch commits should still be recognized as pending merge work"
            );

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&context.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            pool.close().await;
        });
    }

    #[test]
    fn pending_merge_uses_worktree_head_when_detached() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("prepare context");
            let initial_task_branch_head =
                run_git_text(&repo_path, &["rev-parse", &context.task_branch]).expect("task ref");

            run_git_command(&context.worktree_path, &["checkout", "--detach"])
                .expect("detach worktree head");
            fs::write(
                PathBuf::from(&context.worktree_path).join("src/main.ts"),
                "console.log('detached head change');\n",
            )
            .expect("write detached head update");
            run_git_command(&context.worktree_path, &["add", "src/main.ts"])
                .expect("stage detached head update");
            run_git_command(
                &context.worktree_path,
                &["commit", "-q", "-m", "detached head update"],
            )
            .expect("commit detached head update");

            let detached_head =
                run_git_text(&context.worktree_path, &["rev-parse", "HEAD"]).expect("head");
            let task_branch_head =
                run_git_text(&repo_path, &["rev-parse", &context.task_branch]).expect("task ref");

            assert_ne!(
                detached_head, task_branch_head,
                "detached HEAD commit should not advance the tracked task branch ref"
            );
            assert_eq!(
                task_branch_head, initial_task_branch_head,
                "task branch ref should stay unchanged when committing from detached HEAD"
            );
            assert!(
                task_git_context_has_pending_merge_local(&context)
                    .expect("detached head pending merge state"),
                "worktree HEAD commits should still be recognized as pending merge work"
            );

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&context.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            pool.close().await;
        });
    }

    #[test]
    fn request_and_cancel_git_action_invalidate_token() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("prepare context");
            let mut stored = fetch_task_git_context_by_id(&pool, &context.id)
                .await
                .expect("fetch context");
            let input = RequestGitActionInput {
                task_git_context_id: context.id.clone(),
                action_type: "stash".to_string(),
                payload: serde_json::json!({ "include_untracked": true }),
            };
            let normalized_payload = normalize_git_action_payload("stash", &stored, &input.payload)
                .expect("normalize payload");
            let expires_at = sqlite_now_with_offset(PENDING_ACTION_TTL_MINUTES);
            let nonce = Uuid::new_v4().to_string();
            let signature = build_pending_action_signature(
                &stored.id,
                "stash",
                &normalized_payload,
                &nonce,
                &expires_at,
                stored.context_version + 1,
            );
            stored.state = TASK_GIT_STATE_ACTION_PENDING.to_string();
            stored.context_version += 1;
            stored.pending_action_type = Some("stash".to_string());
            stored.pending_action_token_hash = Some(signature.clone());
            stored.pending_action_payload_json = Some(normalized_payload);
            stored.pending_action_nonce = Some(nonce.clone());
            stored.pending_action_requested_at = Some(now_sqlite());
            stored.pending_action_expires_at = Some(expires_at);
            stored.pending_action_repo_revision =
                Some(run_git_text(&stored.worktree_path, &["rev-parse", "HEAD"]).expect("head"));
            stored.pending_action_bound_context_version = Some(stored.context_version);
            stored.updated_at = now_sqlite();
            let saved = save_task_git_context(&pool, &stored)
                .await
                .expect("save pending action");

            let cancelled = {
                let token = format!("{}.{}", nonce, signature);
                let mut context = fetch_task_git_context_by_id(&pool, &saved.id)
                    .await
                    .expect("fetch saved");
                let (parsed_nonce, parsed_signature) = parse_token(&token).expect("parse token");
                assert_eq!(parsed_nonce, nonce);
                assert_eq!(parsed_signature, signature);
                clear_pending_action_fields(&mut context);
                context.context_version += 1;
                context.state = TASK_GIT_STATE_MERGE_READY.to_string();
                context.updated_at = now_sqlite();
                save_task_git_context(&pool, &context)
                    .await
                    .expect("cancel save")
            };

            assert_eq!(cancelled.pending_action_type, None);
            assert_eq!(cancelled.state, TASK_GIT_STATE_MERGE_READY);

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&context.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            pool.close().await;
        });
    }

    #[test]
    fn merge_action_merges_task_branch_into_target_branch() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let repo_root = PathBuf::from(&repo_path);
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("prepare context");

            fs::write(repo_root.join("README.md"), "# demo repo\n").expect("write repo file");
            run_git_command(&repo_path, &["add", "README.md"]).expect("stage repo file");
            run_git_command(&repo_path, &["commit", "-q", "-m", "repo baseline"])
                .expect("commit repo baseline");
            run_git_command(&context.worktree_path, &["rebase", "main"]).expect("sync worktree");

            fs::write(
                PathBuf::from(&context.worktree_path).join("src/main.ts"),
                "console.log('merged from task branch');\n",
            )
            .expect("write worktree file");
            run_git_command(&context.worktree_path, &["add", "src/main.ts"])
                .expect("stage worktree change");
            run_git_command(
                &context.worktree_path,
                &["commit", "-q", "-m", "task change"],
            )
            .expect("commit worktree change");

            let main_before =
                run_git_text(&repo_path, &["rev-parse", "main"]).expect("main before");
            let task_head =
                run_git_text(&context.worktree_path, &["rev-parse", "HEAD"]).expect("task head");

            let message =
                merge_task_branch_into_target_local(&repo_path, &context, "main", "ort", true)
                    .expect("merge task branch");

            let main_after = run_git_text(&repo_path, &["rev-parse", "main"]).expect("main after");
            let current_branch =
                run_git_text(&repo_path, &["rev-parse", "--abbrev-ref", "HEAD"]).expect("branch");
            let merged_source =
                fs::read_to_string(repo_root.join("src/main.ts")).expect("read merged file");

            assert_eq!(
                message,
                format!("已将任务分支 {} 合并到目标分支 main", context.task_branch)
            );
            assert_ne!(main_before, main_after);
            assert_eq!(main_after, task_head);
            assert_eq!(current_branch, "main");
            assert!(merged_source.contains("merged from task branch"));

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&context.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            pool.close().await;
        });
    }

    #[test]
    fn delete_task_git_context_removes_record() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("prepare context");

            delete_task_git_context(&pool, &context.id)
                .await
                .expect("delete task git context");

            let deleted = fetch_task_git_context_by_id(&pool, &context.id).await;
            assert!(deleted.is_err(), "task git context should be deleted");

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            pool.close().await;
        });
    }
}
