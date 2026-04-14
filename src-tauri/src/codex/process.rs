use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader as StdBufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::time::sleep;

use crate::app::{
    fetch_codex_session_by_id, insert_codex_session_event, insert_codex_session_record, now_sqlite,
    sqlite_pool, update_codex_session_record, validate_runtime_working_dir,
};
use crate::codex::{
    ensure_sdk_runtime_layout, inspect_sdk_runtime, load_codex_settings, new_codex_command,
    new_node_command, sdk_bridge_script_path, CodexManager,
};
use crate::db::models::{CodexExit, CodexOutput, CodexSession};

const SUPPORTED_MODELS: &[&str] = &["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex", "gpt-5.2"];
const SUPPORTED_REASONING_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh"];
const SESSION_ID_PREFIX: &str = "session id:";

#[derive(Debug, Deserialize)]
struct AiSubtasksPayload {
    subtasks: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SdkBridgeResponse {
    ok: bool,
    text: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CodexExecutionProvider {
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
}

fn normalize_model(model: Option<&str>) -> &'static str {
    match model {
        Some(value) if SUPPORTED_MODELS.contains(&value) => match value {
            "gpt-5.4" => "gpt-5.4",
            "gpt-5.4-mini" => "gpt-5.4-mini",
            "gpt-5.3-codex" => "gpt-5.3-codex",
            "gpt-5.2" => "gpt-5.2",
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

fn extract_session_id_from_output(line: &str) -> Option<String> {
    let normalized = line.trim();
    if !normalized
        .to_ascii_lowercase()
        .starts_with(SESSION_ID_PREFIX)
    {
        return None;
    }

    normalized
        .split_once(':')
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

    format!(
        "[PROMPT] 即将发送给 Codex 的完整提示词\n\
运行通道: {}\n\
模型: {}\n\
推理强度: {}\n\
工作目录: {}\n\
{}\n\n{}",
        provider.label(),
        model,
        reasoning_effort,
        working_dir,
        image_block,
        prompt
    )
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
                Ok(canonical) => available.push(canonical.to_string_lossy().to_string()),
                Err(_) => available.push(trimmed.to_string()),
            }
        } else {
            missing.push(trimmed.to_string());
        }
    }

    (available, missing)
}

fn build_sdk_input_items(prompt: &str, image_paths: &[String]) -> Vec<serde_json::Value> {
    let mut items = vec![serde_json::json!({
        "type": "text",
        "text": prompt,
    })];

    for path in image_paths {
        items.push(serde_json::json!({
            "type": "local_image",
            "path": path,
        }));
    }

    items
}

async fn record_failed_session(
    app: &AppHandle,
    employee_id: &str,
    task_id: Option<&str>,
    working_dir: Option<&str>,
    resume_session_id: Option<&str>,
    message: &str,
) {
    if let Ok(record) = insert_codex_session_record(
        app,
        Some(employee_id),
        task_id,
        working_dir,
        resume_session_id,
        "failed",
    )
    .await
    {
        if let Ok(pool) = sqlite_pool(app).await {
            let _ =
                insert_codex_session_event(&pool, &record.id, "validation_failed", Some(message))
                    .await;
        }
    }
}

async fn bind_cli_session_id(
    app: &AppHandle,
    employee_id: &str,
    task_id: Option<&String>,
    session_record_id: &str,
    cli_session_id: String,
) {
    let _ = update_codex_session_record(
        app,
        session_record_id,
        None,
        Some(Some(cli_session_id.as_str())),
        None,
        None,
    )
    .await;
    if let Ok(pool) = sqlite_pool(app).await {
        let _ = insert_codex_session_event(
            &pool,
            session_record_id,
            "cli_session_bound",
            Some(&format!("CLI 会话已绑定: {}", cli_session_id)),
        )
        .await;
    }
    let _ = app.emit(
        "codex-session",
        CodexSession {
            employee_id: employee_id.to_string(),
            task_id: task_id.cloned(),
            session_id: cli_session_id,
        },
    );
}

async fn wait_until_process_stops(
    state: &State<'_, Arc<Mutex<CodexManager>>>,
    employee_id: &str,
) -> Result<(), String> {
    for _ in 0..100 {
        let still_running = {
            let manager = state.lock().map_err(|error| error.to_string())?;
            manager.is_running(employee_id)
        };
        if !still_running {
            return Ok(());
        }
        sleep(Duration::from_millis(50)).await;
    }

    Err(format!(
        "Timed out waiting for employee {} process to stop",
        employee_id
    ))
}

pub struct CodexChild {
    child: Child,
}

impl CodexChild {
    pub fn start_kill(&mut self) -> Result<(), String> {
        self.child
            .start_kill()
            .map_err(|e: std::io::Error| e.to_string())
    }

    pub fn try_wait(&mut self) -> Result<Option<i32>, String> {
        self.child
            .try_wait()
            .map(|status| status.and_then(|status| status.code()))
            .map_err(|e: std::io::Error| e.to_string())
    }
}

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
    resume_session_id: Option<String>,
    image_paths: Option<Vec<String>>,
) -> Result<(), String> {
    // Check if already running
    {
        let manager = state.lock().map_err(|e| e.to_string())?;
        if manager.is_running(&employee_id) {
            return Err(format!("员工{}的Codex实例已在运行", employee_id));
        }
    }

    let run_cwd = match validate_runtime_working_dir(working_dir.as_deref()) {
        Ok(path) => path,
        Err(error) => {
            record_failed_session(
                &app,
                &employee_id,
                task_id.as_deref(),
                working_dir.as_deref(),
                resume_session_id.as_deref(),
                &error,
            )
            .await;
            return Err(error);
        }
    };

    let session_record = insert_codex_session_record(
        &app,
        Some(&employee_id),
        task_id.as_deref(),
        Some(&run_cwd),
        resume_session_id.as_deref(),
        "pending",
    )
    .await?;
    let pool = sqlite_pool(&app).await?;
    insert_codex_session_event(
        &pool,
        &session_record.id,
        "session_requested",
        Some("Codex 会话创建成功，准备启动运行时"),
    )
    .await?;

    let model = normalize_model(model.as_deref());
    let reasoning_effort = normalize_reasoning_effort(reasoning_effort.as_deref());
    let prompt = compose_codex_prompt(&task_description, system_prompt.as_deref());
    let (image_paths, missing_image_paths) = collect_available_image_paths(image_paths);
    let mut provider = CodexExecutionProvider::Cli;
    let mut session_lookup_started_at = None;

    let command_result: Result<tokio::process::Command, String> =
        if should_use_sdk_for_session(&app).await {
            match load_codex_settings(&app) {
                Ok(settings) => {
                    let install_dir = PathBuf::from(&settings.sdk_install_dir);
                    if let Err(error) = ensure_sdk_runtime_layout(&install_dir) {
                        eprintln!("[codex-sdk] 刷新 SDK bridge 失败，回退 CLI: {error}");
                        let command = new_codex_command()
                            .await
                            .map_err(|cli_error| format!("Failed to spawn codex: {cli_error}"))?;
                        session_lookup_started_at = Some(SystemTime::now());
                        Ok(command)
                    } else {
                        let bridge_path =
                            sdk_bridge_script_path(Path::new(&settings.sdk_install_dir));
                        match new_node_command(settings.node_path_override.as_deref()).await {
                            Ok(mut command) => {
                                provider = CodexExecutionProvider::Sdk;
                                command
                                    .arg(&bridge_path)
                                    .current_dir(&run_cwd)
                                    .stdin(std::process::Stdio::piped())
                                    .stdout(std::process::Stdio::piped())
                                    .stderr(std::process::Stdio::piped());
                                Ok(command)
                            }
                            Err(error) => {
                                eprintln!("[codex-sdk] SDK 任务启动失败，回退 CLI: {error}");
                                let command = new_codex_command().await.map_err(|cli_error| {
                                    format!("Failed to spawn codex: {cli_error}")
                                })?;
                                session_lookup_started_at = Some(SystemTime::now());
                                Ok(command)
                            }
                        }
                    }
                }
                Err(error) => {
                    eprintln!("[codex-sdk] 读取配置失败，回退 CLI: {error}");
                    let command = new_codex_command()
                        .await
                        .map_err(|cli_error| format!("Failed to spawn codex: {cli_error}"))?;
                    session_lookup_started_at = Some(SystemTime::now());
                    Ok(command)
                }
            }
        } else {
            let command = new_codex_command()
                .await
                .map_err(|error| format!("Failed to spawn codex: {error}"))?;
            session_lookup_started_at = Some(SystemTime::now());
            Ok(command)
        };

    let mut cmd = match command_result {
        Ok(command) => command,
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

    if provider == CodexExecutionProvider::Cli {
        cmd.arg("exec");
        cmd.arg("--model").arg(model);
        cmd.arg("-c")
            .arg(format!("model_reasoning_effort=\"{}\"", reasoning_effort));
        cmd.arg("-C").arg(&run_cwd);
        if let Some(ref session_id) = resume_session_id {
            cmd.arg("resume").arg(session_id);
            for image_path in &image_paths {
                cmd.arg("--image").arg(image_path);
            }
            cmd.arg(&prompt);
        } else {
            for image_path in &image_paths {
                cmd.arg("--image").arg(image_path);
            }
            cmd.arg(&prompt);
        }
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
    }

    for missing_path in &missing_image_paths {
        let _ = app.emit(
            "codex-stdout",
            CodexOutput {
                employee_id: employee_id.clone(),
                task_id: task_id.clone(),
                line: format!("[WARN] 附件图片不存在，已跳过: {}", missing_path),
            },
        );
    }

    let _ = app.emit(
        "codex-stdout",
        CodexOutput {
            employee_id: employee_id.clone(),
            task_id: task_id.clone(),
            line: format_session_prompt_log(
                provider,
                model,
                reasoning_effort,
                &run_cwd,
                &prompt,
                &image_paths,
            ),
        },
    );

    let mut child = match cmd.spawn() {
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

    if provider == CodexExecutionProvider::Sdk {
        let payload = serde_json::to_vec(&serde_json::json!({
            "mode": "session",
            "prompt": prompt.clone(),
            "input": build_sdk_input_items(&prompt, &image_paths),
            "model": model,
            "workingDirectory": run_cwd.clone(),
            "resumeSessionId": resume_session_id.clone(),
        }))
        .map_err(|error| format!("Failed to serialize Codex SDK session payload: {}", error))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&payload)
                .await
                .map_err(|error| format!("Failed to write Codex SDK session payload: {}", error))?;
        }
    }

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let child_handle = Arc::new(tokio::sync::Mutex::new(CodexChild { child }));

    {
        let mut manager = state.lock().map_err(|e| e.to_string())?;
        manager.add_process(
            employee_id.clone(),
            child_handle.clone(),
            session_record.id.clone(),
        );
    }
    update_codex_session_record(&app, &session_record.id, Some("running"), None, None, None)
        .await?;
    insert_codex_session_event(
        &pool,
        &session_record.id,
        "session_started",
        Some(&format!(
            "通过 {} 启动，使用模型 {} / 推理强度 {} / 图片 {} 张",
            provider.label(),
            model,
            reasoning_effort,
            image_paths.len()
        )),
    )
    .await?;

    let session_emitted = Arc::new(AtomicBool::new(false));

    if let Some(session_id) = resume_session_id.clone() {
        session_emitted.store(true, Ordering::Relaxed);
        bind_cli_session_id(
            &app,
            &employee_id,
            task_id.as_ref(),
            &session_record.id,
            session_id,
        )
        .await;
    } else if provider == CodexExecutionProvider::Cli {
        let app_clone = app.clone();
        let eid = employee_id.clone();
        let task_id_clone = task_id.clone();
        let run_cwd_clone = run_cwd.clone();
        let session_emitted_clone = session_emitted.clone();
        let session_record_id = session_record.id.clone();
        let session_lookup_started_at =
            session_lookup_started_at.expect("cli session lookup start time");
        tauri::async_runtime::spawn(async move {
            if let Some(session_id) =
                wait_for_exec_session_id(&run_cwd_clone, session_lookup_started_at).await
            {
                if !session_emitted_clone.swap(true, Ordering::Relaxed) {
                    bind_cli_session_id(
                        &app_clone,
                        &eid,
                        task_id_clone.as_ref(),
                        &session_record_id,
                        session_id,
                    )
                    .await;
                }
            }
        });
    }

    // Use a shared dedup set: codex exec writes the same lines to both
    // stdout and stderr. We track recently emitted lines and skip duplicates.
    let seen = Arc::new(Mutex::new(std::collections::HashSet::<String>::new()));

    // Read stdout — emit only unseen lines
    let app_clone = app.clone();
    let eid = employee_id.clone();
    let task_id_for_stdout = task_id.clone();
    let seen_stdout = seen.clone();
    let session_emitted_clone = session_emitted.clone();
    let session_record_id = session_record.id.clone();
    tauri::async_runtime::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if !session_emitted_clone.load(Ordering::Relaxed) {
                if let Some(session_id) = extract_session_id_from_output(&line) {
                    if !session_emitted_clone.swap(true, Ordering::Relaxed) {
                        bind_cli_session_id(
                            &app_clone,
                            &eid,
                            task_id_for_stdout.as_ref(),
                            &session_record_id,
                            session_id,
                        )
                        .await;
                    }
                }
            }

            let is_dup = {
                let mut s = seen_stdout.lock().unwrap();
                if s.contains(&line) {
                    true
                } else {
                    s.insert(line.clone());
                    // Keep set bounded — remove entries older than 200 lines
                    if s.len() > 200 {
                        s.clear();
                    }
                    false
                }
            };
            if !is_dup {
                let _ = app_clone.emit(
                    "codex-stdout",
                    CodexOutput {
                        employee_id: eid.clone(),
                        task_id: task_id_for_stdout.clone(),
                        line,
                    },
                );
            }
        }
    });

    // Read stderr — emit only unseen lines
    let app_clone = app.clone();
    let eid = employee_id.clone();
    let task_id_for_stderr = task_id.clone();
    let seen_stderr = seen.clone();
    tauri::async_runtime::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let is_dup = {
                let mut s = seen_stderr.lock().unwrap();
                if s.contains(&line) {
                    true
                } else {
                    s.insert(line.clone());
                    if s.len() > 200 {
                        s.clear();
                    }
                    false
                }
            };
            if !is_dup {
                let _ = app_clone.emit(
                    "codex-stdout",
                    CodexOutput {
                        employee_id: eid.clone(),
                        task_id: task_id_for_stderr.clone(),
                        line,
                    },
                );
            }
        }
    });

    // Wait for exit — take the child out, wait, then emit exit event
    let app_clone = app.clone();
    let eid = employee_id.clone();
    let run_cwd_clone = run_cwd.clone();
    let task_id_clone = task_id.clone();
    let session_emitted_clone = session_emitted.clone();
    let session_record_id = session_record.id.clone();
    let child_handle_clone = child_handle.clone();
    let provider_for_exit = provider;
    let session_lookup_started_at = session_lookup_started_at;
    tauri::async_runtime::spawn(async move {
        let exit_code = loop {
            let maybe_status = {
                let mut child = child_handle_clone.lock().await;
                child.try_wait()
            };

            match maybe_status {
                Ok(Some(code)) => break Some(code),
                Ok(None) => sleep(Duration::from_millis(200)).await,
                Err(error) => {
                    let pool = sqlite_pool(&app_clone).await.ok();
                    let ended_at = now_sqlite();
                    let _ = update_codex_session_record(
                        &app_clone,
                        &session_record_id,
                        Some("failed"),
                        None,
                        None,
                        Some(Some(ended_at.as_str())),
                    )
                    .await;
                    if let Some(pool) = pool {
                        let _ = insert_codex_session_event(
                            &pool,
                            &session_record_id,
                            "session_failed",
                            Some(&error),
                        )
                        .await;
                    }
                    let manager = app_clone.state::<Arc<Mutex<CodexManager>>>();
                    let mut manager = manager.lock().unwrap();
                    manager.remove_process(&eid);
                    let _ = app_clone.emit(
                        "codex-exit",
                        CodexExit {
                            employee_id: eid.clone(),
                            task_id: task_id_clone.clone(),
                            code: None,
                        },
                    );
                    return;
                }
            }
        };

        {
            let manager = app_clone.state::<Arc<Mutex<CodexManager>>>();
            let mut manager = manager.lock().unwrap();
            manager.remove_process(&eid);
        }

        if provider_for_exit == CodexExecutionProvider::Cli
            && !session_emitted_clone.load(Ordering::Relaxed)
        {
            if let Some(session_id) = find_latest_exec_session_id(
                &run_cwd_clone,
                session_lookup_started_at.expect("cli session lookup start time"),
            ) {
                bind_cli_session_id(
                    &app_clone,
                    &eid,
                    task_id_clone.as_ref(),
                    &session_record_id,
                    session_id,
                )
                .await;
            }
        }

        let final_status = match fetch_codex_session_by_id(&app_clone, &session_record_id).await {
            Ok(record) if record.status == "stopping" => "exited",
            Ok(_) if exit_code == Some(0) => "exited",
            Ok(_) => "failed",
            Err(_) if exit_code == Some(0) => "exited",
            Err(_) => "failed",
        };
        let ended_at = now_sqlite();
        let _ = update_codex_session_record(
            &app_clone,
            &session_record_id,
            Some(final_status),
            None,
            Some(exit_code),
            Some(Some(ended_at.as_str())),
        )
        .await;
        if let Ok(pool) = sqlite_pool(&app_clone).await {
            let event_type = if final_status == "exited" {
                "session_exited"
            } else {
                "session_failed"
            };
            let message = format!("进程退出，exit_code={}", exit_code.unwrap_or_default());
            let _ =
                insert_codex_session_event(&pool, &session_record_id, event_type, Some(&message))
                    .await;
        }

        let _ = app_clone.emit(
            "codex-exit",
            CodexExit {
                employee_id: eid,
                task_id: task_id_clone,
                code: exit_code,
            },
        );
    });

    Ok(())
}

async fn wait_for_exec_session_id(run_cwd: &str, started_at: SystemTime) -> Option<String> {
    for _ in 0..120 {
        if let Some(session_id) = find_latest_exec_session_id(run_cwd, started_at) {
            return Some(session_id);
        }
        sleep(Duration::from_millis(500)).await;
    }
    None
}

fn find_latest_exec_session_id(run_cwd: &str, started_at: SystemTime) -> Option<String> {
    let sessions_root = std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".codex/sessions"))?;
    let mut latest: Option<(SystemTime, String)> = None;
    collect_session_files(&sessions_root)
        .into_iter()
        .filter_map(|path| {
            let modified = fs::metadata(&path).ok()?.modified().ok()?;
            if modified
                < started_at
                    .checked_sub(Duration::from_secs(2))
                    .unwrap_or(started_at)
            {
                return None;
            }
            let file = fs::File::open(&path).ok()?;
            let mut reader = StdBufReader::new(file);
            let mut first_line = String::new();
            reader.read_line(&mut first_line).ok()?;
            let json: serde_json::Value = serde_json::from_str(first_line.trim()).ok()?;
            if json.get("type")?.as_str()? != "session_meta" {
                return None;
            }
            let payload = json.get("payload")?;
            let is_exec_session = payload.get("source").and_then(|v| v.as_str()) == Some("exec")
                || payload.get("originator").and_then(|v| v.as_str()) == Some("codex_exec");
            if !is_exec_session {
                return None;
            }
            if payload.get("cwd").and_then(|v| v.as_str()) != Some(run_cwd) {
                return None;
            }
            let session_id = payload.get("id")?.as_str()?.to_string();
            Some((modified, session_id))
        })
        .for_each(|candidate| {
            if latest
                .as_ref()
                .map(|(modified, _)| candidate.0 > *modified)
                .unwrap_or(true)
            {
                latest = Some(candidate);
            }
        });

    latest.map(|(_, session_id)| session_id)
}

fn collect_session_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(year_dirs) = fs::read_dir(root) {
        for year_dir in year_dirs.flatten() {
            if let Ok(month_dirs) = fs::read_dir(year_dir.path()) {
                for month_dir in month_dirs.flatten() {
                    if let Ok(day_files) = fs::read_dir(month_dir.path()) {
                        for day_file in day_files.flatten() {
                            let path = day_file.path();
                            if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                                files.push(path);
                            }
                        }
                    }
                }
            }
        }
    }
    files
}

#[tauri::command]
pub async fn stop_codex(
    app: AppHandle,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    employee_id: String,
) -> Result<(), String> {
    let running_process = {
        let manager = state.lock().map_err(|e| e.to_string())?;
        manager.get_process(&employee_id)
    };

    if let Some(process) = running_process {
        let pool = sqlite_pool(&app).await?;
        update_codex_session_record(
            &app,
            &process.session_record_id,
            Some("stopping"),
            None,
            None,
            None,
        )
        .await?;
        insert_codex_session_event(
            &pool,
            &process.session_record_id,
            "stopping_requested",
            Some("收到停止请求"),
        )
        .await?;

        let mut child = process.child.lock().await;
        child.start_kill()?;
        drop(child);
        wait_until_process_stops(&state, &employee_id).await
    } else {
        Err(format!(
            "No running codex instance for employee {}",
            employee_id
        ))
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
) -> Result<(), String> {
    let is_running = {
        let manager = state.lock().map_err(|e| e.to_string())?;
        manager.is_running(&employee_id)
    };

    if is_running {
        let running_process = {
            let manager = state.lock().map_err(|e| e.to_string())?;
            manager.get_process(&employee_id)
        };

        if let Some(process) = running_process {
            let pool = sqlite_pool(&app).await?;
            update_codex_session_record(
                &app,
                &process.session_record_id,
                Some("stopping"),
                None,
                None,
                None,
            )
            .await?;
            insert_codex_session_event(
                &pool,
                &process.session_record_id,
                "restart_requested",
                Some("收到重启请求"),
            )
            .await?;

            let mut child = process.child.lock().await;
            child.start_kill()?;
        }

        wait_until_process_stops(&state, &employee_id).await?;
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
    if manager.is_running(&employee_id) {
        Err("Cannot write to stdin in non-interactive mode".to_string())
    } else {
        Err(format!(
            "No running codex instance for employee {}",
            employee_id
        ))
    }
}

/// Run a one-shot AI command using `codex exec`.
fn build_one_shot_exec_args(prompt: &str) -> Vec<String> {
    vec![
        "exec".to_string(),
        "--skip-git-repo-check".to_string(),
        prompt.to_string(),
    ]
}

async fn run_ai_command_via_exec(prompt: String) -> Result<String, String> {
    let mut cmd = new_codex_command()
        .await
        .map_err(|error| format!("Failed to spawn codex exec: {}", error))?;
    let output = cmd
        // 打包后的桌面应用工作目录通常不在受信任仓库内，
        // one-shot AI 功能也不依赖仓库上下文，因此这里显式跳过检查。
        .args(build_one_shot_exec_args(&prompt))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e: std::io::Error| format!("Failed to spawn codex exec: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("codex exec failed: {}", stderr.trim()))
    }
}

fn parse_sdk_bridge_output(stdout: &[u8], stderr: &[u8]) -> Result<String, String> {
    let raw_stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let raw_stderr = String::from_utf8_lossy(stderr).trim().to_string();

    if !raw_stdout.is_empty() {
        match serde_json::from_str::<SdkBridgeResponse>(&raw_stdout) {
            Ok(response) if response.ok => {
                return Ok(response.text.unwrap_or_default().trim().to_string())
            }
            Ok(response) => {
                return Err(response
                    .error
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "Codex SDK 返回了失败响应".to_string()))
            }
            Err(_) => {}
        }
    }

    if !raw_stderr.is_empty() {
        return Err(raw_stderr);
    }

    if !raw_stdout.is_empty() {
        return Err(raw_stdout);
    }

    Err("Codex SDK 返回空响应".to_string())
}

async fn run_ai_command_via_sdk(app: &AppHandle, prompt: &str) -> Result<String, String> {
    let settings = load_codex_settings(app)?;
    let install_dir = PathBuf::from(&settings.sdk_install_dir);
    ensure_sdk_runtime_layout(&install_dir)?;
    let bridge_path = sdk_bridge_script_path(&install_dir);
    if !bridge_path.exists() {
        return Err("Codex SDK bridge 脚本不存在，请在设置中重新安装 SDK".to_string());
    }

    let mut command = new_node_command(settings.node_path_override.as_deref()).await?;
    command
        .arg(&bridge_path)
        .current_dir(&install_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to spawn Codex SDK bridge: {}", error))?;
    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_vec(&serde_json::json!({ "prompt": prompt }))
            .map_err(|error| format!("Failed to serialize SDK request: {}", error))?;
        stdin
            .write_all(&payload)
            .await
            .map_err(|error| format!("Failed to write SDK request: {}", error))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|error| format!("Failed to wait for Codex SDK bridge: {}", error))?;
    parse_sdk_bridge_output(&output.stdout, &output.stderr)
}

async fn run_ai_command(app: &AppHandle, prompt: String) -> Result<String, String> {
    let mut sdk_error = None;

    if let Ok(settings) = load_codex_settings(app) {
        if settings.one_shot_sdk_enabled {
            let runtime = inspect_sdk_runtime(app, &settings).await;
            if runtime.one_shot_effective_provider == "sdk" {
                match run_ai_command_via_sdk(app, &prompt).await {
                    Ok(result) => return Ok(result),
                    Err(error) => {
                        eprintln!("[codex-sdk] 调用失败，回退到 codex exec: {error}");
                        sdk_error = Some(error);
                    }
                }
            } else {
                eprintln!("[codex-sdk] {}", runtime.status_message);
            }
        }
    }

    match run_ai_command_via_exec(prompt).await {
        Ok(result) => Ok(result),
        Err(exec_error) => match sdk_error {
            Some(sdk_error) => Err(format!(
                "Codex SDK 调用失败后回退 exec 也失败：SDK: {sdk_error}; exec: {exec_error}"
            )),
            None => Err(exec_error),
        },
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

fn extract_markdown_code_blocks(raw: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut remaining = raw;

    while let Some(start) = remaining.find("```") {
        let after_start = &remaining[start + 3..];
        let Some(end) = after_start.find("```") else {
            break;
        };

        let block = after_start[..end].trim();
        let block = block
            .strip_prefix("json")
            .or_else(|| block.strip_prefix("JSON"))
            .map(str::trim)
            .unwrap_or(block);

        if !block.is_empty() {
            blocks.push(block.to_string());
        }

        remaining = &after_start[end + 3..];
    }

    blocks
}

fn extract_balanced_json_segment(raw: &str, open: char, close: char) -> Option<String> {
    let start = raw.find(open)?;
    let mut depth = 0;

    for (offset, ch) in raw[start..].char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                let end = start + offset + ch.len_utf8();
                return Some(raw[start..end].to_string());
            }
        }
    }

    None
}

fn normalize_subtask_titles(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .collect()
}

fn parse_ai_subtasks_response(raw: &str) -> Result<Vec<String>, String> {
    let trimmed = raw.trim();
    let mut candidates = Vec::new();

    if !trimmed.is_empty() {
        candidates.push(trimmed.to_string());
    }
    candidates.extend(extract_markdown_code_blocks(trimmed));
    if let Some(object) = extract_balanced_json_segment(trimmed, '{', '}') {
        candidates.push(object);
    }
    if let Some(array) = extract_balanced_json_segment(trimmed, '[', ']') {
        candidates.push(array);
    }

    for candidate in candidates {
        if let Ok(payload) = serde_json::from_str::<AiSubtasksPayload>(&candidate) {
            let subtasks = normalize_subtask_titles(payload.subtasks);
            if !subtasks.is_empty() {
                return Ok(subtasks);
            }
        }

        if let Ok(payload) = serde_json::from_str::<Vec<String>>(&candidate) {
            let subtasks = normalize_subtask_titles(payload);
            if !subtasks.is_empty() {
                return Ok(subtasks);
            }
        }
    }

    Err("AI response did not contain valid subtasks JSON".to_string())
}

fn build_ai_generate_plan_prompt(
    task_title: &str,
    task_description: &str,
    task_status: &str,
    task_priority: &str,
    subtasks: &[String],
) -> String {
    let normalized_subtasks = subtasks
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    let subtasks_block = if normalized_subtasks.is_empty() {
        "（暂无）".to_string()
    } else {
        normalized_subtasks
            .iter()
            .enumerate()
            .map(|(index, title)| format!("{}. {}", index + 1, title))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "你是任务规划助手。请基于给定任务信息输出一份接近 Codex /plan 风格的中文 Markdown 执行计划。\n\
要求：\n\
- 只返回 Markdown 正文，不要代码块，不要 JSON，不要额外客套\n\
- 不要假装你已经读取仓库、查看文件、运行命令或完成验证；缺失信息请写入“风险与依赖”或“假设”\n\
- 必须包含以下标题：# 标题、## 目标与范围、## 实施步骤、## 验收与验证、## 风险与依赖、## 假设\n\
- “实施步骤”使用 1. 2. 3. 编号，步骤需要可执行、可验证，并吸收已有子任务中的有效信息\n\
- 结合当前状态、优先级、任务描述和子任务安排顺序，避免空泛表述\n\
- 如果信息不足，也要输出完整计划，并明确说明前提、依赖和缺口\n\n\
任务标题：{}\n\
当前状态：{}\n\
当前优先级：{}\n\
任务描述：{}\n\
现有子任务：\n{}",
        task_title.trim(),
        task_status.trim(),
        task_priority.trim(),
        if task_description.trim().is_empty() {
            "（未填写）"
        } else {
            task_description.trim()
        },
        subtasks_block
    )
}

#[cfg(test)]
mod tests {
    use super::{
        build_ai_generate_plan_prompt, build_one_shot_exec_args, compose_codex_prompt,
        extract_session_id_from_output, format_session_prompt_log, parse_ai_subtasks_response,
        parse_sdk_bridge_output, CodexExecutionProvider,
    };

    #[test]
    fn extracts_session_id_from_stdout_line() {
        assert_eq!(
            extract_session_id_from_output("session id: 019d8726-4730-7d71-b00c-aeade2188cb1"),
            Some("019d8726-4730-7d71-b00c-aeade2188cb1".to_string())
        );
    }

    #[test]
    fn ignores_non_session_lines() {
        assert_eq!(extract_session_id_from_output("codex"), None);
        assert_eq!(extract_session_id_from_output("hook: SessionStart"), None);
    }

    #[test]
    fn composes_prompt_with_employee_system_prompt() {
        let prompt = compose_codex_prompt("修复看板状态问题", Some("你是资深前端工程师"));

        assert!(prompt.contains("你是资深前端工程师"));
        assert!(prompt.contains("修复看板状态问题"));
        assert!(prompt.contains("<employee_system_prompt>"));
    }

    #[test]
    fn leaves_prompt_unchanged_without_employee_system_prompt() {
        assert_eq!(compose_codex_prompt("只执行任务", None), "只执行任务");
        assert_eq!(
            compose_codex_prompt("只执行任务", Some("   ")),
            "只执行任务"
        );
    }

    #[test]
    fn parses_subtasks_from_json_object() {
        let subtasks = parse_ai_subtasks_response(
            r#"{"subtasks":["整理需求说明","拆分前端交互","补充后端接口"]}"#,
        )
        .expect("should parse subtasks");

        assert_eq!(
            subtasks,
            vec!["整理需求说明", "拆分前端交互", "补充后端接口"]
        );
    }

    #[test]
    fn parses_subtasks_from_markdown_code_block() {
        let subtasks = parse_ai_subtasks_response(
            "下面是结果：\n```json\n{\"subtasks\":[\"梳理现状\",\"实现按钮\"]}\n```",
        )
        .expect("should parse fenced json");

        assert_eq!(subtasks, vec!["梳理现状", "实现按钮"]);
    }

    #[test]
    fn parses_subtasks_from_json_array() {
        let subtasks =
            parse_ai_subtasks_response("[\"任务一\", \"任务二\"]").expect("should parse array");

        assert_eq!(subtasks, vec!["任务一", "任务二"]);
    }

    #[test]
    fn one_shot_exec_args_skip_git_repo_check() {
        let args = build_one_shot_exec_args("分析任务");

        assert_eq!(
            args,
            vec![
                "exec".to_string(),
                "--skip-git-repo-check".to_string(),
                "分析任务".to_string()
            ]
        );
    }

    #[test]
    fn parses_sdk_bridge_success_output() {
        let output = parse_sdk_bridge_output(br#"{"ok":true,"text":"sdk output"}"#, &[])
            .expect("parse sdk bridge success");

        assert_eq!(output, "sdk output");
    }

    #[test]
    fn formats_prompt_log_with_runtime_context() {
        let log = format_session_prompt_log(
            CodexExecutionProvider::Sdk,
            "gpt-5.4",
            "high",
            "/tmp/demo",
            "任务标题:\n修复问题",
            &[
                "/tmp/demo/ui.png".to_string(),
                "/tmp/demo/flow.jpg".to_string(),
            ],
        );

        assert!(log.contains("[PROMPT]"));
        assert!(log.contains("运行通道: SDK"));
        assert!(log.contains("模型: gpt-5.4"));
        assert!(log.contains("推理强度: high"));
        assert!(log.contains("工作目录: /tmp/demo"));
        assert!(log.contains("附带图片: 2 张"));
        assert!(log.contains("1. ui.png"));
        assert!(log.contains("任务标题:\n修复问题"));
    }

    #[test]
    fn builds_plan_prompt_with_required_sections_and_context() {
        let prompt = build_ai_generate_plan_prompt(
            "看板任务详情增加 AI 生成计划",
            "在任务详情里新增 AI 生成计划，并支持插入详情。",
            "todo",
            "high",
            &[
                "补后端命令".to_string(),
                "补前端预览".to_string(),
                "补插入确认弹框".to_string(),
            ],
        );

        assert!(prompt.contains("# 标题"));
        assert!(prompt.contains("## 目标与范围"));
        assert!(prompt.contains("## 实施步骤"));
        assert!(prompt.contains("## 验收与验证"));
        assert!(prompt.contains("## 风险与依赖"));
        assert!(prompt.contains("## 假设"));
        assert!(prompt.contains("任务标题：看板任务详情增加 AI 生成计划"));
        assert!(prompt.contains("当前状态：todo"));
        assert!(prompt.contains("当前优先级：high"));
        assert!(prompt.contains("1. 补后端命令"));
        assert!(prompt.contains("2. 补前端预览"));
        assert!(prompt.contains("不要假装你已经读取仓库"));
    }
}

#[tauri::command]
pub async fn ai_suggest_assignee(
    app: AppHandle,
    task_description: String,
    employee_list: String,
) -> Result<String, String> {
    let prompt = format!(
        "Based on the following task description, suggest the best assignee from the employee list.\n\nTask: {}\n\nEmployees: {}\n\nRespond with just the employee ID and a brief reason.",
        task_description, employee_list
    );
    run_ai_command(&app, prompt).await
}

#[tauri::command]
pub async fn ai_analyze_complexity(
    app: AppHandle,
    task_description: String,
) -> Result<String, String> {
    let prompt = format!(
        "Analyze the complexity of this task on a scale of 1-10, and provide a brief breakdown.\n\nTask: {}",
        task_description
    );
    run_ai_command(&app, prompt).await
}

#[tauri::command]
pub async fn ai_generate_comment(
    app: AppHandle,
    task_title: String,
    task_description: String,
    context: String,
) -> Result<String, String> {
    let prompt = format!(
        "Generate a progress assessment comment for this task.\n\nTitle: {}\nDescription: {}\nContext: {}",
        task_title, task_description, context
    );
    run_ai_command(&app, prompt).await
}

#[tauri::command]
pub async fn ai_generate_plan(
    app: AppHandle,
    task_title: String,
    task_description: String,
    task_status: String,
    task_priority: String,
    subtasks: Vec<String>,
) -> Result<String, String> {
    let prompt = build_ai_generate_plan_prompt(
        &task_title,
        &task_description,
        &task_status,
        &task_priority,
        &subtasks,
    );
    run_ai_command(&app, prompt).await
}

#[tauri::command]
pub async fn ai_split_subtasks(
    app: AppHandle,
    task_title: String,
    task_description: String,
) -> Result<Vec<String>, String> {
    let prompt = format!(
        "你是任务拆分助手。请根据任务标题和描述拆分 3 到 8 个可执行、可验证、粒度适中的子任务。\n\
要求：\n\
- 只返回 JSON，不要 Markdown，不要额外解释\n\
- 返回格式必须是 {{\"subtasks\":[\"子任务1\",\"子任务2\"]}}\n\
- 每个子任务一句话，使用中文，避免重复和空泛表述\n\
- 如果描述信息有限，也基于现有信息给出合理拆分\n\n\
任务标题：{}\n\
任务描述：{}",
        task_title.trim(),
        task_description.trim()
    );
    let raw = run_ai_command(&app, prompt).await?;
    parse_ai_subtasks_response(&raw)
}
