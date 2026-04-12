use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::codex::CodexManager;
use crate::db::models::{CodexExit, CodexOutput};

pub struct CodexChild {
    child: Child,
}

impl CodexChild {
    pub async fn kill(&mut self) -> Result<(), String> {
        self.child.kill().await.map_err(|e: std::io::Error| e.to_string())
    }

    pub async fn wait(&mut self) -> Result<Option<i32>, String> {
        let status = self.child.wait().await.map_err(|e: std::io::Error| e.to_string())?;
        Ok(status.code())
    }
}

#[tauri::command]
pub async fn start_codex(
    app: AppHandle,
    state: State<'_, Arc<Mutex<CodexManager>>>,
    employee_id: String,
    task_description: String,
    working_dir: Option<String>,
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

    let mut cmd = Command::new("codex");
    cmd.arg("exec");
    if let Some(ref dir) = working_dir {
        cmd.arg("-C").arg(dir);
    }
    cmd.arg(&task_description)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

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

    // Use a shared dedup set: codex exec writes the same lines to both
    // stdout and stderr. We track recently emitted lines and skip duplicates.
    let seen = Arc::new(Mutex::new(std::collections::HashSet::<String>::new()));

    // Read stdout — emit only unseen lines
    let app_clone = app.clone();
    let eid = employee_id.clone();
    let seen_stdout = seen.clone();
    tauri::async_runtime::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
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
    start_codex(app, state, employee_id, task_description, working_dir).await
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
pub async fn ai_analyze_complexity(
    task_description: String,
) -> Result<String, String> {
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
