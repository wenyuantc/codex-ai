use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::Command;

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{FromRow, SqlitePool};
use tauri::{AppHandle, Runtime};
use uuid::Uuid;

use crate::app::{
    fetch_project_by_id, fetch_task_by_id, insert_activity_log, now_sqlite, sqlite_pool,
    PROJECT_TYPE_LOCAL,
};
use crate::db::models::{Project, Task};

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

fn git_ref_exists(repo_path: &str, full_ref: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["show-ref", "--verify", "--quiet", full_ref])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn ensure_git_repository(repo_path: &str) -> Result<(), String> {
    let git_dir = Path::new(repo_path).join(".git");
    if git_dir.exists() {
        Ok(())
    } else {
        Err(format!("工作目录 {} 不是 Git 仓库，缺少 .git", repo_path))
    }
}

fn ensure_local_project_repo(project: &Project) -> Result<String, String> {
    if project.project_type != PROJECT_TYPE_LOCAL {
        return Err("SSH 项目暂不支持自动 prepare task git context".to_string());
    }
    let repo_path = project
        .repo_path
        .clone()
        .ok_or_else(|| "当前项目未配置本地仓库目录".to_string())?;
    ensure_git_repository(&repo_path)?;
    Ok(repo_path)
}

fn determine_default_branch(repo_path: &str) -> Result<String, String> {
    if let Ok(value) = run_git_text(repo_path, &["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
    {
        let branch = value.rsplit('/').next().unwrap_or(value.as_str()).trim();
        if !branch.is_empty() {
            return Ok(branch.to_string());
        }
    }

    if let Ok(branch) = run_git_text(repo_path, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        let branch = branch.trim();
        if !branch.is_empty() && branch != "HEAD" {
            return Ok(branch.to_string());
        }
    }

    if git_ref_exists(repo_path, "refs/heads/main") {
        return Ok("main".to_string());
    }
    if git_ref_exists(repo_path, "refs/heads/master") {
        return Ok("master".to_string());
    }

    Err("无法解析默认目标分支".to_string())
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
        .join(format!(".codex-ai-worktrees-{}", sanitize_git_fragment(&repo_name)))
        .join(sanitize_git_fragment(task_id));
    Ok(path.to_string_lossy().to_string())
}

fn context_is_healthy(repo_path: &str, context: &TaskGitContextRecord) -> bool {
    if !git_ref_exists(repo_path, &format!("refs/heads/{}", context.task_branch)) {
        return false;
    }
    let worktree = Path::new(&context.worktree_path);
    worktree.exists() && worktree.join(".git").exists()
}

fn ensure_task_branch(repo_path: &str, task_branch: &str, target_branch: &str) -> Result<(), String> {
    let full_ref = format!("refs/heads/{task_branch}");
    if git_ref_exists(repo_path, &full_ref) {
        return Ok(());
    }
    run_git_command(repo_path, &["branch", task_branch, target_branch])
}

fn ensure_task_worktree(repo_path: &str, worktree_path: &str, task_branch: &str) -> Result<(), String> {
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
        fs::create_dir_all(parent).map_err(|error| format!("创建 worktree 父目录失败: {}", error))?;
    }
    run_git_command(repo_path, &["worktree", "add", worktree_path, task_branch])
}

fn current_head_commit(repo_path: &str, revision: &str) -> Result<String, String> {
    run_git_text(repo_path, &["rev-parse", revision])
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
    map.get(key).and_then(|value| value.as_bool()).unwrap_or(default)
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

    serde_json::to_string(&normalized).map_err(|error| format!("序列化规范化 payload 失败: {}", error))
}

fn execute_normalized_action(
    repo_path: &str,
    context: &TaskGitContextRecord,
    action_type: &str,
    normalized_payload_json: &str,
) -> Result<String, String> {
    let payload: Value = serde_json::from_str(normalized_payload_json)
        .map_err(|error| format!("解析规范化 payload 失败: {}", error))?;
    let map = payload_object(&payload)?;
    match action_type {
        "merge" => {
            let target_branch = payload_string(map, "target_branch")
                .ok_or_else(|| "merge 缺少 target_branch".to_string())?;
            let strategy = payload_string(map, "strategy").unwrap_or_else(|| "ort".to_string());
            let allow_ff = payload_bool(map, "allow_ff", true);
            let mut args = vec!["merge"];
            if !allow_ff {
                args.push("--no-ff");
            }
            let strategy_arg = format!("--strategy={strategy}");
            args.push(strategy_arg.as_str());
            args.push(target_branch.as_str());
            run_git_command(&context.worktree_path, &args)?;
            Ok(format!("已在任务分支合并目标分支 {}", target_branch))
        }
        "push" => {
            let remote_name = payload_string(map, "remote_name")
                .ok_or_else(|| "push 缺少 remote_name".to_string())?;
            let source_branch = payload_string(map, "source_branch")
                .ok_or_else(|| "push 缺少 source_branch".to_string())?;
            let target_ref = payload_string(map, "target_ref")
                .ok_or_else(|| "push 缺少 target_ref".to_string())?;
            let force_mode = payload_string(map, "force_mode").unwrap_or_else(|| "none".to_string());
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
            let owned = commit_ids;
            for commit_id in &owned {
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
            let stash_ref = payload_string(map, "stash_ref").unwrap_or_else(|| "stash@{0}".to_string());
            run_git_command(&context.worktree_path, &["stash", "pop", stash_ref.as_str()])?;
            Ok(format!("已恢复 {}", stash_ref))
        }
        "cleanup_worktree" => {
            let delete_branch = payload_bool(map, "delete_branch", false);
            let prune_worktree = payload_bool(map, "prune_worktree", true);
            run_git_command(repo_path, &["worktree", "remove", context.worktree_path.as_str(), "--force"])?;
            if delete_branch && git_ref_exists(repo_path, &format!("refs/heads/{}", context.task_branch)) {
                run_git_command(repo_path, &["branch", "-D", context.task_branch.as_str()])?;
            }
            if prune_worktree {
                run_git_command(repo_path, &["worktree", "prune"])?;
            }
            Ok("已清理任务 worktree".to_string())
        }
        _ => Err("不支持的 git action".to_string()),
    }
}

async fn update_context_after_prepare(
    pool: &SqlitePool,
    task: &Task,
    project: &Project,
    preferred_target_branch: Option<String>,
) -> Result<TaskGitContextRecord, String> {
    let repo_path = ensure_local_project_repo(project)?;
    let target_branch = preferred_target_branch.unwrap_or(determine_default_branch(&repo_path)?);
    let task_branch = build_task_branch(&task.id);
    let worktree_path = build_worktree_path(&repo_path, &task.id)?;
    let head_commit = current_head_commit(&repo_path, &target_branch)?;

    if let Some(mut existing) = fetch_task_git_context_by_task_id(pool, &task.id).await? {
        if existing.target_branch != target_branch {
            return Err(format!(
                "当前任务已绑定目标分支 {}，不能切换到 {}",
                existing.target_branch, target_branch
            ));
        }
        if context_is_healthy(&repo_path, &existing) {
            return Ok(existing);
        }

        ensure_task_branch(&repo_path, &existing.task_branch, &existing.target_branch)?;
        ensure_task_worktree(&repo_path, &existing.worktree_path, &existing.task_branch)?;
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

    ensure_task_branch(&repo_path, &task_branch, &target_branch)?;
    ensure_task_worktree(&repo_path, &worktree_path, &task_branch)?;

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
                if context_is_healthy(&repo_path, &existing) {
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

async fn refresh_context_state(
    pool: &SqlitePool,
    context: &mut TaskGitContextRecord,
    repo_path: &str,
) -> Result<TaskGitContextRecord, String> {
    if context_is_healthy(repo_path, context) {
        return Ok(context.clone());
    }
    context.state = TASK_GIT_STATE_DRIFTED.to_string();
    context.context_version += 1;
    context.last_error = Some("检测到 worktree 或任务分支已漂移".to_string());
    clear_pending_action_fields(context);
    context.updated_at = now_sqlite();
    let saved = save_task_git_context(pool, context).await?;
    insert_activity_log(
        pool,
        "task_git_context_drift_detected",
        "检测到任务 worktree / branch 漂移",
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
        return Err(format!("当前 task git context 状态不允许启动执行：{}", context.state));
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
        context.state = TASK_GIT_STATE_MERGE_READY.to_string();
        context.last_error = None;
        let saved = save_task_git_context(pool, &context).await?;
        insert_activity_log(
            pool,
            "task_merge_ready",
            "任务执行完成，等待后续 Git 确认动作",
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
    let rows = sqlx::query_as::<_, TaskGitContextRecord>(
        "SELECT * FROM task_git_contexts WHERE project_id = $1 ORDER BY updated_at DESC, created_at DESC",
    )
    .bind(project_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("查询 task git contexts 失败: {}", error))?;
    Ok(rows.into_iter().map(TaskGitContextSummary::from).collect())
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
    Ok(row.map(TaskGitContextSummary::from))
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
    let context = update_context_after_prepare(&pool, &task, &project, preferred_target_branch).await?;
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
    let repo_path = ensure_local_project_repo(&project)?;
    let refreshed = refresh_context_state(&pool, &mut context, &repo_path).await?;
    Ok(TaskGitContextSummary::from(refreshed))
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
    let reconciled = update_context_after_prepare(&pool, &task, &project, Some(context.target_branch)).await?;
    Ok(TaskGitContextSummary::from(reconciled))
}

#[tauri::command]
pub async fn request_git_action<R: Runtime>(
    app: AppHandle<R>,
    input: RequestGitActionInput,
) -> Result<GitActionRequestResult, String> {
    let pool = sqlite_pool(&app).await?;
    let mut context = fetch_task_git_context_by_id(&pool, &input.task_git_context_id).await?;
    let project = fetch_project_by_id(&pool, &context.project_id).await?;
    let repo_path = ensure_local_project_repo(&project)?;
    let action_type = normalize_action_type(&input.action_type)?;
    if !context_is_healthy(&repo_path, &context) {
        let refreshed = refresh_context_state(&pool, &mut context, &repo_path).await?;
        return Err(format!("task git context 不可用，当前状态：{}", refreshed.state));
    }
    if matches!(
        context.state.as_str(),
        TASK_GIT_STATE_PROVISIONING | TASK_GIT_STATE_FAILED | TASK_GIT_STATE_DRIFTED | TASK_GIT_STATE_COMPLETED
    ) {
        return Err(format!("当前状态不允许 request git action：{}", context.state));
    }

    let normalized_payload_json = normalize_git_action_payload(action_type, &context, &input.payload)?;
    let nonce = Uuid::new_v4().to_string();
    let expires_at = sqlite_now_with_offset(PENDING_ACTION_TTL_MINUTES);
    let next_version = context.context_version + 1;
    let repo_revision = current_head_commit(&context.worktree_path, "HEAD")?;
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
    let repo_path = ensure_local_project_repo(&project)?;

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
    if !context_is_healthy(&repo_path, &context) {
        reject_pending_action(
            &pool,
            &mut context,
            "worktree / task branch 已漂移，不能执行 confirm",
            true,
        )
        .await?;
        return Err("task git context 已漂移".to_string());
    }

    let current_revision = current_head_commit(&context.worktree_path, "HEAD")?;
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

    let execution_result = execute_normalized_action(&repo_path, &context, &action_type, &payload_json);
    match execution_result {
        Ok(message) => {
            clear_pending_action_fields(&mut context);
            context.context_version += 1;
            context.state = if action_type == "cleanup_worktree" || action_type == "merge" {
                TASK_GIT_STATE_COMPLETED.to_string()
            } else {
                TASK_GIT_STATE_MERGE_READY.to_string()
            };
            context.last_error = None;
            context.updated_at = now_sqlite();
            let saved = save_task_git_context(&pool, &context).await?;
            insert_activity_log(
                &pool,
                "git_action_confirmed",
                &message,
                None,
                Some(&saved.task_id),
                Some(&saved.project_id),
            )
            .await?;
            if action_type == "cleanup_worktree" {
                insert_activity_log(
                    &pool,
                    "task_worktree_cleanup_completed",
                    "任务 worktree 已清理",
                    None,
                    Some(&saved.task_id),
                    Some(&saved.project_id),
                )
                .await?;
            }
            Ok(ConfirmGitActionResult {
                context: TaskGitContextSummary::from(saved),
                action_type,
                message,
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
            project_type: PROJECT_TYPE_LOCAL.to_string(),
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
            r#"INSERT INTO tasks (id, title, description, status, priority, project_id, assignee_id, reviewer_id, complexity, ai_suggestion, automation_mode, last_codex_session_id, last_review_session_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, NULL, NULL, NULL, NULL, NULL, NULL, NULL, $7, $8)"#,
        )
        .bind(&task.id)
        .bind(&task.title)
        .bind(task.description.as_deref())
        .bind(&task.status)
        .bind(&task.priority)
        .bind(&task.project_id)
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

    #[test]
    fn prepare_task_git_execution_is_idempotent() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;

            let first = update_context_after_prepare(&pool, &task, &project, None)
                .await
                .expect("first prepare");
            let second = update_context_after_prepare(&pool, &task, &project, None)
                .await
                .expect("second prepare");

            assert_eq!(first.id, second.id);
            assert_eq!(first.worktree_path, second.worktree_path);
            assert!(Path::new(&first.worktree_path).join(".git").exists());

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(PathBuf::from(&first.worktree_path).parent().unwrap_or(Path::new("")));
            pool.close().await;
        });
    }

    #[test]
    fn request_and_cancel_git_action_invalidate_token() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;
            let repo_path = init_git_repo();
            let (project, task) = insert_project_and_task(&pool, &repo_path).await;
            let context = update_context_after_prepare(&pool, &task, &project, None)
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
            stored.pending_action_repo_revision = Some(current_head_commit(&stored.worktree_path, "HEAD").expect("head"));
            stored.pending_action_bound_context_version = Some(stored.context_version);
            stored.updated_at = now_sqlite();
            let saved = save_task_git_context(&pool, &stored).await.expect("save pending action");

            let cancelled = {
                let token = format!("{}.{}", nonce, signature);
                let mut context = fetch_task_git_context_by_id(&pool, &saved.id).await.expect("fetch saved");
                let (parsed_nonce, parsed_signature) = parse_token(&token).expect("parse token");
                assert_eq!(parsed_nonce, nonce);
                assert_eq!(parsed_signature, signature);
                clear_pending_action_fields(&mut context);
                context.context_version += 1;
                context.state = TASK_GIT_STATE_MERGE_READY.to_string();
                context.updated_at = now_sqlite();
                save_task_git_context(&pool, &context).await.expect("cancel save")
            };

            assert_eq!(cancelled.pending_action_type, None);
            assert_eq!(cancelled.state, TASK_GIT_STATE_MERGE_READY);

            let _ = fs::remove_dir_all(PathBuf::from(&repo_path));
            let _ = fs::remove_dir_all(PathBuf::from(&context.worktree_path).parent().unwrap_or(Path::new("")));
            pool.close().await;
        });
    }
}
