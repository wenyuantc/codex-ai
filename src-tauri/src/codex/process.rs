use std::fs;
use std::io::{BufRead, BufReader as StdBufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::sleep;

use crate::codex::CodexManager;
use crate::db::models::{CodexExit, CodexOutput, CodexSession};

const SUPPORTED_MODELS: &[&str] = &["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex", "gpt-5.2"];
const SUPPORTED_REASONING_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh"];
const SESSION_ID_PREFIX: &str = "session id:";

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

pub struct CodexChild {
    child: Child,
}

impl CodexChild {
    pub async fn kill(&mut self) -> Result<(), String> {
        self.child
            .kill()
            .await
            .map_err(|e: std::io::Error| e.to_string())
    }

    pub async fn wait(&mut self) -> Result<Option<i32>, String> {
        let status = self
            .child
            .wait()
            .await
            .map_err(|e: std::io::Error| e.to_string())?;
        Ok(status.code())
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

    let run_cwd = match working_dir.clone() {
        Some(dir) => dir,
        None => std::env::current_dir()
            .map_err(|e| format!("Failed to determine current directory: {}", e))?
            .to_string_lossy()
            .to_string(),
    };

    let mut cmd = Command::new("codex");
    let model = normalize_model(model.as_deref());
    let reasoning_effort = normalize_reasoning_effort(reasoning_effort.as_deref());
    let prompt = compose_codex_prompt(&task_description, system_prompt.as_deref());
    cmd.arg("exec");
    cmd.arg("--model").arg(model);
    cmd.arg("-c")
        .arg(format!("model_reasoning_effort=\"{}\"", reasoning_effort));
    if let Some(ref dir) = working_dir {
        cmd.arg("-C").arg(dir);
    }
    if let Some(ref session_id) = resume_session_id {
        cmd.arg("resume").arg(session_id).arg(&prompt);
    } else {
        cmd.arg(&prompt);
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let session_lookup_started_at = SystemTime::now();

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn codex: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    // Store the child process
    {
        let mut manager = state.lock().map_err(|e| e.to_string())?;
        manager.add_process(employee_id.clone(), CodexChild { child });
    }

    let session_emitted = Arc::new(AtomicBool::new(false));

    if let Some(session_id) = resume_session_id.clone() {
        session_emitted.store(true, Ordering::Relaxed);
        let _ = app.emit(
            "codex-session",
            CodexSession {
                employee_id: employee_id.clone(),
                task_id: task_id.clone(),
                session_id,
            },
        );
    } else {
        let app_clone = app.clone();
        let eid = employee_id.clone();
        let task_id_clone = task_id.clone();
        let run_cwd_clone = run_cwd.clone();
        let session_emitted_clone = session_emitted.clone();
        tauri::async_runtime::spawn(async move {
            if let Some(session_id) =
                wait_for_exec_session_id(&run_cwd_clone, session_lookup_started_at).await
            {
                if !session_emitted_clone.swap(true, Ordering::Relaxed) {
                    let _ = app_clone.emit(
                        "codex-session",
                        CodexSession {
                            employee_id: eid,
                            task_id: task_id_clone,
                            session_id,
                        },
                    );
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
    tauri::async_runtime::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if !session_emitted_clone.load(Ordering::Relaxed) {
                if let Some(session_id) = extract_session_id_from_output(&line) {
                    if !session_emitted_clone.swap(true, Ordering::Relaxed) {
                        let _ = app_clone.emit(
                            "codex-session",
                            CodexSession {
                                employee_id: eid.clone(),
                                task_id: task_id_clone.clone(),
                                session_id,
                            },
                        );
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
    tauri::async_runtime::spawn(async move {
        // Take child out of manager to avoid holding MutexGuard across .await
        let mut codex_child = {
            let manager = app_clone.state::<Arc<Mutex<CodexManager>>>();
            let mut manager = manager.lock().unwrap();
            manager.remove_process(&eid)
        };

        let exit_code = if let Some(ref mut child) = codex_child {
            child.wait().await.ok().flatten()
        } else {
            None
        };

        // Child is already removed from manager
        drop(codex_child);

        if !session_emitted_clone.load(Ordering::Relaxed) {
            if let Some(session_id) =
                find_latest_exec_session_id(&run_cwd_clone, session_lookup_started_at)
            {
                let _ = app_clone.emit(
                    "codex-session",
                    CodexSession {
                        employee_id: eid.clone(),
                        task_id: task_id_clone,
                        session_id,
                    },
                );
            }
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
    state: State<'_, Arc<Mutex<CodexManager>>>,
    employee_id: String,
) -> Result<(), String> {
    let codex_child = {
        let mut manager = state.lock().map_err(|e| e.to_string())?;
        manager.remove_process(&employee_id)
    };

    if let Some(mut child) = codex_child {
        child.kill().await
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
    // Stop existing instance if running
    let codex_child = {
        let mut manager = state.lock().map_err(|e| e.to_string())?;
        manager.remove_process(&employee_id)
    };
    if let Some(mut child) = codex_child {
        let _ = child.kill().await;
    }
    // Start a new instance
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

#[cfg(test)]
mod tests {
    use super::{compose_codex_prompt, extract_session_id_from_output};

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
