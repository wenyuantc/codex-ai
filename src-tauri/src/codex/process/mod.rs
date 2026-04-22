use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader as StdBufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::Deserialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::io::AsyncWriteExt;
use tokio::process::Child;
use tokio::time::sleep;

use crate::app::{
    build_remote_shell_command, build_ssh_command, ensure_remote_sdk_runtime_layout,
    execute_ssh_command, fetch_codex_session_by_id, fetch_ssh_config_record_by_id,
    insert_activity_log, insert_codex_session_event, insert_codex_session_event_with_id,
    insert_codex_session_record, inspect_remote_codex_runtime, normalize_runtime_path_string,
    now_sqlite, parse_review_verdict_json, path_to_runtime_string, remote_sdk_bridge_path,
    remote_shell_path_expression, replace_codex_session_file_changes, sqlite_pool,
    sync_task_attachments_to_remote, update_codex_session_record, validate_project_repo_path,
    validate_runtime_working_dir, ARTIFACT_CAPTURE_MODE_LOCAL_FULL, ARTIFACT_CAPTURE_MODE_SSH_FULL,
    ARTIFACT_CAPTURE_MODE_SSH_GIT_STATUS, ARTIFACT_CAPTURE_MODE_SSH_NONE, EXECUTION_TARGET_LOCAL,
    EXECUTION_TARGET_SSH,
};
use crate::codex::{
    ensure_sdk_runtime_layout, inspect_sdk_runtime, load_codex_settings,
    load_remote_codex_settings, new_codex_command, new_node_command, resolve_codex_executable_path,
    sdk_bridge_script_path, CodexManager,
};
use crate::db::models::{
    CodexExit, CodexOutput, CodexSession, CodexSessionFileChangeDetailInput,
    CodexSessionFileChangeInput, SshConfigRecord,
};
use crate::git_workflow::{mark_task_git_context_running, validate_task_git_context_launch};
use crate::process_spawn::configure_std_command;
use crate::task_automation;

mod ai_commands;
mod changes;
mod command_builders;
mod context;
mod lifecycle;
mod one_shot;
mod prompts;
mod session_launch;
mod session_runtime;
mod session_support;
mod stream;

pub use self::ai_commands::*;
pub use self::lifecycle::CodexChild;

pub(crate) use self::lifecycle::stop_codex_for_automation_restart;

use self::{
    changes::*, command_builders::*, context::*, lifecycle::*, one_shot::*, prompts::*,
    session_launch::*, session_runtime::*, session_support::*, stream::*,
};

const SUPPORTED_MODELS: &[&str] = &[
    "gpt-5.4",
    "gpt-5.2-codex",
    "gpt-5.1-codex-max",
    "gpt-5.4-mini",
    "gpt-5.3-codex",
    "gpt-5.3-codex-spark",
    "gpt-5.2",
    "gpt-5.1-codex-mini",
];
const SUPPORTED_REASONING_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh"];
const SESSION_ID_PREFIX: &str = "session id:";
const SDK_FILE_CHANGE_EVENT_PREFIX: &str = "[CODEX_FILE_CHANGE]";
const REVIEW_VERDICT_START_TAG: &str = "<review_verdict>";
const REVIEW_VERDICT_END_TAG: &str = "</review_verdict>";
const REVIEW_REPORT_START_TAG: &str = "<review_report>";
const REVIEW_REPORT_END_TAG: &str = "</review_report>";
const STOP_WAIT_POLL_MS: u64 = 50;
const STOP_WAIT_MAX_ATTEMPTS: usize = 600;
const FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT: u64 = 256 * 1024;
const REMOTE_SNAPSHOT_MISSING_EXIT_CODE: i32 = 3;
const REMOTE_SNAPSHOT_UNAVAILABLE_EXIT_CODE: i32 = 4;
// 远程 codex exec 需要和本地 exec 一样走非 TTY 管道，否则 SSH PTY 容易把输出变成
// 终端刷新流，导致前端按行读取时拿不到实时日志。
const REMOTE_EXEC_SSH_ALLOCATE_TTY: bool = false;

#[derive(Debug, Deserialize)]
struct SdkBridgeResponse {
    ok: bool,
    text: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SdkFileChangeEvent {
    changes: Vec<SdkFileChangePayload>,
}

#[derive(Debug, Deserialize)]
struct SdkFileChangePayload {
    kind: Option<String>,
    path: Option<String>,
    #[serde(
        default,
        alias = "previousPath",
        alias = "oldPath",
        alias = "old_path",
        alias = "from"
    )]
    previous_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CodexExecutionProvider {
    Cli,
    Sdk,
}

impl CodexExecutionProvider {
    fn label(self) -> &'static str {
        match self {
            CodexExecutionProvider::Cli => "CLI",
            CodexExecutionProvider::Sdk => "SDK",
        }
    }

    fn capture_mode(self) -> &'static str {
        match self {
            CodexExecutionProvider::Cli => "git_fallback",
            CodexExecutionProvider::Sdk => "sdk_event",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CliJsonStreamState {
    command_outputs: HashMap<String, String>,
    agent_messages: HashMap<String, String>,
    reasoning_messages: HashMap<String, String>,
    last_todo_summary: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CliJsonParsedEvent {
    session_id: Option<String>,
    lines: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CliJsonOutputFlag {
    Json,
    ExperimentalJson,
}

impl CliJsonOutputFlag {
    fn as_arg(self) -> &'static str {
        match self {
            CliJsonOutputFlag::Json => "--json",
            CliJsonOutputFlag::ExperimentalJson => "--experimental-json",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexSessionKind {
    Execution,
    Review,
}

impl CodexSessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CodexSessionKind::Execution => "execution",
            CodexSessionKind::Review => "review",
        }
    }

    fn activity_start_action(self, resumed: bool) -> &'static str {
        match self {
            CodexSessionKind::Execution => {
                if resumed {
                    "task_execution_resumed"
                } else {
                    "task_execution_started"
                }
            }
            CodexSessionKind::Review => "task_review_started",
        }
    }
}

fn normalize_session_kind(session_kind: Option<&str>) -> CodexSessionKind {
    match session_kind {
        Some("review") => CodexSessionKind::Review,
        _ => CodexSessionKind::Execution,
    }
}

fn normalize_model(model: Option<&str>) -> &'static str {
    match model {
        Some(value) if SUPPORTED_MODELS.contains(&value) => match value {
            "gpt-5.4" => "gpt-5.4",
            "gpt-5.2-codex" => "gpt-5.2-codex",
            "gpt-5.1-codex-max" => "gpt-5.1-codex-max",
            "gpt-5.4-mini" => "gpt-5.4-mini",
            "gpt-5.3-codex" => "gpt-5.3-codex",
            "gpt-5.3-codex-spark" => "gpt-5.3-codex-spark",
            "gpt-5.2" => "gpt-5.2",
            "gpt-5.1-codex-mini" => "gpt-5.1-codex-mini",
            _ => "gpt-5.4",
        },
        _ => "gpt-5.4",
    }
}

fn normalize_reasoning_effort(reasoning_effort: Option<&str>) -> &'static str {
    match reasoning_effort {
        Some(value) if SUPPORTED_REASONING_EFFORTS.contains(&value) => match value {
            "low" => "low",
            "medium" => "medium",
            "high" => "high",
            "xhigh" => "xhigh",
            _ => "high",
        },
        _ => "high",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionChangeBaseline {
    repo_path: String,
    execution_target: String,
    ssh_config_id: Option<String>,
    entries: HashMap<String, WorkingTreeSnapshotEntry>,
}

pub(crate) type SdkFileChangeStore = Arc<Mutex<HashMap<String, CodexSessionFileChangeInput>>>;

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextSnapshot {
    status: TextSnapshotStatus,
    text: Option<String>,
    truncated: bool,
}

impl TextSnapshot {
    fn missing() -> Self {
        Self {
            status: TextSnapshotStatus::Missing,
            text: None,
            truncated: false,
        }
    }

    fn unavailable() -> Self {
        Self {
            status: TextSnapshotStatus::Unavailable,
            text: None,
            truncated: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TextSnapshotStatus {
    Text,
    Missing,
    Binary,
    Unavailable,
}

impl TextSnapshotStatus {
    fn as_str(self) -> &'static str {
        match self {
            TextSnapshotStatus::Text => "text",
            TextSnapshotStatus::Missing => "missing",
            TextSnapshotStatus::Binary => "binary",
            TextSnapshotStatus::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkingTreeSnapshotEntry {
    path: String,
    previous_path: Option<String>,
    status_x: char,
    status_y: char,
    content_hash: Option<String>,
    text_snapshot: TextSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionFileChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

impl SessionFileChangeKind {
    fn as_str(self) -> &'static str {
        match self {
            SessionFileChangeKind::Added => "added",
            SessionFileChangeKind::Modified => "modified",
            SessionFileChangeKind::Deleted => "deleted",
            SessionFileChangeKind::Renamed => "renamed",
        }
    }
}

fn normalize_session_file_change_kind(value: Option<&str>) -> Option<SessionFileChangeKind> {
    match value.map(|item| item.trim().to_ascii_lowercase()) {
        Some(value) if matches!(value.as_str(), "add" | "added" | "create" | "created") => {
            Some(SessionFileChangeKind::Added)
        }
        Some(value)
            if matches!(
                value.as_str(),
                "modify"
                    | "modified"
                    | "update"
                    | "updated"
                    | "change"
                    | "changed"
                    | "edit"
                    | "edited"
            ) =>
        {
            Some(SessionFileChangeKind::Modified)
        }
        Some(value) if matches!(value.as_str(), "delete" | "deleted" | "remove" | "removed") => {
            Some(SessionFileChangeKind::Deleted)
        }
        Some(value) if matches!(value.as_str(), "rename" | "renamed" | "move" | "moved") => {
            Some(SessionFileChangeKind::Renamed)
        }
        _ => None,
    }
}

fn upsert_sdk_file_change_event(store: &SdkFileChangeStore, event: SdkFileChangeEvent) {
    let mut guard = store.lock().unwrap();
    for change in event.changes {
        let path = change.path.unwrap_or_default().trim().to_string();
        if path.is_empty() {
            continue;
        }

        let Some(change_kind) = normalize_session_file_change_kind(change.kind.as_deref()) else {
            continue;
        };
        guard.insert(
            path.clone(),
            CodexSessionFileChangeInput {
                path,
                change_type: change_kind.as_str().to_string(),
                capture_mode: CodexExecutionProvider::Sdk.capture_mode().to_string(),
                previous_path: change
                    .previous_path
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                detail: None,
            },
        );
    }
}

fn extract_tagged_block(raw: &str, start_tag: &str, end_tag: &str) -> Option<String> {
    let escaped_end_tag = end_tag
        .strip_prefix("</")
        .map(|value| format!("<\\/{}", value));
    let lines = raw.lines().collect::<Vec<_>>();
    for end_index in (0..lines.len()).rev() {
        let line = lines[end_index].trim();
        let Some(end_prefix) = line.strip_suffix(end_tag).or_else(|| {
            escaped_end_tag
                .as_deref()
                .and_then(|tag| line.strip_suffix(tag))
        }) else {
            continue;
        };

        if let Some(content) = end_prefix.trim().strip_prefix(start_tag) {
            let content = content.trim();
            if !content.is_empty() {
                return Some(content.to_string());
            }
            continue;
        }

        let start_index = lines[..end_index].iter().rposition(|candidate| {
            let candidate = candidate.trim();
            candidate == start_tag || candidate.starts_with(start_tag)
        })?;
        let start_line = lines[start_index].trim();
        let start_suffix = start_line
            .strip_prefix(start_tag)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let end_prefix = end_prefix.trim();
        let mut parts = Vec::new();
        if let Some(start_suffix) = start_suffix {
            parts.push(start_suffix);
        }
        parts.extend(lines[start_index + 1..end_index].iter().copied());
        if !end_prefix.is_empty() {
            parts.push(end_prefix);
        }
        let content = parts.join("\n");
        let content = content.trim();
        if !content.is_empty() {
            return Some(content.to_string());
        }
    }

    None
}

pub(crate) fn extract_review_report(raw: &str) -> Option<String> {
    extract_tagged_block(raw, REVIEW_REPORT_START_TAG, REVIEW_REPORT_END_TAG).or_else(|| {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

pub(crate) fn extract_review_verdict(raw: &str) -> Option<String> {
    extract_tagged_block(raw, REVIEW_VERDICT_START_TAG, REVIEW_VERDICT_END_TAG)
}

fn compose_codex_prompt(task_description: &str, system_prompt: Option<&str>) -> String {
    let task_description = task_description.trim();
    let system_prompt = system_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match system_prompt {
        Some(system_prompt) => format!(
            "请先严格遵循以下员工提示词，再执行后续任务。\n\n<employee_system_prompt>\n{}\n</employee_system_prompt>\n\n<task>\n{}\n</task>",
            system_prompt, task_description
        ),
        None => task_description.to_string(),
    }
}

fn format_session_prompt_log(
    provider: CodexExecutionProvider,
    model: &str,
    reasoning_effort: &str,
    execution_target: &str,
    ssh_config_name: Option<&str>,
    ssh_host: Option<&str>,
    target_host_label: Option<&str>,
    working_dir: &str,
    prompt: &str,
    image_paths: &[String],
) -> String {
    let image_block = if image_paths.is_empty() {
        "附带图片: 0 张".to_string()
    } else {
        let lines = image_paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let label = Path::new(path)
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                format!("{}. {}", index + 1, label)
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("附带图片: {} 张\n{}", image_paths.len(), lines)
    };

    let runtime_block = if execution_target == EXECUTION_TARGET_SSH {
        let ssh_name = ssh_config_name.unwrap_or("未命名 SSH 配置");
        let ssh_host = ssh_host.unwrap_or("未知主机");
        let ssh_login = target_host_label.unwrap_or("未知登录目标");
        format!(
            "执行环境: SSH 远程运行\nSSH 名称: {}\nSSH 主机/IP: {}\nSSH 登录: {}",
            ssh_name, ssh_host, ssh_login
        )
    } else {
        "执行环境: 本地运行".to_string()
    };

    format!(
        "[PROMPT] 即将发送给 Codex 的完整提示词\n\
运行通道: {}\n\
模型: {}\n\
推理强度: {}\n\
{}\n\
工作目录: {}\n\
{}\n\n{}",
        provider.label(),
        model,
        reasoning_effort,
        runtime_block,
        working_dir,
        image_block,
        prompt
    )
}

async fn emit_session_terminal_line(
    app: &AppHandle,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: CodexSessionKind,
    line: String,
) {
    let session_event_id =
        match insert_codex_session_event_with_id(pool, session_record_id, "stdout", Some(&line))
            .await
        {
            Ok(event_id) => Some(event_id),
            Err(error) => {
                eprintln!(
                    "[codex-session] 写入会话日志失败(session={}, kind={}): {}",
                    session_record_id,
                    session_kind.as_str(),
                    error
                );
                None
            }
        };

    let _ = app.emit(
        "codex-stdout",
        CodexOutput {
            employee_id: employee_id.to_string(),
            task_id: task_id.map(ToOwned::to_owned),
            session_kind: session_kind.as_str().to_string(),
            session_record_id: session_record_id.to_string(),
            session_event_id,
            line,
        },
    );
}

fn collect_available_image_paths(image_paths: Option<Vec<String>>) -> (Vec<String>, Vec<String>) {
    let mut seen = HashSet::new();
    let mut available = Vec::new();
    let mut missing = Vec::new();

    for raw in image_paths.unwrap_or_default() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }

        let path = Path::new(trimmed);
        if path.is_file() {
            match path.canonicalize() {
                Ok(canonical) => available.push(path_to_runtime_string(&canonical)),
                Err(_) => available.push(trimmed.to_string()),
            }
        } else {
            missing.push(trimmed.to_string());
        }
    }

    (available, missing)
}

async fn prepare_execution_image_paths<R: Runtime>(
    app: &AppHandle<R>,
    task_id: Option<&str>,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    image_paths: Option<Vec<String>>,
) -> Result<(Vec<String>, Vec<String>, usize), String> {
    if execution_target == EXECUTION_TARGET_SSH {
        if let (Some(task_id), Some(ssh_config_id)) = (task_id, ssh_config_id) {
            let sync_result = sync_task_attachments_to_remote(app, ssh_config_id, task_id).await?;
            return Ok((sync_result.remote_paths, sync_result.skipped_local_paths, 0));
        }

        let ignored_count = image_paths.unwrap_or_default().len();
        return Ok((Vec::new(), Vec::new(), ignored_count));
    }

    let (available, missing) = collect_available_image_paths(image_paths);
    Ok((available, missing, 0))
}

fn sdk_codex_path_override_allowed_for_os(path: &Path, target_os: &str) -> bool {
    if target_os != "windows" {
        return true;
    }

    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
}

fn sdk_codex_path_override_from_resolved_path(path: &Path) -> Option<String> {
    sdk_codex_path_override_allowed_for_os(path, std::env::consts::OS)
        .then(|| path_to_runtime_string(path))
}

#[cfg(unix)]
fn configure_process_group(command: &mut tokio::process::Command) {
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut tokio::process::Command) {}

#[tauri::command]
pub async fn start_codex(
    app: AppHandle,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    employee_id: String,
    task_description: String,
    model: Option<String>,
    reasoning_effort: Option<String>,
    system_prompt: Option<String>,
    working_dir: Option<String>,
    task_id: Option<String>,
    task_git_context_id: Option<String>,
    resume_session_id: Option<String>,
    image_paths: Option<Vec<String>>,
    session_kind: Option<String>,
) -> Result<(), String> {
    start_codex_with_manager(
        app,
        state.inner().clone(),
        employee_id,
        task_description,
        model,
        reasoning_effort,
        system_prompt,
        working_dir,
        task_id,
        task_git_context_id,
        resume_session_id,
        image_paths,
        session_kind,
    )
    .await
}

pub async fn list_live_employee_processes<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<CodexManager>>,
    employee_id: &str,
) -> Result<Vec<crate::codex::manager::ManagedCodexProcess>, String> {
    get_live_managed_processes_with_manager(app, manager_state, employee_id).await
}

pub async fn get_live_task_process<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<CodexManager>>,
    employee_id: &str,
    task_id: &str,
    session_kind: CodexSessionKind,
) -> Result<Option<crate::codex::manager::ManagedCodexProcess>, String> {
    get_live_task_process_with_manager(app, manager_state, employee_id, task_id, session_kind).await
}

pub async fn get_live_task_process_by_task<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<CodexManager>>,
    task_id: &str,
    session_kind: CodexSessionKind,
) -> Result<Option<crate::codex::manager::ManagedCodexProcess>, String> {
    get_live_task_process_by_task_with_manager(app, manager_state, task_id, session_kind).await
}

pub async fn start_codex_with_manager(
    app: AppHandle,
    manager_state: Arc<Mutex<CodexManager>>,
    employee_id: String,
    task_description: String,
    model: Option<String>,
    reasoning_effort: Option<String>,
    system_prompt: Option<String>,
    working_dir: Option<String>,
    task_id: Option<String>,
    task_git_context_id: Option<String>,
    resume_session_id: Option<String>,
    image_paths: Option<Vec<String>>,
    session_kind: Option<String>,
) -> Result<(), String> {
    let session_kind = normalize_session_kind(session_kind.as_deref());

    if let Some(task_id) = task_id.as_deref() {
        if get_live_task_process_by_task_with_manager(&app, &manager_state, task_id, session_kind)
            .await?
            .is_some()
        {
            return Err(format!(
                "任务{}的{}会话已在运行",
                task_id,
                session_kind.as_str()
            ));
        }
    } else if get_live_managed_process_with_manager(&app, &manager_state, &employee_id)
        .await?
        .is_some()
    {
        return Err(format!(
            "员工{}已有未绑定任务的 Codex 会话在运行",
            employee_id
        ));
    }

    let execution_context =
        match resolve_session_execution_context(&app, task_id.as_deref(), working_dir.as_deref())
            .await
        {
            Ok(context) => context,
            Err(error) => {
                record_failed_session(
                    &app,
                    &employee_id,
                    task_id.as_deref(),
                    working_dir.as_deref(),
                    resume_session_id.as_deref(),
                    session_kind,
                    &error,
                )
                .await;
                return Err(error);
            }
        };

    if let (Some(task_id), Some(task_git_context_id)) =
        (task_id.as_deref(), task_git_context_id.as_deref())
    {
        let validated_worktree = validate_task_git_context_launch(
            &app,
            task_id,
            task_git_context_id,
            execution_context.working_dir.as_deref(),
        )
        .await?;
        if execution_context.working_dir.as_deref() != Some(validated_worktree.as_str()) {
            return Err("task git context 与 working_dir 不一致".to_string());
        }
    }

    let run_cwd = if execution_context.execution_target == EXECUTION_TARGET_LOCAL {
        match validate_runtime_working_dir(execution_context.working_dir.as_deref()) {
            Ok(path) => path,
            Err(error) => {
                record_failed_session(
                    &app,
                    &employee_id,
                    task_id.as_deref(),
                    execution_context.working_dir.as_deref(),
                    resume_session_id.as_deref(),
                    session_kind,
                    &error,
                )
                .await;
                return Err(error);
            }
        }
    } else {
        execution_context
            .working_dir
            .clone()
            .ok_or_else(|| "SSH 项目缺少远程仓库目录，无法启动 Codex。".to_string())?
    };

    let session_record = insert_codex_session_record(
        &app,
        Some(&employee_id),
        task_id.as_deref(),
        task_git_context_id.as_deref(),
        Some(&run_cwd),
        resume_session_id.as_deref(),
        session_kind.as_str(),
        "pending",
        &execution_context.execution_target,
        execution_context.ssh_config_id.as_deref(),
        execution_context.target_host_label.as_deref(),
        &execution_context.artifact_capture_mode,
    )
    .await?;
    let pool = sqlite_pool(&app).await?;
    if let Some(task_git_context_id) = task_git_context_id.as_deref() {
        mark_task_git_context_running(&pool, task_git_context_id).await?;
    }
    insert_codex_session_event(
        &pool,
        &session_record.id,
        "session_requested",
        Some("Codex 会话创建成功，准备启动运行时"),
    )
    .await?;
    if execution_context.execution_target == EXECUTION_TARGET_SSH {
        emit_session_terminal_line(
            &app,
            &pool,
            &session_record.id,
            &employee_id,
            task_id.as_deref(),
            session_kind,
            format!(
                "[SSH] 正在准备远程会话，目标 {}，工作目录 {}",
                execution_context
                    .target_host_label
                    .as_deref()
                    .unwrap_or("未知登录目标"),
                run_cwd
            ),
        )
        .await;
    }

    let model = normalize_model(model.as_deref());
    let reasoning_effort = normalize_reasoning_effort(reasoning_effort.as_deref());
    let prompt = compose_codex_prompt(&task_description, system_prompt.as_deref());
    let (image_paths, missing_image_paths, ignored_remote_image_count) =
        match prepare_execution_image_paths(
            &app,
            task_id.as_deref(),
            &execution_context.execution_target,
            execution_context.ssh_config_id.as_deref(),
            image_paths,
        )
        .await
        {
            Ok(result) => result,
            Err(error) => {
                let ended_at = now_sqlite();
                update_codex_session_record(
                    &app,
                    &session_record.id,
                    Some("failed"),
                    None,
                    None,
                    Some(Some(ended_at.as_str())),
                )
                .await?;
                insert_codex_session_event(
                    &pool,
                    &session_record.id,
                    "session_image_prepare_failed",
                    Some(&error),
                )
                .await?;
                return Err(error);
            }
        };
    let session_launch = match prepare_session_launch(
        &app,
        &pool,
        &session_record.id,
        &employee_id,
        task_id.as_deref(),
        session_kind,
        &execution_context,
        model,
        reasoning_effort,
        &run_cwd,
        &image_paths,
        resume_session_id.as_deref(),
    )
    .await
    {
        Ok(launch) => launch,
        Err(error) => {
            let ended_at = now_sqlite();
            update_codex_session_record(
                &app,
                &session_record.id,
                Some("failed"),
                None,
                None,
                Some(Some(ended_at.as_str())),
            )
            .await?;
            insert_codex_session_event(&pool, &session_record.id, "spawn_failed", Some(&error))
                .await?;
            return Err(error);
        }
    };
    let SessionLaunch {
        provider,
        mut command,
        cleanup_paths,
        cli_json_output_flag,
        session_lookup_started_at,
        sdk_codex_path_override,
        ssh_config_name,
        ssh_host,
        ssh_config_for_artifact_capture,
        remote_sdk_fallback_error,
        remote_sdk_fallback_logged,
    } = session_launch;
    let execution_change_baseline = capture_session_execution_change_baseline(
        &app,
        &pool,
        &session_record.id,
        &employee_id,
        task_id.as_deref(),
        session_kind,
        &execution_context,
        &run_cwd,
        ssh_config_for_artifact_capture.as_ref(),
    )
    .await?;

    configure_process_group(&mut command);

    if provider == CodexExecutionProvider::Cli {
        command
            .args(build_session_exec_args(
                model,
                reasoning_effort,
                &run_cwd,
                &image_paths,
                resume_session_id.as_deref(),
                cli_json_output_flag,
            ))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
    }

    emit_session_launch_diagnostics(
        &app,
        &pool,
        &session_record.id,
        &employee_id,
        task_id.as_deref(),
        session_kind,
        provider,
        &execution_context,
        &run_cwd,
        model,
        reasoning_effort,
        &prompt,
        &image_paths,
        &missing_image_paths,
        ignored_remote_image_count,
        ssh_config_name.as_deref(),
        ssh_host.as_deref(),
        remote_sdk_fallback_error.as_deref(),
        remote_sdk_fallback_logged,
        cli_json_output_flag,
    )
    .await;

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let message = format!("Failed to spawn codex {}: {}", provider.label(), error);
            let ended_at = now_sqlite();
            update_codex_session_record(
                &app,
                &session_record.id,
                Some("failed"),
                None,
                None,
                Some(Some(ended_at.as_str())),
            )
            .await?;
            insert_codex_session_event(&pool, &session_record.id, "spawn_failed", Some(&message))
                .await?;
            return Err(message);
        }
    };

    match provider {
        CodexExecutionProvider::Sdk => {
            let payload = serde_json::to_vec(&serde_json::json!({
                "mode": "session",
                "prompt": prompt.clone(),
                "input": build_sdk_input_items(&prompt, &image_paths),
                "model": model,
                "modelReasoningEffort": reasoning_effort,
                "codexPathOverride": sdk_codex_path_override.clone(),
                "workingDirectory": run_cwd.clone(),
                "resumeSessionId": resume_session_id.clone(),
            }))
            .map_err(|error| format!("Failed to serialize Codex SDK session payload: {}", error))?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(&payload).await.map_err(|error| {
                    format!("Failed to write Codex SDK session payload: {}", error)
                })?;
                stdin.shutdown().await.map_err(|error| {
                    format!("Failed to close Codex SDK session stdin: {}", error)
                })?;
            }
        }
        CodexExecutionProvider::Cli => {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(prompt.as_bytes()).await.map_err(|error| {
                    format!("Failed to write Codex CLI session prompt: {}", error)
                })?;
                stdin.shutdown().await.map_err(|error| {
                    format!("Failed to close Codex CLI session stdin: {}", error)
                })?;
            }
        }
    }

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;
    let sdk_file_change_store = (provider == CodexExecutionProvider::Sdk
        && session_kind == CodexSessionKind::Execution)
        .then(|| {
            Arc::new(Mutex::new(
                HashMap::<String, CodexSessionFileChangeInput>::new(),
            ))
        });

    let child_handle = Arc::new(tokio::sync::Mutex::new(CodexChild::new(child)));

    {
        let mut manager = manager_state.lock().map_err(|e| e.to_string())?;
        manager.add_process(
            employee_id.clone(),
            task_id.clone(),
            session_kind,
            child_handle.clone(),
            session_record.id.clone(),
            provider,
            execution_change_baseline.clone(),
            sdk_file_change_store.clone(),
            cleanup_paths,
        );
    }
    update_codex_session_record(&app, &session_record.id, Some("running"), None, None, None)
        .await?;
    insert_codex_session_event(
        &pool,
        &session_record.id,
        "session_started",
        Some(&format!(
            "通过 {} 启动，使用模型 {} / 推理强度 {} / 图片 {} 张{}",
            provider.label(),
            model,
            reasoning_effort,
            image_paths.len(),
            if execution_context.execution_target == EXECUTION_TARGET_SSH {
                format!(
                    " / SSH {} / 主机 {} / 登录 {}",
                    ssh_config_name.as_deref().unwrap_or("未命名 SSH 配置"),
                    ssh_host.as_deref().unwrap_or("未知主机"),
                    execution_context
                        .target_host_label
                        .as_deref()
                        .unwrap_or("未知登录目标")
                )
            } else {
                String::new()
            }
        )),
    )
    .await?;
    write_task_session_activity(
        &app,
        &pool,
        &session_record.id,
        &employee_id,
        task_id.as_deref(),
        session_kind,
        resume_session_id.as_deref(),
    )
    .await;

    attach_session_runtime_tasks(
        &app,
        &pool,
        &employee_id,
        task_id.clone(),
        session_record.id.clone(),
        session_kind,
        provider,
        resume_session_id.clone(),
        session_lookup_started_at,
        run_cwd.clone(),
        stdout,
        stderr,
        child_handle.clone(),
        execution_change_baseline.clone(),
        sdk_file_change_store.clone(),
        cli_json_output_flag,
    )
    .await;

    Ok(())
}

#[tauri::command]
pub async fn stop_codex_session(
    app: AppHandle,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    session_record_id: String,
) -> Result<(), String> {
    if stop_managed_process_with_manager(
        &app,
        state.inner(),
        &session_record_id,
        "stopping_requested",
        "收到停止请求",
    )
    .await?
    {
        Ok(())
    } else {
        Err(format!(
            "No running codex instance for session {}",
            session_record_id
        ))
    }
}

#[tauri::command]
pub async fn stop_codex(
    app: AppHandle,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    employee_id: String,
) -> Result<(), String> {
    let live_processes =
        get_live_managed_processes_with_manager(&app, state.inner(), &employee_id).await?;
    if live_processes.is_empty() {
        Err(format!(
            "No running codex instance for employee {}",
            employee_id
        ))
    } else {
        for process in live_processes {
            stop_managed_process_with_manager(
                &app,
                state.inner(),
                &process.session_record_id,
                "stopping_requested",
                "收到停止请求",
            )
            .await?;
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn restart_codex(
    app: AppHandle,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    employee_id: String,
    task_description: String,
    model: Option<String>,
    reasoning_effort: Option<String>,
    system_prompt: Option<String>,
    working_dir: Option<String>,
    task_git_context_id: Option<String>,
) -> Result<(), String> {
    let live_processes =
        get_live_managed_processes_with_manager(&app, state.inner(), &employee_id).await?;
    for process in live_processes {
        stop_managed_process_with_manager(
            &app,
            state.inner(),
            &process.session_record_id,
            "restart_requested",
            "收到重启请求",
        )
        .await?;
    }

    start_codex(
        app,
        state,
        employee_id,
        task_description,
        model,
        reasoning_effort,
        system_prompt,
        working_dir,
        None,
        task_git_context_id,
        None,
        None,
        None,
    )
    .await
}

#[tauri::command]
pub async fn send_codex_input(
    state: State<'_, Arc<Mutex<CodexManager>>>,
    employee_id: String,
    _input: String,
) -> Result<(), String> {
    let manager = state.lock().map_err(|e| e.to_string())?;
    if manager.has_employee_processes(&employee_id) {
        Err("Cannot write to stdin in non-interactive mode".to_string())
    } else {
        Err(format!(
            "No running codex instance for employee {}",
            employee_id
        ))
    }
}

async fn should_use_sdk_for_session(app: &AppHandle) -> bool {
    match load_codex_settings(app) {
        Ok(settings) if settings.task_sdk_enabled => {
            let runtime = inspect_sdk_runtime(app, &settings).await;
            runtime.task_execution_effective_provider == "sdk"
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
