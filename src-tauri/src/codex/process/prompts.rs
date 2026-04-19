use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct AiSubtasksPayload {
    subtasks: Vec<String>,
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
    items
        .into_iter()
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .collect()
}

pub(super) fn parse_ai_subtasks_response(raw: &str) -> Result<Vec<String>, String> {
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

fn normalize_ai_optimize_prompt_field(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("未填写")
        .to_string()
}

fn resolve_ai_optimize_prompt_scene(
    scene: &str,
) -> Result<(&'static str, &'static str, &'static str), String> {
    match scene.trim() {
        "task_create" => Ok((
            "新建任务",
            "请输出一段适合作为任务详情的中文正文，帮助后续 AI / Codex 更准确地理解目标、范围、约束和预期产出。",
            "可以补齐任务背景、目标、关键限制、验收期望，但不要伪造仓库细节或未提供的事实。",
        )),
        "task_continue" => Ok((
            "任务继续对话",
            "请输出一段适合作为续聊输入的中文正文，用于推动当前任务继续执行。",
            "可以明确当前目标、下一步动作、需要重点检查的约束和期望反馈，让续聊内容更利于继续执行。",
        )),
        "session_continue" => Ok((
            "Session 继续对话",
            "请输出一段适合作为续聊输入的中文正文，用于在既有 Session 上继续推进工作。",
            "可以结合 Session 摘要和关联任务，聚焦下一步动作、约束与期望结果，让续聊内容更便于延续上下文。",
        )),
        other => Err(format!("不支持的提示词优化场景: {}", other)),
    }
}

pub(super) fn build_ai_optimize_prompt_prompt(
    scene: &str,
    project_name: &str,
    project_description: Option<&str>,
    project_repo_path: Option<&str>,
    title: Option<&str>,
    description: Option<&str>,
    current_prompt: Option<&str>,
    task_title: Option<&str>,
    session_summary: Option<&str>,
) -> Result<String, String> {
    let (scene_label, output_goal, scene_requirement) = resolve_ai_optimize_prompt_scene(scene)?;

    Ok(format!(
        "你是提示词优化助手。请基于给定的项目上下文和当前输入，直接输出一段已经优化好的中文提示词正文。\n\
场景：{}\n\
输出目标：{}\n\
场景补充要求：{}\n\
\n\
统一要求：\n\
- 只返回可直接使用的中文正文，不要 Markdown 代码块，不要解释，不要额外前后缀\n\
- 项目上下文始终优先，输出需要贴合项目领域、已有任务信息和当前输入\n\
- 可以补齐更利于执行的信息结构，但不要捏造未提供的事实、文件、接口或验证结果\n\
- 如果当前输入为空或信息不足，也要输出一个可直接使用的项目导向默认草稿\n\
- 保持语气明确、可执行、便于 AI / Codex 理解\n\
\n\
项目信息：\n\
- 项目名称：{}\n\
- 项目描述：{}\n\
- 仓库路径：{}\n\
\n\
当前上下文：\n\
- 标题：{}\n\
- 描述：{}\n\
- 当前续聊输入：{}\n\
- 任务标题：{}\n\
- Session 摘要：{}",
        scene_label,
        output_goal,
        scene_requirement,
        normalize_ai_optimize_prompt_field(Some(project_name)),
        normalize_ai_optimize_prompt_field(project_description),
        normalize_ai_optimize_prompt_field(project_repo_path),
        normalize_ai_optimize_prompt_field(title),
        normalize_ai_optimize_prompt_field(description),
        normalize_ai_optimize_prompt_field(current_prompt),
        normalize_ai_optimize_prompt_field(task_title),
        normalize_ai_optimize_prompt_field(session_summary),
    ))
}

pub(super) fn build_ai_generate_plan_prompt(
    task_title: &str,
    task_description: &str,
    task_status: &str,
    task_priority: &str,
    subtasks: &[String],
) -> String {
    let normalized_subtasks = subtasks
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    let subtasks_block = if normalized_subtasks.is_empty() {
        "（暂无）".to_string()
    } else {
        normalized_subtasks
            .iter()
            .enumerate()
            .map(|(index, title)| format!("{}. {}", index + 1, title))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "你是任务规划助手。请基于给定任务信息输出一份接近 Codex /plan 风格的中文 Markdown 执行计划。\n\
要求：\n\
- 只返回 Markdown 正文，不要代码块，不要 JSON，不要额外客套\n\
- 不要假装你已经读取仓库、查看文件、运行命令或完成验证；缺失信息请写入“风险与依赖”或“假设”\n\
- 如果本次输入附带任务图片，也要把图片内容作为计划依据之一\n\
- 必须包含以下标题：# 标题、## 目标与范围、## 实施步骤、## 验收与验证、## 风险与依赖、## 假设\n\
- “实施步骤”使用 1. 2. 3. 编号，步骤需要可执行、可验证，并吸收已有子任务中的有效信息\n\
- 结合当前状态、优先级、任务描述和子任务安排顺序，避免空泛表述\n\
- 如果信息不足，也要输出完整计划，并明确说明前提、依赖和缺口\n\n\
任务标题：{}\n\
当前状态：{}\n\
当前优先级：{}\n\
任务描述：{}\n\
现有子任务：\n{}",
        task_title.trim(),
        task_status.trim(),
        task_priority.trim(),
        if task_description.trim().is_empty() {
            "（未填写）"
        } else {
            task_description.trim()
        },
        subtasks_block
    )
}

pub(super) fn build_ai_generate_commit_message_prompt(
    project_name: &str,
    current_branch: Option<&str>,
    working_tree_summary: Option<&str>,
    staged_changes: &[String],
) -> String {
    let normalized_project_name = normalize_ai_optimize_prompt_field(Some(project_name));
    let normalized_branch = normalize_ai_optimize_prompt_field(current_branch);
    let normalized_summary = normalize_ai_optimize_prompt_field(working_tree_summary);
    let staged_changes_block = if staged_changes.is_empty() {
        "（暂无已暂存文件）".to_string()
    } else {
        staged_changes
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(|value| format!("- {}", value))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "你是 Git commit message 助手。请基于项目上下文和当前待提交改动，生成一条可直接使用的 commit message。\n\
要求：\n\
- 只返回最终 commit message，不要 Markdown，不要代码块，不要解释，不要前后缀\n\
- 输出默认采用两段结构：第一行是 Conventional Commits 标题，空一行后补充 2 到 4 行正文\n\
- 标题优先使用 Conventional Commits 风格：<type>(<scope>): <description>\n\
- type 仅可从 feat、fix、refactor、chore、docs、test、style、perf、build、ci 中选择\n\
- 如果 scope 不明确，可以省略 scope，仅输出 type: description\n\
- 标题里的 description 使用中文，明确说明这批改动的共同目的，不要过短，不要只写“更新”“调整”\n\
- 标题和正文都必须描述真实代码或产品层面的改动结果，例如“调整首页文案”“修复任务状态刷新”，不要描述 Git 操作过程\n\
- 不要在标题或正文里出现“暂存”“已暂存”“工作区”“提交信息”“commit message”“核对内容”“文件列表”这类过程词，除非本次改动本身就在修改 Git 提交流程\n\
- 不要因为输入来自暂存区就默认使用 chore；只有明确是维护、脚手架或非业务改动时才使用 chore\n\
- 正文需要补充说明核心改动点、影响范围或交互变化，让 commit message 比单行标题更完整\n\
- 正文可以写完整句子，也可以写简短条目，但不要逐条机械复述文件列表\n\
- 不要虚构未给出的实现细节，不要输出多种候选\n\
- 新功能优先用 feat，缺陷修复优先用 fix，结构整理优先用 refactor，杂项维护优先用 chore\n\
- 如果改动确实很小，也仍然优先输出“标题 + 至少 1 行正文”，不要只返回单行\n\
\n\
项目信息：\n\
- 项目名称：{}\n\
- 当前分支：{}\n\
- 工作区摘要：{}\n\
\n\
已暂存文件：\n{}",
        normalized_project_name,
        normalized_branch,
        normalized_summary,
        staged_changes_block
    )
}
