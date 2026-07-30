use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::app::{
    build_remote_shell_command, build_ssh_command, fetch_ssh_config_record_by_id,
    insert_activity_log, insert_codex_session_event, insert_codex_session_event_with_id,
    insert_codex_session_record, now_sqlite, remote_shell_path_expression,
    shell_escape_single_quoted, sqlite_pool, update_codex_session_record,
    validate_runtime_working_dir, EXECUTION_TARGET_LOCAL, EXECUTION_TARGET_SSH,
};
use crate::codex::{CodexManager, CodexSessionKind, ExecutionChangeBaseline};
use crate::db::models::{GrokOutput, SshConfigRecord};
use crate::git_workflow::{
    mark_task_git_context_running, mark_task_git_context_session_finished,
    validate_task_git_context_launch,
};
use crate::grok::{
    load_grok_settings, normalize_grok_model, normalize_grok_reasoning_effort,
    resolve_grok_executable_path, GrokManager,
};

mod context;
mod lifecycle;
mod session_runtime;
mod stream;

pub use self::lifecycle::GrokChild;
pub use self::stream::aggregate_grok_one_shot_output;

use self::{context::*, session_runtime::*};

const STOP_WAIT_POLL_MS: u64 = 50;
const STOP_WAIT_MAX_ATTEMPTS: usize = 600;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrokSessionKind {
    Execution,
    Review,
}

impl GrokSessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            GrokSessionKind::Execution => "execution",
            GrokSessionKind::Review => "review",
        }
    }

    fn activity_start_action(self, resumed: bool) -> &'static str {
        match self {
            GrokSessionKind::Execution => {
                if resumed {
                    "task_execution_resumed"
                } else {
                    "task_execution_started"
                }
            }
            GrokSessionKind::Review => "task_review_started",
        }
    }
}

fn normalize_session_kind(session_kind: Option<&str>) -> GrokSessionKind {
    match session_kind {
        Some("review") => GrokSessionKind::Review,
        _ => GrokSessionKind::Execution,
    }
}

fn grok_session_kind_to_codex(session_kind: GrokSessionKind) -> CodexSessionKind {
    match session_kind {
        GrokSessionKind::Execution => CodexSessionKind::Execution,
        GrokSessionKind::Review => CodexSessionKind::Review,
    }
}

fn cleanup_process_artifacts(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

/// Grok headless CLI 参数。
/// 使用 `--permission-mode bypassPermissions` 实现非交互自动批准工具调用。
///
/// `prompt_json` 为 Some 时用 `--prompt-json`（可含图片 content blocks），否则用 `-p`。
/// 员工 system prompt 仅通过 `--system-prompt-override` 注入，不再嵌进 user prompt。
pub fn build_grok_cli_args(
    prompt: &str,
    model: &str,
    effort: &str,
    system_prompt: Option<&str>,
    resume_session_id: Option<&str>,
    cwd: Option<&str>,
    prompt_json: Option<&str>,
) -> Vec<String> {
    let mut args = Vec::new();

    if let Some(json) = prompt_json.map(str::trim).filter(|value| !value.is_empty()) {
        args.push("--prompt-json".to_string());
        args.push(json.to_string());
    } else {
        args.push("-p".to_string());
        args.push(prompt.to_string());
    }

    args.extend([
        "-m".to_string(),
        model.to_string(),
        "--reasoning-effort".to_string(),
        effort.to_string(),
        "--output-format".to_string(),
        "streaming-json".to_string(),
        "--permission-mode".to_string(),
        "bypassPermissions".to_string(),
    ]);

    if let Some(cwd) = cwd.map(str::trim).filter(|value| !value.is_empty()) {
        args.push("--cwd".to_string());
        args.push(cwd.to_string());
    }

    if let Some(sp) = system_prompt.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--system-prompt-override".to_string());
        args.push(sp.to_string());
    }

    if let Some(resume_id) = resume_session_id.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--resume".to_string());
        args.push(resume_id.to_string());
    }

    args
}

/// One-shot 固定 `--output-format json`，便于结构化提取最终文本。
pub fn build_grok_one_shot_cli_args(model: &str, effort: &str) -> Vec<String> {
    vec![
        "-p".to_string(),
        "-m".to_string(),
        model.to_string(),
        "--reasoning-effort".to_string(),
        effort.to_string(),
        "--output-format".to_string(),
        "json".to_string(),
        "--permission-mode".to_string(),
        "bypassPermissions".to_string(),
    ]
}

fn image_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "image/png",
    }
}

/// 构造 Grok ACP content blocks JSON（text + base64 images）。
pub fn build_grok_prompt_json(prompt: &str, image_paths: &[String]) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    use serde_json::{json, Value};

    let mut blocks: Vec<Value> = vec![json!({
        "type": "text",
        "text": prompt,
    })];

    for path in image_paths {
        let path = Path::new(path);
        let bytes = fs::read(path).map_err(|error| {
            format!(
                "读取 Grok 图片附件失败（{}）: {error}",
                path.display()
            )
        })?;
        let data = BASE64.encode(bytes);
        blocks.push(json!({
            "type": "image",
            "mimeType": image_mime_type(path),
            "data": data,
        }));
    }

    serde_json::to_string(&blocks).map_err(|error| format!("序列化 Grok prompt-json 失败: {error}"))
}

pub fn build_remote_grok_session_command(run_cwd: &str, cli_args: &[String]) -> String {
    let escaped_args = cli_args
        .iter()
        .map(|arg| shell_escape_single_quoted(arg))
        .collect::<Vec<_>>();

    build_remote_shell_command(
        &format!(
            "cd {} && exec grok {}",
            remote_shell_path_expression(run_cwd),
            escaped_args.join(" "),
        ),
        None,
    )
}

/// User prompt 仅包含任务正文；员工 system prompt 走 `--system-prompt-override`。
fn compose_grok_prompt(task_description: &str) -> String {
    task_description.trim().to_string()
}

fn format_grok_session_prompt_log(
    model: &str,
    effort: &str,
    execution_target: &str,
    target_host_label: Option<&str>,
    working_dir: &str,
    prompt: &str,
    system_prompt: Option<&str>,
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

    let system_block = system_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("System Prompt (--system-prompt-override):\n{value}\n\n"))
        .unwrap_or_default();

    format!(
        "[PROMPT] 即将发送给 Grok 的完整提示词\n\
运行通道: Grok CLI\n\
模型: {}\n\
推理强度: {}\n\
{}\n\
工作目录: {}\n\
{}\n\n{}User Prompt:\n{}",
        model, effort, runtime_block, working_dir, image_block, system_block, prompt
    )
}

pub(crate) fn extract_review_report(raw: &str) -> Option<String> {
    crate::codex::extract_review_report(raw)
}

pub(crate) fn extract_review_verdict(raw: &str) -> Option<String> {
    crate::codex::extract_review_verdict(raw)
}

async fn emit_session_terminal_line<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: GrokSessionKind,
    line: String,
) {
    let event_id =
        insert_codex_session_event_with_id(pool, session_record_id, "stdout", Some(&line))
            .await
            .ok();

    let _ = app.emit(
        "grok-stdout",
        GrokOutput {
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
        "SELECT title, project_id FROM tasks WHERE id = $1 AND deleted_at IS NULL LIMIT 1",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        format!(
            "Failed to resolve task {} for Grok activity log: {}",
            task_id, error
        )
    })
}

async fn write_grok_task_session_activity<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: GrokSessionKind,
    resume_session_id: Option<&str>,
    execution_target: &str,
) {
    let Some(task_id) = task_id else {
        return;
    };

    let result = async {
        let (task_title, project_id) = fetch_task_activity_context(pool, task_id).await?;
        let action = if execution_target == EXECUTION_TARGET_SSH
            && session_kind == GrokSessionKind::Execution
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
            format!("[WARN] Grok 活动日志写入失败: {error}"),
        )
        .await;
    }
}

async fn ensure_no_cross_provider_conflict<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: GrokSessionKind,
) -> Result<(), String> {
    if let Some(codex_state) = app.try_state::<Arc<std::sync::Mutex<CodexManager>>>() {
        if let Some(task_id) = task_id {
            if crate::codex::get_live_task_process_by_task(
                app,
                codex_state.inner(),
                task_id,
                grok_session_kind_to_codex(session_kind),
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
    }

    if let Some(claude_state) =
        app.try_state::<Arc<tokio::sync::Mutex<crate::claude::ClaudeManager>>>()
    {
        let manager = claude_state.lock().await;
        if let Some(task_id) = task_id {
            if manager
                .get_task_process_any(
                    task_id,
                    match session_kind {
                        GrokSessionKind::Execution => crate::claude::ClaudeSessionKind::Execution,
                        GrokSessionKind::Review => crate::claude::ClaudeSessionKind::Review,
                    },
                )
                .is_some()
            {
                return Err(format!(
                    "任务{}的{}会话已在运行",
                    task_id,
                    session_kind.as_str()
                ));
            }
        } else if manager.has_employee_processes(employee_id) {
            return Err(format!(
                "员工{}已有未绑定任务的 Claude 会话在运行",
                employee_id
            ));
        }
    }

    if let Some(opencode_state) =
        app.try_state::<Arc<tokio::sync::Mutex<crate::opencode::OpenCodeManager>>>()
    {
        let manager = opencode_state.lock().await;
        if let Some(task_id) = task_id {
            if manager
                .get_task_process_any(
                    task_id,
                    crate::opencode::OpenCodeSessionKind::Execution,
                )
                .is_some()
            {
                return Err(format!(
                    "任务{}的{}会话已在运行",
                    task_id,
                    session_kind.as_str()
                ));
            }
        } else if manager.has_employee_processes(employee_id) {
            return Err(format!(
                "员工{}已有未绑定任务的 OpenCode 会话在运行",
                employee_id
            ));
        }
    }

    Ok(())
}

async fn finalize_grok_launch_failure<R: Runtime>(
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

async fn capture_grok_execution_change_baseline<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: GrokSessionKind,
    execution_target: &str,
    run_cwd: &str,
    ssh_config: Option<&SshConfigRecord>,
) -> Option<ExecutionChangeBaseline> {
    if session_kind != GrokSessionKind::Execution {
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
            "[SSH] 正在采集远程仓库基线，用于展示本次 Grok 会话改动...".to_string(),
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
                    "[WARN] Grok 会话文件基线采集失败，文件详情将退化为最佳努力快照: {error}"
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
pub async fn start_grok(
    app: AppHandle,
    state: State<'_, Arc<tokio::sync::Mutex<GrokManager>>>,
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
    start_grok_with_manager(
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

pub async fn start_grok_with_manager(
    app: AppHandle,
    manager_state: Arc<tokio::sync::Mutex<GrokManager>>,
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
                "员工{}已有未绑定任务的 Grok 会话在运行",
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
            .ok_or_else(|| "SSH 项目缺少远程仓库目录，无法启动 Grok。".to_string())?
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

    let grok_settings = load_grok_settings(&app)?;
    let model = normalize_grok_model(
        model
            .as_deref()
            .or(Some(grok_settings.default_model.as_str())),
    );
    let effort = normalize_grok_reasoning_effort(
        reasoning_effort
            .as_deref()
            .or(Some(grok_settings.default_reasoning_effort.as_str())),
    );
    let prompt = compose_grok_prompt(&task_description);
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
        Some("grok"),
        None,
    )
    .await?;

    let mut git_context_marked_running = false;
    if let Some(task_git_context_id) = task_git_context_id.as_deref() {
        if let Err(error) = mark_task_git_context_running(&pool, task_git_context_id).await {
            finalize_grok_launch_failure(
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
        Some("Grok 会话创建成功，准备启动运行时"),
    )
    .await
    {
        finalize_grok_launch_failure(
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
                "[SSH] 正在准备远程 Grok 会话，目标 {}，工作目录 {}",
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
                finalize_grok_launch_failure(
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
            format!("[WARN] Grok 附件图片不存在，已跳过: {missing_path}"),
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
                "[WARN] SSH Grok 会话缺少任务上下文，已忽略 {} 张本地图片附件。",
                ignored_remote_image_count
            ),
        )
        .await;
    }

    // 本地：通过 --prompt-json 内嵌 base64 图片。SSH：命令行过长风险，仍跳过图片。
    let (effective_image_paths, prompt_json) =
        if execution_context.execution_target == EXECUTION_TARGET_SSH {
            if !image_paths.is_empty() {
                emit_session_terminal_line(
                    &app,
                    &pool,
                    &session_record.id,
                    &employee_id,
                    task_id.as_deref(),
                    session_kind,
                    format!(
                        "[WARN] SSH Grok 会话暂不通过命令行附带图片（避免参数过长），已跳过 {} 张图片附件。",
                        image_paths.len()
                    ),
                )
                .await;
            }
            (Vec::new(), None)
        } else if image_paths.is_empty() {
            (Vec::new(), None)
        } else {
            match build_grok_prompt_json(&prompt, &image_paths) {
                Ok(json) => {
                    emit_session_terminal_line(
                        &app,
                        &pool,
                        &session_record.id,
                        &employee_id,
                        task_id.as_deref(),
                        session_kind,
                        format!(
                            "[INFO] 已通过 prompt-json 附带 {} 张图片。",
                            image_paths.len()
                        ),
                    )
                    .await;
                    (image_paths.clone(), Some(json))
                }
                Err(error) => {
                    emit_session_terminal_line(
                        &app,
                        &pool,
                        &session_record.id,
                        &employee_id,
                        task_id.as_deref(),
                        session_kind,
                        format!("[WARN] 构造 Grok 图片 prompt-json 失败，已跳过图片：{error}"),
                    )
                    .await;
                    (Vec::new(), None)
                }
            }
        };
    let _ = effective_image_paths;

    let ssh_config_for_artifact_capture =
        if execution_context.execution_target == EXECUTION_TARGET_SSH {
            let ssh_config_id = match execution_context.ssh_config_id.as_deref() {
                Some(ssh_config_id) => ssh_config_id,
                None => {
                    let error = "SSH 会话缺少 ssh_config_id".to_string();
                    finalize_grok_launch_failure(
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
                    finalize_grok_launch_failure(
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

    let execution_change_baseline = capture_grok_execution_change_baseline(
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

    // 远程：cwd 由 shell `cd` 提供，避免与 --cwd 冲突；本地：spawn cwd + 可选 --cwd。
    let cli_args = if execution_context.execution_target == EXECUTION_TARGET_SSH {
        build_grok_cli_args(
            &prompt,
            &model,
            &effort,
            system_prompt.as_deref(),
            resume_session_id.as_deref(),
            None,
            None,
        )
    } else {
        build_grok_cli_args(
            &prompt,
            &model,
            &effort,
            system_prompt.as_deref(),
            resume_session_id.as_deref(),
            Some(&run_cwd),
            prompt_json.as_deref(),
        )
    };

    let (child, cleanup_paths) = if execution_context.execution_target == EXECUTION_TARGET_SSH {
        let ssh_config = ssh_config_for_artifact_capture
            .as_ref()
            .expect("SSH config is prepared before Grok launch");

        let remote_command = build_remote_grok_session_command(&run_cwd, &cli_args);

        let (mut command, askpass_path) =
            match build_ssh_command(&app, &ssh_config, Some(&remote_command), true, false).await {
                Ok(result) => result,
                Err(error) => {
                    finalize_grok_launch_failure(
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
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        configure_process_group(&mut command);

        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let message = format!("启动远程 Grok 会话失败: {error}");
                let cleanup_paths = askpass_path.iter().cloned().collect::<Vec<_>>();
                cleanup_process_artifacts(&cleanup_paths);
                finalize_grok_launch_failure(
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
        (child, askpass_path.into_iter().collect::<Vec<_>>())
    } else {
        let grok_bin = match resolve_grok_executable_path(&grok_settings).await {
            Ok(path) => path,
            Err(error) => {
                finalize_grok_launch_failure(
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
        let mut command = tokio::process::Command::new(&grok_bin);
        command
            .args(&cli_args)
            .current_dir(&run_cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        configure_process_group(&mut command);

        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let message = format!(
                    "启动 Grok 会话失败: {error}。请确认已安装 Grok Build CLI（`grok`），并完成 `grok login`。"
                );
                finalize_grok_launch_failure(
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
        (child, Vec::new())
    };

    if let Err(error) =
        update_codex_session_record(&app, &session_record.id, Some("running"), None, None, None)
            .await
    {
        // 进程已启动但状态更新失败：尽量清理。
        let mut child = child;
        let _ = child.kill().await;
        cleanup_process_artifacts(&cleanup_paths);
        finalize_grok_launch_failure(
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
        format!("[Grok] 通过 Grok CLI 启动会话{target_label} model={model} effort={effort}"),
    )
    .await;
    let prompt_line = format_grok_session_prompt_log(
        &model,
        &effort,
        &execution_context.execution_target,
        execution_context.target_host_label.as_deref(),
        &run_cwd,
        &prompt,
        system_prompt.as_deref(),
        &effective_image_paths,
    );
    let prompt_event_id = insert_codex_session_event_with_id(
        &pool,
        &session_record.id,
        "user_prompt",
        Some(&prompt_line),
    )
    .await
    .ok();
    let _ = app.emit(
        "grok-stdout",
        GrokOutput {
            employee_id: employee_id.to_string(),
            task_id: task_id.clone(),
            session_kind: session_kind.as_str().to_string(),
            session_record_id: session_record.id.clone(),
            session_event_id: prompt_event_id,
            line: prompt_line,
        },
    );

    let grok_child = GrokChild::new(child);
    let child_arc = Arc::new(tokio::sync::Mutex::new(grok_child));

    {
        let mut manager = manager_state.lock().await;
        manager.add_process(
            employee_id.clone(),
            task_id.clone(),
            session_kind,
            child_arc.clone(),
            session_record.id.clone(),
            cleanup_paths,
            (),
        );
    }

    if let Ok(pool) = sqlite_pool(&app).await {
        write_grok_task_session_activity(
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

    spawn_grok_session_runtime(
        app,
        manager_state,
        child_arc,
        session_record.id,
        employee_id,
        task_id,
        task_git_context_id,
        session_kind,
        execution_change_baseline,
        run_cwd,
    );

    Ok(())
}

pub async fn list_live_grok_employee_processes(
    manager_state: &Arc<tokio::sync::Mutex<GrokManager>>,
    employee_id: &str,
) -> Vec<crate::grok::manager::ManagedGrokProcess> {
    let manager = manager_state.lock().await;
    manager.get_employee_processes(employee_id)
}

async fn wait_until_grok_process_stops(
    manager_state: &Arc<tokio::sync::Mutex<GrokManager>>,
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

async fn stop_grok_process_with_manager<R: Runtime>(
    app: &AppHandle<R>,
    manager_state: &Arc<tokio::sync::Mutex<GrokManager>>,
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
        format!("[Grok] {message}"),
    )
    .await;

    let mut child = process.child.lock().await;
    if let Err(error) = child.kill_process_group() {
        eprintln!("[grok-stop] killpg failed, fallback to child.kill(): {error}");
    }
    child.kill().await?;
    drop(child);
    wait_until_grok_process_stops(manager_state, session_record_id).await;

    Ok(true)
}

pub(crate) async fn stop_grok_for_automation_restart<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: &str,
    expected_session_record_id: Option<&str>,
    message: &str,
) -> Result<bool, String> {
    let manager_state = app
        .state::<Arc<tokio::sync::Mutex<GrokManager>>>()
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

    stop_grok_process_with_manager(
        app,
        &manager_state,
        expected_session_record_id,
        "automation_restart_requested",
        message,
    )
    .await
}

#[tauri::command]
pub async fn stop_grok_session(
    app: AppHandle,
    state: State<'_, Arc<tokio::sync::Mutex<GrokManager>>>,
    session_record_id: String,
) -> Result<(), String> {
    if !stop_grok_process_with_manager(
        &app,
        state.inner(),
        &session_record_id,
        "stopping_requested",
        "收到停止请求",
    )
    .await?
    {
        return Err(format!("未找到 Grok 会话 {session_record_id}"));
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_grok(
    app: AppHandle,
    state: State<'_, Arc<tokio::sync::Mutex<GrokManager>>>,
    employee_id: String,
) -> Result<(), String> {
    let processes = {
        let manager = state.lock().await;
        manager.get_employee_processes(&employee_id)
    };

    for process in processes {
        stop_grok_process_with_manager(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grok_cli_args_include_headless_contract() {
        let args = build_grok_cli_args(
            "修复 bug",
            "grok-4.5",
            "high",
            Some("你是工程师"),
            Some("sess-1"),
            Some("/tmp/project"),
            None,
        );

        assert!(args.windows(2).any(|pair| pair[0] == "-p" && pair[1] == "修复 bug"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-m" && pair[1] == "grok-4.5"));
        assert!(args.windows(2).any(|pair| {
            pair[0] == "--reasoning-effort" && pair[1] == "high"
        }));
        assert!(args.windows(2).any(|pair| {
            pair[0] == "--output-format" && pair[1] == "streaming-json"
        }));
        assert!(args.windows(2).any(|pair| {
            pair[0] == "--permission-mode" && pair[1] == "bypassPermissions"
        }));
        assert!(args.windows(2).any(|pair| {
            pair[0] == "--system-prompt-override" && pair[1] == "你是工程师"
        }));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--resume" && pair[1] == "sess-1"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--cwd" && pair[1] == "/tmp/project"));
    }

    #[test]
    fn remote_grok_session_command_uses_shell_bootstrap() {
        let args = build_grok_cli_args("task", "grok-4.5", "high", None, None, None, None);
        let command = build_remote_grok_session_command("~/repo with space", &args);

        assert!(command.starts_with("sh -lc "));
        assert!(command.contains("exec grok"));
        assert!(command.contains("$HOME/repo with space"));
    }

    #[test]
    fn grok_prompt_log_includes_system_and_user_prompt() {
        let prompt = compose_grok_prompt("修复自动质控");
        let log = format_grok_session_prompt_log(
            "grok-4.5",
            "high",
            EXECUTION_TARGET_LOCAL,
            None,
            "/tmp/project",
            &prompt,
            Some("你是审查员"),
            &[],
        );

        assert!(log.contains("[PROMPT] 即将发送给 Grok 的完整提示词"));
        assert!(log.contains("运行通道: Grok CLI"));
        assert!(log.contains("--system-prompt-override"));
        assert!(log.contains("你是审查员"));
        assert!(log.contains("User Prompt:"));
        assert!(log.contains("修复自动质控"));
        assert!(!log.contains("<employee_system_prompt>"));
    }

    #[test]
    fn grok_cli_args_prefer_prompt_json_over_plain_prompt() {
        let args = build_grok_cli_args(
            "fallback",
            "grok-4.5",
            "high",
            None,
            None,
            None,
            Some(r#"[{"type":"text","text":"hi"}]"#),
        );
        assert!(args.windows(2).any(|pair| {
            pair[0] == "--prompt-json" && pair[1].contains(r#""type":"text""#)
        }));
        assert!(!args.windows(2).any(|pair| pair[0] == "-p" && pair[1] == "fallback"));
    }
}
