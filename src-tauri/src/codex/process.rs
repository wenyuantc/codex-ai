use std::fs;
use std::io::{BufRead, BufReader as StdBufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::sleep;

use crate::app::{
    fetch_codex_session_by_id, insert_codex_session_event, insert_codex_session_record,
    now_sqlite, sqlite_pool, update_codex_session_record, validate_runtime_working_dir,
};
use crate::codex::CodexManager;
use crate::db::models::{CodexExit, CodexOutput, CodexSession};

const SUPPORTED_MODELS: &[&str] = &["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex", "gpt-5.2"];
const SUPPORTED_REASONING_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh"];
const SESSION_ID_PREFIX: &str = "session id:";

#[derive(Debug, Deserialize)]
struct AiSubtasksPayload {
    subtasks: Vec<String>,
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
            let _ = insert_codex_session_event(&pool, &record.id, "validation_failed", Some(message))
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

    Err(format!("Timed out waiting for employee {} process to stop", employee_id))
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
) -> Result<(), String> {
    // Check if already running
    {
        let manager = state.lock().map_err(|e| e.to_string())?;
        if manager.is_running(&employee_id) {
            return Err(format!(
                "Codex instance for employee {} is already running",
                employee_id
            ));
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
        Some("Codex 会话创建成功，准备启动 CLI"),
    )
    .await?;

    let mut cmd = Command::new("codex");
    let model = normalize_model(model.as_deref());
    let reasoning_effort = normalize_reasoning_effort(reasoning_effort.as_deref());
    let prompt = compose_codex_prompt(&task_description, system_prompt.as_deref());
    cmd.arg("exec");
    cmd.arg("--model").arg(model);
    cmd.arg("-c")
        .arg(format!("model_reasoning_effort=\"{}\"", reasoning_effort));
    cmd.arg("-C").arg(&run_cwd);
    if let Some(ref session_id) = resume_session_id {
        cmd.arg("resume").arg(session_id).arg(&prompt);
    } else {
        cmd.arg(&prompt);
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let session_lookup_started_at = SystemTime::now();

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => {
            let message = format!("Failed to spawn codex: {}", error);
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
                "spawn_failed",
                Some(&message),
            )
            .await?;
            return Err(message);
        }
    };

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
    update_codex_session_record(
        &app,
        &session_record.id,
        Some("running"),
        None,
        None,
        None,
    )
    .await?;
    insert_codex_session_event(
        &pool,
        &session_record.id,
        "session_started",
        Some(&format!("使用模型 {} / 推理强度 {}", model, reasoning_effort)),
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
    } else {
        let app_clone = app.clone();
        let eid = employee_id.clone();
        let task_id_clone = task_id.clone();
        let run_cwd_clone = run_cwd.clone();
        let session_emitted_clone = session_emitted.clone();
        let session_record_id = session_record.id.clone();
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
    let task_id_clone = task_id.clone();
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
                            task_id_clone.as_ref(),
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
                        line,
                    },
                );
            }
        }
    });

    // Read stderr — emit only unseen lines
    let app_clone = app.clone();
    let eid = employee_id.clone();
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

        if !session_emitted_clone.load(Ordering::Relaxed) {
            if let Some(session_id) =
                find_latest_exec_session_id(&run_cwd_clone, session_lookup_started_at)
            {
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
            let _ = insert_codex_session_event(&pool, &session_record_id, event_type, Some(&message))
                .await;
        }

        let _ = app_clone.emit(
            "codex-exit",
            CodexExit {
                employee_id: eid,
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
        child.start_kill()
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
async fn run_ai_command(prompt: String) -> Result<String, String> {
    let output = Command::new("codex")
        .args(["exec", &prompt])
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
    items.into_iter()
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

#[cfg(test)]
mod tests {
    use super::{compose_codex_prompt, extract_session_id_from_output, parse_ai_subtasks_response};

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
        let subtasks = parse_ai_subtasks_response("[\"任务一\", \"任务二\"]")
            .expect("should parse array");

        assert_eq!(subtasks, vec!["任务一", "任务二"]);
    }
}

#[tauri::command]
pub async fn ai_suggest_assignee(
    task_description: String,
    employee_list: String,
) -> Result<String, String> {
    let prompt = format!(
        "Based on the following task description, suggest the best assignee from the employee list.\n\nTask: {}\n\nEmployees: {}\n\nRespond with just the employee ID and a brief reason.",
        task_description, employee_list
    );
    run_ai_command(prompt).await
}

#[tauri::command]
pub async fn ai_analyze_complexity(task_description: String) -> Result<String, String> {
    let prompt = format!(
        "Analyze the complexity of this task on a scale of 1-10, and provide a brief breakdown.\n\nTask: {}",
        task_description
    );
    run_ai_command(prompt).await
}

#[tauri::command]
pub async fn ai_generate_comment(
    task_title: String,
    task_description: String,
    context: String,
) -> Result<String, String> {
    let prompt = format!(
        "Generate a progress assessment comment for this task.\n\nTitle: {}\nDescription: {}\nContext: {}",
        task_title, task_description, context
    );
    run_ai_command(prompt).await
}

#[tauri::command]
pub async fn ai_split_subtasks(
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
    let raw = run_ai_command(prompt).await?;
    parse_ai_subtasks_response(&raw)
}
