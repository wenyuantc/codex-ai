use tauri::AppHandle;

use super::{
    build_ai_generate_commit_message_prompt, build_ai_generate_plan_prompt,
    build_ai_optimize_prompt_prompt, parse_ai_subtasks_response, run_ai_command,
};
use crate::app::{fetch_project_by_id, insert_activity_log, sqlite_pool};

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
    run_ai_command(&app, prompt, image_paths, task_id, None, working_dir).await
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
    run_ai_command(&app, prompt, image_paths, task_id, None, working_dir).await
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
    run_ai_command(&app, prompt, image_paths, task_id, None, working_dir).await
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
    run_ai_command(&app, prompt, image_paths, task_id, None, working_dir).await
}

#[tauri::command]
pub async fn ai_generate_commit_message(
    app: AppHandle,
    project_id: String,
    current_branch: Option<String>,
    working_tree_summary: Option<String>,
    staged_changes: Vec<String>,
) -> Result<String, String> {
    let normalized_staged_changes = staged_changes
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if normalized_staged_changes.is_empty() {
        return Err("当前没有可用于生成提交信息的已暂存文件".to_string());
    }

    let pool = sqlite_pool(&app).await?;
    let project = fetch_project_by_id(&pool, &project_id).await?;
    let prompt = build_ai_generate_commit_message_prompt(
        project.name.trim(),
        current_branch.as_deref(),
        working_tree_summary.as_deref(),
        &normalized_staged_changes,
    );
    let result = run_ai_command(&app, prompt, None, None, Some(project.id.clone()), None).await?;
    let mut normalized_lines = Vec::new();
    let mut previous_blank = true;
    for raw_line in result.lines() {
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

    let details = format!(
        "AI 已生成提交信息：{}",
        normalized.lines().next().unwrap_or("未命名提交")
    );
    insert_activity_log(
        &pool,
        "project_git_commit_message_generated",
        &details,
        None,
        None,
        Some(&project.id),
    )
    .await?;

    Ok(normalized)
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

    run_ai_command(&app, prompt, None, task_id, None, working_dir).await
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
    let raw = run_ai_command(&app, prompt, image_paths, task_id, None, working_dir).await?;
    parse_ai_subtasks_response(&raw)
}
