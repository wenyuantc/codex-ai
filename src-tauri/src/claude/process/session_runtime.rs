use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::app::{
    fetch_codex_session_by_id, insert_codex_session_event, insert_codex_session_event_with_id,
    now_sqlite, parse_review_verdict_json, sqlite_pool, update_codex_session_record,
};
use crate::codex::{CodexExecutionProvider, CodexSessionKind, ExecutionChangeBaseline};
use crate::db::models::{ClaudeExit, ClaudeOutput, ClaudeSession};
use crate::git_workflow::mark_task_git_context_session_finished;
use crate::task_automation;

use super::stream::{
    extract_session_id, parse_claude_cli_json_event_line, parse_claude_file_change_event,
    ClaudeCliJsonStreamState,
};
use super::{
    extract_review_report, extract_review_verdict, upsert_sdk_file_change_event, ClaudeChild,
    ClaudeExecutionProvider, ClaudeManager, ClaudeSessionKind, SdkFileChangeStore,
    CLAUDE_FILE_CHANGE_EVENT_PREFIX, SESSION_ID_PREFIX, STOP_WAIT_MAX_ATTEMPTS, STOP_WAIT_POLL_MS,
};

fn push_captured_line(captured: &Arc<Mutex<Vec<String>>>, line: String) {
    let mut captured = captured.lock().unwrap();
    captured.push(line);
    if captured.len() > 2000 {
        let drain_to = captured.len().saturating_sub(2000);
        if drain_to > 0 {
            captured.drain(0..drain_to);
        }
    }
}

fn resolve_final_claude_status(
    current_status: Option<&str>,
    exit_code: Option<i32>,
) -> &'static str {
    match (current_status, exit_code) {
        (Some("stopping"), _) => "exited",
        (_, Some(0)) => "exited",
        _ => "failed",
    }
}

fn claude_session_kind_to_codex(session_kind: ClaudeSessionKind) -> CodexSessionKind {
    match session_kind {
        ClaudeSessionKind::Execution => CodexSessionKind::Execution,
        ClaudeSessionKind::Review => CodexSessionKind::Review,
    }
}

fn claude_provider_to_codex(provider: ClaudeExecutionProvider) -> CodexExecutionProvider {
    match provider {
        ClaudeExecutionProvider::Sdk => CodexExecutionProvider::Sdk,
        ClaudeExecutionProvider::Cli => CodexExecutionProvider::Cli,
    }
}

async fn bind_claude_session_id(
    app: &AppHandle,
    employee_id: &str,
    task_id: Option<&String>,
    session_kind: ClaudeSessionKind,
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
            Some(&format!("Claude CLI 会话已绑定: {}", cli_session_id)),
        )
        .await;
    }

    let _ = app.emit(
        "claude-session",
        ClaudeSession {
            employee_id: employee_id.to_string(),
            task_id: task_id.cloned(),
            session_kind: session_kind.as_str().to_string(),
            session_record_id: session_record_id.to_string(),
            session_id: cli_session_id,
        },
    );
}

async fn emit_claude_output_line(
    app: &AppHandle,
    pool: &sqlx::SqlitePool,
    employee_id: &str,
    task_id: Option<&String>,
    session_kind: ClaudeSessionKind,
    session_record_id: &str,
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
            task_id: task_id.cloned(),
            session_kind: session_kind.as_str().to_string(),
            session_record_id: session_record_id.to_string(),
            session_event_id: event_id,
            line,
        },
    );
}

pub(super) fn spawn_claude_session_runtime(
    app: AppHandle,
    manager_state: Arc<tokio::sync::Mutex<ClaudeManager>>,
    child_arc: Arc<tokio::sync::Mutex<ClaudeChild>>,
    session_record_id: String,
    employee_id: String,
    task_id: Option<String>,
    task_git_context_id: Option<String>,
    session_kind: ClaudeSessionKind,
    provider: ClaudeExecutionProvider,
    execution_change_baseline: Option<ExecutionChangeBaseline>,
    sdk_file_change_store: SdkFileChangeStore,
    _run_cwd: String,
) {
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let captured_output = (session_kind == ClaudeSessionKind::Review)
            .then(|| Arc::new(Mutex::new(Vec::<String>::new())));
        let stdout = {
            let mut child = child_arc.lock().await;
            child.take_stdout()
        };
        let stderr = {
            let mut child = child_arc.lock().await;
            child.take_stderr()
        };

        let stdout_handle = if let Some(stdout) = stdout {
            let app = app_clone.clone();
            let session_id = session_record_id.clone();
            let emp_id = employee_id.clone();
            let t_id = task_id.clone();
            let sk = session_kind;
            let store = sdk_file_change_store.clone();
            let captured = captured_output.clone();
            let mut cli_json_state =
                (provider == ClaudeExecutionProvider::Cli).then(ClaudeCliJsonStreamState::default);

            Some(tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                let pool = match sqlite_pool(&app).await {
                    Ok(p) => p,
                    Err(_) => return,
                };

                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(session_id_value) = extract_session_id(&line, SESSION_ID_PREFIX) {
                        bind_claude_session_id(
                            &app,
                            &emp_id,
                            t_id.as_ref(),
                            sk,
                            &session_id,
                            session_id_value,
                        )
                        .await;
                        continue;
                    }

                    if let Some(event) =
                        parse_claude_file_change_event(&line, CLAUDE_FILE_CHANGE_EVENT_PREFIX)
                    {
                        upsert_sdk_file_change_event(&store, event);
                        continue;
                    }

                    if let Some(state) = cli_json_state.as_mut() {
                        if let Some(parsed) = parse_claude_cli_json_event_line(&line, state) {
                            if let Some(session_id_value) = parsed.session_id {
                                bind_claude_session_id(
                                    &app,
                                    &emp_id,
                                    t_id.as_ref(),
                                    sk,
                                    &session_id,
                                    session_id_value,
                                )
                                .await;
                            }

                            for event in parsed.file_change_events {
                                upsert_sdk_file_change_event(&store, event);
                            }

                            for emitted_line in parsed.lines {
                                if let Some(captured) = captured.as_ref() {
                                    push_captured_line(captured, emitted_line.clone());
                                }
                                emit_claude_output_line(
                                    &app,
                                    &pool,
                                    &emp_id,
                                    t_id.as_ref(),
                                    sk,
                                    &session_id,
                                    emitted_line,
                                )
                                .await;
                            }
                            continue;
                        }
                    }

                    if let Some(captured) = captured.as_ref() {
                        push_captured_line(captured, line.clone());
                    }

                    emit_claude_output_line(
                        &app,
                        &pool,
                        &emp_id,
                        t_id.as_ref(),
                        sk,
                        &session_id,
                        line,
                    )
                    .await;
                }
            }))
        } else {
            None
        };

        let stderr_handle = if let Some(stderr) = stderr {
            let app = app_clone.clone();
            let session_id = session_record_id.clone();
            let emp_id = employee_id.clone();
            let t_id = task_id.clone();
            let sk = session_kind;
            let captured = captured_output.clone();

            Some(tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                let pool = match sqlite_pool(&app).await {
                    Ok(p) => p,
                    Err(_) => return,
                };

                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(captured) = captured.as_ref() {
                        push_captured_line(captured, line.clone());
                    }

                    let _ =
                        insert_codex_session_event(&pool, &session_id, "stderr", Some(&line)).await;

                    let _ = app.emit(
                        "claude-stdout",
                        ClaudeOutput {
                            employee_id: emp_id.clone(),
                            task_id: t_id.clone(),
                            session_kind: sk.as_str().to_string(),
                            session_record_id: session_id.clone(),
                            session_event_id: None,
                            line: format!("[STDERR] {line}"),
                        },
                    );
                }
            }))
        } else {
            None
        };

        if let Some(handle) = stdout_handle {
            let _ = handle.await;
        }
        if let Some(handle) = stderr_handle {
            let _ = handle.await;
        }

        let exit_code = {
            let mut child = child_arc.lock().await;
            let mut attempts = 0;
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => break status.code(),
                    Ok(None) => {
                        attempts += 1;
                        if attempts >= STOP_WAIT_MAX_ATTEMPTS {
                            let _ = child.kill().await;
                            break child
                                .try_wait()
                                .ok()
                                .flatten()
                                .and_then(|status| status.code());
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(STOP_WAIT_POLL_MS))
                            .await;
                    }
                    Err(_) => break None,
                }
            }
        };

        {
            let mut manager = manager_state.lock().await;
            if let Some(process) = manager.remove_process(&session_record_id) {
                for path in process.cleanup_paths {
                    let _ = std::fs::remove_file(path);
                }
            }
        }

        let current_status = fetch_codex_session_by_id(&app_clone, &session_record_id)
            .await
            .ok()
            .map(|record| record.status);
        let final_status = resolve_final_claude_status(current_status.as_deref(), exit_code);
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

        crate::codex::persist_external_execution_change_history(
            &app_clone,
            &session_record_id,
            claude_session_kind_to_codex(session_kind),
            claude_provider_to_codex(provider),
            execution_change_baseline.as_ref(),
            Some(&sdk_file_change_store),
        )
        .await;

        if let Some(task_git_context_id) = task_git_context_id.as_deref() {
            if let Ok(pool) = sqlite_pool(&app_clone).await {
                let success = final_status == "exited" && exit_code == Some(0);
                let failure_message = (!success)
                    .then(|| format!("进程退出，exit_code={}", exit_code.unwrap_or_default()));
                let _ = mark_task_git_context_session_finished(
                    &pool,
                    task_git_context_id,
                    success,
                    failure_message.as_deref(),
                )
                .await;
            }
        }

        if let Ok(pool) = sqlite_pool(&app_clone).await {
            if session_kind == ClaudeSessionKind::Review {
                let raw_output = captured_output
                    .as_ref()
                    .map(|captured| captured.lock().unwrap().join("\n"))
                    .unwrap_or_default();
                if let Some(verdict_raw) = extract_review_verdict(&raw_output) {
                    if parse_review_verdict_json(&verdict_raw).is_ok() {
                        let _ = insert_codex_session_event(
                            &pool,
                            &session_record_id,
                            "review_verdict",
                            Some(&verdict_raw),
                        )
                        .await;
                    }
                }
                if let Some(report) = extract_review_report(&raw_output) {
                    let _ = insert_codex_session_event(
                        &pool,
                        &session_record_id,
                        "review_report",
                        Some(&report),
                    )
                    .await;
                }
            }

            let exit_msg = format!(
                "Claude 会话已结束，退出码: {}",
                exit_code.map_or("未知".to_string(), |code| code.to_string())
            );
            let event_id = insert_codex_session_event_with_id(
                &pool,
                &session_record_id,
                if final_status == "exited" {
                    "session_exited"
                } else {
                    "session_failed"
                },
                Some(&exit_msg),
            )
            .await
            .ok();

            task_automation::handle_session_exit_blocking(
                app_clone.clone(),
                session_record_id.clone(),
            )
            .await;

            let _ = app_clone.emit(
                "claude-exit",
                ClaudeExit {
                    employee_id: employee_id.clone(),
                    task_id: task_id.clone(),
                    session_kind: session_kind.as_str().to_string(),
                    session_record_id: session_record_id.clone(),
                    session_event_id: event_id,
                    status: final_status.to_string(),
                    line: Some(if final_status == "exited" {
                        format!("[EXIT] {exit_msg}")
                    } else {
                        format!("[ERROR] {exit_msg}")
                    }),
                    code: exit_code,
                },
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_final_status_keeps_successful_executions_automation_eligible() {
        assert_eq!(
            resolve_final_claude_status(Some("running"), Some(0)),
            "exited"
        );
        assert_eq!(resolve_final_claude_status(None, Some(0)), "exited");
    }

    #[test]
    fn resolve_final_status_preserves_stopping_as_exited_without_success() {
        assert_eq!(
            resolve_final_claude_status(Some("stopping"), Some(143)),
            "exited"
        );
    }

    #[test]
    fn resolve_final_status_marks_failed_processes_failed() {
        assert_eq!(
            resolve_final_claude_status(Some("running"), Some(1)),
            "failed"
        );
        assert_eq!(resolve_final_claude_status(Some("running"), None), "failed");
    }
}
