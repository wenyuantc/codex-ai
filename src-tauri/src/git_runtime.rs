use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use tauri::{AppHandle, Runtime};

use crate::app::{
    build_remote_shell_command, ensure_remote_sdk_runtime_layout, execute_ssh_command,
    execute_ssh_command_with_input, fetch_ssh_config_record_by_id, insert_activity_log,
    remote_shell_path_expression, sqlite_pool, EXECUTION_TARGET_SSH,
};
use crate::codex::{
    ensure_sdk_runtime_layout, load_codex_settings, load_remote_codex_settings, new_node_command,
    new_npm_command,
};

const GIT_BRIDGE_FILE_NAME: &str = "git-bridge.mjs";
const SIMPLE_GIT_PACKAGE_NAME: &str = "simple-git";
pub(crate) const GIT_RUNTIME_PROVIDER_SIMPLE_GIT: &str = "simple_git";
pub(crate) const GIT_RUNTIME_STATUS_READY: &str = "ready";
pub(crate) const GIT_RUNTIME_STATUS_BOOTSTRAPPING: &str = "bootstrapping";
pub(crate) const GIT_RUNTIME_STATUS_UNAVAILABLE: &str = "unavailable";
const REMOTE_INSTALL_MARKER: &str = "__CODEX_AI_SIMPLE_GIT_INSTALLED__";

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GitRuntimeCommit {
    pub sha: String,
    pub short_sha: String,
    pub subject: String,
    pub author_name: String,
    pub authored_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GitRuntimeOverview {
    pub default_branch: String,
    pub current_branch: Option<String>,
    pub project_branches: Vec<String>,
    pub head_commit_sha: String,
    pub working_tree_summary: Option<String>,
    pub ahead_commits: Option<u32>,
    pub behind_commits: Option<u32>,
    pub recent_commits: Vec<GitRuntimeCommit>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GitRuntimeChange {
    pub path: String,
    pub previous_path: Option<String>,
    pub change_type: String,
    pub stage_status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GitRuntimeTextSnapshot {
    pub status: String,
    pub text: Option<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GitRuntimeSnapshotEntry {
    pub path: String,
    pub previous_path: Option<String>,
    pub status_x: String,
    pub status_y: String,
    pub content_hash: Option<String>,
    pub text_snapshot: GitRuntimeTextSnapshot,
}

#[derive(Debug, Deserialize)]
struct GitBridgeEnvelope<T> {
    ok: bool,
    result: Option<T>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitBridgeExistsResult {
    exists: bool,
}

#[derive(Debug, Deserialize)]
struct GitBridgeRevisionResult {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct GitBridgeMessageResult {
    message: String,
}

#[derive(Debug, Deserialize)]
struct GitBridgeReviewContextResult {
    context: String,
}

#[derive(Debug, Deserialize)]
struct GitBridgeStatusChangesResult {
    changes: Vec<GitRuntimeChange>,
}

#[derive(Debug, Deserialize)]
struct GitBridgeSnapshotResult {
    entries: Vec<GitRuntimeSnapshotEntry>,
}

#[derive(Debug, Deserialize)]
struct GitBridgeHashResult {
    content_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitBridgeTextSnapshotResult {
    snapshot: GitRuntimeTextSnapshot,
}

#[derive(Debug)]
struct LocalRuntimeSpec {
    bridge_path: PathBuf,
    node_path_override: Option<String>,
    install_message: Option<String>,
}

#[derive(Debug)]
struct RemoteRuntimeSpec {
    bridge_path: String,
    install_dir: String,
    node_path_override: Option<String>,
    ssh_config_id: String,
    install_message: Option<String>,
}

fn source_git_bridge_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(GIT_BRIDGE_FILE_NAME)
}

fn workspace_simple_git_package_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| {
            path.join("node_modules")
                .join(SIMPLE_GIT_PACKAGE_NAME)
                .join("package.json")
        })
        .unwrap_or_else(|| {
            PathBuf::from("node_modules")
                .join(SIMPLE_GIT_PACKAGE_NAME)
                .join("package.json")
        })
}

fn simple_git_package_json_path(install_dir: &Path) -> PathBuf {
    install_dir
        .join("node_modules")
        .join(SIMPLE_GIT_PACKAGE_NAME)
        .join("package.json")
}

fn git_bridge_script_path(install_dir: &Path) -> PathBuf {
    install_dir.join(GIT_BRIDGE_FILE_NAME)
}

fn write_local_git_bridge(install_dir: &Path) -> Result<PathBuf, String> {
    ensure_sdk_runtime_layout(install_dir)?;
    let bridge_path = git_bridge_script_path(install_dir);
    let bridge_content = include_str!("git_bridge.mjs");
    let should_write = match fs::read_to_string(&bridge_path) {
        Ok(existing) => existing != bridge_content,
        Err(_) => true,
    };
    if should_write {
        fs::write(&bridge_path, bridge_content)
            .map_err(|error| format!("写入 Git bridge 脚本失败: {error}"))?;
    }
    Ok(bridge_path)
}

async fn ensure_local_runtime<R: Runtime>(app: &AppHandle<R>) -> Result<LocalRuntimeSpec, String> {
    let settings = load_codex_settings(app)?;
    let source_bridge = source_git_bridge_path();
    let workspace_simple_git = workspace_simple_git_package_path();
    if source_bridge.exists() && workspace_simple_git.exists() {
        return Ok(LocalRuntimeSpec {
            bridge_path: source_bridge,
            node_path_override: settings.node_path_override.clone(),
            install_message: None,
        });
    }

    let install_dir = PathBuf::from(&settings.sdk_install_dir);
    let bridge_path = write_local_git_bridge(&install_dir)?;
    if simple_git_package_json_path(&install_dir).exists() {
        return Ok(LocalRuntimeSpec {
            bridge_path,
            node_path_override: settings.node_path_override.clone(),
            install_message: None,
        });
    }

    let mut npm_command = new_npm_command(settings.node_path_override.as_deref()).await?;
    npm_command
        .current_dir(&install_dir)
        .arg("install")
        .arg("--no-audit")
        .arg("--no-fund")
        .arg(SIMPLE_GIT_PACKAGE_NAME)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = npm_command
        .output()
        .await
        .map_err(|error| format!("安装本地 Git runtime 失败: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if stderr.is_empty() {
            "安装本地 Git runtime 失败".to_string()
        } else {
            format!("安装本地 Git runtime 失败：{stderr}")
        };
        let pool = sqlite_pool(app).await?;
        let _ = insert_activity_log(
            &pool,
            "git_runtime_install_failed",
            &message,
            None,
            None,
            None,
        )
        .await;
        return Err(message);
    }

    let pool = sqlite_pool(app).await?;
    let detail = settings.sdk_install_dir.clone();
    let _ = insert_activity_log(&pool, "git_runtime_installed", &detail, None, None, None).await;

    Ok(LocalRuntimeSpec {
        bridge_path,
        node_path_override: settings.node_path_override.clone(),
        install_message: Some("本地 Git runtime 已自动补齐".to_string()),
    })
}

async fn ensure_remote_runtime<R: Runtime>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
) -> Result<RemoteRuntimeSpec, String> {
    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, ssh_config_id).await?;
    let remote_settings = ensure_remote_sdk_runtime_layout(app, ssh_config_id).await?;
    let install_dir = remote_settings.sdk_install_dir.clone();
    let bridge_path = format!(
        "{}/{}",
        install_dir.trim_end_matches('/'),
        GIT_BRIDGE_FILE_NAME
    );

    let bridge_output = execute_ssh_command_with_input(
        app,
        &ssh_config,
        &build_remote_shell_command(
            &format!("cat > {}", remote_shell_path_expression(&bridge_path)),
            remote_settings.node_path_override.as_deref(),
        ),
        include_str!("git_bridge.mjs").as_bytes(),
        true,
    )
    .await?;
    if !bridge_output.status.success() {
        let stderr = String::from_utf8_lossy(&bridge_output.stderr)
            .trim()
            .to_string();
        return Err(if stderr.is_empty() {
            "写入远程 Git bridge 脚本失败".to_string()
        } else {
            format!("写入远程 Git bridge 脚本失败：{stderr}")
        });
    }

    let remote_script = format!(
        "install_dir={install_dir}; pkg=\"$install_dir/node_modules/{pkg}/package.json\"; \
if [ -f \"$pkg\" ]; then printf 'READY\\n'; \
else cd \"$install_dir\" && npm install --no-audit --no-fund {pkg_name} && printf '{marker}\\n'; fi",
        install_dir = remote_shell_path_expression(&install_dir),
        pkg = SIMPLE_GIT_PACKAGE_NAME,
        pkg_name = SIMPLE_GIT_PACKAGE_NAME,
        marker = REMOTE_INSTALL_MARKER,
    );
    let output = execute_ssh_command(
        app,
        &ssh_config,
        &build_remote_shell_command(
            &remote_script,
            remote_settings.node_path_override.as_deref(),
        ),
        true,
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if stderr.is_empty() {
            "安装远程 Git runtime 失败".to_string()
        } else {
            format!("安装远程 Git runtime 失败：{stderr}")
        };
        let _ = insert_activity_log(
            &pool,
            "remote_git_runtime_install_failed",
            &message,
            None,
            None,
            None,
        )
        .await;
        return Err(message);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let install_message = if stdout.contains(REMOTE_INSTALL_MARKER) {
        let detail = format!("{}@{}", ssh_config.username, ssh_config.host);
        let _ = insert_activity_log(
            &pool,
            "remote_git_runtime_installed",
            &detail,
            None,
            None,
            None,
        )
        .await;
        Some("远程 Git runtime 已自动补齐".to_string())
    } else {
        None
    };

    Ok(RemoteRuntimeSpec {
        bridge_path,
        install_dir,
        node_path_override: load_remote_codex_settings(app, ssh_config_id)?.node_path_override,
        ssh_config_id: ssh_config_id.to_string(),
        install_message,
    })
}

async fn parse_bridge_response<T: DeserializeOwned>(
    stdout: &[u8],
    stderr: &[u8],
) -> Result<T, String> {
    let stdout_text = String::from_utf8_lossy(stdout).trim().to_string();
    if stdout_text.is_empty() {
        let stderr_text = String::from_utf8_lossy(stderr).trim().to_string();
        return Err(if stderr_text.is_empty() {
            "Git bridge 未返回结果".to_string()
        } else {
            stderr_text
        });
    }

    let envelope = serde_json::from_str::<GitBridgeEnvelope<T>>(&stdout_text)
        .map_err(|error| format!("解析 Git bridge 输出失败: {error}"))?;
    if envelope.ok {
        envelope
            .result
            .ok_or_else(|| "Git bridge 缺少 result 字段".to_string())
    } else {
        Err(envelope
            .error
            .unwrap_or_else(|| "Git bridge 执行失败".to_string()))
    }
}

async fn call_local_bridge<R: Runtime, T: DeserializeOwned>(
    app: &AppHandle<R>,
    payload: serde_json::Value,
) -> Result<T, String> {
    let runtime = ensure_local_runtime(app).await?;
    let input = serde_json::to_vec(&payload)
        .map_err(|error| format!("序列化 Git bridge payload 失败: {error}"))?;
    let mut command = new_node_command(runtime.node_path_override.as_deref()).await?;
    command
        .arg(&runtime.bridge_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动本地 Git bridge 失败: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(&input)
            .await
            .map_err(|error| format!("写入本地 Git bridge 输入失败: {error}"))?;
        stdin
            .shutdown()
            .await
            .map_err(|error| format!("关闭本地 Git bridge 输入失败: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .await
        .map_err(|error| format!("等待本地 Git bridge 完成失败: {error}"))?;
    if !output.status.success() {
        return parse_bridge_response(&output.stdout, &output.stderr).await;
    }
    parse_bridge_response(&output.stdout, &output.stderr).await
}

async fn call_remote_bridge<R: Runtime, T: DeserializeOwned>(
    app: &AppHandle<R>,
    ssh_config_id: &str,
    payload: serde_json::Value,
) -> Result<T, String> {
    let runtime = ensure_remote_runtime(app, ssh_config_id).await?;
    let pool = sqlite_pool(app).await?;
    let ssh_config = fetch_ssh_config_record_by_id(&pool, &runtime.ssh_config_id).await?;
    let input = serde_json::to_vec(&payload)
        .map_err(|error| format!("序列化远程 Git bridge payload 失败: {error}"))?;
    let remote_command = build_remote_shell_command(
        &format!(
            "cd {} && node {}",
            remote_shell_path_expression(&runtime.install_dir),
            remote_shell_path_expression(&runtime.bridge_path),
        ),
        runtime.node_path_override.as_deref(),
    );
    let output =
        execute_ssh_command_with_input(app, &ssh_config, &remote_command, &input, true).await?;
    if !output.status.success() {
        return parse_bridge_response(&output.stdout, &output.stderr).await;
    }
    parse_bridge_response(&output.stdout, &output.stderr).await
}

async fn call_bridge<R: Runtime, T: DeserializeOwned>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    payload: serde_json::Value,
) -> Result<T, String> {
    if execution_target == EXECUTION_TARGET_SSH {
        let ssh_config_id =
            ssh_config_id.ok_or_else(|| "SSH Git 操作缺少 ssh_config_id".to_string())?;
        call_remote_bridge(app, ssh_config_id, payload).await
    } else {
        call_local_bridge(app, payload).await
    }
}

pub(crate) async fn collect_git_overview<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    recent_commit_limit: usize,
) -> Result<GitRuntimeOverview, String> {
    call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "overview",
            "repoPath": repo_path,
            "recentCommitLimit": recent_commit_limit,
        }),
    )
    .await
}

pub(crate) async fn git_ref_exists<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    full_ref: &str,
) -> Result<bool, String> {
    let result: GitBridgeExistsResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "ref_exists",
            "repoPath": repo_path,
            "fullRef": full_ref,
        }),
    )
    .await?;
    Ok(result.exists)
}

pub(crate) async fn stage_path<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    target_path: &str,
) -> Result<String, String> {
    let result: GitBridgeMessageResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "stage_path",
            "repoPath": repo_path,
            "targetPath": target_path,
        }),
    )
    .await?;
    Ok(result.message)
}

pub(crate) async fn unstage_path<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    target_path: &str,
) -> Result<String, String> {
    let result: GitBridgeMessageResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "unstage_path",
            "repoPath": repo_path,
            "targetPath": target_path,
        }),
    )
    .await?;
    Ok(result.message)
}

pub(crate) async fn stage_all<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
) -> Result<String, String> {
    let result: GitBridgeMessageResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "stage_all",
            "repoPath": repo_path,
        }),
    )
    .await?;
    Ok(result.message)
}

pub(crate) async fn unstage_all<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
) -> Result<String, String> {
    let result: GitBridgeMessageResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "unstage_all",
            "repoPath": repo_path,
        }),
    )
    .await?;
    Ok(result.message)
}

pub(crate) async fn commit_changes<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    message: &str,
) -> Result<String, String> {
    let result: GitBridgeMessageResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "commit_changes",
            "repoPath": repo_path,
            "message": message,
        }),
    )
    .await?;
    Ok(result.message)
}

pub(crate) async fn push_branch<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    remote_name: &str,
    branch_name: &str,
    force_mode: &str,
) -> Result<String, String> {
    let result: GitBridgeMessageResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "push_branch",
            "repoPath": repo_path,
            "remoteName": remote_name,
            "branchName": branch_name,
            "forceMode": force_mode,
        }),
    )
    .await?;
    Ok(result.message)
}

pub(crate) async fn pull_branch<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    remote_name: &str,
    branch_name: &str,
    mode: &str,
    auto_stash: bool,
) -> Result<String, String> {
    let result: GitBridgeMessageResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "pull_branch",
            "repoPath": repo_path,
            "remoteName": remote_name,
            "branchName": branch_name,
            "mode": mode,
            "autoStash": auto_stash,
        }),
    )
    .await?;
    Ok(result.message)
}

pub(crate) async fn path_exists<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    target_path: &str,
) -> Result<bool, String> {
    let result: GitBridgeExistsResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "path_exists",
            "repoPath": repo_path,
            "targetPath": target_path,
        }),
    )
    .await?;
    Ok(result.exists)
}

pub(crate) async fn ensure_task_branch<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    task_branch: &str,
    target_branch: &str,
) -> Result<(), String> {
    let _: serde_json::Value = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "ensure_task_branch",
            "repoPath": repo_path,
            "taskBranch": task_branch,
            "targetBranch": target_branch,
        }),
    )
    .await?;
    Ok(())
}

pub(crate) async fn ensure_task_worktree<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    worktree_path: &str,
    task_branch: &str,
    target_branch: &str,
) -> Result<(), String> {
    let _: serde_json::Value = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "ensure_task_worktree",
            "repoPath": repo_path,
            "worktreePath": worktree_path,
            "taskBranch": task_branch,
            "targetBranch": target_branch,
        }),
    )
    .await?;
    Ok(())
}

pub(crate) async fn rev_parse<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    revision: &str,
) -> Result<String, String> {
    let result: GitBridgeRevisionResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "rev_parse",
            "repoPath": repo_path,
            "revision": revision,
        }),
    )
    .await?;
    Ok(result.sha)
}

pub(crate) async fn execute_action<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    worktree_path: &str,
    task_branch: &str,
    action_type: &str,
    payload: &str,
) -> Result<String, String> {
    let payload_json = serde_json::from_str::<serde_json::Value>(payload)
        .map_err(|error| format!("解析 Git action payload 失败: {error}"))?;
    let result: GitBridgeMessageResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "execute_action",
            "repoPath": repo_path,
            "worktreePath": worktree_path,
            "taskBranch": task_branch,
            "actionType": action_type,
            "payload": payload_json,
        }),
    )
    .await?;
    Ok(result.message)
}

pub(crate) async fn collect_review_context<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
) -> Result<String, String> {
    let result: GitBridgeReviewContextResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "collect_review_context",
            "repoPath": repo_path,
        }),
    )
    .await?;
    Ok(result.context)
}

pub(crate) async fn collect_status_changes<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
) -> Result<Vec<GitRuntimeChange>, String> {
    let result: GitBridgeStatusChangesResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "status_changes",
            "repoPath": repo_path,
        }),
    )
    .await?;
    Ok(result.changes)
}

pub(crate) async fn collect_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    capture_text_snapshots: bool,
) -> Result<Vec<GitRuntimeSnapshotEntry>, String> {
    let result: GitBridgeSnapshotResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "collect_snapshot",
            "repoPath": repo_path,
            "captureTextSnapshots": capture_text_snapshots,
        }),
    )
    .await?;
    Ok(result.entries)
}

pub(crate) async fn hash_worktree_path<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    relative_path: &str,
) -> Result<Option<String>, String> {
    let result: GitBridgeHashResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "hash_worktree_path",
            "repoPath": repo_path,
            "relativePath": relative_path,
        }),
    )
    .await?;
    Ok(result.content_hash)
}

pub(crate) async fn capture_worktree_text_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    relative_path: &str,
) -> Result<GitRuntimeTextSnapshot, String> {
    let result: GitBridgeTextSnapshotResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "capture_worktree_text_snapshot",
            "repoPath": repo_path,
            "relativePath": relative_path,
        }),
    )
    .await?;
    Ok(result.snapshot)
}

pub(crate) async fn capture_head_text_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    repo_path: &str,
    relative_path: &str,
) -> Result<GitRuntimeTextSnapshot, String> {
    let result: GitBridgeTextSnapshotResult = call_bridge(
        app,
        execution_target,
        ssh_config_id,
        serde_json::json!({
            "command": "capture_head_text_snapshot",
            "repoPath": repo_path,
            "relativePath": relative_path,
        }),
    )
    .await?;
    Ok(result.snapshot)
}
