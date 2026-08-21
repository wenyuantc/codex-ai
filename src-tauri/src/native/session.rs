use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::sync::{mpsc, Mutex};

use crate::app::{
    apply_codex_session_usage, fetch_employee_by_id, fetch_ssh_config_record_by_id,
    insert_activity_log, insert_codex_session_event, insert_codex_session_event_with_id,
    insert_codex_session_record, now_sqlite, persist_review_session_events_from_session_logs,
    sqlite_pool, update_codex_session_record, validate_runtime_working_dir, EXECUTION_TARGET_LOCAL,
    EXECUTION_TARGET_SSH,
};
use crate::codex::{CodexExecutionProvider, CodexSessionKind, ExecutionChangeBaseline};
use crate::db::models::ChannelModelConfig;
use crate::db::models::{CodexExit, CodexOutput, CodexSession, SshConfigRecord};
use crate::engine::context::resolve_session_execution_context;
use crate::native::agent::r#loop::AgentRunner;
use crate::native::channels::{fetch_channel_record, require_channel_api_key};
use crate::native::manager::{
    NativeAgentManager, NativeFollowup, NativeLiveSession, NativeSessionInfo,
};
use crate::native::model::{ModelClient, ModelClientConfig, RetryConfig};
use crate::native::model_catalog::{apply_catalog_defaults, fill_from_catalog};
use crate::native::protocol::record_to_channel;
use crate::native::tools::{local::LocalWorkspace, ssh::SshToolRuntime};

const ENGINE_LABEL: &str = "内置 Agent";

fn session_kind(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some("review") => "review".to_string(),
        _ => "execution".to_string(),
    }
}

fn native_kind_to_codex(kind: &str) -> CodexSessionKind {
    match kind {
        "review" => CodexSessionKind::Review,
        _ => CodexSessionKind::Execution,
    }
}

fn should_capture_native_execution_changes(kind: &str) -> bool {
    kind == "execution"
}

async fn capture_native_execution_change_baseline(
    app: &AppHandle,
    pool: &sqlx::SqlitePool,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    kind: &str,
    execution_target: &str,
    run_cwd: &str,
    ssh_config: Option<&SshConfigRecord>,
) -> Option<ExecutionChangeBaseline> {
    if !should_capture_native_execution_changes(kind) {
        return None;
    }

    if execution_target == EXECUTION_TARGET_SSH {
        emit_native_line(
            app,
            session_record_id,
            employee_id,
            task_id,
            kind,
            "[SSH] 正在采集远程仓库基线，用于展示本次内置 Agent 会话改动...".to_string(),
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
                emit_native_line(
                    app,
                    session_record_id,
                    employee_id,
                    task_id,
                    kind,
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
            emit_native_line(
                app,
                session_record_id,
                employee_id,
                task_id,
                kind,
                format!(
                    "[WARN] 内置 Agent 会话文件基线采集失败，文件详情将退化为最佳努力快照: {error}"
                ),
            )
            .await;
            None
        }
    }
}

fn extra_headers_map(raw: Option<&str>) -> HashMap<String, String> {
    let Some(text) = raw.filter(|item| !item.trim().is_empty()) else {
        return HashMap::new();
    };
    serde_json::from_str::<HashMap<String, String>>(text).unwrap_or_default()
}

async fn emit_native_line(
    app: &AppHandle,
    session_record_id: &str,
    employee_id: &str,
    task_id: Option<&str>,
    session_kind: &str,
    line: String,
) {
    let pool = match sqlite_pool(app).await {
        Ok(pool) => pool,
        Err(_) => return,
    };
    let event_id =
        insert_codex_session_event_with_id(&pool, session_record_id, "stdout", Some(&line))
            .await
            .ok();
    let _ = app.emit(
        "native-stdout",
        CodexOutput {
            employee_id: employee_id.to_string(),
            task_id: task_id.map(ToOwned::to_owned),
            session_kind: session_kind.to_string(),
            session_record_id: session_record_id.to_string(),
            session_event_id: event_id,
            line,
        },
    );
}

struct NativeRunSettings {
    client: ModelClient,
    model: String,
    effort: Option<String>,
    max_output_tokens: Option<u32>,
    thinking_enabled: bool,
    context_tokens: Option<u32>,
    employee_system_prompt: Option<String>,
}

fn resolve_run_model_config(
    channel_models: &[ChannelModelConfig],
    model: &str,
) -> ChannelModelConfig {
    let mut config = channel_models
        .iter()
        .find(|item| item.id == model)
        .cloned()
        .unwrap_or_else(|| apply_catalog_defaults(model));
    fill_from_catalog(&mut config);
    config
}

async fn load_native_client(
    pool: &sqlx::SqlitePool,
    employee: &crate::db::models::Employee,
) -> Result<NativeRunSettings, String> {
    if employee.ai_provider != "native" {
        return Err("员工不是内置 Agent".to_string());
    }
    let channel_id = employee
        .ai_channel_id
        .as_deref()
        .ok_or_else(|| "请先为内置 Agent 员工配置渠道".to_string())?;
    let record = fetch_channel_record(pool, channel_id).await?;
    if record.enabled == 0 {
        return Err(format!("渠道「{}」已停用", record.name));
    }
    let (record, api_key) = require_channel_api_key(pool, record).await?;
    let channel = record_to_channel(record)?;
    let model = if employee.model.trim().is_empty() {
        channel
            .models
            .first()
            .map(|item| item.id.clone())
            .unwrap_or_else(|| "default".to_string())
    } else {
        employee.model.clone()
    };
    let model_config = resolve_run_model_config(&channel.models, &model);
    let thinking_enabled = model_config.thinking_enabled.unwrap_or(false);
    let effort = if thinking_enabled {
        let from_employee = employee.reasoning_effort.trim();
        if from_employee.is_empty() {
            model_config.thinking_level.clone()
        } else {
            Some(from_employee.to_string())
        }
    } else {
        None
    };
    let client = ModelClient::new(ModelClientConfig {
        protocol: channel.protocol.clone(),
        base_url: channel.base_url.clone(),
        api_key,
        extra_headers: extra_headers_map(channel.extra_headers_json.as_deref()),
        retry: RetryConfig::default(),
        timeout: Duration::from_secs(120),
    })?;
    Ok(NativeRunSettings {
        client,
        model,
        effort,
        max_output_tokens: model_config.max_output_tokens,
        thinking_enabled,
        context_tokens: model_config.context_tokens,
        employee_system_prompt: None,
    })
}

fn native_one_shot_text(content: &str) -> Result<String, String> {
    let text = content.trim();
    if text.is_empty() {
        Err("内置 Agent 未返回可用内容".to_string())
    } else {
        Ok(text.to_string())
    }
}

/// Employee-scoped one-shot (coordinator plan, tester acceptance). HTTP only, no
/// tool loop and no Codex SDK/exec fallback. Images stay on this machine.
pub async fn run_native_one_shot<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: &str,
    prompt: String,
    image_paths: Option<Vec<String>>,
    model_override: Option<String>,
    reasoning_effort_override: Option<String>,
) -> Result<String, String> {
    let pool = sqlite_pool(app).await?;
    let mut employee = fetch_employee_by_id(&pool, employee_id).await?;
    if let Some(model) = model_override
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        employee.model = model.to_string();
    }
    if let Some(effort) = reasoning_effort_override
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        employee.reasoning_effort = effort.to_string();
    }
    let run = load_native_client(&pool, &employee).await?;
    let loaded = crate::native::images::load_native_images(image_paths.as_deref());
    for path in &loaded.missing {
        eprintln!("[native] one-shot 附件图片不存在，已跳过: {path}");
    }
    for reason in &loaded.skipped {
        eprintln!("[native] one-shot 跳过图片: {reason}");
    }
    let user = if loaded.images.is_empty() {
        crate::native::model::types::Message::user(prompt)
    } else {
        crate::native::model::types::Message::user_with_images(prompt, loaded.images)
    };
    let (message, _usage) = run
        .client
        .chat(crate::native::model::client::ChatRequest {
            messages: &[user],
            tools: &[],
            model: &run.model,
            effort: run.effort.as_deref(),
            max_output_tokens: run.max_output_tokens,
            thinking_enabled: run.thinking_enabled,
        })
        .await
        .map_err(|error| format!("内置 Agent 一次性调用失败：{error}"))?;
    native_one_shot_text(&message.content)
}

#[tauri::command]
pub async fn start_native_session(
    app: AppHandle,
    state: State<'_, Arc<Mutex<NativeAgentManager>>>,
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
) -> Result<crate::run_queue::StartSessionOutcome, String> {
    let mut reservation = None;
    if crate::run_queue::should_gate_task_run(
        task_id.as_deref(),
        resume_session_id.as_deref(),
        session_kind.as_deref(),
    ) {
        let gated_task_id = task_id.clone().expect("gated run has task_id");
        let run = crate::run_queue::QueuedTaskRun {
            provider: "native".to_string(),
            employee_id: employee_id.clone(),
            task_description: task_description.clone(),
            model: model.clone(),
            reasoning_effort: reasoning_effort.clone(),
            system_prompt: system_prompt.clone(),
            working_dir: working_dir.clone(),
            task_git_context_id: task_git_context_id.clone(),
            image_paths: image_paths.clone(),
        };
        match crate::run_queue::gate_or_enqueue(&app, &gated_task_id, run).await? {
            crate::run_queue::GateOutcome::Queued { position } => {
                return Ok(crate::run_queue::StartSessionOutcome::Queued { position });
            }
            crate::run_queue::GateOutcome::Proceed(slot) => reservation = Some(slot),
        }
    }

    let result = start_native_with_manager(
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
    .await;
    drop(reservation);
    result.map(|_| crate::run_queue::StartSessionOutcome::Started)
}

#[allow(clippy::too_many_arguments)]
pub async fn start_native_with_manager(
    app: AppHandle,
    manager_state: Arc<Mutex<NativeAgentManager>>,
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
    session_kind_arg: Option<String>,
) -> Result<(), String> {
    let kind = session_kind(session_kind_arg.as_deref());
    {
        let manager = manager_state.lock().await;
        if let Some(task_id) = task_id.as_deref() {
            if manager.get_task_process_any(task_id, &kind).is_some() {
                return Err(format!("任务{task_id}的{kind}会话已在运行"));
            }
        } else if manager.has_employee_processes(&employee_id) {
            return Err(format!(
                "员工{employee_id}已有未绑定任务的内置 Agent 会话在运行"
            ));
        }
    }

    let execution_context = resolve_session_execution_context(
        &app,
        task_id.as_deref(),
        working_dir.as_deref(),
        ENGINE_LABEL,
    )
    .await?;
    let run_cwd = if execution_context.execution_target == EXECUTION_TARGET_LOCAL {
        validate_runtime_working_dir(execution_context.working_dir.as_deref())?
    } else {
        execution_context
            .working_dir
            .clone()
            .ok_or_else(|| "SSH 项目缺少远程仓库目录，无法启动内置 Agent。".to_string())?
    };

    let pool = sqlite_pool(&app).await?;
    let mut employee = fetch_employee_by_id(&pool, &employee_id).await?;
    if let Some(model) = model
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        employee.model = model.to_string();
    }
    if let Some(effort) = reasoning_effort
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        employee.reasoning_effort = effort.to_string();
    }
    let mut run = load_native_client(&pool, &employee).await?;
    run.employee_system_prompt = system_prompt
        .as_deref()
        .or(employee.system_prompt.as_deref())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned);
    let prompt = task_description;

    let session_record = insert_codex_session_record(
        &app,
        Some(&employee_id),
        task_id.as_deref(),
        task_git_context_id.as_deref(),
        Some(&run_cwd),
        resume_session_id.as_deref(),
        &kind,
        "running",
        &execution_context.execution_target,
        execution_context.ssh_config_id.as_deref(),
        execution_context.target_host_label.as_deref(),
        &execution_context.artifact_capture_mode,
        Some("native"),
        None,
    )
    .await?;

    insert_codex_session_event(
        &pool,
        &session_record.id,
        "session_requested",
        Some("内置 Agent 会话已创建"),
    )
    .await?;

    let ssh = if execution_context.execution_target == EXECUTION_TARGET_SSH {
        let ssh_id = execution_context
            .ssh_config_id
            .as_deref()
            .ok_or_else(|| "SSH 项目缺少 ssh_config_id".to_string())?;
        let config = fetch_ssh_config_record_by_id(&pool, ssh_id).await?;
        Some(SshToolRuntime {
            app: app.clone(),
            config,
            root: run_cwd.clone(),
        })
    } else {
        None
    };

    let execution_change_baseline = capture_native_execution_change_baseline(
        &app,
        &pool,
        &session_record.id,
        &employee_id,
        task_id.as_deref(),
        &kind,
        &execution_context.execution_target,
        &run_cwd,
        ssh.as_ref().map(|item| &item.config),
    )
    .await;

    let (followup_tx, followup_rx) = mpsc::channel(8);
    let cancel = crate::native::tools::CancelFlag::new();
    let session_record_id = session_record.id.clone();
    let employee_id_spawn = employee_id.clone();
    let task_id_spawn = task_id.clone();
    let kind_spawn = kind.clone();
    let manager_spawn = manager_state.clone();
    let app_spawn = app.clone();
    let await_followups = task_id.is_none();
    let cancel_run = cancel.clone();
    let join = tokio::spawn(async move {
        run_native_loop(
            app_spawn,
            manager_spawn,
            run,
            prompt,
            run_cwd,
            ssh,
            cancel_run,
            followup_rx,
            session_record_id,
            employee_id_spawn,
            task_id_spawn,
            kind_spawn,
            await_followups,
            image_paths,
            execution_change_baseline,
        )
        .await;
    });

    manager_state.lock().await.add_session(NativeLiveSession {
        info: NativeSessionInfo {
            employee_id: employee_id.clone(),
            task_id: task_id.clone(),
            session_kind: kind.clone(),
            session_record_id: session_record.id.clone(),
        },
        cancel,
        followup_tx,
        join,
    });

    let _ = app.emit(
        "native-session",
        CodexSession {
            employee_id: employee_id.clone(),
            task_id: task_id.clone(),
            session_kind: kind,
            session_record_id: session_record.id.clone(),
            session_id: session_record.id.clone(),
        },
    );

    if let Some(task_id) = task_id.as_deref() {
        let _ = insert_activity_log(
            &pool,
            "task_execution_started",
            "开始任务会话（内置 Agent）",
            Some(&employee_id),
            Some(task_id),
            session_record.project_id.as_deref(),
        )
        .await;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_native_loop(
    app: AppHandle,
    manager_state: Arc<Mutex<NativeAgentManager>>,
    run: NativeRunSettings,
    first_prompt: String,
    run_cwd: String,
    ssh: Option<SshToolRuntime>,
    cancel: crate::native::tools::CancelFlag,
    mut followup_rx: mpsc::Receiver<NativeFollowup>,
    session_record_id: String,
    employee_id: String,
    task_id: Option<String>,
    kind: String,
    await_followups: bool,
    image_paths: Option<Vec<String>>,
    execution_change_baseline: Option<ExecutionChangeBaseline>,
) {
    let mut runner = AgentRunner::new(LocalWorkspace::new(PathBuf::from(&run_cwd)));
    runner.ctx.ssh = ssh;
    runner.ctx.cancel = cancel.clone();
    runner.max_turns = crate::native::settings::effective_max_turns(&app);
    if let Some(context_tokens) = run.context_tokens {
        runner.context_char_limit = (context_tokens as usize).saturating_mul(3).max(8_000);
    }
    let system = crate::native::prompt::compose_system(&crate::native::prompt::NativePromptParts {
        cwd: run_cwd.clone(),
        model: run.model.clone(),
        platform: std::env::consts::OS.to_string(),
        git: if let Some(ssh) = runner.ctx.ssh.as_ref() {
            crate::native::prompt::detect_ssh_git(ssh).await
        } else {
            crate::native::prompt::detect_local_git(&run_cwd)
        },
        global_template: crate::native::prompt::load_global_template(&app),
        project_agents: if let Some(ssh) = runner.ctx.ssh.as_ref() {
            crate::native::prompt::read_ssh_project_agents(ssh).await
        } else {
            crate::native::prompt::read_local_project_agents(&run_cwd)
        },
        employee_prompt: run.employee_system_prompt.clone().unwrap_or_default(),
    });
    runner
        .messages
        .push(crate::native::model::types::Message::system(system));
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    runner.on_event = Some(event_tx);
    let (usage_tx, mut usage_rx) = mpsc::unbounded_channel();
    runner.on_usage = Some(usage_tx);
    let emit_app = app.clone();
    let emit_session = session_record_id.clone();
    let emit_employee = employee_id.clone();
    let emit_task = task_id.clone();
    let emit_kind = kind.clone();
    let emit_join = tokio::spawn(async move {
        while let Some(line) = event_rx.recv().await {
            emit_native_line(
                &emit_app,
                &emit_session,
                &emit_employee,
                emit_task.as_deref(),
                &emit_kind,
                line,
            )
            .await;
        }
    });
    let usage_app = app.clone();
    let usage_session = session_record_id.clone();
    let usage_join = tokio::spawn(async move {
        while let Some(delta) = usage_rx.recv().await {
            if let Ok(pool) = sqlite_pool(&usage_app).await {
                let _ = apply_codex_session_usage(&pool, &usage_session, &delta).await;
            }
        }
    });

    let loaded_images = crate::native::images::load_native_images(image_paths.as_deref());
    for line in crate::native::images::image_log_lines(&loaded_images) {
        emit_native_line(
            &app,
            &session_record_id,
            &employee_id,
            task_id.as_deref(),
            &kind,
            line,
        )
        .await;
    }
    let mut pending_images = loaded_images.images;

    let mut next = Some(first_prompt);
    let mut last_error: Option<String> = None;
    while let Some(prompt) = next.take() {
        emit_native_line(
            &app,
            &session_record_id,
            &employee_id,
            task_id.as_deref(),
            &kind,
            format!("[USER_INPUT] {prompt}"),
        )
        .await;
        let images = std::mem::take(&mut pending_images);
        match runner
            .run_with_client(
                &run.client,
                &prompt,
                &run.model,
                run.effort.as_deref(),
                run.max_output_tokens,
                run.thinking_enabled,
                images,
            )
            .await
        {
            Ok(_) => {}
            Err(error) => {
                last_error = Some(error.clone());
                if let Some(tx) = &runner.on_event {
                    let _ = tx.send(format!("[ERROR] {error}"));
                } else {
                    emit_native_line(
                        &app,
                        &session_record_id,
                        &employee_id,
                        task_id.as_deref(),
                        &kind,
                        format!("[ERROR] {error}"),
                    )
                    .await;
                }
                break;
            }
        }
        if !await_followups || cancel.is_cancelled() {
            break;
        }
        match followup_rx.recv().await {
            Some(NativeFollowup::Input(input)) => next = Some(input),
            Some(NativeFollowup::Finish) | None => break,
        }
    }

    runner.on_event.take();
    runner.on_usage.take();
    let _ = emit_join.await;
    let _ = usage_join.await;

    if kind == "review" {
        if let Ok(pool) = sqlite_pool(&app).await {
            let _ =
                persist_review_session_events_from_session_logs(&pool, &session_record_id).await;
        }
    }

    let failed = last_error.is_some() && last_error.as_deref() != Some("已取消");
    let status = if failed { "failed" } else { "exited" };
    let code = if failed { Some(1) } else { Some(0) };
    let ended_at = now_sqlite();
    let _ = update_codex_session_record(
        &app,
        &session_record_id,
        Some(status),
        None,
        Some(code),
        Some(Some(ended_at.as_str())),
    )
    .await;
    crate::codex::persist_external_execution_change_history(
        &app,
        &session_record_id,
        native_kind_to_codex(&kind),
        CodexExecutionProvider::Cli,
        execution_change_baseline.as_ref(),
        None,
    )
    .await;
    let _ = app.emit(
        "native-exit",
        CodexExit {
            employee_id: employee_id.clone(),
            task_id: task_id.clone(),
            session_kind: kind,
            session_record_id: session_record_id.clone(),
            session_event_id: None,
            line: None,
            code,
        },
    );
    manager_state
        .lock()
        .await
        .remove_session(&session_record_id);
    crate::task_automation::handle_session_exit_blocking(app.clone(), session_record_id).await;
    crate::run_queue::spawn_drain(app);
}

pub async fn list_live_native_employee_processes(
    state: &Arc<Mutex<NativeAgentManager>>,
    employee_id: &str,
) -> Vec<NativeSessionInfo> {
    state.lock().await.get_employee_processes(employee_id)
}

async fn stop_native_process(
    manager_state: &Arc<Mutex<NativeAgentManager>>,
    session_record_id: &str,
) -> Result<bool, String> {
    let session = {
        let mut manager = manager_state.lock().await;
        manager.remove_session(session_record_id)
    };
    let Some(session) = session else {
        return Ok(false);
    };
    session.cancel.cancel();
    let _ = session.followup_tx.send(NativeFollowup::Finish).await;
    let _ = session.join.await;
    Ok(true)
}

#[tauri::command]
pub async fn stop_native_session(
    state: State<'_, Arc<Mutex<NativeAgentManager>>>,
    session_record_id: String,
) -> Result<(), String> {
    if !stop_native_process(state.inner(), &session_record_id).await? {
        return Err(format!("未找到内置 Agent 会话 {session_record_id}"));
    }
    Ok(())
}

#[tauri::command]
pub async fn stop_native(
    state: State<'_, Arc<Mutex<NativeAgentManager>>>,
    employee_id: String,
) -> Result<(), String> {
    let processes = list_live_native_employee_processes(state.inner(), &employee_id).await;
    for process in processes {
        let _ = stop_native_process(state.inner(), &process.session_record_id).await?;
    }
    Ok(())
}

pub async fn stop_native_for_automation_restart<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: &str,
    expected_session_record_id: Option<&str>,
    _message: &str,
) -> Result<bool, String> {
    let Some(expected_session_record_id) = expected_session_record_id else {
        return Err("当前自动化步骤缺少会话标识，无法安全重启".to_string());
    };
    let manager_state = app
        .state::<Arc<Mutex<NativeAgentManager>>>()
        .inner()
        .clone();
    let running = {
        let manager = manager_state.lock().await;
        manager.get_session(expected_session_record_id).map(|item| {
            (
                item.info.employee_id.clone(),
                item.info.session_record_id.clone(),
            )
        })
    };
    let Some((running_employee, session_id)) = running else {
        return Ok(false);
    };
    if running_employee != employee_id {
        return Err("当前员工正在执行其他任务，无法重启这条自动化步骤".to_string());
    }
    stop_native_process(&manager_state, &session_id).await
}

#[tauri::command]
pub async fn send_native_input(
    state: State<'_, Arc<Mutex<NativeAgentManager>>>,
    employee_id: String,
    input: String,
) -> Result<(), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("输入内容不能为空".to_string());
    }
    let tx = {
        let manager = state.lock().await;
        manager
            .get_employee_processes(&employee_id)
            .first()
            .and_then(|info| {
                manager
                    .get_session(&info.session_record_id)
                    .map(|session| session.followup_tx.clone())
            })
    };
    let Some(tx) = tx else {
        return Err(format!(
            "员工 {employee_id} 当前没有运行中的内置 Agent 会话"
        ));
    };
    tx.send(NativeFollowup::Input(trimmed.to_string()))
        .await
        .map_err(|_| "内置 Agent 会话已结束，无法发送输入".to_string())
}

#[tauri::command]
pub async fn finish_native_input(
    state: State<'_, Arc<Mutex<NativeAgentManager>>>,
    employee_id: String,
) -> Result<(), String> {
    let processes = list_live_native_employee_processes(state.inner(), &employee_id).await;
    if processes.is_empty() {
        return Err(format!(
            "员工 {employee_id} 当前没有运行中的内置 Agent 会话"
        ));
    }
    for process in processes {
        let _ = stop_native_process(state.inner(), &process.session_record_id).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn restart_native_session(
    app: AppHandle,
    state: State<'_, Arc<Mutex<NativeAgentManager>>>,
    employee_id: String,
    task_description: String,
    model: Option<String>,
    reasoning_effort: Option<String>,
    system_prompt: Option<String>,
    working_dir: Option<String>,
    task_id: Option<String>,
    task_git_context_id: Option<String>,
    image_paths: Option<Vec<String>>,
    session_kind: Option<String>,
) -> Result<crate::run_queue::StartSessionOutcome, String> {
    stop_native(state.clone(), employee_id.clone()).await?;
    start_native_session(
        app,
        state,
        employee_id,
        task_description,
        model,
        reasoning_effort,
        system_prompt,
        working_dir,
        task_id,
        task_git_context_id,
        None,
        image_paths,
        session_kind,
    )
    .await
}

#[tauri::command]
pub async fn resume_native_session(
    app: AppHandle,
    state: State<'_, Arc<Mutex<NativeAgentManager>>>,
    employee_id: String,
    task_description: String,
    model: Option<String>,
    reasoning_effort: Option<String>,
    system_prompt: Option<String>,
    working_dir: Option<String>,
    task_id: Option<String>,
    task_git_context_id: Option<String>,
    resume_session_id: String,
    session_kind: Option<String>,
) -> Result<crate::run_queue::StartSessionOutcome, String> {
    start_native_session(
        app,
        state,
        employee_id,
        task_description,
        model,
        reasoning_effort,
        system_prompt,
        working_dir,
        task_id,
        task_git_context_id,
        Some(resume_session_id),
        None,
        session_kind,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{native_kind_to_codex, should_capture_native_execution_changes};
    use crate::codex::CodexSessionKind;

    #[test]
    fn execution_sessions_capture_git_changes_and_review_does_not() {
        assert!(should_capture_native_execution_changes("execution"));
        assert!(!should_capture_native_execution_changes("review"));
        assert_eq!(
            native_kind_to_codex("execution"),
            CodexSessionKind::Execution
        );
        assert_eq!(native_kind_to_codex("review"), CodexSessionKind::Review);
    }

    #[test]
    fn native_one_shot_text_requires_non_empty_assistant() {
        assert_eq!(super::native_one_shot_text("  ok  ").as_deref(), Ok("ok"));
        assert_eq!(
            super::native_one_shot_text("   ").unwrap_err(),
            "内置 Agent 未返回可用内容"
        );
    }
}
