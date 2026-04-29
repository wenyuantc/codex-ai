use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::io::AsyncWriteExt;

use crate::app::{
    build_remote_shell_command, build_ssh_command, fetch_ssh_config_record_by_id,
    insert_activity_log, insert_codex_session_event, insert_codex_session_event_with_id,
    insert_codex_session_record, now_sqlite, remote_shell_path_expression,
    shell_escape_single_quoted, sqlite_pool, update_codex_session_record,
    validate_runtime_working_dir, EXECUTION_TARGET_LOCAL, EXECUTION_TARGET_SSH,
};
use crate::claude::{
    ensure_claude_sdk_runtime_layout, inspect_claude_sdk_runtime, load_claude_settings,
    normalize_claude_model, sdk_bridge_script_path, ClaudeManager,
};
use crate::codex::{new_node_command, CodexManager, CodexSessionKind, ExecutionChangeBaseline};
use crate::db::models::{ClaudeOutput, CodexSessionFileChangeInput, SshConfigRecord};
use crate::git_workflow::{
    mark_task_git_context_running, mark_task_git_context_session_finished,
    validate_task_git_context_launch,
};

mod context;
mod lifecycle;
mod session_runtime;
mod stream;

pub use self::lifecycle::ClaudeChild;

use self::{context::*, session_runtime::*, stream::*};

const SUPPORTED_REASONING_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max", "auto"];
const DEFAULT_REASONING_EFFORT: &str = "high";
const SESSION_ID_PREFIX: &str = "session id:";
const CLAUDE_FILE_CHANGE_EVENT_PREFIX: &str = "[CLAUDE_FILE_CHANGE]";
const STOP_WAIT_POLL_MS: u64 = 50;
const STOP_WAIT_MAX_ATTEMPTS: usize = 600;

pub type SdkFileChangeStore = Arc<Mutex<HashMap<String, CodexSessionFileChangeInput>>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaudeSessionKind {
    Execution,
    Review,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClaudeExecutionProvider {
    Sdk,
    Cli,
}

impl ClaudeExecutionProvider {
    fn label(self) -> &'static str {
        match self {
            ClaudeExecutionProvider::Sdk => "Claude Agent SDK",
            ClaudeExecutionProvider::Cli => "Claude CLI",
        }
    }
}

impl ClaudeSessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ClaudeSessionKind::Execution => "execution",
            ClaudeSessionKind::Review => "review",
        }
    }

    fn activity_start_action(self, resumed: bool) -> &'static str {
        match self {
            ClaudeSessionKind::Execution => {
                if resumed {
                    "task_execution_resumed"
                } else {
                    "task_execution_started"
                }
            }
            ClaudeSessionKind::Review => "task_review_started",
        }
    }
}

fn normalize_session_kind(session_kind: Option<&str>) -> ClaudeSessionKind {
    match session_kind {
        Some("review") => ClaudeSessionKind::Review,
        _ => ClaudeSessionKind::Execution,
    }
}

fn claude_session_kind_to_codex(session_kind: ClaudeSessionKind) -> CodexSessionKind {
    match session_kind {
        ClaudeSessionKind::Execution => CodexSessionKind::Execution,
        ClaudeSessionKind::Review => CodexSessionKind::Review,
    }
}

fn cleanup_process_artifacts(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn normalize_model(model: Option<&str>) -> String {
    normalize_claude_model(model)
}

fn normalize_reasoning_effort(effort: Option<&str>) -> &'static str {
    match effort {
        Some(value) if SUPPORTED_REASONING_EFFORTS.contains(&value) => match value {
            "low" => "low",
            "medium" => "medium",
            "high" => "high",
            "xhigh" => "xhigh",
            "max" => "max",
            "auto" => "auto",
            _ => DEFAULT_REASONING_EFFORT,
        },
        _ => DEFAULT_REASONING_EFFORT,
    }
}

fn build_claude_cli_args(
    model: &str,
    effort: &str,
    system_prompt: Option<&str>,
    resume_session_id: Option<&str>,
) -> Vec<String> {
    let mut args = vec!["-p".to_string(), "--model".to_string(), model.to_string()];

    if effort != "auto" {
        args.push("--effort".to_string());
        args.push(effort.to_string());
    }

    args.extend([
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--permission-mode".to_string(),
        "bypassPermissions".to_string(),
    ]);

    if let Some(sp) = system_prompt.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--system-prompt".to_string());
        args.push(sp.to_string());
    }

    if let Some(resume_id) = resume_session_id.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--resume".to_string());
        args.push(resume_id.to_string());
    }

    args
}

fn build_remote_claude_session_command(run_cwd: &str, cli_args: &[String]) -> String {
    let escaped_args = cli_args
        .iter()
        .map(|arg| shell_escape_single_quoted(arg))
        .collect::<Vec<_>>();

    build_remote_shell_command(
        &format!(
            "cd {} && exec claude {}",
            remote_shell_path_expression(run_cwd),
            escaped_args.join(" "),
        ),
        None,
    )
}

fn effort_to_thinking_budget(effort: &str) -> Option<i32> {
    match effort {
        "low" => Some(5_000),
        "medium" => Some(10_000),
        "high" => Some(16_000),
        "xhigh" => Some(32_000),
        "max" => Some(128_000),
        "auto" => None,
        _ => Some(10_000),
    }
}

fn thinking_budget_to_effort(budget: i32) -> &'static str {
    if budget >= 128_000 {
        "max"
    } else if budget >= 32_000 {
        "xhigh"
    } else if budget >= 16_000 {
        "high"
    } else if budget >= 10_000 {
        "medium"
    } else {
        "low"
    }
}

fn compose_claude_prompt(task_description: &str, system_prompt: Option<&str>) -> String {
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

fn format_claude_session_prompt_log(
    provider: ClaudeExecutionProvider,
    model: &str,
    effort: &str,
    execution_target: &str,
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
        format!(
            "执行环境: SSH 远程运行\nSSH 登录: {}",
            target_host_label.unwrap_or("未知登录目标")
        )
    } else {
        "执行环境: 本地运行".to_string()
    };

    format!(
        "[PROMPT] 即将发送给 Claude 的完整提示词\n\
运行通道: {}\n\
模型: {}\n\
推理强度: {}\n\
{}\n\
工作目录: {}\n\
{}\n\n{}",
        provider.label(),
        model,
        effort,
        runtime_block,
        working_dir,
        image_block,
        prompt
    )
}

pub(crate) fn extract_review_report(raw: &str) -> Option<String> {
    crate::codex::extract_review_report(raw)
}

pub(crate) fn extract_review_verdict(raw: &str) -> Option<String> {
    crate::codex::extract_review_verdict(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_prompt_log_includes_full_prompt_and_runtime() {
        let prompt = compose_claude_prompt("修复自动质控", Some("你是审查员"));
        let log = format_claude_session_prompt_log(
            ClaudeExecutionProvider::Sdk,
            "sonnet",
            "high",
            EXECUTION_TARGET_LOCAL,
            None,
            "/tmp/project",
            &prompt,
            &[],
        );

        assert!(log.contains("[PROMPT] 即将发送给 Claude 的完整提示词"));
        assert!(log.contains("运行通道: Claude Agent SDK"));
        assert!(log.contains("<employee_system_prompt>"));
        assert!(log.contains("你是审查员"));
        assert!(log.contains("<task>"));
        assert!(log.contains("修复自动质控"));
    }

    #[test]
    fn thinking_budget_defaults_map_to_nearest_effort() {
        assert_eq!(thinking_budget_to_effort(5_000), "low");
        assert_eq!(thinking_budget_to_effort(10_000), "medium");
        assert_eq!(thinking_budget_to_effort(16_000), "high");
        assert_eq!(thinking_budget_to_effort(32_000), "xhigh");
        assert_eq!(thinking_budget_to_effort(128_000), "max");
    }

    #[test]
    fn claude_cli_args_skip_auto_effort() {
        let args = build_claude_cli_args("sonnet", "auto", None, None);

        assert!(!args.contains(&"--effort".to_string()));
        assert!(!args.contains(&"auto".to_string()));
    }

    #[test]
    fn claude_cli_args_keep_supported_effort_and_resume() {
        let args = build_claude_cli_args("sonnet", "high", Some("审查代码"), Some("session-123"));

        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--effort" && pair[1] == "high"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--system-prompt" && pair[1] == "审查代码"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--resume" && pair[1] == "session-123"));
    }

    #[test]
    fn remote_claude_session_command_uses_shell_bootstrap() {
        let args = build_claude_cli_args("sonnet", "high", None, None);
        let command = build_remote_claude_session_command("~/repo with space", &args);

        assert!(command.starts_with("sh -lc "));
        assert!(command.contains("PATH="));
        assert!(command.contains("\"$HOME/.local/bin\""));
        assert!(command.contains("exec claude"));
        assert!(command.contains("$HOME/repo with space"));
        assert!(!command.contains("修复"));
        assert!(!command.contains("bug"));
    }
}

fn resolve_claude_binary_path(
    settings: &crate::db::models::ClaudeSettings,
) -> Result<PathBuf, String> {
    if let Some(cli_path_override) = settings
        .cli_path_override
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(PathBuf::from(cli_path_override));
    }

    let install_dir = PathBuf::from(&settings.sdk_install_dir);
    let bin_name = if cfg!(target_os = "windows") {
        "claude.exe"
    } else {
        "claude"
    };
    let pkg_bin = install_dir
        .join("node_modules")
        .join("@anthropic-ai")
        .join("claude-code")
        .join("bin")
        .join(bin_name);
    if pkg_bin.exists() {
        return Ok(pkg_bin);
    }

    Ok(PathBuf::from("claude"))
}

fn upsert_sdk_file_change_event(store: &SdkFileChangeStore, event: SdkFileChangeEvent) {
    let mut guard = store.lock().unwrap();
    for change in event.changes {
        let path = change.path.unwrap_or_default().trim().to_string();
        if path.is_empty() {
            continue;
        }
        let Some(change_kind) = normalize_file_change_kind(change.kind.as_deref()) else {
            continue;
        };
        guard.insert(
            path.clone(),
            CodexSessionFileChangeInput {
                path,
                change_type: change_kind.to_string(),
                capture_mode: "sdk_event".to_string(),
                previous_path: change
                    .previous_path
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty()),
                detail: None,
            },
        );
    }
}

fn normalize_file_change_kind(value: Option<&str>) -> Option<&'static str> {
    match value.map(|v| v.trim().to_ascii_lowercase()) {
        Some(v) if matches!(v.as_str(), "add" | "added" | "create" | "created") => Some("added"),
        Some(v)
            if matches!(
                v.as_str(),
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
            Some("modified")
        }
        Some(v) if matches!(v.as_str(), "delete" | "deleted" | "remove" | "removed") => {
            Some("deleted")
        }
        Some(v) if matches!(v.as_str(), "rename" | "renamed" | "move" | "moved") => Some("renamed"),
        _ => None,
    }
}

async fn emit_session_terminal_line<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: ClaudeSessionKind,
    line: String,
) {
    let event_id =
        insert_codex_session_event_with_id(pool, session_record_id, "stdout", Some(&line))
            .await
            .ok();

    let _ = app.emit(
        "claude-stdout",
        ClaudeOutput {
            employee_id: employee_id.to_string(),
            task_id: task_id.map(String::from),
            session_kind: session_kind.as_str().to_string(),
            session_record_id: session_record_id.to_string(),
            session_event_id: event_id,
            line,
        },
    );
}

async fn fetch_task_activity_context(
    pool: &SqlitePool,
    task_id: &str,
) -> Result<(String, String), String> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT title, project_id FROM tasks WHERE id = $1 LIMIT 1",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        format!(
            "Failed to resolve task {} for Claude activity log: {}",
            task_id, error
        )
    })
}

async fn write_claude_task_session_activity<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: ClaudeSessionKind,
    resume_session_id: Option<&str>,
    execution_target: &str,
) {
    let Some(task_id) = task_id else {
        return;
    };

    let result = async {
        let (task_title, project_id) = fetch_task_activity_context(pool, task_id).await?;
        let action = if execution_target == EXECUTION_TARGET_SSH
            && session_kind == ClaudeSessionKind::Execution
        {
            "remote_task_session_started"
        } else {
            session_kind.activity_start_action(resume_session_id.is_some())
        };

        insert_activity_log(
            pool,
            action,
            &task_title,
            Some(employee_id),
            Some(task_id),
            Some(project_id.as_str()),
        )
        .await
    }
    .await;

    if let Err(error) = result {
        let _ = insert_codex_session_event(
            pool,
            session_record_id,
            "activity_log_failed",
            Some(&error),
        )
        .await;
        emit_session_terminal_line(
            app,
            pool,
            session_record_id,
            employee_id,
            Some(task_id),
            session_kind,
            format!("[WARN] Claude 活动日志写入失败: {error}"),
        )
        .await;
    }
}

async fn ensure_no_cross_provider_conflict<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: ClaudeSessionKind,
) -> Result<(), String> {
    let Some(codex_state) = app.try_state::<Arc<Mutex<CodexManager>>>() else {
        return Ok(());
    };

    if let Some(task_id) = task_id {
        if crate::codex::get_live_task_process_by_task(
            app,
            codex_state.inner(),
            task_id,
            claude_session_kind_to_codex(session_kind),
        )
        .await?
        .is_some()
        {
            return Err(format!(
                "任务{}的{}会话已在运行",
                task_id,
                session_kind.as_str()
            ));
        }
    } else if !crate::codex::list_live_employee_processes(app, codex_state.inner(), employee_id)
        .await?
        .is_empty()
    {
        return Err(format!(
            "员工{}已有未绑定任务的 Codex 会话在运行",
            employee_id
        ));
    }

    Ok(())
}

async fn finalize_claude_launch_failure<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    session_record_id: &str,
    task_git_context_id: Option<&str>,
    git_context_marked_running: bool,
    event_type: &str,
    error: &str,
) {
    let ended_at = now_sqlite();
    let _ = update_codex_session_record(
        app,
        session_record_id,
        Some("failed"),
        None,
        None,
        Some(Some(ended_at.as_str())),
    )
    .await;
    let _ = insert_codex_session_event(pool, session_record_id, event_type, Some(error)).await;

    if git_context_marked_running {
        if let Some(task_git_context_id) = task_git_context_id {
            let _ = mark_task_git_context_session_finished(
                pool,
                task_git_context_id,
                false,
                Some(error),
            )
            .await;
        }
    }
}

async fn capture_claude_execution_change_baseline<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: ClaudeSessionKind,
    execution_target: &str,
    run_cwd: &str,
    ssh_config: Option<&SshConfigRecord>,
) -> Option<ExecutionChangeBaseline> {
    if session_kind != ClaudeSessionKind::Execution {
        return None;
    }

    if execution_target == EXECUTION_TARGET_SSH {
        emit_session_terminal_line(
            app,
            pool,
            session_record_id,
            employee_id,
            task_id,
            session_kind,
            "[SSH] 正在采集远程仓库基线，用于展示本次 Claude 会话改动...".to_string(),
        )
        .await;
    }

    let baseline_result = if execution_target == EXECUTION_TARGET_SSH {
        match ssh_config {
            Some(ssh_config) => {
                crate::codex::capture_external_remote_execution_change_baseline(
                    app, ssh_config, run_cwd,
                )
                .await
            }
            None => Err("SSH 会话缺少 SSH 配置，无法采集远程文件基线".to_string()),
        }
    } else {
        crate::codex::capture_external_execution_change_baseline(run_cwd)
    };

    match baseline_result {
        Ok(baseline) => {
            if execution_target == EXECUTION_TARGET_SSH {
                emit_session_terminal_line(
                    app,
                    pool,
                    session_record_id,
                    employee_id,
                    task_id,
                    session_kind,
                    "[SSH] 远程仓库基线采集完成。".to_string(),
                )
                .await;
            }
            Some(baseline)
        }
        Err(error) => {
            let _ = insert_codex_session_event(
                pool,
                session_record_id,
                "session_file_changes_baseline_failed",
                Some(&error),
            )
            .await;
            emit_session_terminal_line(
                app,
                pool,
                session_record_id,
                employee_id,
                task_id,
                session_kind,
                format!(
                    "[WARN] Claude 会话文件基线采集失败，文件详情将退化为最佳努力快照: {error}"
                ),
            )
            .await;
            None
        }
    }
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
pub async fn start_claude(
    app: AppHandle,
    state: State<'_, Arc<tokio::sync::Mutex<ClaudeManager>>>,
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
    start_claude_with_manager(
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

pub async fn start_claude_with_manager(
    app: AppHandle,
    manager_state: Arc<tokio::sync::Mutex<ClaudeManager>>,
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
        let manager = manager_state.lock().await;
        if manager
            .get_task_process_any(task_id, session_kind)
            .is_some()
        {
            return Err(format!(
                "任务{}的{}会话已在运行",
                task_id,
                session_kind.as_str()
            ));
        }
    } else {
        let manager = manager_state.lock().await;
        if manager.has_employee_processes(&employee_id) {
            return Err(format!(
                "员工{}已有未绑定任务的 Claude 会话在运行",
                employee_id
            ));
        }
    }
    ensure_no_cross_provider_conflict(&app, &employee_id, task_id.as_deref(), session_kind).await?;

    let execution_context =
        resolve_session_execution_context(&app, task_id.as_deref(), working_dir.as_deref()).await?;

    let run_cwd = if execution_context.execution_target == EXECUTION_TARGET_LOCAL {
        match validate_runtime_working_dir(execution_context.working_dir.as_deref()) {
            Ok(path) => path,
            Err(error) => return Err(error),
        }
    } else {
        execution_context
            .working_dir
            .clone()
            .ok_or_else(|| "SSH 项目缺少远程仓库目录，无法启动 Claude。".to_string())?
    };

    if let (Some(task_id), Some(task_git_context_id)) =
        (task_id.as_deref(), task_git_context_id.as_deref())
    {
        let validated_worktree =
            validate_task_git_context_launch(&app, task_id, task_git_context_id, Some(&run_cwd))
                .await?;
        if run_cwd != validated_worktree {
            return Err("task git context 与 working_dir 不一致".to_string());
        }
    }

    let pool = sqlite_pool(&app).await?;

    let claude_settings = load_claude_settings(&app)?;
    let model = normalize_model(
        model
            .as_deref()
            .or(Some(claude_settings.default_model.as_str())),
    );
    let requested_effort = reasoning_effort
        .as_deref()
        .map(|effort| normalize_reasoning_effort(Some(effort)));
    let effort = requested_effort
        .unwrap_or_else(|| thinking_budget_to_effort(claude_settings.default_thinking_budget));
    let thinking_budget = match requested_effort {
        Some(effort) => effort_to_thinking_budget(effort),
        None => Some(claude_settings.default_thinking_budget),
    };
    let prompt = compose_claude_prompt(&task_description, system_prompt.as_deref());
    let requested_image_paths = image_paths;

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
        Some("claude"),
        thinking_budget,
    )
    .await?;

    let mut git_context_marked_running = false;
    if let Some(task_git_context_id) = task_git_context_id.as_deref() {
        if let Err(error) = mark_task_git_context_running(&pool, task_git_context_id).await {
            finalize_claude_launch_failure(
                &app,
                &pool,
                &session_record.id,
                Some(task_git_context_id),
                false,
                "launch_failed",
                &error,
            )
            .await;
            return Err(error);
        }
        git_context_marked_running = true;
    }

    if let Err(error) = insert_codex_session_event(
        &pool,
        &session_record.id,
        "session_requested",
        Some("Claude 会话创建成功，准备启动运行时"),
    )
    .await
    {
        finalize_claude_launch_failure(
            &app,
            &pool,
            &session_record.id,
            task_git_context_id.as_deref(),
            git_context_marked_running,
            "launch_failed",
            &error,
        )
        .await;
        return Err(error);
    }

    if execution_context.execution_target == EXECUTION_TARGET_SSH {
        emit_session_terminal_line(
            &app,
            &pool,
            &session_record.id,
            &employee_id,
            task_id.as_deref(),
            session_kind,
            format!(
                "[SSH] 正在准备远程 Claude 会话，目标 {}，工作目录 {}",
                execution_context
                    .target_host_label
                    .as_deref()
                    .unwrap_or("未知登录目标"),
                run_cwd
            ),
        )
        .await;
    }

    let (image_paths, missing_image_paths, ignored_remote_image_count) =
        match crate::codex::process::prepare_execution_image_paths(
            &app,
            task_id.as_deref(),
            &execution_context.execution_target,
            execution_context.ssh_config_id.as_deref(),
            requested_image_paths,
        )
        .await
        {
            Ok(result) => result,
            Err(error) => {
                finalize_claude_launch_failure(
                    &app,
                    &pool,
                    &session_record.id,
                    task_git_context_id.as_deref(),
                    git_context_marked_running,
                    "session_image_prepare_failed",
                    &error,
                )
                .await;
                return Err(error);
            }
        };
    for missing_path in &missing_image_paths {
        emit_session_terminal_line(
            &app,
            &pool,
            &session_record.id,
            &employee_id,
            task_id.as_deref(),
            session_kind,
            format!("[WARN] Claude 附件图片不存在，已跳过: {missing_path}"),
        )
        .await;
    }
    if ignored_remote_image_count > 0 {
        emit_session_terminal_line(
            &app,
            &pool,
            &session_record.id,
            &employee_id,
            task_id.as_deref(),
            session_kind,
            format!(
                "[WARN] SSH Claude 会话缺少任务上下文，已忽略 {} 张本地图片附件。",
                ignored_remote_image_count
            ),
        )
        .await;
    }

    let claude_health = inspect_claude_sdk_runtime(&app, &claude_settings).await;

    let cli_args = build_claude_cli_args(
        &model,
        effort,
        system_prompt.as_deref(),
        resume_session_id.as_deref(),
    );

    let ssh_config_for_artifact_capture =
        if execution_context.execution_target == EXECUTION_TARGET_SSH {
            let ssh_config_id = match execution_context.ssh_config_id.as_deref() {
                Some(ssh_config_id) => ssh_config_id,
                None => {
                    let error = "SSH 会话缺少 ssh_config_id".to_string();
                    finalize_claude_launch_failure(
                        &app,
                        &pool,
                        &session_record.id,
                        task_git_context_id.as_deref(),
                        git_context_marked_running,
                        "runtime_prepare_failed",
                        &error,
                    )
                    .await;
                    return Err(error);
                }
            };

            match fetch_ssh_config_record_by_id(&pool, ssh_config_id).await {
                Ok(ssh_config) => Some(ssh_config),
                Err(error) => {
                    finalize_claude_launch_failure(
                        &app,
                        &pool,
                        &session_record.id,
                        task_git_context_id.as_deref(),
                        git_context_marked_running,
                        "runtime_prepare_failed",
                        &error,
                    )
                    .await;
                    return Err(error);
                }
            }
        } else {
            None
        };

    let execution_change_baseline = capture_claude_execution_change_baseline(
        &app,
        &pool,
        &session_record.id,
        &employee_id,
        task_id.as_deref(),
        session_kind,
        &execution_context.execution_target,
        &run_cwd,
        ssh_config_for_artifact_capture.as_ref(),
    )
    .await;

    let (mut child, cleanup_paths, provider) = if execution_context.execution_target
        == EXECUTION_TARGET_SSH
    {
        let ssh_config = ssh_config_for_artifact_capture
            .as_ref()
            .expect("SSH config is prepared before Claude launch");

        let remote_command = build_remote_claude_session_command(&run_cwd, &cli_args);

        let (mut command, askpass_path) =
            match build_ssh_command(&app, &ssh_config, Some(&remote_command), true, false).await {
                Ok(result) => result,
                Err(error) => {
                    finalize_claude_launch_failure(
                        &app,
                        &pool,
                        &session_record.id,
                        task_git_context_id.as_deref(),
                        git_context_marked_running,
                        "spawn_failed",
                        &error,
                    )
                    .await;
                    return Err(error);
                }
            };
        command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        configure_process_group(&mut command);

        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let message = format!("启动远程 Claude 会话失败: {error}");
                let cleanup_paths = askpass_path.iter().cloned().collect::<Vec<_>>();
                cleanup_process_artifacts(&cleanup_paths);
                finalize_claude_launch_failure(
                    &app,
                    &pool,
                    &session_record.id,
                    task_git_context_id.as_deref(),
                    git_context_marked_running,
                    "spawn_failed",
                    &message,
                )
                .await;
                return Err(message);
            }
        };
        (
            child,
            askpass_path.into_iter().collect::<Vec<_>>(),
            ClaudeExecutionProvider::Cli,
        )
    } else if claude_health.effective_provider == "sdk" {
        let install_dir = PathBuf::from(&claude_settings.sdk_install_dir);
        let bridge_path = sdk_bridge_script_path(&install_dir);
        let sdk_child = match ensure_claude_sdk_runtime_layout(&install_dir) {
            Ok(()) => match new_node_command(claude_settings.node_path_override.as_deref()).await {
                Ok(mut command) => {
                    command
                        .arg(&bridge_path)
                        .current_dir(&run_cwd)
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped());
                    configure_process_group(&mut command);

                    match command.spawn() {
                        Ok(child) => Some(child),
                        Err(error) => {
                            eprintln!("[claude-sdk] SDK 会话启动失败，回退 CLI: {error}");
                            None
                        }
                    }
                }
                Err(error) => {
                    eprintln!("[claude-sdk] Node 不可用，回退 CLI: {error}");
                    None
                }
            },
            Err(error) => {
                eprintln!("[claude-sdk] 刷新 SDK bridge 失败，回退 CLI: {error}");
                None
            }
        };

        if let Some(child) = sdk_child {
            (child, Vec::new(), ClaudeExecutionProvider::Sdk)
        } else {
            let claude_bin = match resolve_claude_binary_path(&claude_settings) {
                Ok(path) => path,
                Err(error) => {
                    finalize_claude_launch_failure(
                        &app,
                        &pool,
                        &session_record.id,
                        task_git_context_id.as_deref(),
                        git_context_marked_running,
                        "spawn_failed",
                        &error,
                    )
                    .await;
                    return Err(error);
                }
            };
            let mut command = tokio::process::Command::new(&claude_bin);
            command
                .args(&cli_args)
                .current_dir(&run_cwd)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            configure_process_group(&mut command);

            let child = match command.spawn() {
                Ok(child) => child,
                Err(error) => {
                    let message = format!("启动 Claude 会话失败: {error}");
                    finalize_claude_launch_failure(
                        &app,
                        &pool,
                        &session_record.id,
                        task_git_context_id.as_deref(),
                        git_context_marked_running,
                        "spawn_failed",
                        &message,
                    )
                    .await;
                    return Err(message);
                }
            };
            (child, Vec::new(), ClaudeExecutionProvider::Cli)
        }
    } else {
        let claude_bin = match resolve_claude_binary_path(&claude_settings) {
            Ok(path) => path,
            Err(error) => {
                finalize_claude_launch_failure(
                    &app,
                    &pool,
                    &session_record.id,
                    task_git_context_id.as_deref(),
                    git_context_marked_running,
                    "spawn_failed",
                    &error,
                )
                .await;
                return Err(error);
            }
        };
        let mut command = tokio::process::Command::new(&claude_bin);
        command
            .args(&cli_args)
            .current_dir(&run_cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        configure_process_group(&mut command);

        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let message = format!("启动 Claude 会话失败: {error}");
                finalize_claude_launch_failure(
                    &app,
                    &pool,
                    &session_record.id,
                    task_git_context_id.as_deref(),
                    git_context_marked_running,
                    "spawn_failed",
                    &message,
                )
                .await;
                return Err(message);
            }
        };
        (child, Vec::new(), ClaudeExecutionProvider::Cli)
    };

    if provider == ClaudeExecutionProvider::Cli && !image_paths.is_empty() {
        emit_session_terminal_line(
            &app,
            &pool,
            &session_record.id,
            &employee_id,
            task_id.as_deref(),
            session_kind,
            format!(
                "[WARN] Claude CLI 当前不支持直接附带本地图片，已跳过 {} 张图片附件。",
                image_paths.len()
            ),
        )
        .await;
    }

    if provider == ClaudeExecutionProvider::Cli {
        match child.stdin.take() {
            Some(mut stdin) => {
                if let Err(error) = stdin.write_all(prompt.as_bytes()).await {
                    let message = format!("写入 Claude CLI 提示词失败: {error}");
                    let _ = child.kill().await;
                    cleanup_process_artifacts(&cleanup_paths);
                    finalize_claude_launch_failure(
                        &app,
                        &pool,
                        &session_record.id,
                        task_git_context_id.as_deref(),
                        git_context_marked_running,
                        "spawn_failed",
                        &message,
                    )
                    .await;
                    return Err(message);
                }
                if let Err(error) = stdin.shutdown().await {
                    let message = format!("关闭 Claude CLI stdin 失败: {error}");
                    let _ = child.kill().await;
                    cleanup_process_artifacts(&cleanup_paths);
                    finalize_claude_launch_failure(
                        &app,
                        &pool,
                        &session_record.id,
                        task_git_context_id.as_deref(),
                        git_context_marked_running,
                        "spawn_failed",
                        &message,
                    )
                    .await;
                    return Err(message);
                }
            }
            None => {
                let message = "Claude CLI stdin 不可用，无法发送提示词".to_string();
                let _ = child.kill().await;
                cleanup_process_artifacts(&cleanup_paths);
                finalize_claude_launch_failure(
                    &app,
                    &pool,
                    &session_record.id,
                    task_git_context_id.as_deref(),
                    git_context_marked_running,
                    "spawn_failed",
                    &message,
                )
                .await;
                return Err(message);
            }
        }
    }

    if provider == ClaudeExecutionProvider::Sdk {
        let payload = match serde_json::to_vec(&serde_json::json!({
            "mode": "session",
            "prompt": prompt.clone(),
            "imagePaths": image_paths.clone(),
            "model": model.clone(),
            "effort": requested_effort,
            "thinkingBudgetTokens": thinking_budget,
            "workingDirectory": run_cwd.clone(),
            "resumeSessionId": resume_session_id.clone(),
            "claudePathOverride": claude_settings.cli_path_override.clone(),
        })) {
            Ok(payload) => payload,
            Err(error) => {
                let message = format!("序列化 Claude SDK 会话参数失败: {error}");
                let _ = child.kill().await;
                cleanup_process_artifacts(&cleanup_paths);
                finalize_claude_launch_failure(
                    &app,
                    &pool,
                    &session_record.id,
                    task_git_context_id.as_deref(),
                    git_context_marked_running,
                    "spawn_failed",
                    &message,
                )
                .await;
                return Err(message);
            }
        };

        if let Some(mut stdin) = child.stdin.take() {
            if let Err(error) = stdin.write_all(&payload).await {
                let message = format!("写入 Claude SDK 会话参数失败: {error}");
                let _ = child.kill().await;
                cleanup_process_artifacts(&cleanup_paths);
                finalize_claude_launch_failure(
                    &app,
                    &pool,
                    &session_record.id,
                    task_git_context_id.as_deref(),
                    git_context_marked_running,
                    "spawn_failed",
                    &message,
                )
                .await;
                return Err(message);
            }
            if let Err(error) = stdin.shutdown().await {
                let message = format!("关闭 Claude SDK stdin 失败: {error}");
                let _ = child.kill().await;
                cleanup_process_artifacts(&cleanup_paths);
                finalize_claude_launch_failure(
                    &app,
                    &pool,
                    &session_record.id,
                    task_git_context_id.as_deref(),
                    git_context_marked_running,
                    "spawn_failed",
                    &message,
                )
                .await;
                return Err(message);
            }
        }
    }

    if let Err(error) =
        update_codex_session_record(&app, &session_record.id, Some("running"), None, None, None)
            .await
    {
        let _ = child.kill().await;
        cleanup_process_artifacts(&cleanup_paths);
        finalize_claude_launch_failure(
            &app,
            &pool,
            &session_record.id,
            task_git_context_id.as_deref(),
            git_context_marked_running,
            "launch_failed",
            &error,
        )
        .await;
        return Err(error);
    }

    let target_label = if execution_context.execution_target == EXECUTION_TARGET_SSH {
        format!(
            " [SSH:{}]",
            execution_context
                .target_host_label
                .as_deref()
                .unwrap_or("?")
        )
    } else {
        String::new()
    };

    emit_session_terminal_line(
        &app,
        &pool,
        &session_record.id,
        &employee_id,
        task_id.as_deref(),
        session_kind,
        format!(
            "[Claude] 通过 {} 启动会话{target_label} model={model} effort={effort}",
            provider.label(),
        ),
    )
    .await;
    emit_session_terminal_line(
        &app,
        &pool,
        &session_record.id,
        &employee_id,
        task_id.as_deref(),
        session_kind,
        format_claude_session_prompt_log(
            provider,
            &model,
            effort,
            &execution_context.execution_target,
            execution_context.target_host_label.as_deref(),
            &run_cwd,
            &prompt,
            if provider == ClaudeExecutionProvider::Sdk {
                &image_paths
            } else {
                &[]
            },
        ),
    )
    .await;

    let sdk_file_change_store: SdkFileChangeStore = Arc::new(Mutex::new(HashMap::new()));

    let claude_child = ClaudeChild::new(child);
    let child_arc = Arc::new(tokio::sync::Mutex::new(claude_child));

    {
        let mut manager = manager_state.lock().await;
        manager.add_process(
            employee_id.clone(),
            task_id.clone(),
            session_kind,
            child_arc.clone(),
            session_record.id.clone(),
            cleanup_paths,
        );
    }

    if let Ok(pool) = sqlite_pool(&app).await {
        write_claude_task_session_activity(
            &app,
            &pool,
            &session_record.id,
            &employee_id,
            task_id.as_deref(),
            session_kind,
            resume_session_id.as_deref(),
            &execution_context.execution_target,
        )
        .await;
    }

    spawn_claude_session_runtime(
        app,
        manager_state,
        child_arc,
        session_record.id,
        employee_id,
        task_id,
        task_git_context_id,
        session_kind,
        provider,
        execution_change_baseline,
        sdk_file_change_store,
        run_cwd,
    );

    Ok(())
}

pub async fn list_live_claude_employee_processes(
    manager_state: &Arc<tokio::sync::Mutex<ClaudeManager>>,
    employee_id: &str,
) -> Vec<crate::claude::manager::ManagedClaudeProcess> {
    let manager = manager_state.lock().await;
    manager.get_employee_processes(employee_id)
}

async fn wait_until_claude_process_stops(
    manager_state: &Arc<tokio::sync::Mutex<ClaudeManager>>,
    session_record_id: &str,
) {
    for _ in 0..STOP_WAIT_MAX_ATTEMPTS {
        let is_running = {
            let manager = manager_state.lock().await;
            manager.get_process(session_record_id).is_some()
        };
        if !is_running {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(STOP_WAIT_POLL_MS)).await;
    }

    let stale_process = {
        let mut manager = manager_state.lock().await;
        manager.remove_process(session_record_id)
    };
    if let Some(process) = stale_process {
        cleanup_process_artifacts(&process.cleanup_paths);
    }
}

async fn stop_claude_process_with_manager<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<tokio::sync::Mutex<ClaudeManager>>,
    session_record_id: &str,
    event_type: &str,
    message: &str,
) -> Result<bool, String> {
    let process = {
        let manager = manager_state.lock().await;
        manager.get_process(session_record_id)
    };

    let Some(process) = process else {
        return Ok(false);
    };

    let pool = sqlite_pool(app).await?;
    update_codex_session_record(app, session_record_id, Some("stopping"), None, None, None).await?;
    insert_codex_session_event(&pool, session_record_id, event_type, Some(message)).await?;
    emit_session_terminal_line(
        app,
        &pool,
        session_record_id,
        &process.employee_id,
        process.task_id.as_deref(),
        process.session_kind,
        format!("[Claude] {message}"),
    )
    .await;

    let mut child = process.child.lock().await;
    if let Err(error) = child.kill_process_group() {
        eprintln!("[claude-stop] killpg failed, fallback to child.kill(): {error}");
    }
    child.kill().await?;
    drop(child);
    wait_until_claude_process_stops(manager_state, session_record_id).await;

    Ok(true)
}

pub(crate) async fn stop_claude_for_automation_restart<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: &str,
    expected_session_record_id: Option<&str>,
    message: &str,
) -> Result<bool, String> {
    let manager_state = app
        .state::<Arc<tokio::sync::Mutex<ClaudeManager>>>()
        .inner()
        .clone();
    let Some(expected_session_record_id) = expected_session_record_id else {
        return Err("当前自动化步骤缺少会话标识，无法安全重启".to_string());
    };

    let running_process = {
        let manager = manager_state.lock().await;
        manager.get_process(expected_session_record_id)
    };

    let Some(process) = running_process else {
        return Ok(false);
    };

    if process.employee_id != employee_id {
        return Err("当前员工正在执行其他任务，无法重启这条自动化步骤".to_string());
    }

    stop_claude_process_with_manager(
        app,
        &manager_state,
        expected_session_record_id,
        "automation_restart_requested",
        message,
    )
    .await
}

#[tauri::command]
pub async fn stop_claude_session(
    app: AppHandle,
    state: State<'_, Arc<tokio::sync::Mutex<ClaudeManager>>>,
    session_record_id: String,
) -> Result<(), String> {
    if !stop_claude_process_with_manager(
        &app,
        state.inner(),
        &session_record_id,
        "stopping_requested",
        "收到停止请求",
    )
    .await?
    {
        return Err(format!("未找到 Claude 会话 {session_record_id}"));
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_claude(
    app: AppHandle,
    state: State<'_, Arc<tokio::sync::Mutex<ClaudeManager>>>,
    employee_id: String,
) -> Result<(), String> {
    let processes = {
        let manager = state.lock().await;
        manager.get_employee_processes(&employee_id)
    };

    for process in processes {
        stop_claude_process_with_manager(
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
