use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::sync::{mpsc, Mutex};

use crate::app::{
    apply_codex_session_usage, fetch_employee_by_id, fetch_ssh_config_record_by_id,
    fetch_task_by_id, insert_activity_log, insert_codex_session_event,
    insert_codex_session_event_with_id, insert_codex_session_record, now_sqlite,
    persist_review_session_events_from_session_logs, sqlite_pool, update_codex_session_record,
    validate_runtime_working_dir, EXECUTION_TARGET_LOCAL, EXECUTION_TARGET_SSH,
};
use crate::codex::mcp::{mcp_summary_line, resolve_effective_mcp_for_task};
use crate::codex::{CodexExecutionProvider, CodexSessionKind, ExecutionChangeBaseline};
use crate::db::models::ChannelModelConfig;
use crate::db::models::{CodexExit, CodexOutput, CodexSession, SshConfigRecord};
use crate::engine::context::resolve_session_execution_context;
use crate::native::agent::r#loop::AgentRunner;
use crate::native::channels::{fetch_channel_record, require_channel_api_key};
use crate::native::manager::{
    NativeAgentManager, NativeFollowup, NativeLiveSession, NativeSessionInfo, PendingPermission,
    PermissionRequest,
};
use crate::native::model::{ModelClient, ModelClientConfig, RetryConfig};
use crate::native::model_catalog::{apply_catalog_defaults, fill_from_catalog};
use crate::native::protocol::record_to_channel;
use crate::native::tools::permission::{NativePermissionDecision, NativeToolRiskKind};
use crate::native::tools::{
    connect_mcp_servers, local::LocalWorkspace, ssh::SshToolRuntime, SharedMcp,
};
use serde::Serialize;

const ENGINE_LABEL: &str = "内置 Agent";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativePermissionRequestEvent {
    session_record_id: String,
    request_id: String,
    employee_id: String,
    task_id: Option<String>,
    session_kind: String,
    tool_name: String,
    kind: NativeToolRiskKind,
    summary: String,
    remote: bool,
}

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

fn permission_event(
    session_record_id: &str,
    request: &PermissionRequest,
) -> NativePermissionRequestEvent {
    NativePermissionRequestEvent {
        session_record_id: session_record_id.to_string(),
        request_id: request.request_id.clone(),
        employee_id: request.employee_id.clone(),
        task_id: request.task_id.clone(),
        session_kind: request.session_kind.clone(),
        tool_name: request.tool_name.clone(),
        kind: request.kind,
        summary: request.summary.clone(),
        remote: request.remote,
    }
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

fn apply_bound_subagent(
    runner: &mut AgentRunner,
    parts: &mut crate::native::prompt::NativePromptParts,
    bound: Option<&crate::native::subagents::NativeSubagent>,
) {
    let Some(def) = bound else {
        return;
    };
    parts.required_subagent_name = def.name.clone();
    parts.required_subagent_description = def.description.clone();
    runner.required_subagent_type = Some(def.name.clone());
}

fn attach_subagent_runtime(
    app: &AppHandle,
    runner: &mut AgentRunner,
    parts: &crate::native::prompt::NativePromptParts,
    project_id: Option<&str>,
    bound: Option<&crate::native::subagents::NativeSubagent>,
) {
    runner.workspace_context = crate::native::prompt::workspace_context_block(parts);
    runner.project_agents = parts.project_agents.clone();
    let loaded = crate::native::subagents::load_native_subagents(app).unwrap_or_default();
    runner.custom_subagents =
        crate::native::subagents::catalog_for_session(&loaded, project_id, bound);
    let app_reload = app.clone();
    let project_id_owned = project_id.map(ToOwned::to_owned);
    let bound_owned = bound.cloned();
    runner.reload_custom_subagents = Some(std::sync::Arc::new(move || {
        let loaded =
            crate::native::subagents::load_native_subagents(&app_reload).unwrap_or_default();
        crate::native::subagents::catalog_for_session(
            &loaded,
            project_id_owned.as_deref(),
            bound_owned.as_ref(),
        )
    }));
    let app_load = app.clone();
    runner.child_model_loader = Some(std::sync::Arc::new(move |channel_id, model| {
        let app = app_load.clone();
        Box::pin(async move {
            crate::native::subagents::resolve_child_model(&app, &channel_id, &model).await
        })
    }));
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
    protocol: String,
    channel_name: String,
    bound_subagent: Option<crate::native::subagents::NativeSubagent>,
    catalog_project_id: Option<String>,
}

fn native_startup_banner(
    channel_name: &str,
    protocol: &str,
    model: &str,
    effort: Option<&str>,
    thinking_enabled: bool,
) -> String {
    let effort = effort
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("默认");
    let thinking = if thinking_enabled { "on" } else { "off" };
    format!(
        "[内置 Agent] 启动会话 渠道={channel_name} 协议={protocol} model={model} effort={effort} thinking={thinking}"
    )
}

pub struct NativeOneShotResult {
    pub text: String,
    pub usage_line: Option<String>,
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
        timeout: Duration::from_secs(if thinking_enabled { 300 } else { 120 }),
    })?;
    Ok(NativeRunSettings {
        client,
        model,
        effort,
        max_output_tokens: model_config.max_output_tokens,
        thinking_enabled,
        context_tokens: model_config.context_tokens,
        employee_system_prompt: None,
        protocol: channel.protocol.clone(),
        channel_name: channel.name.clone(),
        bound_subagent: None,
        catalog_project_id: None,
    })
}

fn native_one_shot_text(message: &crate::native::model::types::Message) -> Result<String, String> {
    let content = message.content.trim();
    if !content.is_empty() {
        return Ok(content.to_string());
    }
    let reasoning = message.reasoning_content.trim();
    if reasoning.is_empty() {
        return Err("内置 Agent 未返回可用内容".to_string());
    }
    if one_shot_reasoning_usable(reasoning) {
        return Ok(reasoning.to_string());
    }
    Err(format!(
        "模型只返回了思考内容（{} 字），没有正文。请将推理强度从 max 改为 high 或 low 后重试。",
        reasoning.chars().count()
    ))
}

fn one_shot_reasoning_usable(text: &str) -> bool {
    let trimmed = text.trim();
    (trimmed.contains('{') && trimmed.contains('}'))
        || trimmed.starts_with('#')
        || trimmed.contains("\n# ")
        || trimmed.contains("\n## ")
}

/// Employee-scoped HTTP one-shot (tester acceptance and no-workspace fallback).
/// No tool loop and no Codex SDK/exec fallback. Images stay on this machine.
pub async fn run_native_one_shot<R: Runtime>(
    app: &AppHandle<R>,
    employee_id: &str,
    prompt: String,
    image_paths: Option<Vec<String>>,
    model_override: Option<String>,
    reasoning_effort_override: Option<String>,
) -> Result<NativeOneShotResult, String> {
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
    let (mut message, mut usage) = run
        .client
        .chat(crate::native::model::client::ChatRequest {
            messages: std::slice::from_ref(&user),
            tools: &[],
            model: &run.model,
            effort: run.effort.as_deref(),
            max_output_tokens: run.max_output_tokens,
            thinking_enabled: run.thinking_enabled,
        })
        .await
        .map_err(|error| format!("内置 Agent 一次性调用失败：{error}"))?;
    if native_one_shot_text(&message).is_err() && run.thinking_enabled {
        if let Ok((retry_message, retry_usage)) = run
            .client
            .chat(crate::native::model::client::ChatRequest {
                messages: std::slice::from_ref(&user),
                tools: &[],
                model: &run.model,
                effort: None,
                max_output_tokens: run.max_output_tokens,
                thinking_enabled: false,
            })
            .await
        {
            if native_one_shot_text(&retry_message).is_ok() {
                message = retry_message;
                usage = retry_usage;
            }
        }
    }
    Ok(NativeOneShotResult {
        text: native_one_shot_text(&message)?,
        usage_line: crate::native::model::usage_to_delta(usage)
            .and_then(|delta| delta.format_terminal_line()),
    })
}

/// Coordinator plan generation: in-process read-only tool loop, no session registry.
#[allow(clippy::too_many_arguments)]
pub async fn run_native_read_only_one_shot(
    app: &AppHandle,
    employee_id: &str,
    prompt: String,
    image_paths: Option<Vec<String>>,
    model_override: Option<String>,
    reasoning_effort_override: Option<String>,
    working_dir: &str,
    execution_target: &str,
    ssh_config_id: Option<&str>,
    event_tx: mpsc::UnboundedSender<String>,
) -> Result<NativeOneShotResult, String> {
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
    let mut run = load_native_client(&pool, &employee).await?;
    run.employee_system_prompt = employee
        .system_prompt
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned);

    let run_cwd = if execution_target == EXECUTION_TARGET_LOCAL {
        validate_runtime_working_dir(Some(working_dir))?
    } else {
        working_dir.to_string()
    };
    let ssh = if execution_target == EXECUTION_TARGET_SSH {
        let ssh_id = ssh_config_id
            .map(str::trim)
            .filter(|item| !item.is_empty())
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

    let loaded = crate::native::images::load_native_images(image_paths.as_deref());
    for path in &loaded.missing {
        eprintln!("[native] read-only one-shot 附件图片不存在，已跳过: {path}");
    }
    for reason in &loaded.skipped {
        eprintln!("[native] read-only one-shot 跳过图片: {reason}");
    }

    let mut runner = AgentRunner::new(LocalWorkspace::new(PathBuf::from(&run_cwd)));
    runner.ctx.ssh = ssh;
    runner.set_read_only(true);
    runner.set_allowed_tools(crate::native::tools::READ_ONLY_NATIVE_TOOL_NAMES);
    runner.max_turns = crate::native::settings::effective_max_turns(app);
    runner.max_concurrent_subagents =
        crate::native::settings::effective_max_concurrent_subagents(app);
    runner.subagent_policy = crate::native::settings::effective_subagent_policy(app);
    if let Some(context_tokens) = run.context_tokens {
        runner.context_char_limit = (context_tokens as usize).saturating_mul(3).max(8_000);
    }
    let mut parts = crate::native::prompt::NativePromptParts {
        cwd: run_cwd.clone(),
        model: run.model.clone(),
        platform: std::env::consts::OS.to_string(),
        git: if let Some(ssh) = runner.ctx.ssh.as_ref() {
            crate::native::prompt::detect_ssh_git(ssh).await
        } else {
            crate::native::prompt::detect_local_git(&run_cwd)
        },
        global_template: crate::native::prompt::load_global_template(app),
        project_agents: if let Some(ssh) = runner.ctx.ssh.as_ref() {
            crate::native::prompt::read_ssh_project_agents(ssh).await
        } else {
            crate::native::prompt::read_local_project_agents(&run_cwd)
        },
        employee_prompt: run.employee_system_prompt.clone().unwrap_or_default(),
        max_concurrent_subagents: runner.max_concurrent_subagents,
        subagent_policy: runner.subagent_policy.clone(),
        identity_override: String::new(),
        required_subagent_name: String::new(),
        required_subagent_description: String::new(),
    };
    attach_subagent_runtime(
        app,
        &mut runner,
        &parts,
        run.catalog_project_id.as_deref(),
        run.bound_subagent.as_ref(),
    );
    apply_bound_subagent(&mut runner, &mut parts, run.bound_subagent.as_ref());
    let system = crate::native::prompt::compose_system(&parts);
    runner
        .messages
        .push(crate::native::model::types::Message::system(system));
    runner.on_event = Some(event_tx);
    if let Some(tx) = &runner.on_event {
        let _ = tx.send(native_startup_banner(
            &run.channel_name,
            &run.protocol,
            &run.model,
            run.effort.as_deref(),
            run.thinking_enabled,
        ));
    }
    let text = runner
        .run_with_client(
            &run.client,
            &prompt,
            &run.model,
            run.effort.as_deref(),
            run.max_output_tokens,
            run.thinking_enabled,
            loaded.images,
        )
        .await
        .map_err(|error| format!("内置 Agent 规划调用失败：{error}"))?;
    if text.trim().is_empty() {
        return Err("内置 Agent 未返回可用内容".to_string());
    }
    Ok(NativeOneShotResult {
        text,
        usage_line: None,
    })
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
    if let Some(task_id) = task_id.as_deref() {
        if let Ok(task) = fetch_task_by_id(&pool, task_id).await {
            run.catalog_project_id = Some(task.project_id.clone());
            if kind == "execution" {
                if let Some(bound_id) = task.native_subagent_id.as_deref() {
                    let catalog =
                        crate::native::subagents::load_native_subagents(&app).unwrap_or_default();
                    if let Some(def) =
                        crate::native::subagents::find_native_subagent_by_id(&catalog, bound_id)
                    {
                        run.bound_subagent = Some(def.clone());
                    }
                }
            }
        }
    }
    let prompt = if let Some(def) = run.bound_subagent.as_ref() {
        crate::native::prompt::wrap_prompt_for_required_subagent(&task_description, &def.name)
    } else {
        task_description
    };

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
    let confirm_high_risk = crate::native::settings::confirm_high_risk_enabled(&app);
    let allow_all_high_risk =
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(!confirm_high_risk));
    let session_record_id = session_record.id.clone();
    let employee_id_spawn = employee_id.clone();
    let task_id_spawn = task_id.clone();
    let project_id_spawn = session_record.project_id.clone();
    let kind_spawn = kind.clone();
    let manager_spawn = manager_state.clone();
    let app_spawn = app.clone();
    let await_followups = task_id.is_none();
    let cancel_run = cancel.clone();
    let allow_all_run = allow_all_high_risk.clone();
    let (loop_ready_tx, loop_ready_rx) = tokio::sync::oneshot::channel();
    let join = tokio::spawn(async move {
        let _ = loop_ready_rx.await;
        run_native_loop(
            app_spawn,
            manager_spawn,
            run,
            prompt,
            run_cwd,
            ssh,
            cancel_run,
            allow_all_run,
            followup_rx,
            session_record_id,
            employee_id_spawn,
            task_id_spawn,
            project_id_spawn,
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
        allow_all_high_risk,
        pending_permission: std::collections::VecDeque::new(),
    });
    let _ = loop_ready_tx.send(());

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
    allow_all_high_risk: Arc<std::sync::atomic::AtomicBool>,
    mut followup_rx: mpsc::Receiver<NativeFollowup>,
    session_record_id: String,
    employee_id: String,
    task_id: Option<String>,
    project_id: Option<String>,
    kind: String,
    await_followups: bool,
    image_paths: Option<Vec<String>>,
    execution_change_baseline: Option<ExecutionChangeBaseline>,
) {
    let mut runner = AgentRunner::new(LocalWorkspace::new(PathBuf::from(&run_cwd)));
    runner.ctx.ssh = ssh;
    runner.ctx.cancel = cancel.clone();
    runner.ctx.allow_all_high_risk = allow_all_high_risk;
    let confirm_high_risk = crate::native::settings::confirm_high_risk_enabled(&app);
    if !confirm_high_risk {
        emit_native_line(
            &app,
            &session_record_id,
            &employee_id,
            task_id.as_deref(),
            &kind,
            "[PERMISSION] 已在设置中关闭高风险确认，本会话工具将直接执行".to_string(),
        )
        .await;
    }
    if confirm_high_risk {
        let app_perm = app.clone();
        let manager_perm = manager_state.clone();
        let session_perm = session_record_id.clone();
        let employee_perm = employee_id.clone();
        let task_perm = task_id.clone();
        let kind_perm = kind.clone();
        runner.ctx.request_permission = Some(std::sync::Arc::new(move |prompt, reply| {
            let app = app_perm.clone();
            let manager_state = manager_perm.clone();
            let session_record_id = session_perm.clone();
            let employee_id = employee_perm.clone();
            let task_id = task_perm.clone();
            let kind = kind_perm.clone();
            tauri::async_runtime::spawn(async move {
                let request_id = uuid::Uuid::new_v4().to_string();
                let request = PermissionRequest {
                    request_id: request_id.clone(),
                    employee_id: employee_id.clone(),
                    task_id: task_id.clone(),
                    session_kind: kind.clone(),
                    tool_name: prompt.tool_name.clone(),
                    kind: prompt.kind,
                    summary: prompt.summary.clone(),
                    remote: prompt.remote,
                };
                let should_emit = {
                    let mut manager = manager_state.lock().await;
                    match manager.enqueue_permission(
                        &session_record_id,
                        PendingPermission {
                            request: request.clone(),
                            reply,
                        },
                    ) {
                        Ok(should_emit) => should_emit,
                        Err(_) => return,
                    }
                };
                let location = if prompt.remote {
                    "远程工作区"
                } else {
                    "本地工作区"
                };
                emit_native_line(
                    &app,
                    &session_record_id,
                    &employee_id,
                    task_id.as_deref(),
                    &kind,
                    format!(
                        "[PERMISSION] 等待确认高风险操作（{location} / {:?}）：{}",
                        prompt.kind, prompt.summary
                    ),
                )
                .await;
                if should_emit {
                    let _ = app.emit(
                        "native-permission-request",
                        permission_event(&session_record_id, &request),
                    );
                }
            });
        }));
    }
    runner.max_turns = crate::native::settings::effective_max_turns(&app);
    runner.max_concurrent_subagents =
        crate::native::settings::effective_max_concurrent_subagents(&app);
    runner.subagent_policy = crate::native::settings::effective_subagent_policy(&app);
    if let Some(context_tokens) = run.context_tokens {
        runner.context_char_limit = (context_tokens as usize).saturating_mul(3).max(8_000);
    }
    let mut parts = crate::native::prompt::NativePromptParts {
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
        max_concurrent_subagents: runner.max_concurrent_subagents,
        subagent_policy: runner.subagent_policy.clone(),
        identity_override: String::new(),
        required_subagent_name: String::new(),
        required_subagent_description: String::new(),
    };
    attach_subagent_runtime(
        &app,
        &mut runner,
        &parts,
        run.catalog_project_id.as_deref(),
        run.bound_subagent.as_ref(),
    );
    apply_bound_subagent(&mut runner, &mut parts, run.bound_subagent.as_ref());
    let system = crate::native::prompt::compose_system(&parts);
    runner
        .messages
        .push(crate::native::model::types::Message::system(system));
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    runner.on_event = Some(event_tx);
    let (usage_tx, mut usage_rx) = mpsc::unbounded_channel();
    runner.on_usage = Some(usage_tx);
    let (activity_tx, mut activity_rx) = mpsc::unbounded_channel::<(String, String)>();
    if task_id.is_some() {
        runner.on_activity = Some(activity_tx);
    } else {
        drop(activity_tx);
    }
    emit_native_line(
        &app,
        &session_record_id,
        &employee_id,
        task_id.as_deref(),
        &kind,
        native_startup_banner(
            &run.channel_name,
            &run.protocol,
            &run.model,
            run.effort.as_deref(),
            run.thinking_enabled,
        ),
    )
    .await;
    if let Some(def) = run.bound_subagent.as_ref() {
        emit_native_line(
            &app,
            &session_record_id,
            &employee_id,
            task_id.as_deref(),
            &kind,
            format!(
                "[子智能体] 本任务必须委派 {}（Agent.subagent_type={}）",
                def.name, def.name
            ),
        )
        .await;
    }
    if let Ok(pool) = sqlite_pool(&app).await {
        match resolve_effective_mcp_for_task(&app, &pool, task_id.as_deref()).await {
            Ok(resolved) => {
                emit_native_line(
                    &app,
                    &session_record_id,
                    &employee_id,
                    task_id.as_deref(),
                    &kind,
                    mcp_summary_line(&resolved),
                )
                .await;
                let ssh_config = runner.ctx.ssh.as_ref().map(|item| item.config.clone());
                if ssh_config.is_some() {
                    emit_native_line(
                        &app,
                        &session_record_id,
                        &employee_id,
                        task_id.as_deref(),
                        &kind,
                        "[MCP] SSH 会话将在远端拉起 MCP，失败不回退本机".to_string(),
                    )
                    .await;
                }
                let connected = connect_mcp_servers(
                    &app,
                    &resolved.servers,
                    ssh_config.as_ref(),
                    &runner.ctx.cancel,
                )
                .await;
                for warning in connected.warnings {
                    emit_native_line(
                        &app,
                        &session_record_id,
                        &employee_id,
                        task_id.as_deref(),
                        &kind,
                        warning,
                    )
                    .await;
                }
                if connected.connected.is_empty() {
                    if !resolved.servers.is_empty() {
                        emit_native_line(
                            &app,
                            &session_record_id,
                            &employee_id,
                            task_id.as_deref(),
                            &kind,
                            "[MCP] 没有成功连接的服务器".to_string(),
                        )
                        .await;
                    }
                } else {
                    emit_native_line(
                        &app,
                        &session_record_id,
                        &employee_id,
                        task_id.as_deref(),
                        &kind,
                        format!("[MCP] 已连接：{}", connected.connected.join("、")),
                    )
                    .await;
                }
                runner.set_extra_tools(connected.session.tool_specs());
                runner.ctx.mcp = SharedMcp::from_session(connected.session);
            }
            Err(error) => {
                emit_native_line(
                    &app,
                    &session_record_id,
                    &employee_id,
                    task_id.as_deref(),
                    &kind,
                    format!("[MCP] 读取绑定失败：{error}"),
                )
                .await;
            }
        }
    }
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
    let activity_app = app.clone();
    let activity_employee = employee_id.clone();
    let activity_task = task_id.clone();
    let activity_project = project_id.clone();
    let activity_join = tokio::spawn(async move {
        while let Some((action, details)) = activity_rx.recv().await {
            if let Ok(pool) = sqlite_pool(&activity_app).await {
                let _ = insert_activity_log(
                    &pool,
                    &action,
                    &details,
                    Some(&activity_employee),
                    activity_task.as_deref(),
                    activity_project.as_deref(),
                )
                .await;
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
    runner.on_activity.take();
    runner.ctx.mcp.shutdown().await;
    let _ = emit_join.await;
    let _ = usage_join.await;
    let _ = activity_join.await;

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
        manager.deny_pending_permission(session_record_id);
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
pub async fn resolve_native_tool_permission(
    app: AppHandle,
    state: State<'_, Arc<Mutex<NativeAgentManager>>>,
    session_record_id: String,
    request_id: String,
    decision: NativePermissionDecision,
) -> Result<(), String> {
    let next = state
        .lock()
        .await
        .resolve_permission(&session_record_id, &request_id, decision)?;
    if let Some(request) = next {
        let _ = app.emit(
            "native-permission-request",
            permission_event(&session_record_id, &request),
        );
    }
    Ok(())
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
        let mut message = crate::native::model::types::Message::assistant_text("  ok  ");
        assert_eq!(super::native_one_shot_text(&message).as_deref(), Ok("ok"));
        message.content = "   ".to_string();
        assert_eq!(
            super::native_one_shot_text(&message).unwrap_err(),
            "内置 Agent 未返回可用内容"
        );
    }

    #[test]
    fn native_one_shot_text_uses_plan_shaped_reasoning() {
        let mut message = crate::native::model::types::Message::assistant_text("");
        message.reasoning_content =
            "{\"markdown\":\"# 计划\",\"steps\":[{\"title\":\"a\"}]}".to_string();
        assert!(super::native_one_shot_text(&message)
            .expect("usable reasoning")
            .contains("计划"));
    }

    #[test]
    fn native_one_shot_text_rejects_plain_reasoning() {
        let mut message = crate::native::model::types::Message::assistant_text("");
        message.reasoning_content = "先分析任务边界再给出步骤".to_string();
        let error = super::native_one_shot_text(&message).unwrap_err();
        assert!(error.contains("思考内容"));
        assert!(error.contains("没有正文"));
    }

    #[test]
    fn native_startup_banner_includes_model_and_channel() {
        assert_eq!(
            super::native_startup_banner("CRS", "codex", "gpt-5.6-luna", Some("high"), true),
            "[内置 Agent] 启动会话 渠道=CRS 协议=codex model=gpt-5.6-luna effort=high thinking=on"
        );
        assert_eq!(
            super::native_startup_banner("DeepSeek", "openai", "deepseek-v4-flash", None, false),
            "[内置 Agent] 启动会话 渠道=DeepSeek 协议=openai model=deepseek-v4-flash effort=默认 thinking=off"
        );
    }
}
