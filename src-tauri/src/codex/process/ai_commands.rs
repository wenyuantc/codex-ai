use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use super::{
    build_ai_generate_commit_message_prompt, build_ai_generate_plan_prompt,
    build_ai_generate_plan_prompt_with_attachments, build_ai_generate_tester_acceptance_prompt,
    build_ai_optimize_prompt_prompt, build_ai_revise_plan_prompt, emit_ai_command_line,
    parse_ai_subtasks_response, resolve_one_shot_working_dir, resolve_project_execution_context,
    resolve_task_project_execution_context, run_ai_command, run_ai_command_with_options,
    run_native_ai_command, AiCommandOptions, ExecutionContext,
};
use crate::app::{
    fetch_employee_by_id, fetch_project_by_id, fetch_task_attachments, fetch_task_by_id,
    fetch_task_file_refs, fetch_task_subtasks, format_task_file_refs_prompt_section,
    insert_activity_log, insert_codex_session_event, insert_codex_session_record, new_id,
    now_sqlite, save_task_plan_content, sqlite_pool, task_attachment_is_image,
    update_codex_session_record, PROJECT_TYPE_SSH,
};
use crate::codex::{
    find_ai_prompt_template, load_ai_prompt_templates, load_codex_settings,
    load_remote_codex_settings,
};
use crate::db::models::{Employee, TaskPipelineStep};

/// Process-language phrases that indicate the model is describing git staging
/// workflow rather than product/code changes. Prefer multi-character phrases
/// over bare words like “工作区/核对” to reduce false positives on product copy.
const COMMIT_MESSAGE_PROCESS_PHRASES: &[&str] = &[
    "已暂存",
    "待提交文件",
    "待提交",
    "文件列表",
    "暂存内容",
    "暂存文件",
    "暂存全部",
    "暂存并",
    "暂存区",
    "工作区提交",
    "工作区改动",
    "工作区文件",
    "工作区变更",
    "更新工作区",
    "当前工作区",
    "核对工作区",
    "核对内容",
    "核对文件",
    "核对首页",
];

fn normalize_generated_commit_message(raw: &str) -> Result<String, String> {
    let mut normalized_lines = Vec::new();
    let mut previous_blank = true;
    for raw_line in raw.lines() {
        let trimmed = raw_line.trim();
        if trimmed == "```" || trimmed.starts_with("```") {
            continue;
        }
        if trimmed.is_empty() {
            if !previous_blank && !normalized_lines.is_empty() {
                normalized_lines.push(String::new());
                previous_blank = true;
            }
            continue;
        }
        normalized_lines.push(trimmed.trim_matches('`').trim().to_string());
        previous_blank = false;
    }
    while matches!(normalized_lines.last(), Some(line) if line.is_empty()) {
        normalized_lines.pop();
    }
    let normalized = normalized_lines.join("\n").trim().to_string();
    if normalized.is_empty() {
        return Err("AI 没有返回可用的提交信息".to_string());
    }
    Ok(normalized)
}

pub(crate) fn validate_generated_commit_message(
    message: &str,
    ai_commit_message_length: &str,
) -> Result<(), String> {
    let mut errors = Vec::new();

    if commit_message_uses_process_language(message) {
        errors.push("它在描述提交流程，而不是实际改动".to_string());
    }

    if ai_commit_message_length == "title_only" {
        let non_empty_line_count = message
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        if non_empty_line_count > 1 {
            errors.push("它没有遵守“仅标题”设置，输出了多行内容".to_string());
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("；"))
    }
}

pub(crate) fn commit_message_uses_process_language(message: &str) -> bool {
    let subject = message
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_lowercase();

    if subject.is_empty() {
        return false;
    }

    COMMIT_MESSAGE_PROCESS_PHRASES
        .iter()
        .any(|phrase| subject.contains(&phrase.to_lowercase()))
}

fn truncate_commit_message_for_error(message: &str, max_chars: usize) -> String {
    let trimmed = message.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let preview: String = trimmed.chars().take(max_chars).collect();
    format!("{preview}…")
}

pub(crate) fn format_commit_message_validation_failure(
    ai_commit_message_length: &str,
    validation_error: &str,
    latest_message: &str,
) -> String {
    format!(
        "AI 生成的提交信息仍不符合要求（{}）：{}。最近一次输出：\n{}",
        ai_commit_message_length,
        validation_error.trim(),
        truncate_commit_message_for_error(latest_message, 240)
    )
}

fn build_commit_message_retry_prompt(
    base_prompt: &str,
    previous_message: &str,
    validation_error: &str,
    ai_commit_message_length: &str,
) -> String {
    let length_requirement = if ai_commit_message_length == "title_only" {
        "- 本次长度配置为“仅标题”，只能输出单行 Conventional Commits 标题，不要附带正文\n"
    } else {
        ""
    };
    format!(
        "{base_prompt}\n\n\
上一次输出不合格，因为{validation_error}：\n\
{previous_message}\n\n\
请重新生成，并严格遵守以下附加要求：\n\
- 只描述真实代码或产品改动结果，不要描述暂存、提交、核对、整理文件等过程\n\
- 标题必须像“调整首页文案”“修复任务状态刷新”这样表达真实变化\n\
- 如果无法判断是 feat 还是 fix，优先根据用户可见变化选择，不要默认写成 chore\n\
- 不要复用上一次输出中的不合格结构或措辞\n\
- 返回前先自检是否满足当前长度配置\n\
- 如果长度配置要求仅标题，就不要输出空行\n\
- 不要复用上一次输出中的过程词\n\
{length_requirement}"
    )
}

fn resolve_ai_optimize_prompt_scene_label(scene: &str) -> Result<&'static str, String> {
    match scene.trim() {
        "task_create" => Ok("新建任务"),
        "task_continue" => Ok("任务继续对话"),
        "session_continue" => Ok("Session 继续对话"),
        "employee_system_prompt" => Ok("员工系统提示词生成"),
        other => Err(format!("不支持的提示词优化场景: {}", other)),
    }
}

fn format_ai_optimize_prompt_activity_details(
    project_name: &str,
    scene_label: &str,
    model: &str,
    reasoning_effort: &str,
    generated_at: &str,
) -> String {
    format!(
        "项目：{}；场景：{}；模型：{}；推理等级：{}；生成时间：{}",
        project_name, scene_label, model, reasoning_effort, generated_at
    )
}

fn build_plan_attachment_prompt_items(
    attachments: &[crate::db::models::TaskAttachment],
) -> Vec<String> {
    attachments
        .iter()
        .map(|attachment| {
            format!(
                "{}（类型：{}；大小：{} bytes）",
                attachment.original_name.trim(),
                attachment.mime_type.trim(),
                attachment.file_size
            )
        })
        .collect()
}

const COORDINATOR_SESSION_KIND: &str = "coordinator";

fn format_task_plan_generated_activity_details(
    task_title: &str,
    coordinator_name: &str,
    provider: &str,
    model: &str,
    reasoning_effort: &str,
    generated_at: &str,
    usage_line: Option<&str>,
) -> String {
    let base = format!(
        "任务：{}；协调员：{}；Provider：{}；模型：{}；推理等级：{}；生成时间：{}",
        task_title, coordinator_name, provider, model, reasoning_effort, generated_at
    );
    let Some(usage) = usage_line.map(str::trim).filter(|value| !value.is_empty()) else {
        return base;
    };
    let usage = usage
        .strip_prefix("[用量]")
        .or_else(|| usage.strip_prefix("[计划] 用量："))
        .unwrap_or(usage)
        .trim();
    format!("{base}；用量：{usage}")
}

async fn finish_coordinator_plan_session(
    app: &AppHandle,
    session_id: &str,
    ok: bool,
) -> Result<(), String> {
    let ended_at = now_sqlite();
    update_codex_session_record(
        app,
        session_id,
        Some(if ok { "exited" } else { "failed" }),
        None,
        Some(Some(if ok { 0 } else { 1 })),
        Some(Some(ended_at.as_str())),
    )
    .await
}

async fn coordinator_session_usage_line(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Option<String> {
    let row = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT input_tokens, output_tokens, total_tokens, reasoning_tokens, cached_tokens FROM codex_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;
    crate::engine::UsageDelta {
        input_tokens: row.0.and_then(|value| u64::try_from(value).ok()),
        output_tokens: row.1.and_then(|value| u64::try_from(value).ok()),
        total_tokens: row.2.and_then(|value| u64::try_from(value).ok()),
        reasoning_tokens: row.3.and_then(|value| u64::try_from(value).ok()),
        cached_tokens: row.4.and_then(|value| u64::try_from(value).ok()),
    }
    .format_terminal_line()
}

fn format_tester_acceptance_generated_activity_details(
    task_title: &str,
    tester_name: &str,
    provider: &str,
    model: &str,
    reasoning_effort: &str,
    generated_at: &str,
) -> String {
    format!(
        "任务：{}；测试员：{}；Provider：{}；模型：{}；推理等级：{}；生成时间：{}",
        task_title, tester_name, provider, model, reasoning_effort, generated_at
    )
}

fn format_tester_acceptance_comment(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.starts_with("[验收清单]") {
        trimmed.to_string()
    } else {
        format!("[验收清单]\n{}", trimmed)
    }
}

async fn resolve_tester_for_acceptance(
    pool: &sqlx::SqlitePool,
    task_reviewer_id: Option<&str>,
    tester_id: Option<&str>,
) -> Result<Employee, String> {
    if let Some(tester_id) = tester_id.map(str::trim).filter(|value| !value.is_empty()) {
        let tester = fetch_employee_by_id(pool, tester_id).await?;
        if tester.role != "tester" {
            return Err(format!(
                "员工 {} 不是测试员角色，无法生成验收清单",
                tester.name
            ));
        }
        return Ok(tester);
    }

    let Some(reviewer_id) = task_reviewer_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err("当前任务未指定测试员，请指定 tester_id 或将测试员设为审查员".to_string());
    };

    let tester = fetch_employee_by_id(pool, reviewer_id).await?;
    if tester.role != "tester" {
        return Err(format!(
            "当前审查员 {} 不是测试员角色；请传入 tester_id 或指定测试员",
            tester.name
        ));
    }
    Ok(tester)
}

#[derive(Debug, Clone, Serialize)]
pub struct CoordinatorTaskPlanResult {
    pub markdown: String,
    pub usage_line: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateCoordinatorTaskPlanPayload {
    pub task_id: String,
    pub coordinator_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub working_dir: Option<String>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub revision_instruction: Option<String>,
    #[serde(default)]
    pub current_markdown: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateTesterAcceptancePayload {
    pub task_id: String,
    pub tester_id: Option<String>,
    pub working_dir: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommitMessageAiSelection {
    provider_override: Option<String>,
    model_override: Option<String>,
    reasoning_override: Option<String>,
    native_channel_id: Option<String>,
    effective_provider: String,
    effective_model: String,
    effective_reasoning_effort: String,
}

fn resolve_commit_message_ai_selection(
    settings: &crate::db::models::CodexSettings,
) -> CommitMessageAiSelection {
    if settings.git_preferences.ai_commit_model_source == "custom" {
        let provider = settings
            .git_preferences
            .ai_commit_preferred_provider
            .clone();
        CommitMessageAiSelection {
            provider_override: Some(provider.clone()),
            model_override: Some(settings.git_preferences.ai_commit_model.clone()),
            reasoning_override: Some(settings.git_preferences.ai_commit_reasoning_effort.clone()),
            native_channel_id: settings.git_preferences.ai_commit_native_channel_id.clone(),
            effective_provider: provider,
            effective_model: settings.git_preferences.ai_commit_model.clone(),
            effective_reasoning_effort: settings.git_preferences.ai_commit_reasoning_effort.clone(),
        }
    } else {
        CommitMessageAiSelection {
            provider_override: None,
            model_override: None,
            reasoning_override: None,
            native_channel_id: settings.one_shot_native_channel_id.clone(),
            effective_provider: settings.one_shot_preferred_provider.clone(),
            effective_model: settings.one_shot_model.clone(),
            effective_reasoning_effort: settings.one_shot_reasoning_effort.clone(),
        }
    }
}

/// 执行 Git 提交信息生成：内置 Agent（native）走 AI 渠道本地 HTTP 调用，
/// 其余提供商保持走 run_ai_command（外部 CLI / SDK）。
async fn run_commit_message_ai_command<R: Runtime>(
    app: &AppHandle<R>,
    prompt: String,
    project_id: String,
    ai_selection: &CommitMessageAiSelection,
) -> Result<String, String> {
    if ai_selection.effective_provider == "native" {
        let channel_id = ai_selection
            .native_channel_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                if ai_selection.provider_override.is_some() {
                    "Git AI 使用内置 Agent 时请先选择 AI 渠道".to_string()
                } else {
                    "一次性 AI 使用内置 Agent 时请先选择 AI 渠道".to_string()
                }
            })?;
        let shot = crate::native::run_native_one_shot_via_channel(
            app,
            channel_id,
            &ai_selection.effective_model,
            &ai_selection.effective_reasoning_effort,
            prompt,
            None,
            Some(project_id.as_str()),
        )
        .await?;
        return Ok(shot.text);
    }
    let raw = run_ai_command(
        app,
        prompt,
        None,
        None,
        Some(project_id),
        None,
        ai_selection.provider_override.clone(),
        ai_selection.model_override.clone(),
        ai_selection.reasoning_override.clone(),
        None,
    )
    .await?;
    Ok(raw.text)
}

fn format_commit_message_activity_details(
    project_name: &str,
    provider: &str,
    model: &str,
    reasoning_effort: &str,
    generated_at: &str,
    message: &str,
) -> String {
    let provider_label = match provider {
        "native" => "内置 Agent",
        other => other,
    };
    format!(
        "项目：{}；Provider：{}；模型：{}；推理等级：{}；生成时间：{}；结果：{}",
        project_name, provider_label, model, reasoning_effort, generated_at, message
    )
}

pub(crate) struct GeneratedCommitMessage {
    pub(crate) message: String,
    pub(crate) project_id: String,
    pub(crate) project_name: String,
    pub(crate) provider: String,
    pub(crate) model: String,
    pub(crate) reasoning_effort: String,
}

async fn resolve_ai_optimize_prompt_activity_context<R: Runtime>(
    app: &AppHandle<R>,
    task_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<(Option<String>, String, String), String> {
    let normalized_task_id = task_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let normalized_project_id = project_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let resolved_project_id = match (normalized_project_id, normalized_task_id.as_deref()) {
        (Some(project_id), _) => Some(project_id),
        (None, Some(task_id)) => {
            let pool = sqlite_pool(app).await?;
            Some(fetch_task_by_id(&pool, task_id).await?.project_id)
        }
        (None, None) => None,
    };

    let execution_context = match normalized_task_id.as_deref() {
        Some(task_id) => resolve_task_project_execution_context(app, task_id).await?,
        None => match resolved_project_id.as_deref() {
            Some(project_id) => resolve_project_execution_context(app, project_id).await?,
            None => ExecutionContext::local_default(),
        },
    };

    let settings = if execution_context.execution_target == PROJECT_TYPE_SSH {
        execution_context
            .ssh_config_id
            .as_deref()
            .map(|ssh_config_id| load_remote_codex_settings(app, ssh_config_id))
            .transpose()?
            .or_else(|| load_codex_settings(app).ok())
    } else {
        load_codex_settings(app).ok()
    };

    let model = settings
        .as_ref()
        .map(|settings| settings.one_shot_model.clone())
        .unwrap_or_else(|| "gpt-5.4".to_string());
    let reasoning_effort = settings
        .as_ref()
        .map(|settings| settings.one_shot_reasoning_effort.clone())
        .unwrap_or_else(|| "high".to_string());

    Ok((resolved_project_id, model, reasoning_effort))
}

pub(crate) async fn generate_commit_message_for_project<R: Runtime>(
    app: &AppHandle<R>,
    project_id: &str,
    current_branch: Option<&str>,
    working_tree_summary: Option<&str>,
    staged_changes: &[String],
) -> Result<GeneratedCommitMessage, String> {
    let normalized_staged_changes = staged_changes
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if normalized_staged_changes.is_empty() {
        return Err("当前没有可用于生成提交信息的已暂存文件".to_string());
    }

    let pool = sqlite_pool(app).await?;
    let project = fetch_project_by_id(&pool, project_id).await?;
    let settings = if project.project_type == PROJECT_TYPE_SSH {
        project
            .ssh_config_id
            .as_deref()
            .map(|ssh_config_id| load_remote_codex_settings(app, ssh_config_id))
            .transpose()?
            .unwrap_or(load_codex_settings(app)?)
    } else {
        load_codex_settings(app)?
    };
    let prompt = build_ai_generate_commit_message_prompt(
        project.name.trim(),
        current_branch,
        working_tree_summary,
        &normalized_staged_changes,
        &settings.git_preferences.ai_commit_message_length,
    );
    let ai_selection = resolve_commit_message_ai_selection(&settings);
    let raw_text =
        run_commit_message_ai_command(app, prompt.clone(), project.id.clone(), &ai_selection)
            .await?;
    let normalized = normalize_generated_commit_message(&raw_text)?;
    let validation_error = match validate_generated_commit_message(
        &normalized,
        &settings.git_preferences.ai_commit_message_length,
    ) {
        Ok(()) => {
            return Ok(GeneratedCommitMessage {
                message: normalized,
                project_id: project.id.clone(),
                project_name: project.name.trim().to_string(),
                provider: ai_selection.effective_provider,
                model: ai_selection.effective_model,
                reasoning_effort: ai_selection.effective_reasoning_effort,
            });
        }
        Err(error) => error,
    };
    let retry_prompt = build_commit_message_retry_prompt(
        &prompt,
        &normalized,
        &validation_error,
        &settings.git_preferences.ai_commit_message_length,
    );
    let retried_raw_text =
        run_commit_message_ai_command(app, retry_prompt, project.id.clone(), &ai_selection).await?;
    let retried = normalize_generated_commit_message(&retried_raw_text)?;
    match validate_generated_commit_message(
        &retried,
        &settings.git_preferences.ai_commit_message_length,
    ) {
        Ok(()) => Ok(GeneratedCommitMessage {
            message: retried,
            project_id: project.id.clone(),
            project_name: project.name.trim().to_string(),
            provider: ai_selection.effective_provider,
            model: ai_selection.effective_model,
            reasoning_effort: ai_selection.effective_reasoning_effort,
        }),
        Err(retry_validation_error) => Err(format_commit_message_validation_failure(
            &settings.git_preferences.ai_commit_message_length,
            &retry_validation_error,
            &retried,
        )),
    }
}

#[tauri::command]
pub async fn ai_suggest_assignee(
    app: AppHandle,
    task_description: String,
    employee_list: String,
    image_paths: Option<Vec<String>>,
    task_id: Option<String>,
    working_dir: Option<String>,
) -> Result<String, String> {
    let templates = load_ai_prompt_templates(&app)
        .unwrap_or_else(|_| crate::codex::default_ai_prompt_templates());
    let template = find_ai_prompt_template(&templates, "suggest_assignee");
    let output_goal = template
        .map(|value| value.output_goal.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(
            "Based on the following task description, suggest the best assignee from the employee list. If task images are attached, consider them as additional context.",
        );
    let scene_requirement = template
        .map(|value| value.scene_requirement.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Respond with just the employee ID and a brief reason.");
    let prompt = format!(
        "{}\n\nTask: {}\n\nEmployees: {}\n\n{}",
        output_goal, task_description, employee_list, scene_requirement
    );
    run_ai_command(
        &app,
        prompt,
        image_paths,
        task_id,
        None,
        working_dir,
        None,
        None,
        None,
        None,
    )
    .await
    .map(|result| result.text)
}

#[tauri::command]
pub async fn ai_analyze_complexity(
    app: AppHandle,
    task_description: String,
    image_paths: Option<Vec<String>>,
    task_id: Option<String>,
    working_dir: Option<String>,
) -> Result<String, String> {
    let prompt = format!(
        "Analyze the complexity of this task on a scale of 1-10, and provide a brief breakdown. If task images are attached, include them in the analysis.\n\nTask: {}",
        task_description
    );
    run_ai_command(
        &app,
        prompt,
        image_paths,
        task_id,
        None,
        working_dir,
        None,
        None,
        None,
        None,
    )
    .await
    .map(|result| result.text)
}

#[tauri::command]
pub async fn ai_generate_comment(
    app: AppHandle,
    task_title: String,
    task_description: String,
    context: String,
    image_paths: Option<Vec<String>>,
    task_id: Option<String>,
    working_dir: Option<String>,
) -> Result<String, String> {
    let prompt = format!(
        "Generate a progress assessment comment for this task. If task images are attached, use them as supporting context.\n\nTitle: {}\nDescription: {}\nContext: {}",
        task_title, task_description, context
    );
    run_ai_command(
        &app,
        prompt,
        image_paths,
        task_id,
        None,
        working_dir,
        None,
        None,
        None,
        None,
    )
    .await
    .map(|result| result.text)
}

#[tauri::command]
pub async fn ai_generate_plan(
    app: AppHandle,
    task_title: String,
    task_description: String,
    task_status: String,
    task_priority: String,
    subtasks: Vec<String>,
    image_paths: Option<Vec<String>>,
    task_id: Option<String>,
    working_dir: Option<String>,
) -> Result<String, String> {
    let templates = load_ai_prompt_templates(&app)
        .unwrap_or_else(|_| crate::codex::default_ai_prompt_templates());
    let plan_template = find_ai_prompt_template(&templates, "coordinator_plan");
    let prompt = build_ai_generate_plan_prompt(
        &task_title,
        &task_description,
        &task_status,
        &task_priority,
        &subtasks,
        plan_template,
    );
    run_ai_command(
        &app,
        prompt,
        image_paths,
        task_id,
        None,
        working_dir,
        None,
        None,
        None,
        None,
    )
    .await
    .map(|result| result.text)
}

#[derive(Debug, Deserialize)]
struct CoordinatorPlanStepDraft {
    title: String,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    success_criteria: Option<String>,
    #[serde(default)]
    employee_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CoordinatorPlanStructured {
    markdown: String,
    steps: Vec<CoordinatorPlanStepDraft>,
}

fn extract_json_object_slice(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        let after_lang = after_fence
            .strip_prefix("json")
            .or_else(|| after_fence.strip_prefix("JSON"))
            .unwrap_or(after_fence);
        let body = after_lang.trim_start_matches(['\r', '\n', ' ']);
        if let Some(end) = body.find("```") {
            let candidate = body[..end].trim();
            if candidate.starts_with('{') {
                return Some(candidate);
            }
        }
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(trimmed[start..=end].trim())
}

fn coordinator_revision_instruction(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn format_pipeline_steps_for_revision_prompt(steps: &[TaskPipelineStep]) -> String {
    if steps.is_empty() {
        return "（暂无工作包）".to_string();
    }
    let payload: Vec<serde_json::Value> = steps
        .iter()
        .map(|step| {
            serde_json::json!({
                "index": step.step_index,
                "title": step.title,
                "goal": step.goal,
                "success_criteria": step.success_criteria,
                "employee_id": step.employee_id,
            })
        })
        .collect();
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "（暂无工作包）".to_string())
}

async fn fetch_pipeline_steps_for_plan(
    pool: &sqlx::SqlitePool,
    task_id: &str,
) -> Result<Vec<TaskPipelineStep>, String> {
    sqlx::query_as::<_, TaskPipelineStep>(
        r#"
        SELECT *
        FROM task_pipeline_steps
        WHERE task_id = $1
        ORDER BY step_index ASC
        "#,
    )
    .bind(task_id)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("Failed to load pipeline steps: {}", error))
}

async fn latest_coordinator_transcript_session_id(
    pool: &sqlx::SqlitePool,
    task_id: &str,
    coordinator_id: &str,
) -> Option<String> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT s.id
        FROM codex_sessions s
        INNER JOIN native_session_transcripts t
            ON t.session_record_id = s.id AND t.deleted_at IS NULL
        WHERE s.task_id = $1
          AND s.employee_id = $2
          AND s.session_kind = 'coordinator'
          AND s.status = 'exited'
        ORDER BY s.started_at DESC
        LIMIT 1
        "#,
    )
    .bind(task_id)
    .bind(coordinator_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

fn parse_coordinator_structured_plan(
    raw: &str,
) -> Result<(String, Vec<CoordinatorPlanStepDraft>), String> {
    if let Some(json_slice) = extract_json_object_slice(raw) {
        if let Ok(parsed) = serde_json::from_str::<CoordinatorPlanStructured>(json_slice) {
            let markdown = parsed.markdown.trim().to_string();
            let steps = parsed
                .steps
                .into_iter()
                .filter(|step| !step.title.trim().is_empty())
                .collect::<Vec<_>>();
            if !markdown.is_empty() && !steps.is_empty() {
                return Ok((markdown, steps));
            }
        }
    }

    // Fallback: treat whole response as markdown and synthesize one step.
    let markdown = raw.trim().to_string();
    if markdown.is_empty() {
        return Err("协调员未返回可用计划".to_string());
    }
    Ok((
        markdown.clone(),
        vec![CoordinatorPlanStepDraft {
            title: "执行协调员计划".to_string(),
            goal: Some(markdown.chars().take(500).collect()),
            success_criteria: Some("完成本任务计划中的主要目标并通过基础验证".to_string()),
            employee_id: None,
        }],
    ))
}

async fn replace_task_pipeline_steps_from_plan(
    pool: &sqlx::SqlitePool,
    task_id: &str,
    steps: &[CoordinatorPlanStepDraft],
    valid_employee_ids: &std::collections::HashSet<String>,
) -> Result<usize, String> {
    sqlx::query("DELETE FROM task_pipeline_steps WHERE task_id = $1")
        .bind(task_id)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to clear previous pipeline steps: {}", error))?;

    let now = now_sqlite();
    for (index, step) in steps.iter().enumerate() {
        let title = step.title.trim();
        if title.is_empty() {
            continue;
        }
        let employee_id = step
            .employee_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .filter(|value| valid_employee_ids.contains(*value))
            .map(str::to_string);
        let goal = step
            .goal
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let success_criteria = step
            .success_criteria
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        sqlx::query(
            r#"
            INSERT INTO task_pipeline_steps (
                id, task_id, step_index, title, goal, success_criteria, employee_id,
                status, session_id, handoff_summary, last_error, started_at, ended_at,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', NULL, NULL, NULL, NULL, NULL, $8, $8)
            "#,
        )
        .bind(new_id())
        .bind(task_id)
        .bind(index as i32)
        .bind(title)
        .bind(&goal)
        .bind(&success_criteria)
        .bind(&employee_id)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|error| format!("Failed to insert pipeline step: {}", error))?;
    }

    let count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM task_pipeline_steps WHERE task_id = $1")
            .bind(task_id)
            .fetch_one(pool)
            .await
            .map_err(|error| format!("Failed to count pipeline steps: {}", error))?;

    if count == 0 {
        return Err("结构化计划未包含有效工作包步骤".to_string());
    }
    Ok(count as usize)
}

#[tauri::command]
pub async fn ai_generate_coordinator_task_plan(
    app: AppHandle,
    payload: GenerateCoordinatorTaskPlanPayload,
) -> Result<CoordinatorTaskPlanResult, String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &payload.task_id).await?;
    let coordinator_id = payload.coordinator_id.trim();
    if coordinator_id.is_empty() {
        return Err("当前任务未指定协调员，无法生成计划".to_string());
    }
    let coordinator = fetch_employee_by_id(&pool, coordinator_id).await?;
    if coordinator.role != "coordinator" {
        return Err(format!(
            "员工 {} 不是协调员角色，无法生成计划",
            coordinator.name
        ));
    }

    let project_employees = sqlx::query_as::<_, Employee>(
        r#"
        SELECT *
        FROM employees
        WHERE project_id = $1 OR project_id IS NULL
        ORDER BY name ASC
        "#,
    )
    .bind(&task.project_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("Failed to list project employees: {}", error))?;

    let employee_lines = if project_employees.is_empty() {
        "（暂无项目员工）".to_string()
    } else {
        project_employees
            .iter()
            .map(|employee| {
                format!(
                    "- id={} | name={} | role={} | provider={} | model={}",
                    employee.id, employee.name, employee.role, employee.ai_provider, employee.model
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let valid_employee_ids = project_employees
        .iter()
        .map(|employee| employee.id.clone())
        .collect::<std::collections::HashSet<_>>();

    let subtasks = fetch_task_subtasks(&pool, &task.id).await?;
    let attachments = fetch_task_attachments(&pool, &task.id).await?;
    let file_refs = fetch_task_file_refs(&pool, &task.id).await?;
    let subtask_titles = subtasks
        .iter()
        .map(|subtask| subtask.title.clone())
        .collect::<Vec<_>>();
    let attachment_items = build_plan_attachment_prompt_items(&attachments);
    let task_title = payload.title.trim();
    let task_description = payload
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| task.description.as_deref().unwrap_or_default());
    let task_status = payload.status.trim();
    let task_priority = payload.priority.trim();
    let templates = load_ai_prompt_templates(&app)
        .unwrap_or_else(|_| crate::codex::default_ai_prompt_templates());
    let revision_instruction =
        coordinator_revision_instruction(payload.revision_instruction.as_deref());
    let is_revision = revision_instruction.is_some();
    if is_revision {
        let has_markdown = payload
            .current_markdown
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
            || task
                .plan_content
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty());
        if !has_markdown {
            return Err("当前没有可修改的计划，请先生成".to_string());
        }
    }
    let plan_scene = if is_revision {
        "coordinator_plan_revise"
    } else {
        "coordinator_plan"
    };
    let plan_template = find_ai_prompt_template(&templates, plan_scene);
    let resolved_title = if task_title.is_empty() {
        task.title.as_str()
    } else {
        task_title
    };
    let resolved_status = if task_status.is_empty() {
        task.status.as_str()
    } else {
        task_status
    };
    let resolved_priority = if task_priority.is_empty() {
        task.priority.as_str()
    } else {
        task_priority
    };
    let base_prompt = if let Some(instruction) = revision_instruction.as_deref() {
        let current_markdown = payload
            .current_markdown
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                task.plan_content
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
            .unwrap_or_default();
        let steps = fetch_pipeline_steps_for_plan(&pool, &task.id).await?;
        build_ai_revise_plan_prompt(
            resolved_title,
            task_description,
            resolved_status,
            resolved_priority,
            &current_markdown,
            &format_pipeline_steps_for_revision_prompt(&steps),
            instruction,
            &subtask_titles,
            &attachment_items,
            plan_template,
        )
    } else {
        build_ai_generate_plan_prompt_with_attachments(
            resolved_title,
            task_description,
            resolved_status,
            resolved_priority,
            &subtask_titles,
            &attachment_items,
            plan_template,
        )
    };
    let file_ref_paths: Vec<String> = file_refs
        .iter()
        .map(|item| item.relative_path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect();
    let file_ref_block = format_task_file_refs_prompt_section(&file_ref_paths)
        .map(|section| format!("\n\n{section}"))
        .unwrap_or_default();
    let prompt = format!(
        "{}{}\n\n可用员工（请仅使用下列 id 作为 steps.employee_id）：\n{}\n任务负责人 assignee_id：{}",
        base_prompt,
        file_ref_block,
        employee_lines,
        task.assignee_id.as_deref().unwrap_or("（未设置）")
    );
    let image_paths = attachments
        .iter()
        .filter(|attachment| task_attachment_is_image(attachment))
        .map(|attachment| attachment.stored_path.clone())
        .collect::<Vec<_>>();

    let execution_context = resolve_task_project_execution_context(&app, &task.id)
        .await
        .unwrap_or_else(|_| ExecutionContext::local_default());
    let working_dir = resolve_one_shot_working_dir(
        &app,
        Some(&task.id),
        Some(&task.project_id),
        payload.working_dir.as_deref(),
    )
    .await
    .ok()
    .flatten()
    .or_else(|| execution_context.working_dir.clone());
    let session = insert_codex_session_record(
        &app,
        Some(&coordinator.id),
        Some(&task.id),
        None,
        working_dir.as_deref(),
        None,
        COORDINATOR_SESSION_KIND,
        "running",
        &execution_context.execution_target,
        execution_context.ssh_config_id.as_deref(),
        execution_context.target_host_label.as_deref(),
        &execution_context.artifact_capture_mode,
        Some(coordinator.ai_provider.as_str()),
        None,
    )
    .await?;
    insert_codex_session_event(
        &pool,
        &session.id,
        "session_requested",
        Some("协调员计划会话已创建"),
    )
    .await?;

    let resume_session_id = if is_revision && coordinator.ai_provider.trim() == "native" {
        latest_coordinator_transcript_session_id(&pool, &task.id, &coordinator.id).await
    } else {
        None
    };
    let command_options = AiCommandOptions {
        progress_request_id: payload.request_id.clone(),
        task_id_for_progress: Some(task.id.clone()),
        session_record_id: Some(session.id.clone()),
        read_only_tools: true,
        resume_session_id: resume_session_id.clone(),
    };
    if is_revision && resume_session_id.is_none() {
        emit_ai_command_line(
            &app,
            &command_options,
            "[计划] 未恢复原会话，已基于当前计划修改",
        )
        .await;
    }
    let run_result = if coordinator.ai_provider.trim() == "native" {
        run_native_ai_command(
            &app,
            coordinator.id.clone(),
            prompt,
            Some(image_paths),
            Some(task.id.clone()),
            Some(task.project_id.clone()),
            payload.working_dir,
            Some(coordinator.model.clone()),
            Some(coordinator.reasoning_effort.clone()),
            &command_options,
        )
        .await
    } else {
        run_ai_command_with_options(
            &app,
            prompt,
            Some(image_paths),
            Some(task.id.clone()),
            Some(task.project_id.clone()),
            payload.working_dir,
            Some(coordinator.ai_provider.clone()),
            Some(coordinator.model.clone()),
            Some(coordinator.reasoning_effort.clone()),
            Some(coordinator.id.clone()),
            &command_options,
        )
        .await
    };
    let result = match run_result {
        Ok(result) => result,
        Err(error) => {
            let _ = finish_coordinator_plan_session(&app, &session.id, false).await;
            return Err(error);
        }
    };

    let outcome = async {
        let (markdown, steps) = parse_coordinator_structured_plan(&result.text)?;
        let step_count =
            replace_task_pipeline_steps_from_plan(&pool, &task.id, &steps, &valid_employee_ids)
                .await?;

        save_task_plan_content(&pool, &task.id, &markdown, Some(&coordinator.id)).await?;

        let usage_line = match result.usage_line.clone() {
            Some(line) => Some(line),
            None => coordinator_session_usage_line(&pool, &session.id).await,
        };
        let generated_at = now_sqlite();
        let details = format_task_plan_generated_activity_details(
            &task.title,
            &coordinator.name,
            &coordinator.ai_provider,
            &coordinator.model,
            &coordinator.reasoning_effort,
            &generated_at,
            usage_line.as_deref(),
        );
        insert_activity_log(
            &pool,
            if is_revision {
                "task_plan_revised"
            } else {
                "task_plan_generated"
            },
            &details,
            Some(&coordinator.id),
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;
        insert_activity_log(
            &pool,
            "task_pipeline_plan_saved",
            &format!("{}（工作包 {} 步）", task.title, step_count),
            Some(&coordinator.id),
            Some(&task.id),
            Some(&task.project_id),
        )
        .await?;

        Ok(CoordinatorTaskPlanResult {
            markdown,
            usage_line,
        })
    }
    .await;
    let _ = finish_coordinator_plan_session(&app, &session.id, outcome.is_ok()).await;
    outcome
}

#[tauri::command]
pub async fn ai_generate_tester_acceptance(
    app: AppHandle,
    payload: GenerateTesterAcceptancePayload,
) -> Result<String, String> {
    let pool = sqlite_pool(&app).await?;
    let task = fetch_task_by_id(&pool, &payload.task_id).await?;
    let tester = resolve_tester_for_acceptance(
        &pool,
        task.reviewer_id.as_deref(),
        payload.tester_id.as_deref(),
    )
    .await?;

    let subtasks = fetch_task_subtasks(&pool, &task.id).await?;
    let subtask_titles = subtasks
        .iter()
        .map(|subtask| subtask.title.clone())
        .collect::<Vec<_>>();
    let templates = load_ai_prompt_templates(&app)
        .unwrap_or_else(|_| crate::codex::default_ai_prompt_templates());
    let acceptance_template = find_ai_prompt_template(&templates, "tester_acceptance");
    let prompt = build_ai_generate_tester_acceptance_prompt(
        &task.title,
        task.description.as_deref().unwrap_or_default(),
        &task.status,
        &task.priority,
        &subtask_titles,
        acceptance_template,
    );

    let result = run_ai_command(
        &app,
        prompt,
        None,
        Some(task.id.clone()),
        Some(task.project_id.clone()),
        payload.working_dir,
        Some(tester.ai_provider.clone()),
        Some(tester.model.clone()),
        Some(tester.reasoning_effort.clone()),
        Some(tester.id.clone()),
    )
    .await?;

    let comment_content = format_tester_acceptance_comment(&result.text);
    if comment_content.trim() == "[验收清单]" {
        return Err("测试员未返回可用的验收清单".to_string());
    }

    let comment_id = new_id();
    sqlx::query(
        "INSERT INTO comments (id, task_id, employee_id, content, is_ai_generated) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&comment_id)
    .bind(&task.id)
    .bind(&tester.id)
    .bind(&comment_content)
    .bind(1_i64)
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to save tester acceptance comment: {}", error))?;

    sqlx::query(
        "UPDATE tasks SET acceptance_checklist = $2, updated_at = $3 WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(&task.id)
    .bind(&result.text)
    .bind(now_sqlite())
    .execute(&pool)
    .await
    .map_err(|error| format!("Failed to save task acceptance checklist: {}", error))?;

    let generated_at = now_sqlite();
    let details = format_tester_acceptance_generated_activity_details(
        &task.title,
        &tester.name,
        &tester.ai_provider,
        &tester.model,
        &tester.reasoning_effort,
        &generated_at,
    );
    insert_activity_log(
        &pool,
        "task_tester_acceptance_generated",
        &details,
        Some(&tester.id),
        Some(&task.id),
        Some(&task.project_id),
    )
    .await?;

    Ok(comment_content)
}

#[tauri::command]
pub async fn ai_generate_commit_message(
    app: AppHandle,
    project_id: String,
    current_branch: Option<String>,
    working_tree_summary: Option<String>,
    staged_changes: Vec<String>,
) -> Result<String, String> {
    let result = generate_commit_message_for_project(
        &app,
        &project_id,
        current_branch.as_deref(),
        working_tree_summary.as_deref(),
        &staged_changes,
    )
    .await?;

    let pool = sqlite_pool(&app).await?;
    let generated_at = now_sqlite();
    let details = format_commit_message_activity_details(
        &result.project_name,
        &result.provider,
        &result.model,
        &result.reasoning_effort,
        &generated_at,
        result.message.lines().next().unwrap_or("未命名提交"),
    );
    insert_activity_log(
        &pool,
        "project_git_commit_message_generated",
        &details,
        None,
        None,
        Some(&result.project_id),
    )
    .await?;

    Ok(result.message)
}

#[tauri::command]
pub async fn ai_optimize_prompt(
    app: AppHandle,
    scene: String,
    project_id: Option<String>,
    project_name: String,
    project_description: Option<String>,
    project_repo_path: Option<String>,
    title: Option<String>,
    description: Option<String>,
    current_prompt: Option<String>,
    task_title: Option<String>,
    session_summary: Option<String>,
    task_id: Option<String>,
    working_dir: Option<String>,
    employee_role: Option<String>,
    employee_specialization: Option<String>,
    employee_draft_system_prompt: Option<String>,
) -> Result<String, String> {
    let templates = load_ai_prompt_templates(&app)
        .unwrap_or_else(|_| crate::codex::default_ai_prompt_templates());
    let scene_template = find_ai_prompt_template(&templates, &scene);
    let prompt = build_ai_optimize_prompt_prompt(
        &scene,
        &project_name,
        project_description.as_deref(),
        project_repo_path.as_deref(),
        title.as_deref(),
        description.as_deref(),
        current_prompt.as_deref(),
        task_title.as_deref(),
        session_summary.as_deref(),
        employee_role.as_deref(),
        employee_specialization.as_deref(),
        employee_draft_system_prompt.as_deref(),
        scene_template,
    )?;

    let result = run_ai_command(
        &app,
        prompt,
        None,
        task_id.clone(),
        project_id.clone(),
        working_dir,
        None,
        None,
        None,
        None,
    )
    .await?;
    let result = result.text;

    let scene_label = scene_template
        .map(|template| template.label.as_str())
        .filter(|label| !label.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            resolve_ai_optimize_prompt_scene_label(&scene)
                .unwrap_or("提示词优化")
                .to_string()
        });
    let (resolved_project_id, model, reasoning_effort) =
        resolve_ai_optimize_prompt_activity_context(
            &app,
            task_id.as_deref(),
            project_id.as_deref(),
        )
        .await?;
    let generated_at = now_sqlite();
    let details = format_ai_optimize_prompt_activity_details(
        &project_name,
        &scene_label,
        &model,
        &reasoning_effort,
        &generated_at,
    );
    let pool = sqlite_pool(&app).await?;
    insert_activity_log(
        &pool,
        "ai_prompt_optimized",
        &details,
        None,
        task_id.as_deref(),
        resolved_project_id.as_deref(),
    )
    .await?;

    Ok(result)
}

#[tauri::command]
pub async fn ai_split_subtasks(
    app: AppHandle,
    task_title: String,
    task_description: String,
    image_paths: Option<Vec<String>>,
    task_id: Option<String>,
    working_dir: Option<String>,
) -> Result<Vec<String>, String> {
    let templates = load_ai_prompt_templates(&app)
        .unwrap_or_else(|_| crate::codex::default_ai_prompt_templates());
    let template = find_ai_prompt_template(&templates, "generate_subtasks");
    let output_goal = template
        .map(|value| value.output_goal.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(
            "你是任务拆分助手。请根据任务标题和描述拆分 3 到 8 个可执行、可验证、粒度适中的子任务。",
        );
    let scene_requirement = template
        .map(|value| value.scene_requirement.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(
            "- 只返回 JSON，不要 Markdown，不要额外解释\n\
- 返回格式必须是 {\"subtasks\":[\"子任务1\",\"子任务2\"]}\n\
- 每个子任务一句话，使用中文，避免重复和空泛表述\n\
- 如果本次输入附带图片，也要结合图片内容拆分任务\n\
- 如果描述信息有限，也基于现有信息给出合理拆分",
        );
    let prompt = format!(
        "{}\n\
要求：\n\
{}\n\n\
任务标题：{}\n\
任务描述：{}",
        output_goal,
        scene_requirement,
        task_title.trim(),
        task_description.trim()
    );
    let raw = run_ai_command(
        &app,
        prompt,
        image_paths,
        task_id,
        None,
        working_dir,
        None,
        None,
        None,
        None,
    )
    .await?;
    parse_ai_subtasks_response(&raw.text)
}

#[cfg(test)]
mod tests {
    use crate::db::models::{CodexSettings, GitPreferences};

    use super::{
        build_ai_revise_plan_prompt, coordinator_revision_instruction,
        format_ai_optimize_prompt_activity_details, format_commit_message_activity_details,
        format_pipeline_steps_for_revision_prompt, format_task_plan_generated_activity_details,
        resolve_ai_optimize_prompt_scene_label, resolve_commit_message_ai_selection,
    };
    use crate::codex::find_ai_prompt_template;
    use crate::db::models::TaskPipelineStep;

    fn test_settings(
        one_shot_provider: &str,
        git_provider: &str,
        git_model_source: &str,
    ) -> CodexSettings {
        CodexSettings {
            task_sdk_enabled: false,
            one_shot_sdk_enabled: false,
            one_shot_model: "gpt-5.4".to_string(),
            one_shot_reasoning_effort: "high".to_string(),
            task_automation_default_enabled: false,
            task_automation_max_fix_rounds: 3,
            task_automation_failure_strategy: "blocked".to_string(),
            tester_automation_enabled: false,
            tester_allow_ai_only: false,
            default_test_command: None,
            git_preferences: GitPreferences {
                default_task_use_worktree: false,
                worktree_location_mode: "repo_sibling_hidden".to_string(),
                worktree_custom_root: None,
                ai_commit_message_length: "title_with_body".to_string(),
                ai_commit_preferred_provider: git_provider.to_string(),
                ai_commit_model_source: git_model_source.to_string(),
                ai_commit_model: "sonnet".to_string(),
                ai_commit_reasoning_effort: "xhigh".to_string(),
                ai_commit_native_channel_id: None,
            },
            node_path_override: None,
            sdk_install_dir: "/tmp/codex-sdk".to_string(),
            one_shot_preferred_provider: one_shot_provider.to_string(),
            one_shot_native_channel_id: None,
            max_concurrent_sessions: 3,
        }
    }

    #[test]
    fn resolves_ai_optimize_prompt_scene_labels() {
        assert_eq!(
            resolve_ai_optimize_prompt_scene_label("task_create").expect("task_create"),
            "新建任务"
        );
        assert_eq!(
            resolve_ai_optimize_prompt_scene_label("task_continue").expect("task_continue"),
            "任务继续对话"
        );
        assert_eq!(
            resolve_ai_optimize_prompt_scene_label("session_continue").expect("session_continue"),
            "Session 继续对话"
        );
        assert_eq!(
            resolve_ai_optimize_prompt_scene_label("employee_system_prompt")
                .expect("employee_system_prompt"),
            "员工系统提示词生成"
        );
    }

    #[test]
    fn formats_ai_optimize_prompt_activity_details_with_model_metadata() {
        let details = format_ai_optimize_prompt_activity_details(
            "Codex AI",
            "新建任务",
            "gpt-5.4",
            "high",
            "2026-04-20 10:30:00",
        );

        assert_eq!(
            details,
            "项目：Codex AI；场景：新建任务；模型：gpt-5.4；推理等级：high；生成时间：2026-04-20 10:30:00"
        );
    }

    #[test]
    fn formats_commit_message_activity_details_with_model_metadata() {
        let details = format_commit_message_activity_details(
            "Codex AI",
            "codex",
            "gpt-5.4-mini",
            "medium",
            "2026-04-20 11:00:00",
            "fix: 修复活动日志展示",
        );

        assert_eq!(
            details,
            "项目：Codex AI；Provider：codex；模型：gpt-5.4-mini；推理等级：medium；生成时间：2026-04-20 11:00:00；结果：fix: 修复活动日志展示"
        );
    }

    #[test]
    fn formats_task_plan_generated_activity_details_with_coordinator_metadata() {
        let details = format_task_plan_generated_activity_details(
            "任务 A",
            "协调员小张",
            "claude",
            "sonnet",
            "high",
            "2026-04-28 09:00:00",
            None,
        );

        assert_eq!(
            details,
            "任务：任务 A；协调员：协调员小张；Provider：claude；模型：sonnet；推理等级：high；生成时间：2026-04-28 09:00:00"
        );
    }

    #[test]
    fn formats_task_plan_generated_activity_details_with_usage() {
        let details = format_task_plan_generated_activity_details(
            "任务 A",
            "协调员小张",
            "native",
            "gpt-4o",
            "high",
            "2026-04-28 09:00:00",
            Some("[用量] in=10 out=4 total=14"),
        );

        assert_eq!(
            details,
            "任务：任务 A；协调员：协调员小张；Provider：native；模型：gpt-4o；推理等级：high；生成时间：2026-04-28 09:00:00；用量：in=10 out=4 total=14"
        );
    }

    #[test]
    fn custom_commit_message_ai_selection_uses_git_provider() {
        let settings = test_settings("codex", "claude", "custom");

        let selection = resolve_commit_message_ai_selection(&settings);

        assert_eq!(selection.provider_override.as_deref(), Some("claude"));
        assert_eq!(selection.model_override.as_deref(), Some("sonnet"));
        assert_eq!(selection.reasoning_override.as_deref(), Some("xhigh"));
        assert_eq!(selection.effective_provider, "claude");
        assert_eq!(selection.effective_model, "sonnet");
        assert_eq!(selection.effective_reasoning_effort, "xhigh");
    }

    #[test]
    fn coordinator_revision_instruction_trims_and_rejects_blank() {
        assert_eq!(coordinator_revision_instruction(None), None);
        assert_eq!(coordinator_revision_instruction(Some("  ")), None);
        assert_eq!(
            coordinator_revision_instruction(Some(" 拆第2步 ")).as_deref(),
            Some("拆第2步")
        );
    }

    #[test]
    fn format_pipeline_steps_for_revision_prompt_includes_titles() {
        let steps = vec![TaskPipelineStep {
            id: "s1".to_string(),
            task_id: "t1".to_string(),
            step_index: 0,
            title: "设计接口".to_string(),
            goal: Some("写出 API".to_string()),
            success_criteria: Some("有 OpenAPI".to_string()),
            employee_id: Some("emp-1".to_string()),
            status: "pending".to_string(),
            session_id: None,
            handoff_summary: None,
            last_error: None,
            started_at: None,
            ended_at: None,
            created_at: "2026-09-01 00:00:00".to_string(),
            updated_at: "2026-09-01 00:00:00".to_string(),
        }];
        let rendered = format_pipeline_steps_for_revision_prompt(&steps);
        assert!(rendered.contains("设计接口"));
        assert!(rendered.contains("emp-1"));
        assert_eq!(
            format_pipeline_steps_for_revision_prompt(&[]),
            "（暂无工作包）"
        );
    }

    #[test]
    fn revise_plan_prompt_includes_current_plan_and_instruction() {
        let templates = crate::codex::default_ai_prompt_templates();
        let template = find_ai_prompt_template(&templates, "coordinator_plan_revise");
        let prompt = build_ai_revise_plan_prompt(
            "任务 A",
            "描述",
            "todo",
            "high",
            "# 旧计划",
            "[{\"title\":\"步骤一\"}]",
            "把第1步拆成两步",
            &[],
            &[],
            template,
        );
        assert!(prompt.contains("# 旧计划"));
        assert!(prompt.contains("把第1步拆成两步"));
        assert!(prompt.contains("当前工作包 JSON"));
        assert!(prompt.contains("用户修改意见"));
    }

    #[test]
    fn inherited_commit_message_ai_selection_keeps_one_shot_provider() {
        let settings = test_settings("codex", "claude", "inherit_one_shot");

        let selection = resolve_commit_message_ai_selection(&settings);

        assert_eq!(selection.provider_override, None);
        assert_eq!(selection.model_override, None);
        assert_eq!(selection.reasoning_override, None);
        assert_eq!(selection.effective_provider, "codex");
        assert_eq!(selection.effective_model, "gpt-5.4");
        assert_eq!(selection.effective_reasoning_effort, "high");
    }

    #[test]
    fn custom_native_commit_message_ai_selection_uses_git_channel() {
        let mut settings = test_settings("codex", "native", "custom");
        settings.git_preferences.ai_commit_native_channel_id = Some("chan-1".to_string());

        let selection = resolve_commit_message_ai_selection(&settings);

        assert_eq!(selection.provider_override.as_deref(), Some("native"));
        assert_eq!(selection.model_override.as_deref(), Some("sonnet"));
        assert_eq!(selection.reasoning_override.as_deref(), Some("xhigh"));
        assert_eq!(selection.native_channel_id.as_deref(), Some("chan-1"));
        assert_eq!(selection.effective_provider, "native");
        assert_eq!(selection.effective_model, "sonnet");
        assert_eq!(selection.effective_reasoning_effort, "xhigh");
    }

    #[test]
    fn inherited_native_commit_message_ai_selection_uses_one_shot_channel() {
        let mut settings = test_settings("native", "codex", "inherit_one_shot");
        settings.one_shot_native_channel_id = Some("chan-2".to_string());

        let selection = resolve_commit_message_ai_selection(&settings);

        assert_eq!(selection.provider_override, None);
        assert_eq!(selection.native_channel_id.as_deref(), Some("chan-2"));
        assert_eq!(selection.effective_provider, "native");
        assert_eq!(selection.effective_model, "gpt-5.4");
        assert_eq!(selection.effective_reasoning_effort, "high");
    }
}
