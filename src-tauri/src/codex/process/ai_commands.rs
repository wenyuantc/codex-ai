use tauri::{AppHandle, Runtime};

use super::{
    build_ai_generate_commit_message_prompt, build_ai_generate_plan_prompt,
    build_ai_optimize_prompt_prompt, parse_ai_subtasks_response, run_ai_command,
};
use crate::app::{fetch_project_by_id, insert_activity_log, sqlite_pool, PROJECT_TYPE_SSH};
use crate::codex::{load_codex_settings, load_remote_codex_settings};

const COMMIT_MESSAGE_PROCESS_TERMS: &[&str] = &[
    "暂存",
    "已暂存",
    "工作区",
    "核对",
    "文件列表",
    "待提交文件",
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

    !subject.is_empty()
        && COMMIT_MESSAGE_PROCESS_TERMS
            .iter()
            .any(|term| subject.contains(&term.to_lowercase()))
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

pub(crate) async fn generate_commit_message_for_project<R: Runtime>(
    app: &AppHandle<R>,
    project_id: &str,
    current_branch: Option<&str>,
    working_tree_summary: Option<&str>,
    staged_changes: &[String],
) -> Result<String, String> {
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
    let (model_override, reasoning_override) =
        if settings.git_preferences.ai_commit_model_source == "custom" {
            (
                Some(settings.git_preferences.ai_commit_model.clone()),
                Some(settings.git_preferences.ai_commit_reasoning_effort.clone()),
            )
        } else {
            (None, None)
        };
    let raw = run_ai_command(
        app,
        prompt.clone(),
        None,
        None,
        Some(project.id.clone()),
        None,
        model_override.clone(),
        reasoning_override.clone(),
    )
    .await?;
    let normalized = normalize_generated_commit_message(&raw)?;
    let validation_error = match validate_generated_commit_message(
        &normalized,
        &settings.git_preferences.ai_commit_message_length,
    ) {
        Ok(()) => return Ok(normalized),
        Err(error) => error,
    };
    let retry_prompt = build_commit_message_retry_prompt(
        &prompt,
        &normalized,
        &validation_error,
        &settings.git_preferences.ai_commit_message_length,
    );
    let retried_raw = run_ai_command(
        app,
        retry_prompt,
        None,
        None,
        Some(project.id.clone()),
        None,
        model_override,
        reasoning_override,
    )
    .await?;
    let retried = normalize_generated_commit_message(&retried_raw)?;
    if validate_generated_commit_message(&retried, &settings.git_preferences.ai_commit_message_length)
        .is_ok()
    {
        return Ok(retried);
    }

    Err(format!(
        "AI 生成的提交信息仍不符合要求（{}），请手动确认后再提交",
        settings.git_preferences.ai_commit_message_length
    ))
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
    let prompt = format!(
        "Based on the following task description, suggest the best assignee from the employee list. If task images are attached, consider them as additional context.\n\nTask: {}\n\nEmployees: {}\n\nRespond with just the employee ID and a brief reason.",
        task_description, employee_list
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
    )
    .await
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
    )
    .await
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
    )
    .await
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
    let prompt = build_ai_generate_plan_prompt(
        &task_title,
        &task_description,
        &task_status,
        &task_priority,
        &subtasks,
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
    )
    .await
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
    let details = format!(
        "AI 已生成提交信息：{}",
        result.lines().next().unwrap_or("未命名提交")
    );
    insert_activity_log(
        &pool,
        "project_git_commit_message_generated",
        &details,
        None,
        None,
        Some(&project_id),
    )
    .await?;

    Ok(result)
}

#[tauri::command]
pub async fn ai_optimize_prompt(
    app: AppHandle,
    scene: String,
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
) -> Result<String, String> {
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
    )?;

    run_ai_command(&app, prompt, None, task_id, None, working_dir, None, None).await
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
    let prompt = format!(
        "你是任务拆分助手。请根据任务标题和描述拆分 3 到 8 个可执行、可验证、粒度适中的子任务。\n\
要求：\n\
- 只返回 JSON，不要 Markdown，不要额外解释\n\
- 返回格式必须是 {{\"subtasks\":[\"子任务1\",\"子任务2\"]}}\n\
- 每个子任务一句话，使用中文，避免重复和空泛表述\n\
- 如果本次输入附带图片，也要结合图片内容拆分任务\n\
- 如果描述信息有限，也基于现有信息给出合理拆分\n\n\
任务标题：{}\n\
任务描述：{}",
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
    )
    .await?;
    parse_ai_subtasks_response(&raw)
}
