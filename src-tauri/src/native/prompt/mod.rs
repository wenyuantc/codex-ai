use std::collections::HashSet;
use std::fs;
use std::process::Command;

use tauri::{AppHandle, Runtime};

use crate::codex::{find_ai_prompt_template, load_ai_prompt_templates};
use crate::native::settings::{
    normalize_subagent_policy, SUBAGENT_POLICY_AGGRESSIVE, SUBAGENT_POLICY_CONSERVATIVE,
};
use crate::native::tools::ssh::SshToolRuntime;

const IDENTITY: &str = include_str!("identity.md");
const AGENTS_FILE_CAP: usize = 32_768;
const PROJECT_INSTRUCTION_FILES: &[&str] = &["AGENTS.md", "Agents.md", "CLAUDE.md"];

#[derive(Debug, Default, Clone)]
pub struct NativeGitInfo {
    pub branch: String,
    pub status: String,
    pub log: String,
}

#[derive(Debug, Default, Clone)]
pub struct NativePromptParts {
    pub cwd: String,
    pub model: String,
    pub platform: String,
    pub git: Option<NativeGitInfo>,
    pub global_template: String,
    pub project_agents: String,
    pub employee_prompt: String,
    pub max_concurrent_subagents: u32,
    pub subagent_policy: String,
}

pub fn compose_system(parts: &NativePromptParts) -> String {
    let mut blocks = Vec::new();
    blocks.push(IDENTITY.trim().to_string());
    let policy_block = subagent_policy_block(&parts.subagent_policy);
    if !policy_block.is_empty() {
        blocks.push(policy_block);
    }
    blocks.push(environment_block(parts));
    if let Some(git) = parts.git.as_ref() {
        let git_block = git_block(git);
        if !git_block.is_empty() {
            blocks.push(git_block);
        }
    }
    if !parts.global_template.trim().is_empty() {
        blocks.push(format!("# 全局提示词\n{}", parts.global_template.trim()));
    }
    if !parts.project_agents.trim().is_empty() {
        blocks.push(format!(
            "# 项目指令（AGENTS.md / CLAUDE.md）\n{}",
            parts.project_agents.trim()
        ));
    }
    if !parts.employee_prompt.trim().is_empty() {
        blocks.push(format!("# 员工设定\n{}", parts.employee_prompt.trim()));
    }
    blocks.join("\n\n")
}

fn environment_block(parts: &NativePromptParts) -> String {
    let mut lines = vec![
        "You have been invoked in the following environment:".to_string(),
        format!("- Working directory: {}", parts.cwd),
        format!("- Platform: {}", parts.platform),
        format!("- Date: {}", chrono::Local::now().format("%Y-%m-%d")),
        "- Permission mode: confirm-high-risk".to_string(),
        format!(
            "- Max concurrent sub-agents: {}",
            parts.max_concurrent_subagents.max(1)
        ),
        format!(
            "- Sub-agent policy: {}",
            normalize_subagent_policy(Some(parts.subagent_policy.as_str()))
        ),
    ];
    if !parts.model.trim().is_empty() {
        lines.push(format!(
            "- You are powered by the model named {}.",
            parts.model.trim()
        ));
    }
    lines.join("\n")
}

pub fn subagent_policy_block(policy: &str) -> String {
    let policy = normalize_subagent_policy(Some(policy));
    match policy.as_str() {
        SUBAGENT_POLICY_CONSERVATIVE => "# 子 Agent 策略（conservative）\n尽量自己用 Read / Glob / Grep / Edit 完成。仅当两块工作互不依赖、且各自需要较多探索或改动时，才在同一轮多次调用 Agent。单文件小改、后一步依赖前一步结果时不要委派。".to_string(),
        SUBAGENT_POLICY_AGGRESSIVE => "# 子 Agent 策略（aggressive）\n默认先拆。第一轮优先用 Agent 并行摸底（explore）或按块实现（general），不要自己先把 Glob/Read 做完再决定是否委派。只有改动明确只有一个已知文件时才跳过 Agent。同一轮尽量为独立工作流各调一次 Agent，不超过 Max concurrent sub-agents。".to_string(),
        _ => "# 子 Agent 策略（balanced）\n任务涉及两个以上模块/目录，或用户输入含多步执行计划时：同一轮至少委派 2 个 Agent（可并行的摸底用 explore，可并行的改动用 general）。仅当目标就是改一个已知文件、改动闭包很小，才自己 Read/Edit。不要把强依赖的步骤并行拆开。".to_string(),
    }
}

pub fn agent_tool_description(cap: u32, policy: &str) -> String {
    let policy = normalize_subagent_policy(Some(policy));
    let cap = cap.max(1);
    let policy_hint = match policy.as_str() {
        SUBAGENT_POLICY_CONSERVATIVE => {
            "Policy conservative: delegate only when two workstreams are independent and each needs substantial exploration or edits."
        }
        SUBAGENT_POLICY_AGGRESSIVE => {
            "Policy aggressive: default to spawning parallel Agents this turn unless the change is a single known file."
        }
        _ => {
            "Policy balanced: spawn at least two Agents in one turn when the task spans 2+ modules or includes a multi-step plan."
        }
    };
    format!(
        "Delegate a self-contained subtask to a child agent. Multiple calls in one turn run in parallel (max {cap}). The child does not see this conversation. Use explore for read-only research and general for edits. {policy_hint}"
    )
}

fn git_block(git: &NativeGitInfo) -> String {
    let mut body = String::from("# Git context\n");
    if !git.branch.trim().is_empty() {
        body.push_str(&format!("Branch: {}\n", git.branch.trim()));
    }
    if !git.status.trim().is_empty() {
        body.push_str(&format!("Status:\n{}\n", git.status.trim()));
    }
    if !git.log.trim().is_empty() {
        body.push_str(&format!("Recent commits:\n{}\n", git.log.trim()));
    }
    if body.trim() == "# Git context" {
        String::new()
    } else {
        body
    }
}

pub fn format_global_template(output_goal: &str, scene_requirement: &str) -> String {
    let goal = output_goal.trim();
    let req = scene_requirement.trim();
    match (goal.is_empty(), req.is_empty()) {
        (true, true) => String::new(),
        (false, true) => goal.to_string(),
        (true, false) => req.to_string(),
        (false, false) => format!("{goal}\n\n{req}"),
    }
}

fn cap_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.chars().count() <= AGENTS_FILE_CAP {
        return trimmed.to_string();
    }
    let prefix: String = trimmed.chars().take(AGENTS_FILE_CAP).collect();
    format!("{prefix}…[truncated]")
}

pub fn read_local_project_agents(cwd: &str) -> String {
    let root = std::path::Path::new(cwd);
    let mut seen = HashSet::new();
    let mut chunks = Vec::new();
    for name in PROJECT_INSTRUCTION_FILES {
        let path = root.join(name);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let capped = cap_text(&text);
        if capped.is_empty() || !seen.insert(capped.clone()) {
            continue;
        }
        chunks.push(format!("## {name}\n{capped}"));
    }
    chunks.join("\n\n")
}

pub async fn read_ssh_project_agents(ssh: &SshToolRuntime) -> String {
    let mut seen = HashSet::new();
    let mut chunks = Vec::new();
    for name in PROJECT_INSTRUCTION_FILES {
        let Ok(text) = ssh.read(name).await else {
            continue;
        };
        if text.trim() == "(no output)" {
            continue;
        }
        let capped = cap_text(&text);
        if capped.is_empty() || !seen.insert(capped.clone()) {
            continue;
        }
        chunks.push(format!("## {name}\n{capped}"));
    }
    chunks.join("\n\n")
}

pub fn detect_local_git(cwd: &str) -> Option<NativeGitInfo> {
    let root = std::path::Path::new(cwd);
    if !root.join(".git").exists() {
        let ok = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(root)
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|text| text.trim() == "true")
            .unwrap_or(false);
        if !ok {
            return None;
        }
    }
    let run = |args: &[&str]| -> String {
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    Some(NativeGitInfo {
        branch: run(&["rev-parse", "--abbrev-ref", "HEAD"]),
        status: run(&["status", "-sb"]),
        log: run(&["log", "-5", "--oneline"]),
    })
}

pub async fn detect_ssh_git(ssh: &SshToolRuntime) -> Option<NativeGitInfo> {
    let branch = ssh
        .bash("git rev-parse --abbrev-ref HEAD")
        .await
        .ok()
        .filter(|text| !text.trim().is_empty() && text.trim() != "(no output)")?;
    let status = ssh.bash("git status -sb").await.unwrap_or_default();
    let log = ssh.bash("git log -5 --oneline").await.unwrap_or_default();
    Some(NativeGitInfo {
        branch: branch.trim().to_string(),
        status: status.trim().to_string(),
        log: log.trim().to_string(),
    })
}

pub fn load_global_template<R: Runtime>(app: &AppHandle<R>) -> String {
    let document = load_ai_prompt_templates(app)
        .unwrap_or_else(|_| crate::codex::default_ai_prompt_templates());
    find_ai_prompt_template(&document, "native_agent_global")
        .map(|template| format_global_template(&template.output_goal, &template.scene_requirement))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_includes_identity_global_project_and_employee() {
        let text = compose_system(&NativePromptParts {
            cwd: "/repo".to_string(),
            model: "demo".to_string(),
            platform: "macos".to_string(),
            git: Some(NativeGitInfo {
                branch: "main".to_string(),
                status: "## main".to_string(),
                log: "abc init".to_string(),
            }),
            global_template: "全局规则".to_string(),
            project_agents: "## AGENTS.md\n用 2 空格缩进".to_string(),
            employee_prompt: "角色：reviewer".to_string(),
            max_concurrent_subagents: 5,
            subagent_policy: "aggressive".to_string(),
        });
        assert!(text.contains("内置编程 Agent"));
        assert!(text.contains("confirm-high-risk"));
        assert!(text.contains("Max concurrent sub-agents: 5"));
        assert!(text.contains("Sub-agent policy: aggressive"));
        assert!(text.contains("子 Agent 策略（aggressive）"));
        assert!(text.contains("explore"));
        assert!(text.contains("general"));
        assert!(!text.contains("yolo"));
        assert!(text.contains("Working directory: /repo"));
        assert!(text.contains("Branch: main"));
        assert!(text.contains("全局规则"));
        assert!(text.contains("用 2 空格缩进"));
        assert!(text.contains("角色：reviewer"));
        assert!(!text.contains("任务标题"));
    }

    #[test]
    fn balanced_policy_asks_for_two_agents_on_multi_step_work() {
        let block = subagent_policy_block("balanced");
        assert!(block.contains("至少委派 2 个 Agent"));
        let description = agent_tool_description(4, "balanced");
        assert!(description.contains("max 4"));
        assert!(description.contains("Policy balanced"));
    }

    #[test]
    fn read_local_agents_and_dedup() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codex-ai-agents-{stamp}"));
        fs::create_dir_all(&root).expect("mkdir");
        fs::write(root.join("AGENTS.md"), "project agents here").expect("write agents");
        fs::write(root.join("CLAUDE.md"), "claude notes").expect("write claude");
        let text = read_local_project_agents(&root.to_string_lossy());
        assert!(text.contains("project agents here"));
        assert!(text.contains("claude notes"));
        let _ = fs::remove_dir_all(root);
    }
}
