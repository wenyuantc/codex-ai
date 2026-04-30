use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{ChildStderr, ChildStdout};
use tokio::time::{sleep, Duration};

use super::*;
use crate::git_workflow::mark_task_git_context_session_finished;
use crate::notifications::{publish_one_time_notification, NotificationDraft};

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

fn is_duplicate_line(seen: Option<&Arc<Mutex<HashSet<String>>>>, line: &str) -> bool {
    seen.is_some_and(|seen| {
        let mut entries = seen.lock().unwrap();
        if entries.contains(line) {
            true
        } else {
            entries.insert(line.to_string());
            if entries.len() > 200 {
                entries.clear();
            }
            false
        }
    })
}

fn spawn_stdout_reader(
    app: AppHandle,
    pool: SqlitePool,
    employee_id: String,
    task_id: Option<String>,
    session_kind: CodexSessionKind,
    session_record_id: String,
    stdout: ChildStdout,
    seen: Option<Arc<Mutex<HashSet<String>>>>,
    cli_json_stream_state: Option<Arc<Mutex<CliJsonStreamState>>>,
    captured_output: Option<Arc<Mutex<Vec<String>>>>,
    session_emitted: Arc<AtomicBool>,
    sdk_file_change_store: Option<SdkFileChangeStore>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        loop {
            let line = match lines.next_line().await {
                Ok(Some(line)) => line,
                Ok(None) => break,
                Err(error) => {
                    let error_line = format!("[ERROR] 读取远程 stdout 失败: {}", error);
                    let session_event_id = insert_codex_session_event_with_id(
                        &pool,
                        &session_record_id,
                        "stdout_read_failed",
                        Some(&error_line),
                    )
                    .await
                    .ok();
                    let _ = app.emit(
                        "codex-stdout",
                        CodexOutput {
                            employee_id: employee_id.clone(),
                            task_id: task_id.clone(),
                            session_kind: session_kind.as_str().to_string(),
                            session_record_id: session_record_id.clone(),
                            session_event_id,
                            line: error_line,
                        },
                    );
                    break;
                }
            };

            if let Some(event) = parse_sdk_file_change_event(&line) {
                if let Some(store) = sdk_file_change_store.as_ref() {
                    upsert_sdk_file_change_event(store, event);
                }
                continue;
            }

            if let Some(cli_json_state) = cli_json_stream_state.as_ref() {
                if let Some(parsed) = {
                    let mut state = cli_json_state.lock().unwrap();
                    parse_cli_json_event_line(&line, &mut state)
                } {
                    if !session_emitted.load(Ordering::Relaxed) {
                        if let Some(session_id) = parsed.session_id {
                            if !session_emitted.swap(true, Ordering::Relaxed) {
                                bind_cli_session_id(
                                    &app,
                                    &employee_id,
                                    task_id.as_ref(),
                                    &session_record_id,
                                    session_kind,
                                    session_id,
                                )
                                .await;
                            }
                        }
                    }

                    for emitted_line in parsed.lines {
                        if let Some(captured_output) = captured_output.as_ref() {
                            push_captured_line(captured_output, emitted_line.clone());
                        }
                        emit_session_terminal_line(
                            &app,
                            &pool,
                            &session_record_id,
                            &employee_id,
                            task_id.as_deref(),
                            session_kind,
                            emitted_line,
                        )
                        .await;
                    }
                    continue;
                }
            }

            if !session_emitted.load(Ordering::Relaxed) {
                if let Some(session_id) = extract_session_id_from_output(&line) {
                    if !session_emitted.swap(true, Ordering::Relaxed) {
                        bind_cli_session_id(
                            &app,
                            &employee_id,
                            task_id.as_ref(),
                            &session_record_id,
                            session_kind,
                            session_id,
                        )
                        .await;
                    }
                }
            }

            if !is_duplicate_line(seen.as_ref(), &line) {
                if let Some(captured_output) = captured_output.as_ref() {
                    push_captured_line(captured_output, line.clone());
                }
                let session_event_id = insert_codex_session_event_with_id(
                    &pool,
                    &session_record_id,
                    "stdout",
                    Some(&line),
                )
                .await
                .ok();
                let _ = app.emit(
                    "codex-stdout",
                    CodexOutput {
                        employee_id: employee_id.clone(),
                        task_id: task_id.clone(),
                        session_kind: session_kind.as_str().to_string(),
                        session_record_id: session_record_id.clone(),
                        session_event_id,
                        line,
                    },
                );
            }
        }
    })
}

fn spawn_stderr_reader(
    app: AppHandle,
    pool: SqlitePool,
    employee_id: String,
    task_id: Option<String>,
    session_kind: CodexSessionKind,
    session_record_id: String,
    stderr: ChildStderr,
    seen: Option<Arc<Mutex<HashSet<String>>>>,
    captured_output: Option<Arc<Mutex<Vec<String>>>>,
    sdk_file_change_store: Option<SdkFileChangeStore>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        loop {
            let line = match lines.next_line().await {
                Ok(Some(line)) => line,
                Ok(None) => break,
                Err(error) => {
                    let error_line = format!("[ERROR] 读取远程 stderr 失败: {}", error);
                    let session_event_id = insert_codex_session_event_with_id(
                        &pool,
                        &session_record_id,
                        "stderr_read_failed",
                        Some(&error_line),
                    )
                    .await
                    .ok();
                    let _ = app.emit(
                        "codex-stdout",
                        CodexOutput {
                            employee_id: employee_id.clone(),
                            task_id: task_id.clone(),
                            session_kind: session_kind.as_str().to_string(),
                            session_record_id: session_record_id.clone(),
                            session_event_id,
                            line: error_line,
                        },
                    );
                    break;
                }
            };

            if let Some(event) = parse_sdk_file_change_event(&line) {
                if let Some(store) = sdk_file_change_store.as_ref() {
                    upsert_sdk_file_change_event(store, event);
                }
                continue;
            }

            if !is_duplicate_line(seen.as_ref(), &line) {
                if let Some(captured_output) = captured_output.as_ref() {
                    push_captured_line(captured_output, line.clone());
                }
                let session_event_id = insert_codex_session_event_with_id(
                    &pool,
                    &session_record_id,
                    "stderr",
                    Some(&line),
                )
                .await
                .ok();
                let _ = app.emit(
                    "codex-stdout",
                    CodexOutput {
                        employee_id: employee_id.clone(),
                        task_id: task_id.clone(),
                        session_kind: session_kind.as_str().to_string(),
                        session_record_id: session_record_id.clone(),
                        session_event_id,
                        line,
                    },
                );
            }
        }
    })
}

fn spawn_exit_watcher(
    app: AppHandle,
    pool: SqlitePool,
    employee_id: String,
    task_id: Option<String>,
    session_kind: CodexSessionKind,
    session_record_id: String,
    run_cwd: String,
    child_handle: Arc<tokio::sync::Mutex<CodexChild>>,
    provider: CodexExecutionProvider,
    session_lookup_started_at: Option<SystemTime>,
    session_emitted: Arc<AtomicBool>,
    captured_output: Option<Arc<Mutex<Vec<String>>>>,
    execution_change_baseline: Option<ExecutionChangeBaseline>,
    sdk_file_change_store: Option<SdkFileChangeStore>,
    stdout_reader: tauri::async_runtime::JoinHandle<()>,
    stderr_reader: tauri::async_runtime::JoinHandle<()>,
) {
    tauri::async_runtime::spawn(async move {
        let exit_code = loop {
            let maybe_status = {
                let mut child = child_handle.lock().await;
                child.try_wait()
            };

            match maybe_status {
                Ok(Some(code)) => break Some(code),
                Ok(None) => sleep(Duration::from_millis(200)).await,
                Err(error) => {
                    let ended_at = now_sqlite();
                    let exit_line = Some(format!("[ERROR] {}", error.trim()));
                    let _ = update_codex_session_record(
                        &app,
                        &session_record_id,
                        Some("failed"),
                        None,
                        None,
                        Some(Some(ended_at.as_str())),
                    )
                    .await;
                    let session_event_id = insert_codex_session_event_with_id(
                        &pool,
                        &session_record_id,
                        "session_failed",
                        Some(&error),
                    )
                    .await
                    .ok();
                    persist_execution_change_history(
                        &app,
                        &session_record_id,
                        session_kind,
                        provider,
                        execution_change_baseline.as_ref(),
                        sdk_file_change_store.as_ref(),
                    )
                    .await;
                    {
                        let manager = app.state::<Arc<Mutex<CodexManager>>>();
                        let mut manager = manager.lock().unwrap();
                        let removed = manager.remove_process(&session_record_id);
                        if let Some(process) = removed.as_ref() {
                            cleanup_process_artifacts(&process.cleanup_paths);
                        }
                    }
                    let _ = app.emit(
                        "codex-exit",
                        CodexExit {
                            employee_id: employee_id.clone(),
                            task_id: task_id.clone(),
                            session_kind: session_kind.as_str().to_string(),
                            session_record_id: session_record_id.clone(),
                            session_event_id,
                            line: exit_line,
                            code: None,
                        },
                    );
                    task_automation::handle_session_exit_blocking(
                        app.clone(),
                        session_record_id.clone(),
                    )
                    .await;
                    return;
                }
            }
        };

        {
            let manager = app.state::<Arc<Mutex<CodexManager>>>();
            let mut manager = manager.lock().unwrap();
            let removed = manager.remove_process(&session_record_id);
            if let Some(process) = removed.as_ref() {
                cleanup_process_artifacts(&process.cleanup_paths);
            }
        }

        if provider == CodexExecutionProvider::Cli
            && !session_emitted.load(Ordering::Relaxed)
            && session_lookup_started_at.is_some()
        {
            if let Some(session_id) = find_latest_exec_session_id(
                &run_cwd,
                session_lookup_started_at.unwrap_or(SystemTime::now()),
            ) {
                bind_cli_session_id(
                    &app,
                    &employee_id,
                    task_id.as_ref(),
                    &session_record_id,
                    session_kind,
                    session_id,
                )
                .await;
            }
        }

        let _ = stdout_reader.await;
        let _ = stderr_reader.await;

        let final_status = match fetch_codex_session_by_id(&app, &session_record_id).await {
            Ok(record) if record.status == "stopping" => "exited",
            Ok(_) if exit_code == Some(0) => "exited",
            Ok(_) => "failed",
            Err(_) if exit_code == Some(0) => "exited",
            Err(_) => "failed",
        };
        let ended_at = now_sqlite();
        let _ = update_codex_session_record(
            &app,
            &session_record_id,
            Some(final_status),
            None,
            Some(exit_code),
            Some(Some(ended_at.as_str())),
        )
        .await;
        if session_kind == CodexSessionKind::Execution {
            if let Ok(session) = fetch_codex_session_by_id(&app, &session_record_id).await {
                if let Some(task_git_context_id) = session.task_git_context_id.as_deref() {
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
        }
        persist_execution_change_history(
            &app,
            &session_record_id,
            session_kind,
            provider,
            execution_change_baseline.as_ref(),
            sdk_file_change_store.as_ref(),
        )
        .await;
        let message = format!("进程退出，exit_code={}", exit_code.unwrap_or_default());
        let exit_line = Some(if final_status == "exited" {
            format!("[EXIT] {}", message.trim())
        } else {
            format!("[ERROR] {}", message.trim())
        });
        let mut session_event_id = insert_codex_session_event_with_id(
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

        if session_kind == CodexSessionKind::Review {
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
            let report = extract_review_report(&raw_output);
            if let Some(report) = report.as_ref() {
                let _ = insert_codex_session_event(
                    &pool,
                    &session_record_id,
                    "review_report",
                    Some(report),
                )
                .await;
            }
            if let Some(task_id) = task_id.as_deref() {
                let detail = match report.as_ref() {
                    Some(report) => {
                        let preview = report
                            .lines()
                            .take(3)
                            .collect::<Vec<_>>()
                            .join(" ")
                            .trim()
                            .to_string();
                        if preview.is_empty() {
                            "代码审核完成".to_string()
                        } else {
                            preview
                        }
                    }
                    None if final_status == "exited" => {
                        "代码审核完成，但未提取到结构化报告".to_string()
                    }
                    None => format!("代码审核失败，exit_code={}", exit_code.unwrap_or_default()),
                };
                let action = if final_status == "exited" {
                    "task_review_completed"
                } else {
                    "task_review_failed"
                };
                let project_id = fetch_task_activity_context(&pool, task_id)
                    .await
                    .ok()
                    .map(|(_, project_id)| project_id);
                let _ = insert_activity_log(
                    &pool,
                    action,
                    &detail,
                    Some(&employee_id),
                    Some(task_id),
                    project_id.as_deref(),
                )
                .await;
            }
        }

        if session_kind == CodexSessionKind::Execution {
            if let Some(task_id) = task_id.as_deref() {
                let task_context = sqlx::query_as::<_, (String, String, Option<String>)>(
                    "SELECT title, project_id, automation_mode FROM tasks WHERE id = $1 AND deleted_at IS NULL LIMIT 1",
                )
                .bind(task_id)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten();
                let task_title = task_context
                    .as_ref()
                    .map(|(title, _, _)| title.clone())
                    .unwrap_or_else(|| task_id.to_string());
                let project_id = task_context
                    .as_ref()
                    .map(|(_, project_id, _)| project_id.clone());
                let automation_mode = task_context
                    .as_ref()
                    .and_then(|(_, _, automation_mode)| automation_mode.as_deref());

                if final_status != "exited" {
                    let summary = exit_line.clone().unwrap_or_else(|| {
                        format!("任务执行失败，exit_code={}", exit_code.unwrap_or_default())
                    });
                    let draft = NotificationDraft::one_time(
                        "run_failed",
                        "error",
                        "任务执行",
                        "任务运行失败",
                        format!("任务“{}”执行失败：{}", task_title, summary),
                    )
                    .with_recommendation(
                        "请进入任务详情或会话日志查看失败原因，然后决定重试或修复。",
                    )
                    .with_action("查看任务", format!("/kanban?taskId={task_id}"))
                    .with_related_object("task", task_id)
                    .with_task_id(task_id);
                    let draft = if let Some(project_id) = project_id.as_deref() {
                        draft.with_project_id(project_id)
                    } else {
                        draft
                    };

                    let _ = publish_one_time_notification(&app, draft).await;
                } else if automation_mode.is_none() {
                    let draft = NotificationDraft::one_time(
                        "run_completed",
                        "success",
                        "任务执行",
                        "任务运行完成",
                        format!("任务“{}”已运行完成。", task_title),
                    )
                    .with_recommendation("点击通知可直接查看任务详情与执行记录。")
                    .with_action("查看任务", format!("/kanban?taskId={task_id}"))
                    .with_related_object("task", task_id)
                    .with_task_id(task_id);
                    let draft = if let Some(project_id) = project_id.as_deref() {
                        draft.with_project_id(project_id)
                    } else {
                        draft
                    };

                    let _ = publish_one_time_notification(&app, draft).await;
                }
            }
        }

        task_automation::handle_session_exit_blocking(app.clone(), session_record_id.clone()).await;

        let _ = app.emit(
            "codex-exit",
            CodexExit {
                employee_id,
                task_id,
                session_kind: session_kind.as_str().to_string(),
                session_record_id,
                session_event_id: session_event_id.take(),
                line: exit_line,
                code: exit_code,
            },
        );
    });
}

pub(super) async fn attach_session_runtime_tasks(
    app: &AppHandle,
    pool: &SqlitePool,
    employee_id: &str,
    task_id: Option<String>,
    session_record_id: String,
    session_kind: CodexSessionKind,
    provider: CodexExecutionProvider,
    resume_session_id: Option<String>,
    session_lookup_started_at: Option<SystemTime>,
    run_cwd: String,
    stdout: ChildStdout,
    stderr: ChildStderr,
    child_handle: Arc<tokio::sync::Mutex<CodexChild>>,
    execution_change_baseline: Option<ExecutionChangeBaseline>,
    sdk_file_change_store: Option<SdkFileChangeStore>,
    cli_json_output_flag: Option<CliJsonOutputFlag>,
) {
    let session_emitted = Arc::new(AtomicBool::new(false));

    if let Some(session_id) = resume_session_id {
        session_emitted.store(true, Ordering::Relaxed);
        bind_cli_session_id(
            app,
            employee_id,
            task_id.as_ref(),
            &session_record_id,
            session_kind,
            session_id,
        )
        .await;
    } else if provider == CodexExecutionProvider::Cli && session_lookup_started_at.is_some() {
        let app_clone = app.clone();
        let employee_id = employee_id.to_string();
        let task_id_clone = task_id.clone();
        let run_cwd_clone = run_cwd.clone();
        let session_emitted_clone = session_emitted.clone();
        let session_record_id_clone = session_record_id.clone();
        let lookup_started_at = session_lookup_started_at.unwrap_or(SystemTime::now());
        tauri::async_runtime::spawn(async move {
            if let Some(session_id) =
                wait_for_exec_session_id(&run_cwd_clone, lookup_started_at).await
            {
                if !session_emitted_clone.swap(true, Ordering::Relaxed) {
                    bind_cli_session_id(
                        &app_clone,
                        &employee_id,
                        task_id_clone.as_ref(),
                        &session_record_id_clone,
                        session_kind,
                        session_id,
                    )
                    .await;
                }
            }
        });
    }

    let seen = (provider == CodexExecutionProvider::Cli && cli_json_output_flag.is_none())
        .then(|| Arc::new(Mutex::new(HashSet::<String>::new())));
    let cli_json_stream_state = (provider == CodexExecutionProvider::Cli
        && cli_json_output_flag.is_some())
    .then(|| Arc::new(Mutex::new(CliJsonStreamState::default())));
    let captured_output = (session_kind == CodexSessionKind::Review)
        .then(|| Arc::new(Mutex::new(Vec::<String>::new())));

    let stdout_reader = spawn_stdout_reader(
        app.clone(),
        pool.clone(),
        employee_id.to_string(),
        task_id.clone(),
        session_kind,
        session_record_id.clone(),
        stdout,
        seen.clone(),
        cli_json_stream_state,
        captured_output.clone(),
        session_emitted.clone(),
        sdk_file_change_store.clone(),
    );

    let stderr_reader = spawn_stderr_reader(
        app.clone(),
        pool.clone(),
        employee_id.to_string(),
        task_id.clone(),
        session_kind,
        session_record_id.clone(),
        stderr,
        seen,
        captured_output.clone(),
        sdk_file_change_store.clone(),
    );

    spawn_exit_watcher(
        app.clone(),
        pool.clone(),
        employee_id.to_string(),
        task_id,
        session_kind,
        session_record_id,
        run_cwd,
        child_handle,
        provider,
        session_lookup_started_at,
        session_emitted,
        captured_output,
        execution_change_baseline,
        sdk_file_change_store,
        stdout_reader,
        stderr_reader,
    );
}
