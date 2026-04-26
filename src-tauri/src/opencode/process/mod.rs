use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::io::AsyncBufReadExt;
use tokio::sync::Mutex;

use crate::app::{
    insert_activity_log, insert_codex_session_event, insert_codex_session_event_with_id,
    insert_codex_session_record, now_sqlite, sqlite_pool, update_codex_session_record,
    validate_runtime_working_dir, EXECUTION_TARGET_LOCAL, EXECUTION_TARGET_SSH,
};
use crate::db::models::OpenCodeOutput;
use crate::git_workflow::{
    mark_task_git_context_running, mark_task_git_context_session_finished,
    validate_task_git_context_launch,
};
use crate::opencode::{
    ensure_opencode_sdk_runtime_layout, load_opencode_settings, sdk_bridge_script_path,
    OpenCodeManager,
};

mod context;
mod lifecycle;
mod session_runtime;
pub(crate) mod stream;

pub use self::lifecycle::OpenCodeChild;

use self::{context::*, lifecycle::*, session_runtime::*, stream::*};

const REVIEW_VERDICT_START_TAG: &str = "<review_verdict>";
const REVIEW_VERDICT_END_TAG: &str = "</review_verdict>";
const REVIEW_REPORT_START_TAG: &str = "<review_report>";
const REVIEW_REPORT_END_TAG: &str = "</review_report>";
const STOP_WAIT_POLL_MS: u64 = 50;
const STOP_WAIT_MAX_ATTEMPTS: usize = 600;

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
    if let Some(codex_state) = app.try_state::<Arc<std::sync::Mutex<crate::codex::CodexManager>>>() {
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
                .get_task_process_any(task_id, crate::claude::process::ClaudeSessionKind::Execution)
                .is_some()
            {
                return Err(format!("任务{}的 execution 会话已在 Claude 中运行", task_id));
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

async fn stream_opencode_output<R: Runtime>(
    app: AppHandle<R>,
    pool: SqlitePool,
    session_record_id: String,
    employee_id: String,
    task_id: Option<String>,
    session_kind: OpenCodeSessionKind,
    child: Arc<Mutex<OpenCodeChild>>,
    file_change_store: Option<SdkFileChangeStore>,
) {
    let stdout = child.lock().await.stdout();

    let Some(stdout) = stdout else {
        return;
    };

    let mut reader = tokio::io::BufReader::new(stdout);
    let mut line = String::new();
    let mut idle_timeout = tokio::time::Instant::now();

    loop {
        line.clear();

        // If no output for 120s, assume bridge is done
        if idle_timeout.elapsed() > tokio::time::Duration::from_secs(120) {
            emit_session_terminal_line(
                &app,
                &pool,
                &session_record_id,
                &employee_id,
                task_id.as_deref(),
                session_kind,
                "[INFO] OpenCode 输出空闲超时，自动结束会话。".to_string(),
            )
            .await;
            break;
        }

        let read_fut = reader.read_line(&mut line);
        let timed_out = tokio::time::timeout(tokio::time::Duration::from_secs(30), read_fut).await;

        match timed_out {
            Ok(Ok(0)) => break,
            Ok(Err(error)) => {
                emit_session_terminal_line(
                    &app,
                    &pool,
                    &session_record_id,
                    &employee_id,
                    task_id.as_deref(),
                    session_kind,
                    format!("[ERROR] OpenCode 输出流读取失败: {error}"),
                )
                .await;
                break;
            }
            Err(_) => {
                // 30s timeout with no output → continue loop, idle_timeout will catch 120s total
                continue;
            }
            Ok(Ok(_)) => {
                let trimmed = line.trim_end().to_string();
                if trimmed.is_empty() {
                    continue;
                }

                if let Some(event) = parse_opencode_sdk_file_change_event(&trimmed) {
                    if let Some(ref store) = file_change_store {
                        upsert_sdk_file_change_event(store, event);
                    }
                    continue;
                }

                if let Some(session_id) = extract_session_id_from_opencode_output(&trimmed) {
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

    // OpenCode bridge exited — finalize session as completed
    let ended_at = now_sqlite();
    let _ = update_codex_session_record(
        &app,
        &session_record_id,
        Some("completed"),
        None,
        None,
        Some(Some(ended_at.as_str())),
    )
    .await;

    let _ = app.emit(
        "opencode-exit",
        crate::db::models::OpenCodeExit {
            employee_id: employee_id.clone(),
            task_id: task_id.clone(),
            session_kind: session_kind.as_str().to_string(),
            session_record_id: session_record_id.clone(),
            session_event_id: None,
            status: "completed".to_string(),
            line: None,
            code: Some(0),
        },
    );

    if let Some(ref task_id) = task_id {
        if let Ok(Some(task_git_context_id)) =
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT task_git_context_id FROM codex_sessions WHERE id = $1",
            )
            .bind(&session_record_id)
            .fetch_one(&pool)
            .await
        {
            let _ = mark_task_git_context_session_finished(
                &pool,
                &task_git_context_id,
                true,
                None,
            )
            .await;
        }
    }
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
        if manager.has_employee_processes(&employee_id) {
            return Err(format!("员工{}已有未绑定任务的 OpenCode 会话在运行", employee_id));
        }
        if let Some(ref task_id) = task_id {
            if manager.get_task_process_any(task_id, session_kind).is_some() {
                return Err(format!("任务{}的 execution 会话已在 OpenCode 中运行", task_id));
            }
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
    let reasoning_effort = sqlx::query_scalar::<_, String>(
        "SELECT reasoning_effort FROM employees WHERE id = $1",
    )
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

    // Write reasoning_effort to opencode.json if it's not default
    let effort_to_write = reasoning_effort
        .as_deref()
        .and_then(|v| {
            if v != "default" && !v.trim().is_empty() {
                Some(v.to_string())
            } else {
                None
            }
        });

    if let Some(ref effort) = effort_to_write {
        let config_path = std::path::Path::new(&run_cwd).join("opencode.json");
        let existing_config: serde_json::Value = match std::fs::read_to_string(&config_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or(serde_json::json!({})),
            Err(_) => serde_json::json!({}),
        };

        let provider_id = model.split_once('/').map(|(p, _)| p).unwrap_or("opencode-go");
        let model_id = model.split_once('/').map(|(_, m)| m).unwrap_or(&model);

        let mut config = existing_config;
        config["provider"][provider_id]["models"][model_id]["options"]["reasoning_effort"] =
            serde_json::Value::String(effort.clone());

        if let Ok(json_str) = serde_json::to_string_pretty(&config) {
            let _ = std::fs::write(&config_path, json_str);
        }
    }

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
        host: opencode_settings.host.clone(),
        port: opencode_settings.port,
        working_directory: run_cwd.clone(),
        prompt: prompt.clone(),
        system_prompt: None,
        resume_session_id,
        image_paths: image_paths_resolved,
        install_dir: install_dir.clone(),
    };

    let child = launch_opencode_bridge(&bridge_config, &bridge_path).await?;
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
    let session_record_id_clone = session_record.id.clone();
    let employee_id_clone = employee_id.clone();
    let task_id_clone = task_id.clone();
    let session_kind_clone = session_kind;
    let child_clone = child.clone();
    let file_change_store_clone = file_change_store.clone();

    tauri::async_runtime::spawn(async move {
        stream_opencode_output(
            app_clone.clone(),
            pool_clone,
            session_record_id_clone,
            employee_id_clone,
            task_id_clone,
            session_kind_clone,
            child_clone,
            file_change_store_clone,
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
    let mut manager = state.lock().await;

    let target = manager
        .get_process(&session_record_id)
        .ok_or_else(|| format!("未找到 session {} 的运行中 OpenCode 会话", session_record_id))?;

    let employee_id = target.employee_id.clone();
    let kind = target.session_kind;
    let mut child = target.child.lock().await;
    child.kill_process_group().await?;

    let exit_status = wait_for_exit(&mut child).await;
    let status = if exit_status.map(|s| s.success()).unwrap_or(false) {
        "completed"
    } else {
        "terminated"
    };

    let pool = sqlite_pool(&app).await.map_err(|e| e.to_string())?;
    let ended_at = now_sqlite();
    let _ = update_codex_session_record(
        &app,
        &target.session_record_id,
        Some(status),
        None,
        None,
        Some(Some(ended_at.as_str())),
    )
    .await;

    let exit_code = exit_status.and_then(|s| s.code());
    let _ = app.emit(
        "opencode-exit",
        crate::db::models::OpenCodeExit {
            employee_id: employee_id.clone(),
            task_id: target.task_id.clone(),
            session_kind: kind.as_str().to_string(),
            session_record_id: target.session_record_id.clone(),
            session_event_id: None,
            status: status.to_string(),
            line: None,
            code: exit_code,
        },
    );

    if target.task_id.is_some() {
        if let Ok(Some(task_git_context_id)) =
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT task_git_context_id FROM codex_sessions WHERE id = $1",
            )
            .bind(&target.session_record_id)
            .fetch_one(&pool)
            .await
        {
            let _ = mark_task_git_context_session_finished(
                &pool,
                &task_git_context_id,
                status == "completed",
                None,
            )
            .await;
        }

        let _ = insert_codex_session_event(
            &pool,
            &target.session_record_id,
            "session_stopped",
            Some(&format!("OpenCode 会话已停止（状态: {status}）")),
        )
        .await;
    }

    drop(manager);

    Ok(())
}

#[tauri::command]
pub async fn stop_opencode(
    app: AppHandle,
    state: State<'_, Arc<Mutex<OpenCodeManager>>>,
    employee_id: String,
) -> Result<(), String> {
    let kind = OpenCodeSessionKind::Execution;
    let mut manager = state.lock().await;

    let processes = manager.get_employee_processes(&employee_id);
    let target = processes
        .into_iter()
        .find(|p| p.session_kind == kind)
        .ok_or_else(|| format!("未找到员工 {} 的运行中 OpenCode 会话", employee_id))?;

    let mut child = target.child.lock().await;
    child.kill_process_group().await?;

    let exit_status = wait_for_exit(&mut child).await;
    let status = if exit_status.map(|s| s.success()).unwrap_or(false) {
        "completed"
    } else {
        "terminated"
    };

    let pool = sqlite_pool(&app).await.map_err(|e| e.to_string())?;
    let ended_at = now_sqlite();
    let _ = update_codex_session_record(
        &app,
        &target.session_record_id,
        Some(status),
        None,
        None,
        Some(Some(ended_at.as_str())),
    )
    .await;

    let exit_code = exit_status.and_then(|s| s.code());
    let _ = app.emit(
        "opencode-exit",
        crate::db::models::OpenCodeExit {
            employee_id: employee_id.clone(),
            task_id: target.task_id.clone(),
            session_kind: kind.as_str().to_string(),
            session_record_id: target.session_record_id.clone(),
            session_event_id: None,
            status: status.to_string(),
            line: None,
            code: exit_code,
        },
    );

    if target.task_id.is_some() {
        if let Ok(Some(task_git_context_id)) =
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT task_git_context_id FROM codex_sessions WHERE id = $1",
            )
            .bind(&target.session_record_id)
            .fetch_one(&pool)
            .await
        {
            let _ = mark_task_git_context_session_finished(
                &pool,
                &task_git_context_id,
                status == "completed",
                None,
            )
            .await;
        }

        let _ = insert_codex_session_event(
            &pool,
            &target.session_record_id,
            "session_stopped",
            Some(&format!("OpenCode 会话已停止（状态: {status}）")),
        )
        .await;
    }

    drop(manager);

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BridgeProvidersPayload {
    #[serde(rename = "type")]
    event_type: String,
    data: BridgeProvidersData,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BridgeProvidersData {
    providers: Vec<OpenCodeModelInfo>,
    defaults: serde_json::Value,
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
        return Err("OpenCode SDK 尚未安装。请先在设置页点击「安装 SDK」按钮，安装完成后即可获取模型列表。".to_string());
    }

    let mut command = crate::codex::new_node_command(None).await?;
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
    let _ = timeout(Duration::from_secs(5), stderr.read_to_string(&mut err_output)).await;

    let _ = child.wait().await;

    // Try to parse each line as a bridge event
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(payload) = serde_json::from_str::<BridgeProvidersPayload>(trimmed) {
            if payload.event_type == "providers" {
                return Ok(payload.data.providers);
            }
        }
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
