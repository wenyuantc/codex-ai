use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::io::AsyncBufReadExt;
use tokio::sync::Mutex;

use crate::app::{
    fetch_codex_session_by_id, insert_activity_log, insert_codex_session_event,
    insert_codex_session_event_with_id, insert_codex_session_record, now_sqlite, sqlite_pool,
    update_codex_session_record, validate_runtime_working_dir, EXECUTION_TARGET_LOCAL,
    EXECUTION_TARGET_SSH,
};
use crate::codex::{CodexExecutionProvider, CodexSessionKind, ExecutionChangeBaseline};
use crate::db::models::OpenCodeOutput;
use crate::git_workflow::{
    mark_task_git_context_running, mark_task_git_context_session_finished,
    validate_task_git_context_launch,
};
use crate::opencode::{
    ensure_opencode_sdk_runtime_layout, load_opencode_settings, sdk_bridge_script_path,
    OpenCodeManager,
};
use crate::task_automation;

mod context;
mod lifecycle;
mod session_runtime;
pub(crate) mod stream;

pub use self::lifecycle::OpenCodeChild;

use self::{context::*, session_runtime::*, stream::*};

const REVIEW_VERDICT_START_TAG: &str = "<review_verdict>";
const REVIEW_VERDICT_END_TAG: &str = "</review_verdict>";
const REVIEW_REPORT_START_TAG: &str = "<review_report>";
const REVIEW_REPORT_END_TAG: &str = "</review_report>";
const STOP_WAIT_POLL_MS: u64 = 50;
const STOP_WAIT_MAX_ATTEMPTS: usize = 600;
const OPENCODE_PROVIDER_CHUNK_TIMEOUT_MS: i64 = 30 * 60 * 1000;

pub use super::stream::SdkFileChangeStore;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenCodeSessionKind {
    Execution,
}

impl OpenCodeSessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            OpenCodeSessionKind::Execution => "execution",
        }
    }

    fn activity_start_action(self, resumed: bool) -> &'static str {
        match self {
            OpenCodeSessionKind::Execution => {
                if resumed {
                    "task_execution_resumed"
                } else {
                    "task_execution_started"
                }
            }
        }
    }
}

fn cleanup_process_artifacts(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

async fn write_opencode_sdk_server_activity<R: Runtime>(
    app: &AppHandle<R>,
    action: &str,
    details: &str,
) {
    match sqlite_pool(app).await {
        Ok(pool) => {
            let _ = insert_activity_log(&pool, action, details, None, None, None).await;
        }
        Err(error) => {
            eprintln!("[opencode-sdk-server] 活动日志写入跳过: {error}");
        }
    }
}

async fn stop_existing_opencode_sdk_server(
    manager_state: &Arc<Mutex<OpenCodeManager>>,
) -> Result<(), String> {
    let existing = {
        let mut manager = manager_state.lock().await;
        manager.remove_sdk_server()
    };

    if let Some(server) = existing {
        let mut child = server.child.lock().await;
        if child.try_wait()?.is_none() {
            if let Err(error) = child.kill_process_group().await {
                eprintln!("[opencode-sdk-server] killpg failed, fallback to child.kill(): {error}");
            }
            let _ = child.kill().await;
        }
    }

    Ok(())
}

async fn ensure_opencode_sdk_server_started(
    app: AppHandle,
    manager_state: Arc<Mutex<OpenCodeManager>>,
) -> Result<(), String> {
    let opencode_settings = load_opencode_settings(&app)?;
    if !opencode_settings.sdk_enabled {
        return Ok(());
    }

    let install_dir = PathBuf::from(&opencode_settings.sdk_install_dir);
    ensure_opencode_sdk_runtime_layout(&install_dir)?;
    let health = crate::opencode::inspect_opencode_sdk_runtime(&app, &opencode_settings).await;
    if health.effective_provider != "sdk" {
        let message = format!("OpenCode SDK 已启用但未就绪：{}", health.sdk_status_message);
        write_opencode_sdk_server_activity(&app, "opencode_sdk_server_start_failed", &message)
            .await;
        return Err(message);
    }

    let existing = {
        let manager = manager_state.lock().await;
        manager.get_sdk_server()
    };
    if let Some(server) = existing {
        let is_running = {
            let mut child = server.child.lock().await;
            child.try_wait()?.is_none()
        };
        if is_running
            && server.host == opencode_settings.host
            && server.port == opencode_settings.port
        {
            return Ok(());
        }
        stop_existing_opencode_sdk_server(&manager_state).await?;
    }

    let bridge_path = sdk_bridge_script_path(&install_dir);
    let server_config = OpenCodeServerBridgeConfig {
        host: opencode_settings.host.clone(),
        port: opencode_settings.port,
        parent_pid: std::process::id(),
        node_path_override: opencode_settings.node_path_override.clone(),
        install_dir,
    };
    let child = launch_opencode_server_bridge(&server_config, &bridge_path).await?;
    let child = Arc::new(Mutex::new(child));

    {
        let mut manager = manager_state.lock().await;
        manager.set_sdk_server(
            opencode_settings.host.clone(),
            opencode_settings.port,
            child.clone(),
        );
    }

    stream_opencode_sdk_server_output(
        app,
        manager_state,
        child,
        opencode_settings.host,
        opencode_settings.port,
    );

    Ok(())
}

pub fn spawn_opencode_sdk_server_on_startup(
    app: AppHandle,
    manager_state: Arc<Mutex<OpenCodeManager>>,
) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = ensure_opencode_sdk_server_started(app.clone(), manager_state).await {
            eprintln!("[opencode-sdk-server] 启动失败: {error}");
        }
    });
}

fn stream_opencode_sdk_server_output(
    app: AppHandle,
    manager_state: Arc<Mutex<OpenCodeManager>>,
    child: Arc<Mutex<OpenCodeChild>>,
    host: String,
    port: u16,
) {
    tauri::async_runtime::spawn(async move {
        let (stdout, stderr) = {
            let mut child = child.lock().await;
            (child.stdout(), child.stderr())
        };
        let mut ready_logged = false;
        let mut bridge_error: Option<String> = None;

        if let Some(stderr) = stderr {
            tauri::async_runtime::spawn(async move {
                let mut reader = tokio::io::BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let trimmed = line.trim_end();
                            if !trimmed.is_empty() {
                                eprintln!("[opencode-sdk-server] {trimmed}");
                            }
                        }
                        Err(error) => {
                            eprintln!("[opencode-sdk-server] stderr 读取失败: {error}");
                            break;
                        }
                    }
                }
            });
        }

        if let Some(stdout) = stdout {
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim_end();
                        if trimmed.is_empty() {
                            continue;
                        }

                        if let Some(event) = parse_opencode_bridge_event(trimmed) {
                            match event.event_type.as_str() {
                                "info" => {
                                    if let Some(message) = bridge_event_message(&event) {
                                        eprintln!("[opencode-sdk-server] {message}");
                                    }
                                }
                                "server-ready" => {
                                    if !ready_logged {
                                        ready_logged = true;
                                        let url = bridge_event_string(&event, "url")
                                            .unwrap_or_else(|| format!("http://{host}:{port}"));
                                        write_opencode_sdk_server_activity(
                                            &app,
                                            "opencode_sdk_server_started",
                                            &format!("OpenCode SDK server 已启动：{url}"),
                                        )
                                        .await;
                                    }
                                }
                                "error" => {
                                    let message =
                                        bridge_event_message(&event).unwrap_or_else(|| {
                                            "OpenCode SDK server 启动失败".to_string()
                                        });
                                    bridge_error = Some(message.clone());
                                    write_opencode_sdk_server_activity(
                                        &app,
                                        "opencode_sdk_server_start_failed",
                                        &message,
                                    )
                                    .await;
                                    eprintln!("[opencode-sdk-server] {message}");
                                }
                                "done" => break,
                                _ => eprintln!("[opencode-sdk-server] {trimmed}"),
                            }
                        } else {
                            eprintln!("[opencode-sdk-server] {trimmed}");
                        }
                    }
                    Err(error) => {
                        let message = format!("OpenCode SDK server 输出流读取失败: {error}");
                        bridge_error = Some(message.clone());
                        write_opencode_sdk_server_activity(
                            &app,
                            "opencode_sdk_server_start_failed",
                            &message,
                        )
                        .await;
                        break;
                    }
                }
            }
        }

        {
            let mut child = child.lock().await;
            let _ = wait_for_exit(&mut child).await;
        }

        {
            let mut manager = manager_state.lock().await;
            manager.remove_sdk_server_if_child(&child);
        }

        if !ready_logged && bridge_error.is_none() {
            write_opencode_sdk_server_activity(
                &app,
                "opencode_sdk_server_start_failed",
                "OpenCode SDK server 进程未完成启动即退出",
            )
            .await;
        }
    });
}

pub(crate) fn extract_review_report(raw: &str) -> Option<String> {
    let start = raw.find(REVIEW_REPORT_START_TAG)?;
    let end = raw.find(REVIEW_REPORT_END_TAG)?;
    let content_start = start + REVIEW_REPORT_START_TAG.len();
    Some(raw[content_start..end].trim().to_string())
}

pub(crate) fn extract_review_verdict(raw: &str) -> Option<String> {
    let start = raw.find(REVIEW_VERDICT_START_TAG)?;
    let end = raw.find(REVIEW_VERDICT_END_TAG)?;
    let content_start = start + REVIEW_VERDICT_START_TAG.len();
    Some(raw[content_start..end].trim().to_string())
}

async fn emit_session_terminal_line<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: OpenCodeSessionKind,
    line: String,
) {
    let event_id =
        insert_codex_session_event_with_id(pool, session_record_id, "stdout", Some(&line))
            .await
            .ok();

    let _ = app.emit(
        "opencode-stdout",
        OpenCodeOutput {
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
            "Failed to resolve task {} for OpenCode activity log: {}",
            task_id, error
        )
    })
}

async fn write_opencode_task_session_activity<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: OpenCodeSessionKind,
    resume_session_id: Option<&str>,
    execution_target: &str,
) {
    let Some(task_id) = task_id else {
        return;
    };

    let result = async {
        let (task_title, project_id) = fetch_task_activity_context(pool, task_id).await?;
        let action = if execution_target == EXECUTION_TARGET_SSH
            && session_kind == OpenCodeSessionKind::Execution
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
            format!("[WARN] OpenCode 活动日志写入失败: {error}"),
        )
        .await;
    }
}

async fn ensure_no_cross_provider_conflict<R: Runtime>(
    app: &AppHandle<R>,
    _employee_id: &str,
    task_id: Option<&str>,
    _session_kind: OpenCodeSessionKind,
) -> Result<(), String> {
    if let Some(codex_state) = app.try_state::<Arc<std::sync::Mutex<crate::codex::CodexManager>>>()
    {
        if let Some(task_id) = task_id {
            if crate::codex::get_live_task_process_by_task(
                app,
                codex_state.inner(),
                task_id,
                crate::codex::CodexSessionKind::Execution,
            )
            .await?
            .is_some()
            {
                return Err(format!("任务{}的 execution 会话已在 Codex 中运行", task_id));
            }
        }
    }
    if let Some(claude_state) =
        app.try_state::<Arc<tokio::sync::Mutex<crate::claude::ClaudeManager>>>()
    {
        if let Some(task_id) = task_id {
            let manager = claude_state.lock().await;
            if manager
                .get_task_process_any(
                    task_id,
                    crate::claude::process::ClaudeSessionKind::Execution,
                )
                .is_some()
            {
                return Err(format!(
                    "任务{}的 execution 会话已在 Claude 中运行",
                    task_id
                ));
            }
        }
    }

    Ok(())
}

async fn finalize_launch_failure<R: Runtime>(
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

async fn capture_execution_change_baseline<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: OpenCodeSessionKind,
    execution_target: &str,
    run_cwd: &str,
    ssh_config: Option<&crate::db::models::SshConfigRecord>,
) -> Option<crate::codex::ExecutionChangeBaseline> {
    if session_kind != OpenCodeSessionKind::Execution {
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
            "[SSH] 正在采集远程仓库基线，用于展示本次 OpenCode 会话改动...".to_string(),
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
                    "[WARN] OpenCode 会话文件基线采集失败，文件详情将退化为最佳努力快照: {error}"
                ),
            )
            .await;
            None
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct OpenCodeBridgeEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    data: serde_json::Value,
}

fn parse_opencode_bridge_event(line: &str) -> Option<OpenCodeBridgeEvent> {
    serde_json::from_str::<OpenCodeBridgeEvent>(line)
        .ok()
        .filter(|event| !event.event_type.trim().is_empty())
}

fn bridge_event_string(event: &OpenCodeBridgeEvent, key: &str) -> Option<String> {
    event
        .data
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn bridge_event_message(event: &OpenCodeBridgeEvent) -> Option<String> {
    bridge_event_string(event, "message")
}

fn bridge_event_line(event: &OpenCodeBridgeEvent) -> Option<String> {
    bridge_event_string(event, "line")
}

fn bridge_event_session_id(event: &OpenCodeBridgeEvent) -> Option<String> {
    bridge_event_string(event, "session_id").or_else(|| bridge_event_string(event, "id"))
}

fn opencode_session_kind_to_codex(_session_kind: OpenCodeSessionKind) -> CodexSessionKind {
    CodexSessionKind::Execution
}

fn ensure_json_object(
    value: &mut serde_json::Value,
) -> &mut serde_json::Map<String, serde_json::Value> {
    if !value.is_object() {
        *value = serde_json::json!({});
    }
    value.as_object_mut().expect("json value should be object")
}

fn ensure_json_object_field<'a>(
    parent: &'a mut serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> &'a mut serde_json::Map<String, serde_json::Value> {
    let value = parent
        .entry(key.to_string())
        .or_insert_with(|| serde_json::json!({}));
    ensure_json_object(value)
}

fn apply_opencode_runtime_config(
    config: &mut serde_json::Value,
    provider_id: &str,
    model_id: &str,
    reasoning_effort: Option<&str>,
) {
    let root = ensure_json_object(config);
    let provider_map = ensure_json_object_field(root, "provider");
    let provider_config = ensure_json_object_field(provider_map, provider_id);
    let provider_options = ensure_json_object_field(provider_config, "options");
    provider_options.insert("timeout".to_string(), serde_json::Value::Bool(false));
    provider_options.insert(
        "chunkTimeout".to_string(),
        serde_json::Value::Number(OPENCODE_PROVIDER_CHUNK_TIMEOUT_MS.into()),
    );

    let models = ensure_json_object_field(provider_config, "models");
    let model_config = ensure_json_object_field(models, model_id);
    let model_options = ensure_json_object_field(model_config, "options");
    model_options.insert("timeout".to_string(), serde_json::Value::Bool(false));
    model_options.insert(
        "chunkTimeout".to_string(),
        serde_json::Value::Number(OPENCODE_PROVIDER_CHUNK_TIMEOUT_MS.into()),
    );

    if let Some(effort) = reasoning_effort {
        model_options.insert(
            "reasoning_effort".to_string(),
            serde_json::Value::String(effort.to_string()),
        );
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OpenCodeRuntimeConfigBackup {
    path: PathBuf,
    original_content: Option<String>,
}

impl OpenCodeRuntimeConfigBackup {
    pub(crate) fn restore(&self) -> Result<(), String> {
        match &self.original_content {
            Some(content) => fs::write(&self.path, content).map_err(|error| {
                format!(
                    "OpenCode 配置文件 {} 恢复失败: {error}",
                    self.path.display()
                )
            }),
            None => match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(format!(
                    "OpenCode 临时配置文件 {} 清理失败: {error}",
                    self.path.display()
                )),
            },
        }
    }
}

fn git_command_status(run_cwd: &str, args: &[&str]) -> Option<std::process::ExitStatus> {
    std::process::Command::new("git")
        .arg("-C")
        .arg(run_cwd)
        .args(args)
        .status()
        .ok()
}

fn git_command_output(run_cwd: &str, args: &[&str]) -> Option<std::process::Output> {
    std::process::Command::new("git")
        .arg("-C")
        .arg(run_cwd)
        .args(args)
        .output()
        .ok()
}

fn ensure_untracked_opencode_config_is_excluded(run_cwd: &str, config_path: &Path) {
    let Some(file_name) = config_path.file_name().and_then(|value| value.to_str()) else {
        return;
    };

    if git_command_status(run_cwd, &["ls-files", "--error-unmatch", file_name])
        .map(|status| status.success())
        .unwrap_or(false)
    {
        return;
    }

    let Some(output) = git_command_output(run_cwd, &["rev-parse", "--git-path", "info/exclude"])
    else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let exclude_path_text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if exclude_path_text.is_empty() {
        return;
    }

    let exclude_path = PathBuf::from(exclude_path_text);
    let pattern = format!("/{file_name}");
    let existing = fs::read_to_string(&exclude_path).unwrap_or_default();
    if existing.lines().any(|line| line.trim() == pattern) {
        return;
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str("# codex-ai OpenCode runtime config\n");
    updated.push_str(&pattern);
    updated.push('\n');
    let _ = fs::write(exclude_path, updated);
}

pub(crate) fn write_opencode_runtime_config_file(
    run_cwd: &str,
    provider_id: &str,
    model_id: &str,
    reasoning_effort: Option<&str>,
) -> Result<OpenCodeRuntimeConfigBackup, String> {
    let config_path = Path::new(run_cwd).join("opencode.json");
    let original_content = match fs::read_to_string(&config_path) {
        Ok(content) => Some(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!(
                "OpenCode 配置文件 {} 读取失败: {error}",
                config_path.display()
            ));
        }
    };
    let mut config: serde_json::Value = match original_content.as_deref() {
        Some(content) => serde_json::from_str(content).map_err(|error| {
            format!(
                "OpenCode 配置文件 {} 解析失败，已停止启动以避免覆盖用户配置: {error}",
                config_path.display()
            )
        })?,
        None => serde_json::json!({}),
    };
    let backup = OpenCodeRuntimeConfigBackup {
        path: config_path.clone(),
        original_content,
    };

    apply_opencode_runtime_config(&mut config, provider_id, model_id, reasoning_effort);
    let json_str = serde_json::to_string_pretty(&config)
        .map_err(|error| format!("OpenCode 运行配置序列化失败: {error}"))?;
    fs::write(&config_path, json_str).map_err(|error| {
        format!(
            "OpenCode 配置文件 {} 写入失败: {error}",
            config_path.display()
        )
    })?;
    ensure_untracked_opencode_config_is_excluded(run_cwd, &config_path);
    Ok(backup)
}

fn resolve_final_opencode_status(
    current_status: Option<&str>,
    bridge_error: Option<&str>,
    exit_code: Option<i32>,
) -> (&'static str, i32, String) {
    let normalized_exit_code =
        exit_code.unwrap_or_else(|| if bridge_error.is_some() { 1 } else { 0 });

    if current_status == Some("stopping") {
        return (
            "exited",
            normalized_exit_code,
            format!("OpenCode 会话已停止，退出码: {normalized_exit_code}"),
        );
    }

    if let Some(error) = bridge_error {
        return (
            "failed",
            if normalized_exit_code == 0 {
                1
            } else {
                normalized_exit_code
            },
            format!("OpenCode 会话失败: {error}"),
        );
    }

    if normalized_exit_code == 0 {
        (
            "exited",
            normalized_exit_code,
            "OpenCode 会话已完成，退出码: 0".to_string(),
        )
    } else {
        (
            "failed",
            normalized_exit_code,
            format!("OpenCode 会话失败，退出码: {normalized_exit_code}"),
        )
    }
}

async fn stream_opencode_output(
    app: AppHandle,
    manager_state: Arc<Mutex<OpenCodeManager>>,
    pool: SqlitePool,
    session_record_id: String,
    employee_id: String,
    task_id: Option<String>,
    session_kind: OpenCodeSessionKind,
    child: Arc<Mutex<OpenCodeChild>>,
    file_change_store: Option<SdkFileChangeStore>,
    execution_change_baseline: Option<ExecutionChangeBaseline>,
    runtime_config_backup: Option<OpenCodeRuntimeConfigBackup>,
) {
    let stdout = child.lock().await.stdout();
    let mut bridge_error: Option<String> = None;
    let mut bridge_done = false;

    if let Some(stdout) = stdout {
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut line = String::new();
        let mut idle_timeout = tokio::time::Instant::now();

        loop {
            line.clear();

            if idle_timeout.elapsed() > tokio::time::Duration::from_secs(180) {
                let message = "OpenCode 输出空闲超时，自动结束会话。".to_string();
                bridge_error = Some(message.clone());
                emit_session_terminal_line(
                    &app,
                    &pool,
                    &session_record_id,
                    &employee_id,
                    task_id.as_deref(),
                    session_kind,
                    format!("[ERROR] {message}"),
                )
                .await;
                break;
            }

            let read_fut = reader.read_line(&mut line);
            let timed_out =
                tokio::time::timeout(tokio::time::Duration::from_secs(30), read_fut).await;

            match timed_out {
                Ok(Ok(0)) => break,
                Ok(Err(error)) => {
                    let message = format!("OpenCode 输出流读取失败: {error}");
                    bridge_error = Some(message.clone());
                    emit_session_terminal_line(
                        &app,
                        &pool,
                        &session_record_id,
                        &employee_id,
                        task_id.as_deref(),
                        session_kind,
                        format!("[ERROR] {message}"),
                    )
                    .await;
                    break;
                }
                Err(_) => {
                    continue;
                }
                Ok(Ok(_)) => {
                    let trimmed = line.trim_end().to_string();
                    if trimmed.is_empty() {
                        continue;
                    }
                    idle_timeout = tokio::time::Instant::now();

                    if let Some(event) = parse_opencode_sdk_file_change_event(&trimmed) {
                        if let Some(ref store) = file_change_store {
                            upsert_sdk_file_change_event(store, event);
                        }
                        continue;
                    }

                    if let Some(event) = parse_opencode_bridge_event(&trimmed) {
                        match event.event_type.as_str() {
                            "info" => {
                                if let Some(message) = bridge_event_message(&event) {
                                    emit_session_terminal_line(
                                        &app,
                                        &pool,
                                        &session_record_id,
                                        &employee_id,
                                        task_id.as_deref(),
                                        session_kind,
                                        format!("[OpenCode] {message}"),
                                    )
                                    .await;
                                }
                            }
                            "stdout" => {
                                if let Some(output_line) = bridge_event_line(&event) {
                                    emit_session_terminal_line(
                                        &app,
                                        &pool,
                                        &session_record_id,
                                        &employee_id,
                                        task_id.as_deref(),
                                        session_kind,
                                        output_line,
                                    )
                                    .await;
                                }
                            }
                            "session" => {
                                if let Some(session_id) = bridge_event_session_id(&event) {
                                    let _ = update_codex_session_record(
                                        &app,
                                        &session_record_id,
                                        None,
                                        Some(Some(session_id.as_str())),
                                        None,
                                        None,
                                    )
                                    .await;
                                    let _ = app.emit(
                                        "opencode-session",
                                        crate::db::models::OpenCodeSession {
                                            employee_id: employee_id.clone(),
                                            task_id: task_id.clone(),
                                            session_kind: session_kind.as_str().to_string(),
                                            session_record_id: session_record_id.clone(),
                                            session_id: session_id.clone(),
                                        },
                                    );
                                    emit_session_terminal_line(
                                        &app,
                                        &pool,
                                        &session_record_id,
                                        &employee_id,
                                        task_id.as_deref(),
                                        session_kind,
                                        format!("[OpenCode] 会话 ID: {session_id}"),
                                    )
                                    .await;
                                }
                            }
                            "error" => {
                                let message = bridge_event_message(&event)
                                    .unwrap_or_else(|| "OpenCode SDK 返回未知错误".to_string());
                                bridge_error = Some(message.clone());
                                emit_session_terminal_line(
                                    &app,
                                    &pool,
                                    &session_record_id,
                                    &employee_id,
                                    task_id.as_deref(),
                                    session_kind,
                                    format!("[ERROR] {message}"),
                                )
                                .await;
                            }
                            "done" => {
                                bridge_done = true;
                            }
                            _ => {
                                emit_session_terminal_line(
                                    &app,
                                    &pool,
                                    &session_record_id,
                                    &employee_id,
                                    task_id.as_deref(),
                                    session_kind,
                                    trimmed,
                                )
                                .await;
                            }
                        }
                        continue;
                    }

                    if let Some(session_id) = extract_session_id_from_opencode_output(&trimmed) {
                        let _ = update_codex_session_record(
                            &app,
                            &session_record_id,
                            None,
                            Some(Some(session_id.as_str())),
                            None,
                            None,
                        )
                        .await;
                        let _ = app.emit(
                            "opencode-session",
                            crate::db::models::OpenCodeSession {
                                employee_id: employee_id.clone(),
                                task_id: task_id.clone(),
                                session_kind: session_kind.as_str().to_string(),
                                session_record_id: session_record_id.clone(),
                                session_id,
                            },
                        );
                    }

                    emit_session_terminal_line(
                        &app,
                        &pool,
                        &session_record_id,
                        &employee_id,
                        task_id.as_deref(),
                        session_kind,
                        trimmed,
                    )
                    .await;
                }
            }
        }
    } else {
        bridge_error = Some("无法获取 OpenCode bridge stdout".to_string());
    }

    if !bridge_done && bridge_error.is_none() {
        bridge_error = Some("OpenCode bridge 未发送完成事件即退出".to_string());
    }

    let exit_status = {
        let mut child = child.lock().await;
        wait_for_exit(&mut child).await
    };
    let raw_exit_code = exit_status.and_then(|status| status.code());

    if let Some(backup) = runtime_config_backup {
        if let Err(error) = backup.restore() {
            let _ = insert_codex_session_event(
                &pool,
                &session_record_id,
                "opencode_runtime_config_restore_failed",
                Some(&error),
            )
            .await;
            emit_session_terminal_line(
                &app,
                &pool,
                &session_record_id,
                &employee_id,
                task_id.as_deref(),
                session_kind,
                format!("[WARN] {error}"),
            )
            .await;
        }
    }

    {
        let mut manager = manager_state.lock().await;
        if let Some(process) = manager.remove_process(&session_record_id) {
            cleanup_process_artifacts(&process.cleanup_paths);
        }
    }

    crate::codex::persist_external_execution_change_history(
        &app,
        &session_record_id,
        opencode_session_kind_to_codex(session_kind),
        CodexExecutionProvider::Sdk,
        execution_change_baseline.as_ref(),
        file_change_store.as_ref(),
    )
    .await;

    let current_status = fetch_codex_session_by_id(&app, &session_record_id)
        .await
        .ok()
        .map(|record| record.status);
    let (final_status, exit_code, message) = resolve_final_opencode_status(
        current_status.as_deref(),
        bridge_error.as_deref(),
        raw_exit_code,
    );
    let ended_at = now_sqlite();
    let _ = update_codex_session_record(
        &app,
        &session_record_id,
        Some(final_status),
        None,
        Some(Some(exit_code)),
        Some(Some(ended_at.as_str())),
    )
    .await;

    let session_event_id = insert_codex_session_event_with_id(
        &pool,
        &session_record_id,
        if final_status == "exited" {
            "session_exited"
        } else {
            "session_failed"
        },
        Some(&message),
    )
    .await
    .ok();

    if task_id.is_some() {
        if let Ok(Some(task_git_context_id)) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT task_git_context_id FROM codex_sessions WHERE id = $1",
        )
        .bind(&session_record_id)
        .fetch_one(&pool)
        .await
        {
            let success = final_status == "exited" && exit_code == 0;
            let failure_message = (!success).then(|| message.clone());
            let _ = mark_task_git_context_session_finished(
                &pool,
                &task_git_context_id,
                success,
                failure_message.as_deref(),
            )
            .await;
        }
    }

    task_automation::handle_session_exit_blocking(app.clone(), session_record_id.clone()).await;

    let _ = app.emit(
        "opencode-exit",
        crate::db::models::OpenCodeExit {
            employee_id: employee_id.clone(),
            task_id: task_id.clone(),
            session_kind: session_kind.as_str().to_string(),
            session_record_id: session_record_id.clone(),
            session_event_id,
            status: final_status.to_string(),
            line: Some(if final_status == "exited" {
                format!("[EXIT] {message}")
            } else {
                format!("[ERROR] {message}")
            }),
            code: Some(exit_code),
        },
    );
}

async fn wait_until_opencode_process_stops(
    manager_state: &Arc<Mutex<OpenCodeManager>>,
    session_record_id: &str,
) -> bool {
    for _ in 0..STOP_WAIT_MAX_ATTEMPTS {
        let is_running = {
            let manager = manager_state.lock().await;
            manager.get_process(session_record_id).is_some()
        };
        if !is_running {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(STOP_WAIT_POLL_MS)).await;
    }

    false
}

async fn stop_opencode_process_with_manager<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<Mutex<OpenCodeManager>>,
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

    let pool = sqlite_pool(app).await.map_err(|error| error.to_string())?;
    update_codex_session_record(app, session_record_id, Some("stopping"), None, None, None).await?;
    insert_codex_session_event(&pool, session_record_id, event_type, Some(message)).await?;
    emit_session_terminal_line(
        app,
        &pool,
        session_record_id,
        &process.employee_id,
        process.task_id.as_deref(),
        process.session_kind,
        format!("[OpenCode] {message}"),
    )
    .await;

    let mut child = process.child.lock().await;
    if let Err(error) = child.kill_process_group().await {
        eprintln!("[opencode-stop] killpg failed, fallback to child.kill(): {error}");
    }
    child.kill().await?;
    drop(child);
    if !wait_until_opencode_process_stops(manager_state, session_record_id).await {
        let has_exited = {
            let mut child = process.child.lock().await;
            child.try_wait()?.is_some()
        };
        if has_exited {
            let removed_process = {
                let mut manager = manager_state.lock().await;
                manager.remove_process(session_record_id)
            };
            if let Some(process) = removed_process {
                cleanup_process_artifacts(&process.cleanup_paths);
            }
        } else {
            return Err(format!(
                "OpenCode 会话 {session_record_id} 停止超时，进程仍在清理中"
            ));
        }
    }

    Ok(true)
}

#[tauri::command]
pub async fn start_opencode(
    app: AppHandle,
    state: State<'_, Arc<Mutex<OpenCodeManager>>>,
    employee_id: String,
    task_description: String,
    model: Option<String>,
    working_dir: Option<String>,
    task_id: Option<String>,
    task_git_context_id: Option<String>,
    resume_session_id: Option<String>,
    image_paths: Option<Vec<String>>,
) -> Result<(), String> {
    start_opencode_with_manager(
        app,
        state.inner().clone(),
        employee_id,
        task_description,
        model,
        working_dir,
        task_id,
        task_git_context_id,
        resume_session_id,
        image_paths,
    )
    .await
}

pub async fn start_opencode_with_manager(
    app: AppHandle,
    manager_state: Arc<Mutex<OpenCodeManager>>,
    employee_id: String,
    task_description: String,
    model: Option<String>,
    working_dir: Option<String>,
    task_id: Option<String>,
    task_git_context_id: Option<String>,
    resume_session_id: Option<String>,
    image_paths: Option<Vec<String>>,
) -> Result<(), String> {
    let session_kind = OpenCodeSessionKind::Execution;

    {
        let manager = manager_state.lock().await;
        if let Some(ref task_id) = task_id {
            if manager
                .get_task_process_any(task_id, session_kind)
                .is_some()
            {
                return Err(format!(
                    "任务{}的 execution 会话已在 OpenCode 中运行",
                    task_id
                ));
            }
        } else if manager.has_unbound_employee_process(&employee_id, session_kind) {
            return Err(format!(
                "员工{}已有未绑定任务的 OpenCode 会话在运行",
                employee_id
            ));
        }
    }
    ensure_no_cross_provider_conflict(&app, &employee_id, task_id.as_deref(), session_kind).await?;

    let execution_context =
        resolve_opencode_session_context(&app, task_id.as_deref(), working_dir.as_deref()).await?;

    let run_cwd = if execution_context.execution_target == EXECUTION_TARGET_LOCAL {
        match validate_runtime_working_dir(execution_context.working_dir.as_deref()) {
            Ok(path) => path,
            Err(error) => return Err(error),
        }
    } else {
        execution_context
            .working_dir
            .clone()
            .ok_or_else(|| "SSH 项目缺少远程仓库目录，无法启动 OpenCode。".to_string())?
    };

    if let (Some(ref task_id), Some(ref task_git_context_id)) =
        (task_id.as_ref(), task_git_context_id.as_ref())
    {
        let validated_worktree =
            validate_task_git_context_launch(&app, task_id, task_git_context_id, Some(&run_cwd))
                .await?;
        if run_cwd != validated_worktree {
            return Err("task git context 与 working_dir 不一致".to_string());
        }
    }

    let pool = sqlite_pool(&app).await?;

    let opencode_settings = load_opencode_settings(&app)?;
    let model = model.unwrap_or_else(|| opencode_settings.default_model.clone());
    let prompt = task_description;

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
        Some("opencode"),
        None,
    )
    .await?;

    let mut git_context_marked_running = false;
    if let Some(ref task_git_context_id) = task_git_context_id {
        if let Err(error) = mark_task_git_context_running(&pool, task_git_context_id).await {
            finalize_launch_failure(
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

    if execution_context.execution_target == EXECUTION_TARGET_SSH {
        emit_session_terminal_line(
            &app,
            &pool,
            &session_record.id,
            &employee_id,
            task_id.as_deref(),
            session_kind,
            format!(
                "[SSH] 正在准备远程 OpenCode 会话，目标 {}，工作目录 {}",
                execution_context
                    .target_host_label
                    .as_deref()
                    .unwrap_or("未知登录目标"),
                run_cwd
            ),
        )
        .await;

        let _ = insert_codex_session_event(
            &pool,
            &session_record.id,
            "session_requested",
            Some("SSH 远程 OpenCode 会话暂不支持 SDK bridge 模式，请使用本地模式"),
        )
        .await;

        finalize_launch_failure(
            &app,
            &pool,
            &session_record.id,
            task_git_context_id.as_deref(),
            git_context_marked_running,
            "launch_failed",
            "SSH 远程 OpenCode 会话暂不支持",
        )
        .await;
        return Err("OpenCode SDK bridge 远程模式尚未实现，请先在本地项目中使用。".to_string());
    }

    let install_dir = PathBuf::from(&opencode_settings.sdk_install_dir);
    let bridge_path = sdk_bridge_script_path(&install_dir);

    if let Err(error) = ensure_opencode_sdk_runtime_layout(&install_dir) {
        finalize_launch_failure(
            &app,
            &pool,
            &session_record.id,
            task_git_context_id.as_deref(),
            git_context_marked_running,
            "sdk_runtime_setup_failed",
            &error,
        )
        .await;
        return Err(error);
    }

    let (image_paths_resolved, missing_image_paths, _ignored_remote) =
        match crate::codex::process::prepare_execution_image_paths(
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
                finalize_launch_failure(
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
            format!("[WARN] OpenCode 附件图片不存在，已跳过: {missing_path}"),
        )
        .await;
    }

    // Query reasoning_effort from employee settings
    let reasoning_effort =
        sqlx::query_scalar::<_, String>("SELECT reasoning_effort FROM employees WHERE id = $1")
            .bind(&employee_id)
            .fetch_optional(&pool)
            .await
            .ok()
            .flatten();

    // Emit prompt log (same style as Codex)
    let reasoning_label = reasoning_effort
        .as_deref()
        .filter(|v| !v.is_empty() && *v != "default")
        .unwrap_or("default");
    let prompt_log = format!(
        "[PROMPT] 即将发送给 OpenCode 的完整提示词\n\
运行通道: SDK\n\
模型: {model}\n\
推理强度: {reasoning_label}\n\
执行环境: 本地运行\n\
工作目录: {run_cwd}\n\
附带图片: {} 张\n\n{prompt}",
        image_paths_resolved.len(),
    );
    emit_session_terminal_line(
        &app,
        &pool,
        &session_record.id,
        &employee_id,
        task_id.as_deref(),
        session_kind,
        prompt_log,
    )
    .await;

    let execution_change_baseline = capture_execution_change_baseline(
        &app,
        &pool,
        &session_record.id,
        &employee_id,
        task_id.as_deref(),
        session_kind,
        &execution_context.execution_target,
        &run_cwd,
        None,
    )
    .await;

    // Keep provider-side long-running requests alive; OpenCode defaults to 5 minutes.
    let effort_to_write = reasoning_effort.as_deref().and_then(|v| {
        if v != "default" && !v.trim().is_empty() {
            Some(v.to_string())
        } else {
            None
        }
    });

    let provider_id = model
        .split_once('/')
        .map(|(p, _)| p)
        .unwrap_or("opencode-go");
    let model_id = model.split_once('/').map(|(_, m)| m).unwrap_or(&model);
    let runtime_config_backup = match write_opencode_runtime_config_file(
        &run_cwd,
        provider_id,
        model_id,
        effort_to_write.as_deref(),
    ) {
        Ok(backup) => backup,
        Err(error) => {
            finalize_launch_failure(
                &app,
                &pool,
                &session_record.id,
                task_git_context_id.as_deref(),
                git_context_marked_running,
                "opencode_runtime_config_failed",
                &error,
            )
            .await;
            return Err(error);
        }
    };

    emit_session_terminal_line(
        &app,
        &pool,
        &session_record.id,
        &employee_id,
        task_id.as_deref(),
        session_kind,
        "[SDK] 任务已提交，等待 OpenCode 响应...".to_string(),
    )
    .await;

    let bridge_config = OpenCodeBridgeConfig {
        mode: if resume_session_id.is_some() {
            "resume_session".to_string()
        } else {
            "session".to_string()
        },
        model: model.clone(),
        reasoning_effort: effort_to_write.clone(),
        host: opencode_settings.host.clone(),
        port: opencode_settings.port,
        node_path_override: opencode_settings.node_path_override.clone(),
        working_directory: run_cwd.clone(),
        prompt: prompt.clone(),
        system_prompt: None,
        resume_session_id,
        image_paths: image_paths_resolved,
        install_dir: install_dir.clone(),
    };

    let child = match launch_opencode_bridge(&bridge_config, &bridge_path).await {
        Ok(child) => child,
        Err(error) => {
            let _ = runtime_config_backup.restore();
            finalize_launch_failure(
                &app,
                &pool,
                &session_record.id,
                task_git_context_id.as_deref(),
                git_context_marked_running,
                "sdk_bridge_launch_failed",
                &error,
            )
            .await;
            return Err(error);
        }
    };
    if let Err(error) =
        update_codex_session_record(&app, &session_record.id, Some("running"), None, None, None)
            .await
    {
        let mut child = child;
        let _ = child.kill_process_group().await;
        let _ = child.kill().await;
        let _ = runtime_config_backup.restore();
        finalize_launch_failure(
            &app,
            &pool,
            &session_record.id,
            task_git_context_id.as_deref(),
            git_context_marked_running,
            "session_status_update_failed",
            &error,
        )
        .await;
        return Err(error);
    }
    let child = Arc::new(Mutex::new(child));
    let file_change_store: Option<SdkFileChangeStore> =
        Some(Arc::new(std::sync::Mutex::new(HashMap::new())));

    {
        let mut manager = manager_state.lock().await;
        manager.add_process(
            employee_id.clone(),
            task_id.clone(),
            session_kind,
            child.clone(),
            session_record.id.clone(),
            file_change_store.clone(),
            vec![],
        );
    }

    let _ = insert_codex_session_event(
        &pool,
        &session_record.id,
        "session_started",
        Some("OpenCode 会话已启动"),
    )
    .await;

    write_opencode_task_session_activity(
        &app,
        &pool,
        &session_record.id,
        &employee_id,
        task_id.as_deref(),
        session_kind,
        None,
        &execution_context.execution_target,
    )
    .await;

    let pool_clone = pool.clone();
    let app_clone = app.clone();
    let manager_state_clone = manager_state.clone();
    let session_record_id_clone = session_record.id.clone();
    let employee_id_clone = employee_id.clone();
    let task_id_clone = task_id.clone();
    let session_kind_clone = session_kind;
    let child_clone = child.clone();
    let file_change_store_clone = file_change_store.clone();
    let execution_change_baseline_clone = execution_change_baseline.clone();
    let runtime_config_backup_clone = Some(runtime_config_backup.clone());

    tauri::async_runtime::spawn(async move {
        stream_opencode_output(
            app_clone.clone(),
            manager_state_clone,
            pool_clone,
            session_record_id_clone,
            employee_id_clone,
            task_id_clone,
            session_kind_clone,
            child_clone,
            file_change_store_clone,
            execution_change_baseline_clone,
            runtime_config_backup_clone,
        )
        .await;
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_opencode_session(
    app: AppHandle,
    state: State<'_, Arc<Mutex<OpenCodeManager>>>,
    session_record_id: String,
) -> Result<(), String> {
    if stop_opencode_process_with_manager(
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
        Err(format!("未找到 OpenCode 会话 {session_record_id}"))
    }
}

#[tauri::command]
pub async fn stop_opencode(
    app: AppHandle,
    state: State<'_, Arc<Mutex<OpenCodeManager>>>,
    employee_id: String,
) -> Result<(), String> {
    let kind = OpenCodeSessionKind::Execution;
    let processes = {
        let manager = state.lock().await;
        manager.get_employee_processes(&employee_id)
    };
    let targets = processes
        .into_iter()
        .filter(|process| process.session_kind == kind)
        .collect::<Vec<_>>();

    if targets.is_empty() {
        return Err(format!("未找到员工 {} 的运行中 OpenCode 会话", employee_id));
    }

    for process in targets {
        stop_opencode_process_with_manager(
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

use tokio::io::AsyncReadExt;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeModelInfo {
    pub value: String,
    pub label: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model_id: String,
    #[serde(default)]
    pub capabilities: Option<OpenCodeModelCapabilities>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeModelCapabilities {
    #[serde(default)]
    pub reasoning: bool,
}

fn json_string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(|field| field.as_str()))
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .map(ToOwned::to_owned)
}

fn split_opencode_model_value(value: &str) -> (String, String) {
    match value.split_once('/') {
        Some((provider_id, model_id))
            if !provider_id.trim().is_empty() && !model_id.trim().is_empty() =>
        {
            (provider_id.trim().to_string(), model_id.trim().to_string())
        }
        _ => ("opencode".to_string(), value.trim().to_string()),
    }
}

fn extract_opencode_models_from_output(output: &str) -> Option<Vec<OpenCodeModelInfo>> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some(event) = parse_opencode_bridge_event(trimmed) else {
            continue;
        };
        if event.event_type != "providers" {
            continue;
        }

        let providers = event.data.get("providers")?.as_array()?;
        let models = providers
            .iter()
            .filter_map(|provider| {
                let value = json_string_field(provider, &["value"])?;
                let (fallback_provider_id, fallback_model_id) = split_opencode_model_value(&value);
                let label =
                    json_string_field(provider, &["label"]).unwrap_or_else(|| value.clone());
                let provider_id =
                    json_string_field(provider, &["providerId", "providerID", "provider_id"])
                        .unwrap_or(fallback_provider_id);
                let provider_name = json_string_field(provider, &["providerName", "provider_name"])
                    .unwrap_or_else(|| provider_id.clone());
                let model_id = json_string_field(provider, &["modelId", "modelID", "model_id"])
                    .unwrap_or(fallback_model_id);
                let capabilities = provider
                    .get("capabilities")
                    .and_then(|value| value.as_object())
                    .and_then(|capabilities| {
                        capabilities.get("reasoning").and_then(|value| {
                            value
                                .as_bool()
                                .map(|reasoning| OpenCodeModelCapabilities { reasoning })
                        })
                    });

                Some(OpenCodeModelInfo {
                    value,
                    label,
                    provider_id,
                    provider_name,
                    model_id,
                    capabilities,
                })
            })
            .collect::<Vec<_>>();

        return Some(models);
    }

    None
}

#[tauri::command]
pub async fn get_opencode_models<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<OpenCodeModelInfo>, String> {
    use tokio::io::AsyncWriteExt;
    use tokio::time::{timeout, Duration};

    let opencode_settings = load_opencode_settings(&app)?;
    let install_dir = PathBuf::from(&opencode_settings.sdk_install_dir);
    let bridge_path = sdk_bridge_script_path(&install_dir);

    let _ = ensure_opencode_sdk_runtime_layout(&install_dir);

    // Verify @opencode-ai/sdk is actually installed (npm installed, not just bridge file)
    let sdk_pkg = install_dir
        .join("node_modules")
        .join("@opencode-ai")
        .join("sdk")
        .join("package.json");
    if !sdk_pkg.exists() {
        return Err(
            "OpenCode SDK 尚未安装。请先在设置页点击「安装 SDK」按钮，安装完成后即可获取模型列表。"
                .to_string(),
        );
    }

    let mut command =
        crate::codex::new_node_command(opencode_settings.node_path_override.as_deref()).await?;
    command
        .arg(&bridge_path)
        .current_dir(&install_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 OpenCode bridge 失败: {error}"))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "无法获取 OpenCode bridge stdin".to_string())?;

    let config_json = serde_json::json!({
        "mode": "list-providers",
        "host": opencode_settings.host,
        "port": opencode_settings.port,
    });

    let config_str = serde_json::to_string(&config_json)
        .map_err(|error| format!("序列化 bridge 配置失败: {error}"))?;

    let mut stdin_writer = stdin;
    stdin_writer
        .write_all(config_str.as_bytes())
        .await
        .map_err(|error| format!("写入 bridge stdin 失败: {error}"))?;
    stdin_writer
        .flush()
        .await
        .map_err(|error| format!("刷新 bridge stdin 失败: {error}"))?;
    drop(stdin_writer);

    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法获取 OpenCode bridge stderr".to_string())?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法获取 OpenCode bridge stdout".to_string())?;

    let mut output = String::new();
    let mut err_output = String::new();

    // Read stdout and stderr with 30s timeout
    let _ = timeout(Duration::from_secs(30), stdout.read_to_string(&mut output)).await;
    let _ = timeout(
        Duration::from_secs(5),
        stderr.read_to_string(&mut err_output),
    )
    .await;

    let _ = child.wait().await;

    if let Some(models) = extract_opencode_models_from_output(&output) {
        return Ok(models);
    }

    // Check for error output first
    let err_preview = err_output.chars().take(300).collect::<String>();
    if !err_preview.trim().is_empty() {
        return Err(format!("OpenCode SDK 错误: {err_preview}"));
    }

    let preview = output.chars().take(300).collect::<String>();
    Err(format!("未能从 OpenCode SDK 获取模型列表。输出: {preview}"))
}

async fn wait_for_exit(child: &mut OpenCodeChild) -> Option<std::process::ExitStatus> {
    for _ in 0..STOP_WAIT_MAX_ATTEMPTS {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status);
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(STOP_WAIT_POLL_MS)).await;
    }
    let _ = child.kill().await;
    child.try_wait().ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bridge_session_event_reads_nested_session_id() {
        let event = parse_opencode_bridge_event(
            r#"{"type":"session","data":{"session_id":"ses_123"},"timestamp":1}"#,
        )
        .expect("bridge event should parse");

        assert_eq!(event.event_type, "session");
        assert_eq!(bridge_event_session_id(&event).as_deref(), Some("ses_123"));
    }

    #[test]
    fn extract_opencode_models_from_output_reads_provider_event() {
        let output = concat!(
            r#"{"type":"info","data":{"message":"启动 OpenCode SDK (model: default)"},"timestamp":1}"#,
            "\n",
            r#"{"type":"providers","data":{"providers":[{"value":"deepseek/deepseek-chat","label":"DeepSeek Chat","providerId":"deepseek","providerName":"DeepSeek","modelId":"deepseek-chat","capabilities":{"reasoning":true}},{"value":"openrouter/qwen","label":"Qwen","providerId":"openrouter","providerName":"OpenRouter","modelId":"qwen","capabilities":{}}],"defaults":{}},"timestamp":2}"#,
        );

        let models = extract_opencode_models_from_output(output)
            .expect("providers event should produce a model list");

        assert_eq!(models.len(), 2);
        assert_eq!(models[0].provider_id, "deepseek");
        assert_eq!(
            models[0].capabilities.as_ref().map(|value| value.reasoning),
            Some(true)
        );
        assert_eq!(models[1].provider_name, "OpenRouter");
        assert!(models[1].capabilities.is_none());
    }

    #[test]
    fn parse_bridge_server_ready_event_reads_url() {
        let event = parse_opencode_bridge_event(
            r#"{"type":"server-ready","data":{"url":"http://127.0.0.1:4096","connectedExistingServer":false},"timestamp":1}"#,
        )
        .expect("server-ready event should parse");

        assert_eq!(event.event_type, "server-ready");
        assert_eq!(
            bridge_event_string(&event, "url").as_deref(),
            Some("http://127.0.0.1:4096")
        );
    }

    #[test]
    fn extract_opencode_models_from_output_infers_missing_provider_fields() {
        let output = concat!(
            r#"{"type":"info","data":{"message":"已连接到运行中的 OpenCode server (http://127.0.0.1:4096)"},"timestamp":1}"#,
            "\n",
            r#"{"type":"providers","data":{"providers":[{"value":"deepseek/deepseek-chat","label":"DeepSeek Chat"}]},"timestamp":2}"#,
        );

        let models = extract_opencode_models_from_output(output)
            .expect("minimal providers event should produce a model list");

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].value, "deepseek/deepseek-chat");
        assert_eq!(models[0].provider_id, "deepseek");
        assert_eq!(models[0].provider_name, "deepseek");
        assert_eq!(models[0].model_id, "deepseek-chat");
        assert!(models[0].capabilities.is_none());
    }

    #[test]
    fn extract_opencode_models_from_output_reads_opencode_go_minimal_event() {
        let output = concat!(
            r#"{"type":"info","data":{"message":"启动 OpenCode SDK (model: default)"},"timestamp":1}"#,
            "\n",
            r#"{"type":"info","data":{"message":"已连接到运行中的 OpenCode server (http://127.0.0.1:4096)"},"timestamp":2}"#,
            "\n",
            r#"{"type":"providers","data":{"providers":[{"value":"opencode-go/minimax-m2.7","label":"MinMax"}]},"timestamp":3}"#,
        );

        let models = extract_opencode_models_from_output(output)
            .expect("screenshot-shaped providers event should produce a model list");

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].value, "opencode-go/minimax-m2.7");
        assert_eq!(models[0].provider_id, "opencode-go");
        assert_eq!(models[0].provider_name, "opencode-go");
        assert_eq!(models[0].model_id, "minimax-m2.7");
        assert!(models[0].capabilities.is_none());
    }

    #[test]
    fn final_status_marks_bridge_error_as_failed_even_with_zero_exit() {
        let (status, exit_code, message) =
            resolve_final_opencode_status(Some("running"), Some("Prompt timeout"), Some(0));

        assert_eq!(status, "failed");
        assert_eq!(exit_code, 1);
        assert!(message.contains("Prompt timeout"));
    }

    #[test]
    fn final_status_uses_exited_for_successful_bridge_completion() {
        let (status, exit_code, message) =
            resolve_final_opencode_status(Some("running"), None, Some(0));

        assert_eq!(status, "exited");
        assert_eq!(exit_code, 0);
        assert!(message.contains("已完成"));
    }

    #[test]
    fn final_status_preserves_user_stopping_as_exited() {
        let (status, exit_code, message) =
            resolve_final_opencode_status(Some("stopping"), Some("missing done"), Some(143));

        assert_eq!(status, "exited");
        assert_eq!(exit_code, 143);
        assert!(message.contains("已停止"));
    }

    #[test]
    fn opencode_runtime_config_disables_provider_timeout_and_keeps_effort() {
        let mut config = serde_json::json!({
            "$schema": "https://opencode.ai/config.json",
            "provider": {
                "opencode-go": {
                    "models": {
                        "deepseek-v4-flash": {
                            "options": {
                                "reasoning_effort": "low"
                            }
                        }
                    }
                }
            }
        });

        apply_opencode_runtime_config(&mut config, "opencode-go", "deepseek-v4-flash", Some("max"));

        assert_eq!(
            config["provider"]["opencode-go"]["options"]["timeout"],
            serde_json::Value::Bool(false)
        );
        assert_eq!(
            config["provider"]["opencode-go"]["options"]["chunkTimeout"],
            serde_json::Value::Number(OPENCODE_PROVIDER_CHUNK_TIMEOUT_MS.into())
        );
        assert_eq!(
            config["provider"]["opencode-go"]["models"]["deepseek-v4-flash"]["options"]["timeout"],
            serde_json::Value::Bool(false)
        );
        assert_eq!(
            config["provider"]["opencode-go"]["models"]["deepseek-v4-flash"]["options"]
                ["chunkTimeout"],
            serde_json::Value::Number(OPENCODE_PROVIDER_CHUNK_TIMEOUT_MS.into())
        );
        assert_eq!(
            config["provider"]["opencode-go"]["models"]["deepseek-v4-flash"]["options"]
                ["reasoning_effort"],
            serde_json::Value::String("max".to_string())
        );
    }

    #[test]
    fn opencode_runtime_config_file_rejects_invalid_json_without_overwriting() {
        let dir =
            std::env::temp_dir().join(format!("codex-ai-opencode-config-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let config_path = dir.join("opencode.json");
        fs::write(&config_path, "{not json").expect("write invalid config");

        let error = write_opencode_runtime_config_file(
            dir.to_str().expect("utf8 temp dir"),
            "opencode-go",
            "deepseek-v4-flash",
            Some("max"),
        )
        .expect_err("invalid json should be rejected");

        assert!(error.contains("解析失败"));
        assert_eq!(fs::read_to_string(&config_path).unwrap(), "{not json");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn opencode_runtime_config_backup_restores_missing_file() {
        let dir =
            std::env::temp_dir().join(format!("codex-ai-opencode-config-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let config_path = dir.join("opencode.json");

        let backup = write_opencode_runtime_config_file(
            dir.to_str().expect("utf8 temp dir"),
            "opencode-go",
            "deepseek-v4-flash",
            Some("max"),
        )
        .expect("runtime config should write");
        assert!(config_path.exists());

        backup
            .restore()
            .expect("restore should remove generated file");
        assert!(!config_path.exists());
        let _ = fs::remove_dir_all(dir);
    }
}
