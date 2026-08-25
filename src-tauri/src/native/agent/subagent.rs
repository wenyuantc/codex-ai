use serde_json::Value;

pub const MAX_CONCURRENT_SUBAGENTS: usize = 3;
pub const SUBAGENT_MAX_TURNS: u32 = 20;
pub const SUBAGENT_RESULT_CHARS: usize = 16_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentKind {
    General,
    Explore,
}

impl SubagentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Explore => "explore",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubagentSpec {
    pub description: String,
    pub prompt: String,
    pub kind: SubagentKind,
}

pub fn parse_subagent_args(arguments: &str) -> Result<SubagentSpec, String> {
    let value: Value = if arguments.trim().is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_str(arguments)
            .map_err(|error| format!("工具参数不是合法 JSON: {error}"))?
    };
    let prompt = value
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .ok_or_else(|| "prompt 不能为空".to_string())?;
    let description = value
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .unwrap_or("子任务");
    let kind = match value
        .get("subagent_type")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("general")
    {
        "" | "general" => SubagentKind::General,
        "explore" => SubagentKind::Explore,
        other => {
            return Err(format!(
                "未知 subagent_type：{other}，应为 general 或 explore"
            ))
        }
    };
    Ok(SubagentSpec {
        description: description.to_string(),
        prompt: prompt.to_string(),
        kind,
    })
}

const SUBAGENT_LABEL_CHARS: usize = 32;

pub fn format_subagent_log_tag(index: u32, kind: SubagentKind, description: &str) -> String {
    format!(
        "[子 Agent {index}({}) - {}]",
        kind.as_str(),
        sanitize_subagent_label(description)
    )
}

fn sanitize_subagent_label(description: &str) -> String {
    let collapsed = description
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(['[', ']', '(', ')'], "");
    let cleaned = if collapsed.is_empty() {
        "子任务".to_string()
    } else {
        collapsed
    };
    let count = cleaned.chars().count();
    if count <= SUBAGENT_LABEL_CHARS {
        return cleaned;
    }
    let prefix: String = cleaned
        .chars()
        .take(SUBAGENT_LABEL_CHARS.saturating_sub(1))
        .collect();
    format!("{prefix}…")
}

pub fn child_max_turns(parent: u32) -> u32 {
    if parent == 0 {
        SUBAGENT_MAX_TURNS
    } else {
        parent.min(SUBAGENT_MAX_TURNS)
    }
}

pub fn truncate_report(text: &str) -> String {
    let count = text.chars().count();
    if count <= SUBAGENT_RESULT_CHARS {
        return text.to_string();
    }
    let prefix: String = text
        .chars()
        .take(SUBAGENT_RESULT_CHARS.saturating_sub(1))
        .collect();
    format!("{prefix}…")
}

pub fn child_system_prompt(parent_system: Option<&str>, spec: &SubagentSpec) -> String {
    let appendix = format!(
        "# 子 Agent\n你是父会话委派的子 Agent，类型：{}。\n- 只完成交给你的任务，不要再委派子 Agent。\n- 完成后用简洁中文报告：做了什么、改了哪些文件、如何验证；未验证要写明。\n- 不要假装已经改过文件。\n任务标题：{}",
        spec.kind.as_str(),
        spec.description
    );
    match parent_system.map(str::trim).filter(|item| !item.is_empty()) {
        Some(system) => format!("{system}\n\n{appendix}"),
        None => appendix,
    }
}

pub fn format_subagent_result(spec: &SubagentSpec, outcome: Result<&str, &str>) -> String {
    match outcome {
        Ok(report) => format!(
            "子 Agent（{} / {}）完成。\n\n{}",
            spec.description,
            spec.kind.as_str(),
            truncate_report(report)
        ),
        Err(error) => format!(
            "子 Agent（{} / {}）失败：{error}",
            spec.description,
            spec.kind.as_str()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_requires_prompt_and_defaults_general() {
        let spec = parse_subagent_args(r#"{"description":"查权限","prompt":"阅读 permission.rs"}"#)
            .expect("parse");
        assert_eq!(spec.description, "查权限");
        assert_eq!(spec.prompt, "阅读 permission.rs");
        assert_eq!(spec.kind, SubagentKind::General);
        let err = parse_subagent_args(r#"{"description":"x"}"#).expect_err("prompt");
        assert!(err.contains("prompt 不能为空"));
        let err =
            parse_subagent_args(r#"{"prompt":"go","subagent_type":"planner"}"#).expect_err("type");
        assert!(err.contains("未知 subagent_type"));
        let spec =
            parse_subagent_args(r#"{"prompt":"go","subagent_type":"explore"}"#).expect("explore");
        assert_eq!(spec.kind, SubagentKind::Explore);
        assert_eq!(spec.description, "子任务");
    }

    #[test]
    fn child_turns_cap_at_twenty() {
        assert_eq!(child_max_turns(0), 20);
        assert_eq!(child_max_turns(40), 20);
        assert_eq!(child_max_turns(8), 8);
    }

    #[test]
    fn truncate_report_keeps_short_text() {
        assert_eq!(truncate_report("ok"), "ok");
        let long = "测".repeat(SUBAGENT_RESULT_CHARS + 8);
        let out = truncate_report(&long);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= SUBAGENT_RESULT_CHARS);
    }

    #[test]
    fn log_tag_includes_index_kind_and_name() {
        assert_eq!(
            format_subagent_log_tag(2, SubagentKind::Explore, "摸底 RuleService 与 VO"),
            "[子 Agent 2(explore) - 摸底 RuleService 与 VO]"
        );
        assert_eq!(
            format_subagent_log_tag(1, SubagentKind::General, "  查\n规则  "),
            "[子 Agent 1(general) - 查 规则]"
        );
        let long = format_subagent_log_tag(1, SubagentKind::Explore, &"测".repeat(40));
        assert!(long.starts_with("[子 Agent 1(explore) - "));
        assert!(long.contains('…'));
        assert!(!long.contains("测".repeat(40).as_str()));
    }
}
