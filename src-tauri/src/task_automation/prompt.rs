use crate::db::models::{ReviewVerdict, Subtask, Task, TaskAttachment};

pub struct AutomationExecutionInput {
    pub prompt: String,
    pub image_paths: Vec<String>,
}

const SUBTASK_STATUS_LABELS: &[(&str, &str)] = &[
    ("todo", "待办"),
    ("in_progress", "进行中"),
    ("review", "审核中"),
    ("completed", "已完成"),
    ("blocked", "已阻塞"),
];

fn subtask_status_label(status: &str) -> &str {
    SUBTASK_STATUS_LABELS
        .iter()
        .find_map(|(key, label)| (*key == status).then_some(*label))
        .unwrap_or(status)
}

fn attachment_is_image(attachment: &TaskAttachment) -> bool {
    attachment
        .mime_type
        .trim()
        .to_ascii_lowercase()
        .starts_with("image/")
}

pub fn build_automation_fix_prompt(
    task: &Task,
    subtasks: &[Subtask],
    attachments: &[TaskAttachment],
    review_report: &str,
    review_verdict: &ReviewVerdict,
) -> AutomationExecutionInput {
    let mut sections = vec![format!("任务标题:\n{}", task.title.trim())];

    if let Some(description) = task
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sections.push(format!("任务描述:\n{}", description));
    }

    if !subtasks.is_empty() {
        let lines = subtasks
            .iter()
            .enumerate()
            .map(|(index, subtask)| {
                format!(
                    "{}. [{}] {}",
                    index + 1,
                    subtask_status_label(&subtask.status),
                    subtask.title
                )
            })
            .collect::<Vec<_>>();
        sections.push(format!("子任务:\n{}", lines.join("\n")));
    }

    let valid_attachments = attachments
        .iter()
        .filter(|attachment| !attachment.stored_path.trim().is_empty())
        .collect::<Vec<_>>();
    if !valid_attachments.is_empty() {
        let lines = valid_attachments
            .iter()
            .enumerate()
            .map(|(index, attachment)| format!("{}. {}", index + 1, attachment.original_name))
            .collect::<Vec<_>>();
        sections.push(format!(
            "任务附件:\n{}\n\n说明：以上附件已绑定到当前任务；其中图片会随本次任务一并附带给 Codex。",
            lines.join("\n")
        ));
    }

    sections.push(format!(
        "本次补充指令:\n请基于同一个原任务继续修复，不要新建修复任务或拆分新的任务链路。\n\
优先解决本次代码审核中的阻断问题，并确保修改后可重新通过审核。\n\
结构化审核结论：passed={}, needs_human={}, blocking_issue_count={}\n\
审核摘要：{}\n\n审核详细报告：\n{}",
        review_verdict.passed,
        review_verdict.needs_human,
        review_verdict.blocking_issue_count,
        review_verdict.summary.trim(),
        review_report.trim()
    ));

    AutomationExecutionInput {
        prompt: sections.join("\n\n"),
        image_paths: valid_attachments
            .into_iter()
            .filter(|attachment| attachment_is_image(attachment))
            .map(|attachment| attachment.stored_path.clone())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::build_automation_fix_prompt;
    use crate::db::models::{ReviewVerdict, Subtask, Task, TaskAttachment};

    fn build_task() -> Task {
        Task {
            id: "task-1".to_string(),
            title: "修复自动质控".to_string(),
            description: Some("确保自动修复后可重新通过审核".to_string()),
            status: "review".to_string(),
            priority: "high".to_string(),
            project_id: "proj-1".to_string(),
            use_worktree: true,
            assignee_id: Some("emp-1".to_string()),
            reviewer_id: Some("emp-2".to_string()),
            complexity: None,
            ai_suggestion: None,
            automation_mode: Some("review_fix_loop_v1".to_string()),
            last_codex_session_id: None,
            last_review_session_id: None,
            created_at: "2026-04-16 10:00:00".to_string(),
            updated_at: "2026-04-16 10:00:00".to_string(),
        }
    }

    #[test]
    fn build_automation_fix_prompt_matches_schema() {
        let task = build_task();
        let subtasks = vec![Subtask {
            id: "sub-1".to_string(),
            task_id: task.id.clone(),
            title: "修复 review verdict".to_string(),
            status: "in_progress".to_string(),
            sort_order: 1,
            created_at: "2026-04-16 10:00:00".to_string(),
            updated_at: "2026-04-16 10:00:00".to_string(),
        }];
        let attachments = vec![TaskAttachment {
            id: "att-1".to_string(),
            task_id: task.id.clone(),
            original_name: "ui.png".to_string(),
            stored_path: "/tmp/ui.png".to_string(),
            mime_type: "image/png".to_string(),
            file_size: 12,
            sort_order: 1,
            created_at: "2026-04-16 10:00:00".to_string(),
        }];
        let verdict = ReviewVerdict {
            passed: false,
            needs_human: false,
            blocking_issue_count: 2,
            summary: "仍有 2 个阻断问题".to_string(),
        };

        let result = build_automation_fix_prompt(
            &task,
            &subtasks,
            &attachments,
            "## 阻断问题\n1. 缺 verdict\n2. 状态机未收口",
            &verdict,
        );

        assert!(result.prompt.contains("任务标题:\n修复自动质控"));
        assert!(result
            .prompt
            .contains("任务描述:\n确保自动修复后可重新通过审核"));
        assert!(result
            .prompt
            .contains("子任务:\n1. [进行中] 修复 review verdict"));
        assert!(result.prompt.contains("任务附件:\n1. ui.png"));
        assert!(result
            .prompt
            .contains("本次补充指令:\n请基于同一个原任务继续修复"));
        assert_eq!(result.image_paths, vec!["/tmp/ui.png".to_string()]);
    }
}
