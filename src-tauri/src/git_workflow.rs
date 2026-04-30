use std::collections::{hash_map::DefaultHasher, HashMap};
#[cfg(test)]
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
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
use crate::codex::{
    generate_commit_message_for_project, load_codex_settings, load_remote_codex_settings,
};
use crate::db::models::{GitPreferences, Project, Task};
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
const PROJECT_GIT_RECENT_COMMIT_SUMMARY_LIMIT: usize = 5;
const PROJECT_GIT_COMMIT_HISTORY_PAGE_LIMIT_DEFAULT: usize = 20;
const PROJECT_GIT_COMMIT_HISTORY_PAGE_LIMIT_MAX: usize = 50;

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
pub struct ProjectGitCommitHistory {
    pub commits: Vec<ProjectGitCommit>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitCommitFileChange {
    pub path: String,
    pub previous_path: Option<String>,
    pub change_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitCommitDetail {
    pub project_id: String,
    pub execution_target: String,
    pub sha: String,
    pub short_sha: String,
    pub subject: String,
    pub body: Option<String>,
    pub author_name: String,
    pub author_email: Option<String>,
    pub authored_at: String,
    pub diff_text: Option<String>,
    pub diff_truncated: bool,
    pub changed_files: Vec<ProjectGitCommitFileChange>,
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
    pub before_label: String,
    pub before_status: String,
    pub before_text: Option<String>,
    pub before_truncated: bool,
    pub after_label: String,
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
    pub recent_commits_has_more: bool,
    pub active_contexts: Vec<TaskGitContextSummary>,
    pub pending_action_contexts: Vec<TaskGitContextSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGitWorktree {
    pub path: String,
    pub branch: Option<String>,
    pub head_sha: Option<String>,
    pub short_head_sha: Option<String>,
    pub is_main: bool,
    pub is_bare: bool,
    pub is_detached: bool,
    pub is_locked: bool,
    pub lock_reason: Option<String>,
    pub is_prunable: bool,
    pub prunable_reason: Option<String>,
    pub task_git_context_id: Option<String>,
    pub task_id: Option<String>,
    pub task_title: Option<String>,
    pub working_tree_summary: Option<String>,
    pub working_tree_changes: Vec<ProjectGitWorkingTreeChange>,
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

#[derive(Debug, Clone, Default)]
struct RawWorktreeEntry {
    path: String,
    branch_ref: Option<String>,
    head_sha: Option<String>,
    is_bare: bool,
    is_detached: bool,
    is_locked: bool,
    lock_reason: Option<String>,
    is_prunable: bool,
    prunable_reason: Option<String>,
}

impl RawWorktreeEntry {
    fn branch_name(&self) -> Option<String> {
        self.branch_ref
            .as_deref()
            .and_then(normalize_git_branch_ref)
    }
}

#[derive(Debug, Clone, FromRow)]
struct TaskGitContextWorktreeRow {
    id: String,
    task_id: String,
    worktree_path: String,
    task_title: Option<String>,
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

fn normalize_worktree_path_key(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed == "/" {
        return "/".to_string();
    }
    trimmed.trim_end_matches('/').to_string()
}

fn short_git_sha(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(7).collect::<String>())
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

impl GitProjectRuntimeContext {
    fn with_repo_path(&self, repo_path: impl Into<String>) -> Self {
        Self {
            repo_path: repo_path.into(),
            execution_target: self.execution_target.clone(),
            ssh_config_id: self.ssh_config_id.clone(),
        }
    }
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

fn build_task_branch(task_id: &str) -> String {
    format!("codex/task-{}", sanitize_git_fragment(task_id))
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
        PROJECT_GIT_RECENT_COMMIT_SUMMARY_LIMIT,
    )
    .await
    {
        Ok(overview) => {
            let working_tree_changes =
                collect_working_tree_changes(&app, &runtime, &runtime.repo_path)
                    .await
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

async fn resolve_project_runtime_for_git_overview<R: Runtime>(
    app: &AppHandle<R>,
    project_id: &str,
) -> Result<(SqlitePool, Project, GitProjectRuntimeContext), String> {
    let pool = sqlite_pool(app).await?;
    let project = fetch_project_by_id(&pool, project_id).await?;
    let runtime = resolve_project_runtime_context(&project)?;
    Ok((pool, project, runtime))
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

    fn default_test_git_preferences() -> GitPreferences {
        GitPreferences {
            default_task_use_worktree: false,
            worktree_location_mode: "repo_sibling_hidden".to_string(),
            worktree_custom_root: None,
            ai_commit_message_length: "title_with_body".to_string(),
            ai_commit_preferred_provider: "codex".to_string(),
            ai_commit_model_source: "inherit_one_shot".to_string(),
            ai_commit_model: "gpt-5.4".to_string(),
            ai_commit_reasoning_effort: "high".to_string(),
        }
    }

    fn build_task_worktree_path_for_local_test(
        repo_path: &str,
        task_id: &str,
        git_preferences: &GitPreferences,
    ) -> Result<String, String> {
        if git_preferences.worktree_location_mode != "repo_child_hidden" {
            return build_worktree_path(repo_path, task_id, git_preferences);
        }

        let task_slug = sanitize_git_fragment(task_id);
        let git_common_dir_path = resolve_repo_child_worktree_root_local(repo_path)?;
        let path = build_repo_child_worktree_path(&git_common_dir_path, &task_slug)?;
        Ok(path.to_string_lossy().to_string())
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
            deleted_at: None,
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
            coordinator_id: None,
            complexity: None,
            ai_suggestion: None,
            plan_content: None,
            automation_mode: None,
            last_codex_session_id: None,
            last_review_session_id: None,
            time_started_at: None,
            time_spent_seconds: 0,
            completed_at: None,
            deleted_at: None,
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

    fn build_local_preview_project(repo_path: &str) -> Project {
        Project {
            id: "proj-preview".to_string(),
            name: "Preview".to_string(),
            description: None,
            status: "active".to_string(),
            repo_path: Some(repo_path.to_string()),
            project_type: EXECUTION_TARGET_LOCAL.to_string(),
            ssh_config_id: None,
            remote_repo_path: None,
            deleted_at: None,
            created_at: now_sqlite(),
            updated_at: now_sqlite(),
        }
    }

    fn capture_revision_text_snapshot_local(
        repo_path: &str,
        revision: &str,
        relative_path: &str,
    ) -> git_runtime::GitRuntimeTextSnapshot {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(["show", &format!("{revision}:{relative_path}")])
            .output();
        match output {
            Ok(output) if output.status.success() => {
                if output.stdout.contains(&0) {
                    git_runtime::GitRuntimeTextSnapshot {
                        status: "binary".to_string(),
                        text: None,
                        truncated: false,
                    }
                } else {
                    git_runtime::GitRuntimeTextSnapshot {
                        status: "text".to_string(),
                        text: Some(String::from_utf8_lossy(&output.stdout).to_string()),
                        truncated: output.stdout.len() > 256 * 1024,
                    }
                }
            }
            _ => missing_project_git_text_snapshot(),
        }
    }

    fn build_project_git_commit_file_preview_local(
        project: &Project,
        commit_sha: &str,
        relative_path: &str,
        previous_path: Option<String>,
        change_type: Option<String>,
    ) -> Result<ProjectGitFilePreview, String> {
        let runtime = resolve_project_runtime_context(project)?;
        let trimmed_commit_sha = commit_sha.trim().to_string();
        if trimmed_commit_sha.is_empty() {
            return Err("提交 SHA 不能为空".to_string());
        }

        let trimmed_path = relative_path.trim();
        if trimmed_path.is_empty() {
            return Err("文件路径不能为空".to_string());
        }

        let normalized_previous_path = normalize_project_git_relative_path(previous_path);
        let normalized_change_type = normalize_project_git_change_type(change_type);
        let parent_revision = format!("{trimmed_commit_sha}^1");
        let before_path = if normalized_change_type == "renamed" {
            normalized_previous_path.as_deref().unwrap_or(trimmed_path)
        } else {
            trimmed_path
        };
        let before_snapshot = if normalized_change_type == "added" {
            missing_project_git_text_snapshot()
        } else {
            capture_revision_text_snapshot_local(&runtime.repo_path, &parent_revision, before_path)
        };
        let after_snapshot = if normalized_change_type == "deleted" {
            missing_project_git_text_snapshot()
        } else {
            capture_revision_text_snapshot_local(
                &runtime.repo_path,
                &trimmed_commit_sha,
                trimmed_path,
            )
        };
        let (before_label, after_label) =
            build_project_git_commit_preview_labels(&trimmed_commit_sha);

        Ok(build_project_git_file_preview(
            project.id.clone(),
            runtime.execution_target,
            &runtime.repo_path,
            trimmed_path,
            normalized_previous_path,
            normalized_change_type,
            before_label,
            after_label,
            before_snapshot,
            after_snapshot,
        ))
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
            None => determine_current_branch_local(&repo_path)?,
        };
        let task_branch = build_task_branch(&task.id);
        let worktree_path = build_task_worktree_path_for_local_test(
            &repo_path,
            &task.id,
            &default_test_git_preferences(),
        )?;
        let head_commit = run_git_text(&repo_path, &["rev-parse", &target_branch])?;

        if let Some(existing) = fetch_task_git_context_by_task_id(pool, &task.id).await? {
            if existing.target_branch != target_branch {
                return Err(format!(
                    "当前任务已绑定目标分支 {}，不能切换到 {}",
                    existing.target_branch, target_branch
                ));
            }
            if git_ref_exists_local(&repo_path, &format!("refs/heads/{}", existing.task_branch))
                && Path::new(&existing.worktree_path).join(".git").exists()
            {
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
            return mark_task_git_context_reconciled_after_prepare(
                pool,
                existing,
                head_commit,
                "任务 Git 上下文已恢复可用",
            )
            .await;
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
    fn build_worktree_path_supports_all_configured_modes() {
        let repo_path = "/tmp/demo-repo";
        let task_id = "task/42";

        let sibling_path = build_worktree_path(repo_path, task_id, &default_test_git_preferences())
            .expect("build sibling path");
        assert_eq!(sibling_path, "/tmp/.codex-ai-worktrees-demo-repo/task-42");

        let child_path = build_worktree_path(
            repo_path,
            task_id,
            &GitPreferences {
                worktree_location_mode: "repo_child_hidden".to_string(),
                ..default_test_git_preferences()
            },
        )
        .expect("build child path");
        assert_eq!(child_path, "/tmp/demo-repo/.git/codex-ai-worktrees/task-42");

        let custom_path = build_worktree_path(
            repo_path,
            task_id,
            &GitPreferences {
                worktree_location_mode: "custom_root".to_string(),
                worktree_custom_root: Some("/worktrees".to_string()),
                ..default_test_git_preferences()
            },
        )
        .expect("build custom path");
        assert_eq!(custom_path, "/worktrees/demo-repo/task-42");
    }

    #[test]
    fn resolve_repo_child_worktree_root_uses_common_git_dir_for_linked_worktree() {
        let repo_path = init_git_repo();
        let linked_worktree = PathBuf::from(&repo_path)
            .parent()
            .expect("repo parent")
            .join("linked-worktree");
        run_git_command(
            &repo_path,
            &[
                "worktree",
                "add",
                "-b",
                "feature/linked",
                linked_worktree.to_string_lossy().as_ref(),
                "main",
            ],
        )
        .expect("create linked worktree");

        let resolved =
            resolve_repo_child_worktree_root_local(linked_worktree.to_string_lossy().as_ref())
                .expect("resolve linked worktree common git dir");
        assert!(
            resolved.ends_with("/.git"),
            "unexpected common git dir: {resolved}"
        );

        let child_path = build_repo_child_worktree_path(&resolved, "task-42")
            .expect("build repo child worktree path");
        assert!(
            child_path
                .to_string_lossy()
                .ends_with("/.git/codex-ai-worktrees/task-42"),
            "unexpected child path: {}",
            child_path.to_string_lossy()
        );

        let _ = fs::remove_dir_all(linked_worktree);
        let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
    }

    #[test]
    fn parse_worktree_list_porcelain_handles_flags_and_reasons() {
        let parsed = parse_worktree_list_porcelain(
            r#"worktree /tmp/demo
HEAD 0123456789abcdef
branch refs/heads/main

worktree /tmp/demo-feature
HEAD fedcba9876543210
branch refs/heads/feature/demo
locked manual keep

worktree /tmp/demo-stale
HEAD 1111111111111111
detached
prunable gitdir file points to non-existent location
"#,
        )
        .expect("parse worktree list");

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].path, "/tmp/demo");
        assert_eq!(parsed[0].branch_name().as_deref(), Some("main"));
        assert!(!parsed[0].is_locked);
        assert!(!parsed[0].is_prunable);

        assert_eq!(parsed[1].branch_name().as_deref(), Some("feature/demo"));
        assert!(parsed[1].is_locked);
        assert_eq!(parsed[1].lock_reason.as_deref(), Some("manual keep"));

        assert!(parsed[2].is_detached);
        assert!(parsed[2].is_prunable);
        assert_eq!(
            parsed[2].prunable_reason.as_deref(),
            Some("gitdir file points to non-existent location")
        );
    }

    #[test]
    fn parse_worktree_list_porcelain_reads_real_git_output() {
        let repo_path = init_git_repo();
        let linked_worktree = PathBuf::from(&repo_path)
            .parent()
            .expect("repo parent")
            .join(format!("linked-worktree-parse-{}", Uuid::new_v4()));
        run_git_command(
            &repo_path,
            &[
                "worktree",
                "add",
                "-b",
                "feature/parse",
                linked_worktree.to_string_lossy().as_ref(),
                "main",
            ],
        )
        .expect("create linked worktree");

        let output =
            run_git_text(&repo_path, &["worktree", "list", "--porcelain"]).expect("list worktrees");
        let parsed = parse_worktree_list_porcelain(&output).expect("parse real worktree output");
        let linked_worktree_canonical = linked_worktree
            .canonicalize()
            .expect("canonical linked worktree path");

        assert_eq!(parsed.len(), 2);
        assert!(parsed
            .iter()
            .any(|entry| entry.branch_name().as_deref() == Some("main")));
        assert!(parsed.iter().any(|entry| {
            entry.branch_name().as_deref() == Some("feature/parse")
                && PathBuf::from(&entry.path)
                    .canonicalize()
                    .map(|path| path == linked_worktree_canonical)
                    .unwrap_or(false)
        }));

        let _ = fs::remove_dir_all(linked_worktree);
        let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
    }

    #[test]
    fn cleanup_worktree_action_is_allowed_after_completion() {
        assert!(action_allows_completed_context("cleanup_worktree"));
        assert!(!action_allows_completed_context("merge"));
        assert!(!action_allows_completed_context("push"));
    }

    #[test]
    fn cleanup_worktree_force_remove_is_allowed_after_failure() {
        assert!(action_allows_failed_context(
            "cleanup_worktree",
            &serde_json::json!({ "force_remove": true }),
        )
        .expect("force cleanup payload should parse"));
        assert!(
            action_allows_failed_context("cleanup_worktree", &serde_json::json!({}))
                .expect("default cleanup payload should parse")
        );
        assert!(!action_allows_failed_context(
            "cleanup_worktree",
            &serde_json::json!({ "force_remove": false }),
        )
        .expect("non-force cleanup payload should parse"));
        assert!(!action_allows_failed_context(
            "merge",
            &serde_json::json!({ "force_remove": true })
        )
        .expect("non-cleanup action should not inspect payload"));
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
    fn prepare_task_git_execution_recovers_failed_healthy_context() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;

            let first = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("first prepare");
            let mut failed = fetch_task_git_context_by_id(&pool, &first.id)
                .await
                .expect("fetch context");
            failed.state = TASK_GIT_STATE_FAILED.to_string();
            failed.last_error = Some("previous launch failed".to_string());
            failed.context_version += 1;
            let failed = save_task_git_context(&pool, &failed)
                .await
                .expect("save failed context");

            let recovered = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("recover failed context");

            assert_eq!(recovered.id, first.id);
            assert_eq!(recovered.state, TASK_GIT_STATE_READY);
            assert_eq!(recovered.last_error, None);
            assert!(recovered.context_version > failed.context_version);
            assert!(Path::new(&recovered.worktree_path).join(".git").exists());

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&recovered.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            pool.close().await;
        });
    }

    #[test]
    fn prepare_task_git_execution_prefers_current_branch_over_origin_head() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let repo_root = PathBuf::from(&repo_path);
            let remote_root = std::env::temp_dir().join(format!(
                "codex-git-workflow-origin-{}-{}",
                std::process::id(),
                Uuid::new_v4()
            ));
            fs::create_dir_all(&remote_root).expect("create remote dir");
            let remote_root_str = remote_root.to_string_lossy().to_string();
            let run = |args: &[&str]| {
                let status = Command::new("git")
                    .arg("-C")
                    .arg(&repo_path)
                    .args(args)
                    .status()
                    .expect("run git");
                assert!(status.success(), "git {:?} should succeed", args);
            };

            run(&["init", "--bare", "-q", &remote_root_str]);
            run(&["remote", "add", "origin", &remote_root_str]);
            run(&["push", "-u", "origin", "main"]);
            run(&["remote", "set-head", "origin", "main"]);
            run(&["checkout", "-q", "-b", "feature/current-base"]);

            fs::write(repo_root.join("FEATURE.md"), "feature base\n").expect("write feature file");
            run(&["add", "FEATURE.md"]);
            run(&["commit", "-q", "-m", "feature base"]);

            let main_head = run_git_text(&repo_path, &["rev-parse", "main"]).expect("main head");
            let feature_head =
                run_git_text(&repo_path, &["rev-parse", "HEAD"]).expect("feature head");
            assert_ne!(main_head, feature_head);

            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare_for_test(&pool, &task, &project, None)
                .await
                .expect("prepare context");
            let worktree_head = run_git_text(&context.worktree_path, &["rev-parse", "HEAD"])
                .expect("worktree head");

            assert_eq!(context.target_branch, "feature/current-base");
            assert_eq!(context.base_branch, "feature/current-base");
            assert_eq!(
                context.repo_head_commit_at_prepare.as_deref(),
                Some(feature_head.as_str())
            );
            assert_eq!(worktree_head, feature_head);

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(
                PathBuf::from(&context.worktree_path)
                    .parent()
                    .unwrap_or(Path::new("")),
            );
            let _ = fs::remove_dir_all(&remote_root);
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
    fn project_git_commit_file_preview_covers_root_modified_added_renamed_and_deleted() {
        let repo_path = init_git_repo();
        let repo_root = PathBuf::from(&repo_path);
        let project = build_local_preview_project(&repo_path);
        let root_commit =
            run_git_text(&repo_path, &["rev-list", "--max-parents=0", "HEAD"]).expect("root sha");

        fs::create_dir_all(repo_root.join("docs")).expect("create docs dir");
        fs::write(repo_root.join("docs/guide.txt"), "base guide\n").expect("write base guide");
        run_git_command(&repo_path, &["add", "docs/guide.txt"]).expect("stage base guide");
        run_git_command(&repo_path, &["commit", "-q", "-m", "add guide"]).expect("commit guide");

        fs::write(repo_root.join("docs/guide.txt"), "modified guide\n")
            .expect("write modified guide");
        run_git_command(&repo_path, &["add", "docs/guide.txt"]).expect("stage modified guide");
        run_git_command(&repo_path, &["commit", "-q", "-m", "modify guide"])
            .expect("commit modified guide");
        let modified_commit =
            run_git_text(&repo_path, &["rev-parse", "HEAD"]).expect("modified sha");

        fs::write(
            repo_root.join("src/feature.ts"),
            "export const feature = true;\n",
        )
        .expect("write added file");
        run_git_command(&repo_path, &["add", "src/feature.ts"]).expect("stage added file");
        run_git_command(&repo_path, &["commit", "-q", "-m", "add feature"])
            .expect("commit added file");
        let added_commit = run_git_text(&repo_path, &["rev-parse", "HEAD"]).expect("added sha");

        run_git_command(
            &repo_path,
            &["mv", "docs/guide.txt", "docs/guide-renamed.txt"],
        )
        .expect("rename guide");
        run_git_command(&repo_path, &["commit", "-q", "-m", "rename guide"])
            .expect("commit renamed file");
        let renamed_commit = run_git_text(&repo_path, &["rev-parse", "HEAD"]).expect("renamed sha");

        run_git_command(&repo_path, &["rm", "src/feature.ts"]).expect("delete feature");
        run_git_command(&repo_path, &["commit", "-q", "-m", "delete feature"])
            .expect("commit deleted file");
        let deleted_commit = run_git_text(&repo_path, &["rev-parse", "HEAD"]).expect("deleted sha");

        let root_preview = build_project_git_commit_file_preview_local(
            &project,
            &root_commit,
            "src/main.ts",
            None,
            Some("modified".to_string()),
        )
        .expect("root preview");
        assert_eq!(root_preview.before_status, "missing");
        assert_eq!(root_preview.after_status, "text");
        assert_eq!(root_preview.before_label, "父提交");
        assert!(root_preview
            .after_label
            .contains(&root_commit.chars().take(7).collect::<String>()));
        assert_eq!(
            root_preview.after_text.as_deref(),
            Some("console.log('hello');\n")
        );

        let modified_preview = build_project_git_commit_file_preview_local(
            &project,
            &modified_commit,
            "docs/guide.txt",
            None,
            Some("modified".to_string()),
        )
        .expect("modified preview");
        assert_eq!(modified_preview.before_status, "text");
        assert_eq!(modified_preview.after_status, "text");
        assert_eq!(
            modified_preview.before_text.as_deref(),
            Some("base guide\n")
        );
        assert_eq!(
            modified_preview.after_text.as_deref(),
            Some("modified guide\n")
        );

        let added_preview = build_project_git_commit_file_preview_local(
            &project,
            &added_commit,
            "src/feature.ts",
            None,
            Some("added".to_string()),
        )
        .expect("added preview");
        assert_eq!(added_preview.before_status, "missing");
        assert_eq!(added_preview.after_status, "text");
        assert_eq!(
            added_preview.after_text.as_deref(),
            Some("export const feature = true;\n")
        );

        let renamed_preview = build_project_git_commit_file_preview_local(
            &project,
            &renamed_commit,
            "docs/guide-renamed.txt",
            Some("docs/guide.txt".to_string()),
            Some("renamed".to_string()),
        )
        .expect("renamed preview");
        let expected_previous_absolute_path = repo_root.join("docs/guide.txt");
        assert_eq!(
            renamed_preview.previous_path.as_deref(),
            Some("docs/guide.txt")
        );
        assert_eq!(
            renamed_preview.previous_absolute_path.as_deref(),
            Some(expected_previous_absolute_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            renamed_preview.before_text.as_deref(),
            Some("modified guide\n")
        );
        assert_eq!(
            renamed_preview.after_text.as_deref(),
            Some("modified guide\n")
        );

        let deleted_preview = build_project_git_commit_file_preview_local(
            &project,
            &deleted_commit,
            "src/feature.ts",
            None,
            Some("deleted".to_string()),
        )
        .expect("deleted preview");
        assert_eq!(deleted_preview.before_status, "text");
        assert_eq!(deleted_preview.after_status, "missing");
        assert_eq!(
            deleted_preview.before_text.as_deref(),
            Some("export const feature = true;\n")
        );

        let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
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
