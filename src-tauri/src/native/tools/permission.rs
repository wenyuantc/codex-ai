use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeToolRiskKind {
    Overwrite,
    Delete,
    Push,
    ForceGit,
    Mcp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeToolRisk {
    Low,
    High {
        kind: NativeToolRiskKind,
        summary: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativePermissionDecision {
    AllowSession,
    AllowOnce,
    Deny,
}

pub fn classify_native_tool_risk(
    name: &str,
    arguments: &str,
    file_exists: Option<bool>,
    is_mcp: bool,
) -> NativeToolRisk {
    if is_mcp || name.starts_with("mcp_") {
        return NativeToolRisk::High {
            kind: NativeToolRiskKind::Mcp,
            summary: format!("调用 MCP 工具 {name}"),
        };
    }
    match name {
        "Edit" => NativeToolRisk::High {
            kind: NativeToolRiskKind::Overwrite,
            summary: format!("覆盖已有文件 {}", arg_string(arguments, "file_path")),
        },
        "Write" => {
            if file_exists.unwrap_or(false) {
                NativeToolRisk::High {
                    kind: NativeToolRiskKind::Overwrite,
                    summary: format!("覆盖已有文件 {}", arg_string(arguments, "file_path")),
                }
            } else {
                NativeToolRisk::Low
            }
        }
        "Bash" => classify_bash(&arg_string(arguments, "command")),
        _ => NativeToolRisk::Low,
    }
}

fn arg_string(arguments: &str, key: &str) -> String {
    serde_json::from_str::<Value>(arguments)
        .ok()
        .and_then(|value| {
            value
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "(unknown)".to_string())
}

fn classify_bash(command: &str) -> NativeToolRisk {
    let segments = split_shell_segments(command);
    let mut worst: Option<(NativeToolRiskKind, String)> = None;
    for segment in segments {
        let tokens = tokenize(&segment);
        if tokens.is_empty() {
            continue;
        }
        if let Some((kind, summary)) = classify_tokens(&tokens, &segment) {
            worst = Some(pick_worse(worst, kind, summary));
        }
    }
    match worst {
        Some((kind, summary)) => NativeToolRisk::High { kind, summary },
        None => NativeToolRisk::Low,
    }
}

fn pick_worse(
    current: Option<(NativeToolRiskKind, String)>,
    kind: NativeToolRiskKind,
    summary: String,
) -> (NativeToolRiskKind, String) {
    match current {
        None => (kind, summary),
        Some((existing, existing_summary)) => {
            if risk_rank(kind) >= risk_rank(existing) {
                (kind, summary)
            } else {
                (existing, existing_summary)
            }
        }
    }
}

fn risk_rank(kind: NativeToolRiskKind) -> u8 {
    match kind {
        NativeToolRiskKind::Overwrite => 1,
        NativeToolRiskKind::Mcp => 2,
        NativeToolRiskKind::Delete => 3,
        NativeToolRiskKind::Push => 4,
        NativeToolRiskKind::ForceGit => 5,
    }
}

fn classify_tokens(tokens: &[String], original: &str) -> Option<(NativeToolRiskKind, String)> {
    let first = tokens.first()?.as_str();
    if matches!(first, "rm" | "rmdir") {
        return Some((NativeToolRiskKind::Delete, format!("删除：{original}")));
    }
    if first == "git" {
        let sub = tokens.get(1).map(String::as_str).unwrap_or("");
        if sub == "rm" {
            return Some((NativeToolRiskKind::Delete, format!("git rm：{original}")));
        }
        if sub == "push" {
            if tokens
                .iter()
                .any(|token| token == "--force" || token == "-f" || token == "--force-with-lease")
            {
                return Some((
                    NativeToolRiskKind::ForceGit,
                    format!("强制推送：{original}"),
                ));
            }
            return Some((NativeToolRiskKind::Push, format!("推送：{original}")));
        }
        if sub == "reset" && tokens.iter().any(|token| token == "--hard") {
            return Some((
                NativeToolRiskKind::ForceGit,
                format!("git reset --hard：{original}"),
            ));
        }
        if sub == "checkout" && tokens.iter().any(|token| token == "--") && tokens.len() > 3 {
            return Some((
                NativeToolRiskKind::ForceGit,
                format!("丢弃改动：{original}"),
            ));
        }
        if sub == "restore"
            && tokens
                .iter()
                .any(|token| token == "--worktree" || token == "--source" || token == "--")
        {
            return Some((
                NativeToolRiskKind::ForceGit,
                format!("git restore：{original}"),
            ));
        }
    }
    None
}

fn split_shell_segments(command: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        if quote.is_none() && matches!(ch, '|' | ';' | '&' | '\n') {
            if ch == '&' && chars.peek() == Some(&'&') {
                chars.next();
            }
            if ch == '|' && chars.peek() == Some(&'|') {
                chars.next();
            }
            if !current.trim().is_empty() {
                parts.push(current.trim().to_string());
            }
            current.clear();
            continue;
        }
        if let Some(q) = quote {
            current.push(ch);
            if ch == q {
                quote = None;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
        }
        current.push(ch);
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    if parts.is_empty() {
        vec![command.trim().to_string()]
    } else {
        parts
    }
}

fn tokenize(segment: &str) -> Vec<String> {
    segment
        .split_whitespace()
        .map(|item| item.trim_matches(|ch| ch == '\'' || ch == '"'))
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_new_file_is_low_existing_is_high() {
        assert_eq!(
            classify_native_tool_risk("Write", r#"{"file_path":"a.rs"}"#, Some(false), false),
            NativeToolRisk::Low
        );
        assert!(matches!(
            classify_native_tool_risk("Write", r#"{"file_path":"a.rs"}"#, Some(true), false),
            NativeToolRisk::High {
                kind: NativeToolRiskKind::Overwrite,
                ..
            }
        ));
    }

    #[test]
    fn edit_is_always_overwrite() {
        assert!(matches!(
            classify_native_tool_risk(
                "Edit",
                r#"{"file_path":"a.rs","old_string":"a","new_string":"b"}"#,
                None,
                false
            ),
            NativeToolRisk::High {
                kind: NativeToolRiskKind::Overwrite,
                ..
            }
        ));
    }

    #[test]
    fn bash_delete_push_and_force() {
        assert!(matches!(
            classify_native_tool_risk("Bash", r#"{"command":"rm -rf src"}"#, None, false),
            NativeToolRisk::High {
                kind: NativeToolRiskKind::Delete,
                ..
            }
        ));
        assert!(matches!(
            classify_native_tool_risk("Bash", r#"{"command":"git push origin main"}"#, None, false),
            NativeToolRisk::High {
                kind: NativeToolRiskKind::Push,
                ..
            }
        ));
        assert!(matches!(
            classify_native_tool_risk(
                "Bash",
                r#"{"command":"git push --force origin main"}"#,
                None,
                false
            ),
            NativeToolRisk::High {
                kind: NativeToolRiskKind::ForceGit,
                ..
            }
        ));
        assert!(matches!(
            classify_native_tool_risk(
                "Bash",
                r#"{"command":"git reset --hard HEAD"}"#,
                None,
                false
            ),
            NativeToolRisk::High {
                kind: NativeToolRiskKind::ForceGit,
                ..
            }
        ));
    }

    #[test]
    fn read_and_grep_are_low() {
        assert_eq!(
            classify_native_tool_risk("Read", r#"{"file_path":"a.rs"}"#, None, false),
            NativeToolRisk::Low
        );
        assert_eq!(
            classify_native_tool_risk("Grep", r#"{"pattern":"TODO"}"#, None, false),
            NativeToolRisk::Low
        );
    }

    #[test]
    fn mcp_tools_are_high() {
        assert!(matches!(
            classify_native_tool_risk("mcp_fs_read", "{}", None, true),
            NativeToolRisk::High {
                kind: NativeToolRiskKind::Mcp,
                ..
            }
        ));
    }

    #[test]
    fn pipeline_prefers_force_git() {
        assert!(matches!(
            classify_native_tool_risk(
                "Bash",
                r#"{"command":"ls && git push --force"}"#,
                None,
                false
            ),
            NativeToolRisk::High {
                kind: NativeToolRiskKind::ForceGit,
                ..
            }
        ));
    }
}
