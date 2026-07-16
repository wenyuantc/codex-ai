use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::app::{insert_activity_log, sqlite_pool};

const PROMPT_TEMPLATES_FILE_NAME: &str = "ai-prompt-templates.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiPromptTemplate {
    pub scene: String,
    pub label: String,
    pub output_goal: String,
    pub scene_requirement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiPromptTemplatesDocument {
    pub templates: Vec<AiPromptTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAiPromptTemplatesPayload {
    pub templates: Vec<AiPromptTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetAiPromptTemplatesPayload {
    #[serde(default)]
    pub scene: Option<String>,
}

fn app_config_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("无法读取应用配置目录: {error}"))
}

fn prompt_templates_file_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(app_config_dir(app)?.join(PROMPT_TEMPLATES_FILE_NAME))
}

pub fn default_ai_prompt_templates() -> AiPromptTemplatesDocument {
    AiPromptTemplatesDocument {
        templates: vec![
            AiPromptTemplate {
                scene: "task_create".to_string(),
                label: "新建任务".to_string(),
                output_goal: "请输出一段适合作为任务详情的中文正文，帮助后续 AI / Codex 更准确地理解目标、范围、约束和预期产出。".to_string(),
                scene_requirement: "可以补齐任务背景、目标、关键限制、验收期望，但不要伪造仓库细节或未提供的事实。".to_string(),
            },
            AiPromptTemplate {
                scene: "task_continue".to_string(),
                label: "任务继续对话".to_string(),
                output_goal: "请输出一段适合作为续聊输入的中文正文，用于推动当前任务继续执行。".to_string(),
                scene_requirement: "可以明确当前目标、下一步动作、需要重点检查的约束和期望反馈，让续聊内容更利于继续执行。".to_string(),
            },
            AiPromptTemplate {
                scene: "session_continue".to_string(),
                label: "Session 继续对话".to_string(),
                output_goal: "请输出一段适合作为续聊输入的中文正文，用于在既有 Session 上继续推进工作。".to_string(),
                scene_requirement: "可以结合 Session 摘要和关联任务，聚焦下一步动作、约束与期望结果，让续聊内容更便于延续上下文。".to_string(),
            },
            AiPromptTemplate {
                scene: "employee_system_prompt".to_string(),
                label: "员工系统提示词生成".to_string(),
                output_goal: "请输出一段可直接作为 AI 员工 system prompt 使用的中文正文。".to_string(),
                scene_requirement: "必须结合员工角色职责、专长方向和用户已填写的系统提示词草稿进行融合；如果项目上下文存在，要体现项目领域；如果信息不足，也要给出通用但可执行的员工系统提示词。".to_string(),
            },
            AiPromptTemplate {
                scene: "coordinator_plan".to_string(),
                label: "协调员任务计划".to_string(),
                output_goal: "你是任务规划助手。请基于给定任务信息输出一份接近 Codex /plan 风格的中文 Markdown 执行计划。".to_string(),
                scene_requirement: "- 只返回 Markdown 正文，不要代码块，不要 JSON，不要额外客套\n\
- 需要修改/新增的文件代码关键变更\n\
- 不要假装你已经读取仓库、查看文件、运行命令或完成验证；缺失信息请写入“风险与依赖”或“假设”\n\
- 如果本次输入附带任务图片，也要把图片内容作为计划依据之一\n\
- 需要综合附件列表判断上下文；图片附件会额外作为图像输入传入，非图片附件仅能依赖其名称和元信息\n\
- 必须包含以下标题：# 标题、## 目标与范围、## 实施步骤、## 验收与验证、## 风险与依赖、## 假设\n\
- “实施步骤”使用 1. 2. 3. 编号，步骤需要可执行、可验证，并吸收已有子任务中的有效信息\n\
- 结合当前状态、优先级、任务描述和子任务安排顺序，避免空泛表述\n\
- 如果信息不足，也要输出完整计划，并明确说明前提、依赖和缺口".to_string(),
            },
            AiPromptTemplate {
                scene: "review".to_string(),
                label: "代码审查".to_string(),
                output_goal: "你正在执行一次只读代码审查。".to_string(),
                scene_requirement: "- 只允许阅读和分析代码，禁止修改任何文件，禁止执行 git commit/reset/checkout/merge/rebase 等写操作\n\
- 审核范围仅限下方提供的任务信息和当前工作区改动\n\
- 最终结构化判定必须且只能输出在 <review_verdict> 和 </review_verdict> 之间，内容必须是 JSON，对应字段：passed(boolean)、needs_human(boolean)、blocking_issue_count(number)、summary(string)\n\
- 最终人类可读报告必须且只能输出在 <review_report> 和 </review_report> 之间\n\
- 报告必须使用中文 Markdown，包含以下小节：## 结论、## 阻断问题、## 风险提醒、## 改进建议、## 验证缺口\n\
- 如果没有阻断问题，明确写“无阻断问题”\n\
- 如果 diff 信息被截断，要把这件事写进“验证缺口”".to_string(),
            },
            AiPromptTemplate {
                scene: "generate_commit".to_string(),
                label: "生成 Commit Message".to_string(),
                output_goal: "你是 Git commit message 助手。请基于项目上下文和当前待提交改动，生成一条可直接使用的 commit message。".to_string(),
                scene_requirement: "- 只返回最终 commit message，不要 Markdown，不要代码块，不要解释，不要前后缀\n\
- 标题优先使用 Conventional Commits 风格：<type>(<scope>): <description>\n\
- type 仅可从 feat、fix、refactor、chore、docs、test、style、perf、build、ci 中选择\n\
- 如果 scope 不明确，可以省略 scope，仅输出 type: description\n\
- 标题里的 description 使用中文，明确说明这批改动的共同目的，不要过短，不要只写“更新”“调整”\n\
- 标题和正文都必须描述真实代码或产品层面的改动结果，不要描述 Git 操作过程\n\
- 不要虚构未给出的实现细节，不要输出多种候选".to_string(),
            },
            AiPromptTemplate {
                scene: "suggest_assignee".to_string(),
                label: "建议指派人".to_string(),
                output_goal: "Based on the following task description, suggest the best assignee from the employee list. If task images are attached, consider them as additional context.".to_string(),
                scene_requirement: "Respond with just the employee ID and a brief reason.".to_string(),
            },
            AiPromptTemplate {
                scene: "generate_subtasks".to_string(),
                label: "拆分任务".to_string(),
                output_goal: "你是任务拆分助手。请根据任务标题和描述拆分 3 到 8 个可执行、可验证、粒度适中的子任务。".to_string(),
                scene_requirement: "- 只返回 JSON，不要 Markdown，不要额外解释\n\
- 返回格式必须是 {\"subtasks\":[\"子任务1\",\"子任务2\"]}\n\
- 每个子任务一句话，使用中文，避免重复和空泛表述\n\
- 如果本次输入附带图片，也要结合图片内容拆分任务\n\
- 如果描述信息有限，也基于现有信息给出合理拆分".to_string(),
            },
            AiPromptTemplate {
                scene: "tester_acceptance".to_string(),
                label: "测试员验收".to_string(),
                output_goal: "你是资深测试工程师。请基于给定任务信息输出一份中文验收/测试清单，供测试员执行验收。".to_string(),
                scene_requirement: "- 只返回 Markdown 正文，不要代码块，不要 JSON，不要额外客套\n\
- 必须包含以下标题：# 验收清单、## 验收目标、## 前置条件、## 功能验收项、## 边界与异常、## 回归关注点、## 通过标准\n\
- “功能验收项”“边界与异常”使用可勾选的 `- [ ]` 列表，条目具体、可验证、可复现\n\
- 结合任务标题、描述、当前状态、优先级和子任务，覆盖主流程与关键风险点\n\
- 不要假装你已经打开应用、执行测试或查看仓库；缺失信息写入前置条件或通过标准中的假设\n\
- 如果信息不足，也要输出完整清单，并明确说明待确认项".to_string(),
            },
        ],
    }
}

fn validate_templates(templates: &[AiPromptTemplate]) -> Result<(), String> {
    let defaults = default_ai_prompt_templates();
    let allowed: Vec<&str> = defaults
        .templates
        .iter()
        .map(|template| template.scene.as_str())
        .collect();

    if templates.is_empty() {
        return Err("提示词模板列表不能为空".to_string());
    }

    let mut seen = std::collections::HashSet::new();
    for template in templates {
        let scene = template.scene.trim();
        if scene.is_empty() {
            return Err("提示词模板 scene 不能为空".to_string());
        }
        if !allowed.iter().any(|key| *key == scene) {
            return Err(format!("不支持的提示词模板场景: {scene}"));
        }
        if template.label.trim().is_empty() {
            return Err(format!("场景 {scene} 的 label 不能为空"));
        }
        if template.output_goal.trim().is_empty() {
            return Err(format!("场景 {scene} 的 output_goal 不能为空"));
        }
        if template.scene_requirement.trim().is_empty() {
            return Err(format!("场景 {scene} 的 scene_requirement 不能为空"));
        }
        if !seen.insert(scene.to_string()) {
            return Err(format!("提示词模板场景重复: {scene}"));
        }
    }

    for key in allowed {
        if !templates.iter().any(|template| template.scene.trim() == key) {
            return Err(format!("缺少提示词模板场景: {key}"));
        }
    }

    Ok(())
}

fn normalize_templates(templates: Vec<AiPromptTemplate>) -> Vec<AiPromptTemplate> {
    let defaults = default_ai_prompt_templates();
    defaults
        .templates
        .into_iter()
        .map(|default_template| {
            templates
                .iter()
                .find(|template| template.scene.trim() == default_template.scene)
                .map(|template| AiPromptTemplate {
                    scene: default_template.scene.clone(),
                    label: template.label.trim().to_string(),
                    output_goal: template.output_goal.trim().to_string(),
                    scene_requirement: template.scene_requirement.trim().to_string(),
                })
                .unwrap_or(default_template)
        })
        .collect()
}

fn merge_with_defaults(stored: Option<AiPromptTemplatesDocument>) -> AiPromptTemplatesDocument {
    let defaults = default_ai_prompt_templates();
    let Some(stored) = stored else {
        return defaults;
    };

    AiPromptTemplatesDocument {
        templates: normalize_templates(stored.templates),
    }
}

fn read_stored_document<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<AiPromptTemplatesDocument>, String> {
    let path = prompt_templates_file_path(app)?;
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("读取 AI 提示词模板失败: {error}"))?;
    if raw.trim().is_empty() {
        return Ok(None);
    }

    let document = serde_json::from_str::<AiPromptTemplatesDocument>(&raw)
        .map_err(|error| format!("解析 AI 提示词模板失败: {error}"))?;
    Ok(Some(document))
}

fn write_document<R: Runtime>(
    app: &AppHandle<R>,
    document: &AiPromptTemplatesDocument,
) -> Result<(), String> {
    let path = prompt_templates_file_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("创建配置目录失败: {error}"))?;
    }

    let raw = serde_json::to_string_pretty(document)
        .map_err(|error| format!("序列化 AI 提示词模板失败: {error}"))?;
    fs::write(&path, raw).map_err(|error| format!("写入 AI 提示词模板失败: {error}"))
}

fn delete_document_file<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let path = prompt_templates_file_path(app)?;
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("删除 AI 提示词模板失败: {error}"))?;
    }
    Ok(())
}

pub fn load_ai_prompt_templates<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<AiPromptTemplatesDocument, String> {
    let stored = read_stored_document(app)?;
    Ok(merge_with_defaults(stored))
}

pub fn find_ai_prompt_template<'a>(
    document: &'a AiPromptTemplatesDocument,
    scene: &str,
) -> Option<&'a AiPromptTemplate> {
    let key = scene.trim();
    document
        .templates
        .iter()
        .find(|template| template.scene == key)
}

pub fn find_default_ai_prompt_template(scene: &str) -> Option<AiPromptTemplate> {
    find_ai_prompt_template(&default_ai_prompt_templates(), scene).cloned()
}

#[tauri::command]
pub async fn get_ai_prompt_templates<R: Runtime>(
    app: AppHandle<R>,
) -> Result<AiPromptTemplatesDocument, String> {
    load_ai_prompt_templates(&app)
}

#[tauri::command]
pub async fn update_ai_prompt_templates<R: Runtime>(
    app: AppHandle<R>,
    payload: UpdateAiPromptTemplatesPayload,
) -> Result<AiPromptTemplatesDocument, String> {
    validate_templates(&payload.templates)?;
    let document = AiPromptTemplatesDocument {
        templates: normalize_templates(payload.templates),
    };
    write_document(&app, &document)?;

    if let Ok(pool) = sqlite_pool(&app).await {
        let _ = insert_activity_log(
            &pool,
            "ai_prompt_templates_updated",
            &format!("已更新 {} 个 AI 提示词模板", document.templates.len()),
            None,
            None,
            None,
        )
        .await;
    }

    Ok(document)
}

#[tauri::command]
pub async fn reset_ai_prompt_templates<R: Runtime>(
    app: AppHandle<R>,
    payload: Option<ResetAiPromptTemplatesPayload>,
) -> Result<AiPromptTemplatesDocument, String> {
    let scene = payload
        .and_then(|value| value.scene)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let document = match scene {
        Some(scene_key) => {
            let default_template = find_default_ai_prompt_template(&scene_key)
                .ok_or_else(|| format!("不支持的提示词模板场景: {scene_key}"))?;
            let mut current = load_ai_prompt_templates(&app)?;
            for template in &mut current.templates {
                if template.scene == scene_key {
                    *template = default_template.clone();
                    break;
                }
            }
            write_document(&app, &current)?;

            if let Ok(pool) = sqlite_pool(&app).await {
                let _ = insert_activity_log(
                    &pool,
                    "ai_prompt_templates_reset",
                    &format!("已重置提示词模板：{}", default_template.label),
                    None,
                    None,
                    None,
                )
                .await;
            }

            current
        }
        None => {
            delete_document_file(&app)?;
            let defaults = default_ai_prompt_templates();

            if let Ok(pool) = sqlite_pool(&app).await {
                let _ = insert_activity_log(
                    &pool,
                    "ai_prompt_templates_reset",
                    "已重置全部 AI 提示词模板为默认值",
                    None,
                    None,
                    None,
                )
                .await;
            }

            defaults
        }
    };

    Ok(document)
}
